# YML-030：设备行统一配对与连接状态展示

Status: done
Workflow: behavior
Requires-Device: no
Tests: windows/Tests PeerItem 状态优先级 / 操作按钮、生产 WPF 设备行双语展示与更新；Rust 无行为改动，沿用 harness verify

## Scope

修复用户截图中同一 Windows 设备同时显示“已配对”与“已断开”的歧义。用户确认此时应显示“已配对”；设备行只保留一处状态，不修改 Rust 信任、连接、自动重连或协议。

## Spec

- 已信任设备处于 disconnected / discovered 时，统一显示“已配对”，保留“连接”按钮。
- connecting / pairing / connected 等进行中或活动状态优先显示实际状态，保留现有取消 / 断开操作；错误与未知状态不被配对关系遮盖。
- 移除独立的配对标签，中英一致，快照 / 语言更新后及时刷新。
- 用户追加：已配对文字前显示绿色实心点及轻微外扩 / 淡出的波纹；复用 SuccessInk，窗口 / 行隐藏或卸载停止动画，系统关闭动画时保留静态圆点。

## Acceptance

- [x] 用户截图对应的已信任 / 未连接行仅显示一次“已配对”，不会同时显示“已断开”。
- [x] 中英状态与连接按钮、状态更新、未信任 / 错误边界有回归，先 RED 后 GREEN。
- [x] WPF 实际窗口样本、Release 构建、适用完整门禁与独立审查有实际结果。
- [x] 自查 diff、任务 / STATUS、commit / push main 与严格远端检查完成。

## Verification

先通过真实 PeerItem 写状态表回归并运行旧实现 RED，再最小合并状态展示。`dotnet run --project windows/Tests/YM_Link.Tests.csproj` 验证模型与生产 XAML / BAML，中英两尺寸实际原生窗口使用合成数据展示已配对行并保存样本。`python3 scripts/build_windows.py` 构建独立开发产物，`python3 scripts/harness.py verify` 执行完整门禁；不关闭 / 操作用户现有窗口，端口限制保留真实失败。独立只读 CR 后提交推送，`check --require-pushed`。真实 Quest / LAN：not-applicable（仅显示规则，不更改连接语义）。

## Evidence

2026-10-31 基于 a16d6f7，main 干净，fetch / ff-only 同步。Windows PeerItem 分别通过 Trusted 和 Status 输出两段文本；Rust 启动时将持久信任设备标为 disconnected，断开时保留 Trusted，发现后可标为 discovered。双字段均正确，但独立展示造成用户所见矛盾，修复位置在 Windows 展示层。

- 状态回归旧实现 RED：`trusted=True, disconnected` 期望 Paired，实际 Disconnected。合并状态后生产窗口继续 RED：独立 TrustText 造成额外状态文字；删除该标签后通过。状态表覆盖中英各10种输入，保留真实连接按钮与错误 / 未知兜底。
- 波纹为原生 WPF 6px 实心点与8px描边环，2秒外扩至2倍并淡出，复用动态 SuccessInk。系统关闭动画保留实心点；IsVisible / IsEnabled / 最小化 / Unloaded 均移除 storyboard。初次时钟断言在同一 dispatcher turn 读取过早失败，按微软 [Storyboard timing](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/storyboards-overview) 的下一 timing tick 语义修正夹具，使用1秒有界 dispatcher 条件等待，不改生产逻辑规避测试。
- `dotnet run --project windows/Tests/YM_Link.Tests.csproj -c Release` 最终退出0。实际原生合成窗口两语 / 1024×720及860×580共28张本轮样本，已配对→已连接→已配对→撤销信任的同一行更新通过；只有一处状态，配对圆点 / 波纹正确显隐。禁用动画、隐藏恢复及最小化恢复均检查 ellipse 与 transform 动画时钟实际移除 / 恢复。人工抽检中英配对样本通过，截图位于ignored `dist/evidence/yml030/`（旧失败样本另存，不计本轮28张）。未操作用户现有窗口。
- `python3 scripts/build_windows.py` 退出0：Rust Release、完整WPF回归、真实DLL排队 / drain /销毁、win-x64自包含publish与图标核验通过。开发版在ignored `dist/YM_Link`，本轮未覆盖安装版；本机Windows11 Pro x64 build26200。生产编译BAML与事件连接器通过；实际显示窗口使用生产XAML与合成数据，完整开发EXE的后端启动smoke因现有单实例未运行，不将窗口夹具代签LAN。
- `RUST_TEST_THREADS=1 python3 scripts/harness.py verify` 退出1：Python47 /3 skipped、仓库check /rustfmt通过；Rust core23和discovery其余8项通过，`beacon_ports_fall_back_in_order` 因原安装版PID14204占UDP47610而OS10048失败，后续cargo目标not-run。不关闭用户进程，不过滤用例冒作完整通过；等待同源CI完整回归。
- 新独立subagent只读CR受审a16d6f7上本轮完整实现差异，0项待修问题；复核状态 / 信任分离、按钮操作、Row.Update通知、中英 / 高对比动态资源及动效生命周期。未以审查代替测试结果。
- 实现提交 `899870c69189e0a37f5d07887da77260ef82d0e4` 已推送，`check --require-pushed` 退出0。[CI41193929703](https://github.com/forrestzhu/youmu-link/actions/runs/41193929703)首轮Linux /macOS success；Windows Rust117 passed /2 ignored /0 filtered、WPF /真实ABI /Release /完整启动smoke与28张样本上传通过，安装器卸载清理报“exe未移除”导致job失败，保留该失败。[gitleaks41193929734](https://github.com/forrestzhu/youmu-link/actions/runs/41193929734)success。仅重跑失败Windows job，不改实现或包装脚本。
- 对卸载失败追加独立只读核查：smoke在创建MainWindow前走专用分支，不实例化设备行 /波纹；现有错误既可能是卸载器缺失未执行，也可能是卸载返回0后10秒exe仍存在，当前日志不足以判定根因。不得归因为动画 /Defender /单纯等待不足；若重现需另留卸载日志、unins清单、注册表UninstallString及残留进程 /文件证据。
- 同提交仅失败Windows job重跑后，[CI41193929703](https://github.com/forrestzhu/youmu-link/actions/runs/41193929703)最终completed /success，三job全success。逐条回读日志确认Linux Rust104、Windows117 /2 ignored、macOS110 /1 ignored，均0 failed /0 filtered；Windows新状态 /动效WPF回归、真实ABI /Release /完整启动smoke、两轮完整安装 /卸载及制品上传通过，macOS Swift9及既有包检查通过。首轮卸载失败与本机占端口失败保留，不宣称其根因已修复。最终归档只改文档，生产源码与本次成功CI受验提交一致。

## Next

修复、构建、独立CR及同源三平台CI已完成。开发版为 `dist/YM_Link/YM_Link.exe`；当前安装版未覆盖，用户可从托盘退出旧版后打开开发版查看配对状态 /绿色波纹。真实Quest连接与原生安装版LAN继续由YML-023验收，本任务不代签。
