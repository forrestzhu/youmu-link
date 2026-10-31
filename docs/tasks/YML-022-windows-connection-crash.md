# YML-022：Windows 收到 VR 连接时闪退

Status: verifying
Workflow: behavior
Requires-Device: yes
Tests: not-applicable (不改 Rust 或协议；新增 WPF 生产编译 BAML 行实例化回归，以 dotnet run --project windows/Tests/YM_Link.Tests.csproj -c Release 验证；完整 Rust 回归由 harness verify 执行)

## Scope

修复 Windows 安装版收到 VR 连接、设备列表首次出现行时的进程闪退，补足真实编译 XAML 的事件连接回归，构建可供用户重新连接的修复版。保留现有配对与配置，不恢复 YML-017 性能排查，不改变广播身份隐私或配对协议。用户追加的 VR 名称 / 系统类型显示问题一并追踪，先核对 Quest 实现与协议边界。

## Spec

给定 Windows 主窗口已启动，当发现 / 连接头显、设备列表开始实例化设备行时，应显示设备卡片与正确的连接 / 断开按钮，不得因 XAML connectionId / 类型转换异常终止进程；删除信任的菜单及共享 / 传输行原有事件继续可用。回归必须加载生产编译 BAML，不能仅依赖剥离事件后的 loose XAML 预览。保持协议 v4，发现报文不携带身份；加密 Hello 已包含电脑名称和 platform，检查 VR 何时接收 / 展示这些字段。

## Acceptance

- [x] 自动回归复现与系统记录相同的 XAML 崩溃，修复后编译设备 / 共享 / 传输行加载通过。
- [ ] 完整门禁、Windows Release 构建 / ABI / smoke 与实际窗口启动通过，独立审查无阻断。
- [ ] 真实 VR 重新连接不闪退；设备型号、系统版本与结果记入 evidence，不以合成行代签 LAN。
- [ ] 说明并处理 VR 设备名称 / 系统类型显示的实际原因与适用边界。

## Verification

先新增实际编译行回归并运行 Release 观察 red，再最小修复事件连接并观察 green。稳定后执行 `python3 scripts/harness.py verify`、`python3 scripts/build_windows.py --smoke-test`、独立代码审查与实际窗口启动。真实 Quest 点击连接需单独验证；未执行记 not-run。仅记录脱敏异常类型和调用位置，不将原始系统日志、用户路径、IP 或设备名称入库。

## Evidence

2026-10-31 基于 `d21faa9`，初始工作区干净，fetch / ff-only 已同步。Windows Application 事件 1026 在本机时间 13:33:01 / 13:33:56 两次记录相同未处理异常：XamlParseException → InvalidCastException（Button 强转 DockPanel），栈为 MainWindow.IComponentConnector.Connect → FrameworkTemplate.LoadTemplateXaml → VirtualizingStackPanel.Measure。原生 / Rust 崩溃证据未见，定位于 WPF 设备行的编译事件连接。独立只读核验确认 MainWindow.xaml:20 样式 Setter 内 ContextMenu / MenuItem 的 Click 被编译为 DockPanel cast；旧预览测试移除所有事件，因此未覆盖此路径。

新增 `MaterializeCompiledRows`，实际构造生产 MainWindow 并布局设备 / 共享 / 传输行，不 Show、不启动后端。首次 `dotnet run --project windows/Tests/YM_Link.Tests.csproj -c Release` 退出 1，复现同样 XamlParseException / Button→DockPanel，定位 xaml:20。将 Setter 内 MenuItem 的直接 Click 改为原生 EventSetter，生成代码改走 IStyleConnector，错误 cast 消失；同命令退出 0，旧双语布局与新编译 BAML 回归通过。测试复用夹具时关闭语言选择写入，保持生产事件而不调用语言保存或后端；测试关闭前解除生产 Closing，避免测试异步清理竞争。

首轮 `harness verify` 的 Python 47 项 /3 skipped、check、rustfmt 均通过；Rust `oversized_frame_is_rejected_before_allocation_and_large_send_keeps_nonce` 在 loopback listener bind 处遇 Windows OS 10055（socket 缓冲 /队列资源不足），保留首次失败。未改 Rust；以 `RUST_TEST_THREADS=1` 串行重跑同一完整 `harness verify`，退出 0，Rust 107 passed /2 ignored，Python 47 项 /3 skipped，全部门禁通过。

`python3 scripts/build_windows.py --smoke-test` 退出 0：Release 原生编译 BAML /中英布局、真实 DLL 生命周期 /并发 drain、x64 自包含发布、窗口图标与真实 Rust startup / snapshot / shutdown smoke 通过。独立只读审查基线 d21faa9→当前候选，未发现阻断；确认 EventSetter 的 sender、DataContext、Trusted 守卫与 ContextMenu PlacementTarget 语义保留，无既有 MenuItem 样式被覆盖。实际窗口与真实 VR 重连另记录，当前不以自动回归代签。

Quest 只读核对：匿名 LinkPeer 不含 name /platform，配对 LinkClient 已有 hello.name，但配对页 `pairingHost ?: pairingName` 优先显示地址，且状态缺 pairingPlatform；已认证 /已配对视图已有名称和系统。用户要求先基于现有 UI 交付 HTML Demo，设计阶段在游目仓库 YM-331 跟踪；不在本次 Windows 崩溃补丁扩展协议。

## Next

完成 BAML 红绿回归、最小修复与构建，交付实际窗口供 VR 重连验证；并行检查 Quest 名称 / platform 展示，不为展示身份扩展明文 beacon。
