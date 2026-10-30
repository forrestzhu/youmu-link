# Windows 安装版性能观察与前台高 CPU 排查

Commit: 5e85961b4f385209b33f99481e5daf986f418970
Host-OS: Windows 11 Pro x64 10.0.26200；AMD Ryzen 7 5800X，16 逻辑处理器
Result: paused (用户要求等待实际连接VR后继续；高CPU根因未确认)

## 范围与版本

2026-10-31，用户已安装 [YML-014](../tasks/archive/YML-014-windows-performance-package.md) 的预览包，并同意检查性能。最初安装版 PID8332 的 EXE / 托管 DLL / Rust DLL / BUILD-INFO 与交付 CI ZIP 四项 SHA-256 一致。未记录安装位置、共享目录路径或原始 UI 树。

期间并行的 [YML-016](../tasks/archive/YML-016-share-path-display.md) 为回归释放端口，正常关闭同一安装版并启动路径展示修复开发版。该聊天记录有明确 Alt+F4 操作；用户确认未主动关闭，当前查询未找到安装版对应的崩溃事件。最初筛出的 .NET 1026 事件来自测试 EXE 的失败断言，不是应用崩溃，已排除。

后继开发版 PID6220 的来源为 `95839215af98dce19914e2733cfb2685fc648f02`。其 EXE SHA-256 为 `6a84b78bbda5ffe5b0de343e136c3af2af9e0639667566d3b0bf91534e9ab52c`，托管 DLL 为 `665caf75c290a324e65f6e0a43bbfa270315a0732e8331885bf24b4aa030e5ed`。两版本分列，不合并均值，也不当作路径修复的性能 A/B。

## 方法与读数

外部 PowerShell 每秒重新获取 Process，检查 PID / StartTime，读取累计 CPU、working set、private bytes、句柄、线程及 Responding。以 Stopwatch 实际墙钟计算单核百分比；另除以16给出按逻辑处理器归一化的占用，不保证等于任务管理器 CPU utility 的瞬时值。采样区间不主动抓取 UI 树，但无法据此保证 UIA 客户端没有存留订阅或请求。

| 版本 / 状态 | 墙钟 s | CPU s | 单核平均 | /16 归一化 | WS MiB 首→尾 | Private MiB 首→尾 |
| --- | ---: | ---: | ---: | ---: | --- | --- |
| 安装版，激活后第1段 | 30.5624 | 0.28125 | 0.92025% | 0.05752% | 256.180→256.730 | 168.410→168.543 |
| 安装版，激活后第2段 | 30.4775 | 0.25000 | 0.82028% | 0.05127% | 256.730→256.715 | 168.543→168.508 |
| 开发版，交互后 / 结束时最小化 | 30.4901 | 0.015625 | 0.05125% | 0.00320% | 209.566→209.586 | 169.777→169.812 |
| 同一开发版，重新激活前台 | 30.5922 | 20.71875 | **约67.73%** | **约4.2328%** | 212.625→213.027 | 181.480→181.820 |

安装版两段合计61.0399s / CPU0.53125s，约0.87033%单核、0.05440%归一化；句柄1471–1475、线程31–32，Responding均为true。独立 subagent 读取原始数字并复算通过。此期间并行聊天也在核验窗口，窗口可见性 / 输入未完全隔离，这些低读数不能代签始终可见的自然空闲。

更早两段约30s的参考值为单核1.48770% / 0.77237%；其间页面变化且结束时最小化，不计稳定空闲基线。用户后来手工确认三页切换和显示器刷新已完成；计划的同安装版交互后30s采样在约6s时因上述正常关闭而中断，没有有效结果。

开发版重新激活时，原生辅助功能显示共享页、0设备、正在搜索，未镜像 / 传输。该段明确出现高 CPU；不能因后台低读数或短时内存变化小就宣称性能通过。

## 高 CPU 线程与观察工具

高占用后的5.1045s线程计数：主线程37312（0x91C0）CPU时间增量2.234375s；线程37712（0x9350）增量1.6875s；其余观测线程几乎为0。

已安装的 `dotnet-stack report -p 6220` 两次成功。主线程为 native frames / Dispatcher.PushFrameImpl（第二次有Dispatcher.GetMessage）；热线程0x9350两次均经过 `MS.Internal.Automation.ElementUtil.Invoke → Dispatcher.LegacyInvokeImpl / InvokeImpl`，一次等待DispatcherOperation，一次处于RequestForegroundProcessing。原始栈只保留在忽略入库的 `dist/performance/`。

这支持优先检查 UI Automation 请求成本，不能从等待栈直接认定自旋、请求者或 GPU 渲染故障。[.NET 8 ElementUtil 源码](https://github.com/dotnet/wpf/blob/v8.0.0/src/Microsoft.DotNet.Wpf/src/PresentationCore/MS/internal/Automation/ElementUtil.cs#L191) 是一次同步派发，并无重试循环；[ElementProxy](https://github.com/dotnet/wpf/blob/v8.0.0/src/Microsoft.DotNet.Wpf/src/PresentationCore/MS/internal/Automation/ElementProxy.cs#L100) 的属性 / 遍历请求会经过该路径。微软说明逐属性读取会产生多次跨进程调用，批量缓存用于降低成本，见[UIA 缓存说明](https://learn.microsoft.com/en-us/windows/win32/winauto/uiauto-cachingforclients)。

独立只读排查未见常驻动画、CompositionTarget.Rendering / LayoutUpdated、自定义透明窗口、阴影 / 模糊或强制软件渲染。500ms轮询仍每轮Render，是待区分成本；现有栈不足以把它定为根因。路径展示修复仅为字符串转换，未新增I/O或计时器。

computer-use 截图首次失败（Direct3D11CaptureFramePool.CreateFreeThreaded / DXGI_ERROR_DEVICE_REMOVED，0x887A0005）；恢复窗口选择后，无截图的辅助功能读取可用。自动点击因无coordinate geometry失败，键盘尝试未观察到页面切换，已交由用户手动完成。截图工具失败不能直接归因为 WPF 渲染缺陷，见[微软渲染故障诊断](https://learn.microsoft.com/en-us/troubleshoot/developer/dotnet/framework/general/wpf-render-thread-failures)。

随后重置本聊天的 node_repl 观察客户端，计划保持同一应用前台作对照；但另一聊天继续修复边界并再次关闭该应用。10s对照数据没有存续的YM_Link，仅观察到computer-use helper约25.16%单核、TextInputHost为0，因此该对照**无效**，尚不能证明停观察会降低应用CPU。

## 当前结论与下一步

已确认“可见前台出现约68%单核占用”的事实；UIA观察请求是优先假设，根因未证实。没有修改产品代码、无障碍语义、图形或安全设置。

先错开两聊天对同一程序的操作，再以同一版本 / PID / 页面 / 大小 / 可见性做“停止观察 → 单次UIA读取 → 停止观察”的20–30s对照。不能把遮挡或最小化混入对照。若彻底停止观察仍高，采WPR原生CPU栈，区分UIAutomationCore / RPCRT4与wpfgfx_cor3 / D3D / Layout / TextFormatter。`wpr -status` 确认检查时没有现有录制；尚未启动WPR。

数小时运行、内存泄漏排除、真实Quest / LAN / 镜像 / 传输性能均未验收。原始数字与栈保留在本地忽略目录，不提交原始日志。

用户于2026-10-31明确要求先暂停，待后续实际连接VR再看CPU。已停止后续测量与窗口操作，不自动恢复。当前记录保留高占用事实、UIA干扰假设及所有无效 / 中断区间；恢复条件以用户通知为准。只读独立复核修正了自动化 / 用户手工操作的边界和线程计数措辞，未发现剩余数字或归因错误。
