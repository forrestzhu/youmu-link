# YML-034：Windows 托盘右键菜单重设计（Klip 风格）与 macOS 菜单栏统一

Status: verifying
Workflow: feature
Requires-Device: no
Tests: windows/Tests（TrayMenuTests 状态映射 / 定位 / 视觉树行渲染 / 键盘导航，TrayIconTests 重写为事件模型，经 `dotnet run --project windows/Tests/YM_Link.Tests.csproj`）；macOS ModelsTests.testTrayMenuStatusMirrorsConnectionStates 经 `swift test`（本机 Windows 无法执行，由三平台 CI 覆盖）；不改 Rust core / 协议。

## Scope

把 Windows 托盘右键菜单从 WinForms `ContextMenuStrip`（仅"打开 YM_Link / 退出"）重设计为参考 klip 托盘菜单（2026-10-12 已批准 Fluent shell）的自绘 WPF 弹窗，并按用户要求做好跨平台统一：macOS 新增 NSStatusItem 菜单，结构与语义一致（状态头 + 打开 + 退出），外观各随平台原生。视觉基准为 [demo](../../design/prototypes/desktop/2026-10-31-tray-menu/index.html)（用户 2026-11-01 确认方向：默认无镜像开关行，深浅两套主题，状态头第二行含设备 / 速率）。

非目标：不改动 Rust core / 协议；不新增托盘业务动作（镜像开关行未选）；macOS 关窗收起菜单栏（生命周期变更）需真机交互验证，另立任务。

## Spec

- 用户入口：Windows 任务栏托盘图标右键（左键双击保持恢复主窗口）；macOS 菜单栏链环图标点击。
- 菜单结构（双端一致）：状态头（状态点 + 主行 + 可选第二行说明）、分隔线、打开 YM_Link、分隔线、退出。
- 状态映射（`TrayMenuLogic` / `TrayMenuStatus.describe`，给定 Snapshot）：
  - status=starting → 正在启动（灰点）
  - status=error → 状态待确认 + 第二行"打开主窗口查看详情"（红点）
  - 镜像 paused → 镜像已暂停 + 设备名（绿点）；镜像 starting/running → 镜像运行中 + "设备 · fps · 码率"（绿点）
  - status=connected 或有 connected peer → 已连接 · 设备名（绿点）
  - 有可信未连接 peer（PeerItem.ShowsPairedStatus 同语义）→ 已配对 · 设备名（绿点波纹，仅 Windows 动效）
  - 其余 → 正在搜索 Quest…（灰点）
- 视觉规格（Windows，对齐 demo）：卡片宽 220、外边距 16、圆角 10、行高 30、行圆角 6、正文 12px、说明 10.5px；深浅主题跟随 Windows 应用模式；菜单置于托盘锚点上方（可见右缘距锚点 12px、底缘不低于工作区上方 8px）。macOS 使用原生 NSMenu 外观。
- 状态变化：Windows 每次打开菜单时按最新 Snapshot 渲染（MainWindow 轮询持续写入）；macOS 在菜单将展开时重建。
- 错误恢复：后端启动失败时 Windows 状态头停留"正在启动"（主窗口承载失败详情）；操作行始终可用。macOS 后端失败经 status=error 显示"状态待确认"。
- 非目标：不加载 WebView；不引入 HTTP 服务。

## Acceptance

- [x] demo 覆盖 6 状态 × 中英 × 深浅，浏览器实测通过（2026-10-31）。
- [x] 用户确认设计方向（2026-11-01），本 demo 为视觉基准。
- [x] Windows：托盘菜单实现 + `TrayMenuTests` / `TrayIconTests` 回归（含状态映射、定位、视觉树行渲染、键盘导航）全部 PASS；Release 自包含构建通过；实际托盘右键交互按 YML-029/032 先例由用户自验。
- [x] macOS：菜单栏同语义菜单实现；`TrayMenuStatus` 映射与文案回归随 CI `swift test` 执行（本机 Windows 无法本地运行）。

## Verification

- Windows：`dotnet run --project windows/Tests/YM_Link.Tests.csproj` → 全部 PASS（本地 Debug）；`python scripts/build_windows.py` → Release 构建通过（含 --native 真实 DLL 生命周期 PASS）；`--smoke-test` not-run（本机 UDP/单实例互斥量被用户正在运行的已安装版占用，退出码 4，进程 I:\my_program_files\YM_Link\YM_Link.exe，未触碰用户实例）。
- Windows 完整门禁 `harness verify`：python 47 OK / 3 skipped、repo check PASS、rustfmt 无 diff、cargo 除 `beacon_ports_fall_back_in_order` 外全过——该 1 项因同一已安装实例占 UDP 47610 报 AddrInUse，为既有环境性失败（YML-024/028 同因保留），用户关闭已安装版后可复跑转绿。
- macOS：`swift test` / .app 构建随三平台 CI；本机为 Windows，Swift 本地编译 not-run。混合 DPI 多显示器定位 not-run（本机单一缩放，待真机；理论偏差见 Evidence）。
- 真实第二设备层：not-applicable（状态头消费既有 Snapshot，真实 Quest 连接语义沿用已验收展示）。

## Evidence

- 2026-10-31：demo 初版交付于 `design/prototypes/desktop/2026-10-31-tray-menu/`，Chromium (IAB) 实测通过：右键 / 单击开菜单且定位符合规格（可见右缘距锚点 13px、底缘距任务栏 7px）、六状态 × 中英 × 深浅主题、镜像开关行、↑↓/Enter/Esc 键盘导航、退出 toast 与托盘图标自动恢复。截图保存在 ignored 会话工件，不入库。
- 编号说明：创建时本地 main 尚未含远端随后的 YML-031（文件首读）与 YML-032（右键闪退归档），同步发现撞号后改号为 YML-034；033 留给同批并行创建的文件读取吞吐任务改号。
- 2026-11-01：用户确认 demo 方向并授权实现（含跨平台统一）；范围加入 macOS 菜单栏，原"macOS 不在本任务内"的非目标作废。
- 2026-11-01 CR 第一轮（独立 subagent，范围=本任务全部变更文件）：8 项属实发现全部修复并经回归——P1 DpiForMonitor 单出参签名栈越界（改双 out uint）；P2 NotifyIcon e.X/e.Y 恒 (0,0)（改 CursorPosition seam，klip 同法）；P2 OpenAt 未减 ShellMargin 16（加 WindowPosition + 回归）；P2 图标二次右键重开（非动作关闭后 350ms 重开抑制）；P3 SystemTheme 缺值返回深色与注释相反（仅显式 0 深色）；P3 GreenRipple 无波纹（补 ScaleTransform+Opacity 环动画，尊重 ClientAreaAnimation）；P3 前台锁键盘失效（Show 后 SetForegroundWindow，拒绝则鼠标降级）；P3 高对比度（SystemColors 面板 + 单色状态点，随主窗口契约）。
- 2026-11-01 CR 第二轮（新独立 subagent，复核 8 项修复=全部确认 + 全量再审）：P1 行模板缺 ContentPresenter 致行文案不可见——已补 presenter（14/8 内缩对齐 demo），并暴露 Factory `SetValue+TemplateBindingExtension` 不求值的连带缺陷（键盘高亮背景同样失效），改 `SetBinding+RelativeSource TemplatedParent`，新增视觉树断言拦截此类回归；P1 macOS 测试 `Snapshot(status:)` 缺 9 参不可编译——改 `Snapshot.empty` 变更（已核实 memberwise init 无默认值）；P3 注册表测试残键（finally 删根树）；P3 Topmost 缺失（补）；P3 Alt-Tab 空条目（OnSourceInitialized 置 WS_EX_TOOLWINDOW）；P3 分隔线绿通道与 demo 差 1（对齐 #DDDFE3）；P3 波纹线性 vs ease-out（CircleEase）；P3 TrayIcon 死 localizer 字段（删除，TrayIcon 不再持文案）。已接受偏差：混合 DPI 定位 not-run 待真机；fps/码率恰为二分中点时 C# 远离零舍入 vs Swift half-to-even 的极端文案差异（真实快照不可达，仅记录）。
- 修复后回归：Windows 全部测试 PASS（含视觉树行渲染、主题注册表三态），Release 重构建通过。
- 2026-11-01 CR 第三轮（新独立 subagent，受审提交 442d1f5，实跑 Windows 测试套件 PASS）：17 项历史修复逐一对照代码确认落地无回归；资源四文件 9 键逐字节一致；新增 4 项 P3 加固全部处理——OpenAt 预测量宽度改 MenuWidth+ShellMargin×2（消折行内容误估隐患）；macOS 键位循环扩入 describe 消费的 status_starting/connected/paired/unknown 四键（防回退链漂移）；测试补负原点工作区 Place 用例、深色键盘高亮画刷断言（#34373C）与 GreenRipple 波纹环视觉树断言（新 StatusHeaderForTest seam，运动关闭时自动降级）。已接受并记录：托盘图标长按右键 >350ms 后释放会重开菜单的边界行为（固定抑制窗口的权衡）。
- 2026-11-01 CR 第四轮终审（新独立 subagent，受审 442d1f5 + 第三轮工作区叠加，实跑测试 PASS）：三组第三轮修复逐项核算确认（测量宽度 218 与真实 arrange 一致、13 键双语闭环、负坐标/高亮/波纹断言全对），同类面全量扫描无历史漏改，工作区叠加自洽；结论"可合入"。顺手处理 2 项 P3：高对比度环境下高亮断言改用 SystemColors.ControlDarkBrush 分支；波纹注释措辞由"mirrors"改"same looping-ring semantics"。随之 Release 重构建通过。两轮合计新增属实修复 6 项、测试加固 4 组、接受边界 2 项。

## Next

用户在实际任务栏右键自验托盘菜单（Windows 真机交互先例由用户自验）；三平台 CI 绿后本任务 verifying→done。后续候选（不阻塞本任务）：macOS 关窗收起菜单栏的生命周期统一（需真机）、混合 DPI 多显示器定位真机核验、高对比度下自绘菜单的可读性细调。
