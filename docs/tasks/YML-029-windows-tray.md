# YML-029：Windows 关闭收起到托盘与显式退出

Status: verifying
Workflow: behavior
Requires-Device: no
Tests: windows/Tests 实际编译窗口关闭 / 重复关闭 / 显式异步退出及后端排队生命周期，真实不可见 NotifyIcon 菜单 / 中英 / 资源销毁；Release / ABI / smoke；Rust 沿用 harness verify，无协议变更

## Scope

用户要求 Windows 点击 X 收起到右下角托盘，托盘右键提供退出。使用 .NET 内置 NotifyIcon / 原生菜单，复用当前 WPF 窗口和 Rust 后端，不增加第三方 UI 库。X / Alt+F4 收起后后台继续运行；右键“打开 YM_Link”或双击恢复，显式“退出”才走现有轮询取消 / 排队工作 drain / 后端销毁。菜单中英资源化并随语言选择更新。普通最小化按钮仍走系统行为；不新增自启、macOS 菜单栏或第二次启动激活等未请求功能。保留用户指定的手工窗口自验，不操作现有应用。

## Spec

- 给定正常启动且托盘可用，关闭窗口时取消关闭并隐藏；重复关闭不取消轮询 / 断开后端或清空所选显示器，仍持有单实例门禁。
- 给定窗口隐藏 / 最小化，托盘打开或左键双击恢复同一窗口；最大化状态保留，退出中的窗口不得被恢复。
- 给定选择托盘退出，首个请求进入现有异步清理；重复退出不重复销毁，完成后关闭窗口、移除托盘图标并退出。
- 托盘图标资源 / 组件初始化抛错时保持窗口可见，X 回退为原有正常退出，避免隐藏进程。重复启动 / smoke 不额外创建可见托盘；保持 WPF 的系统注销 / 关机行为。
- 中英切换同步原生菜单；托盘图标来自仓库同一应用图标，资源和原生组件在退出时释放。

## Acceptance

- [x] X 收起 / 打开恢复 / 显式退出的代码路径分离，保留后台状态和退出 drain / 重入保护。
- [x] 原生托盘 / 图标 / 中英菜单、语言更新与资源释放有自动回归；关闭回归 RED→GREEN。
- [x] WPF / BAML / 本地化 / 真实 ABI / Release 自包含构建与适用完整门禁有实际结果，保留本机端口限制。
- [ ] 开发后新独立 CR，属实问题修复复验；自查、任务 / STATUS、commit / push main 与严格推送门禁闭环。

## Verification

先在实际编译窗口未显示 / 后端未启动的现有 fixture 中写关闭回归并运行 RED；再实现最小关闭分支 / 托盘菜单与本地化，运行 GREEN、原生菜单组件（保持 Visible=false）、退出 / 重入 / 资源销毁检查。运行 `dotnet run --project windows/Tests/YM_Link.Tests.csproj`、`python3 scripts/build_windows.py` 和 `harness verify`；不关闭用户正在运行的开发版、不得把本机端口占用记为通过。开发完成后新 subagent CR，必要时修复并复审。手工 X / 托盘展开 / 实际恢复 / 高对比不自动操作（用户自验）；真实 Quest / 录屏 not-applicable。提交推送后 `check --require-pushed`。

## Evidence

2026-10-31 main 干净，fetch / ff-only 已最新。当前用户运行分区修正版 `dist/YM_Link-monitor-sections`，本轮标准构建目标 `dist/YM_Link` 可独立准备。现有 WindowClosing 统一进入取消 / DisposeAsync / Shutdown，尚无托盘。独立只读设计审查确认保留此异步清理最小，普通 X 仅增加隐藏分支；托盘使用系统组件并避免额外消息循环。Microsoft [Closing](https://learn.microsoft.com/en-us/dotnet/api/system.windows.window.closing) / [Shutdown](https://learn.microsoft.com/en-us/dotnet/api/system.windows.application.shutdown) 与 [NotifyIcon](https://learn.microsoft.com/en-us/dotnet/api/system.windows.forms.notifyicon) 文档支持该原生行为；不拦截系统会话结束。

- 关闭回归使用实际编译但未显示 / 未启动后端的 MainWindow。旧实现 RED 命中“X must not cancel backend polling when hiding to the tray.”；首个RED还暴露测试线程缺少正常WPF DispatcherSynchronizationContext导致原有 async void 清理续到线程池，测试补齐正确上下文后干净复现同一RED，未把夹具线程问题改成生产修复。
- GREEN：两次真实 Close 不取消轮询 / 不进入 closing / 不销毁 backend、不丢显示器选择；两次显式 Exit 最终等待异步清理后关闭并拒绝新工作。真实但不可见 NotifyIcon 回归覆盖内嵌图标、Open / Exit 菜单事件、仅左键双击恢复、三次中英切换不重复订阅、菜单 / 图标销毁、重复 Dispose、销毁后 Show 拒绝。未创建可见托盘或操作用户窗口。
- `dotnet run --project windows/Tests/YM_Link.Tests.csproj -c Release -- --native target/release/ymlink_desktop.dll` 通过；既有BAML / 原生选择 / 两语两尺寸 / 本地化 / 队列真实DLL drain等回归通过。`python3 scripts/build_windows.py` 退出0，Rust Release、同组Release回归、自包含win-x64 publish与图标验证通过。开发版在 `dist/YM_Link`，未启动 / 覆盖正在运行的分区版，也未运行会被当前单实例锁挡住的本机完整smoke。
- 添加 UseWindowsForms 仅供系统托盘和菜单，移除其隐式namespace防止WPF歧义；保留 WPF 原 manifest 的 PerMonitorV2 并对仅提示改用 WinForms DPI API 的 WFAC010 作有注释的单项排除，不改既有DPI行为。WindowLoaded加一次性门禁，恢复时不重复启动后台；菜单语言由现有 App.SetLanguageResources 同步。组件 / 内嵌图标在App退出释放，Smoke只构造 / 释放不可见托盘资源。
- 新独立CR基线 `3b90cd8f763b9a2b50edcb26c781ac6ee376b551`，审查完整工作区差异及构建 / 安装脚本假设；App blob `9735427881768d53c6d2e0bcd1005e2308f11a16`、MainWindow blob `020aa62f0ce62d3492e6673babeaafa60f31279f`、TrayIcon blob `dfdb73ebf6d728371a3b62ba95af87ebfca5e20b`。无属实待修问题；检查了框架静默Shell注册请求语义，普通Explorer重启由原生组件TaskbarCreated自动重加，未成立永久丢失问题，不增加私有反射或自定义Shell互操作。
- 本机完整 `harness verify`（RUST_TEST_THREADS=1）：Python47项 /3 skipped、check /rustfmt通过，core23、discovery8通过，端口回退用例因用户仍运行的分区版占用UDP47610而 OS10048；后续Rust目标not-run，保留失败，不关闭用户应用或用筛选测试代签。等待同源CI的不筛选完整回归及发布载荷 smoke。

## Next

实现、RED→GREEN、本地Release及新独立CR已完成；推送后核验同源CI的完整Rust / 原生发布启动 / 安装载荷，再归档最终记录。实际托盘展开 / X / 恢复 / 高对比仍由用户自行检查。
