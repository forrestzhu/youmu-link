# YML-026：显示器卡片原生选中与焦点样式

Status: superseded
Superseded-By: [YML-028](YML-028-monitor-card-sections.md)
Workflow: maintenance
Requires-Device: no
Tests: windows/Tests/YM_Link.Tests.csproj (既有生产 BAML、选中绑定与双语两尺寸渲染)；cargo 沿用 harness verify，未改 Rust / 协议；真实窗口验证 hover / 键盘焦点

## Scope

用户明确授权基于 [YML-025 原型](YML-025-monitor-selection-demo.md) 开发。仅改 Windows 显示器卡片：未选中白底 / 中性 hover、选中整卡淡蓝 / 单层蓝边 / 勾选，选中 hover 保持；固定边框 / 内边距避免选择位移，使用显示器专属键盘 FocusVisualStyle 在卡片内部显示短下划线，保留原生键盘选择和系统高对比资源。未改设备 / 共享行、Rust、协议、macOS、安装器或已安装载荷。用户随后明确选择“只完成代码和构建，窗口由我检查”；后续手工窗口交互 / hover / 键盘验收由用户执行，本轮不再进行窗口自动化。

## Acceptance

- [x] 未选显示器内部无蓝色缩略图背景；选中卡片整块淡蓝，hover 不叠边框。
- [x] 保留 ListBox 选择 / 无障碍语义和键盘专用内部焦点样式；人工 mouse / arrow / Tab 交互由用户检查，本轮 not-run。
- [x] 既有生产 XAML / BAML、双语两尺寸实际 Window.Show 渲染与布局回归通过，自包含 Release 构建通过；完整应用手动启动由用户检查。
- [x] 完整 harness verify、自查、独立审查和推送一致性检查有实际结果；本机限制和未验项目保留。

## Verification

先运行现有 WPF 测试并观察旧应用复现双轮廓；修改后通过 `YML_UI_EVIDENCE_DIR` 导出既有原生 Window.Show 样本检查两语 / 两尺寸，运行 `python3 scripts/build_windows.py`、完整 `python3 scripts/harness.py verify`。最初拟正常退出空闲旧应用释放 UDP 端口，但用户 Esc 停止窗口自动化并随后明确要求窗口由自己检查，因此不再关闭 / 启动 / 操作应用。完整应用手动启动及 mouse / keyboard 为 not-run（用户自行检查）。提交推送 main 后 `check --require-pushed`。Quest / LAN / 录屏 not-applicable（纯界面样式）；不将原生样本渲染代签完整应用人工交互或真实镜像。

## Evidence

2026-10-31 开工 main 干净，fetch / ff-only 已是最新。现有 `dotnet run --project windows/Tests/YM_Link.Tests.csproj` 通过并导出 baseline。CUA 恢复最小化的旧安装版，实际截图复现选中卡片外围双蓝线，未选卡片仍有蓝色内层；应用无连接设备、无镜像。独立只读分析确认共享 `ControlFocus` 的负 margin 外框与卡片选中边框叠加；只为显示器覆盖焦点样式，避免影响其余列表。

后续用户指出本轮整卡同色是反向实现，需要图标预览与名称之间的分区 / 分隔线。整卡淡蓝视觉由YML-028取代；本任务及YML-027的构建 / 测试 / CR绿色只证明当时实现的工程结果，不证明误读的产品方向正确。

- 修改后 Debug WPF 回归通过：生产 BAML 事件连接器 / 列表行、显示器选中、双语两尺寸 Window.Show 渲染 / 布局、下拉选择 / 恢复状态。`dist/evidence/monitor-selection-{before,after}/` 保留本机忽略入库的样本；主代理及独立审查均抽检中文宽版和英文窄版 mirror 样本，整卡白底 / 淡蓝与单蓝边符合 demo。
- `python3 scripts/build_windows.py` 退出 0：Rust Release、Release WPF / 本地化 / 真实 DLL ABI 生命周期 / 并发快照命令与 dispose、自包含 win-x64 publish、EXE 图标验证均通过。产物 `dist/YM_Link/YM_Link.exe`（170496 bytes），未覆盖已安装版本，未启动开发版、未生成安装器或发布 Release。
- `python3 scripts/harness.py verify`：Python47 项 / 3 skipped、仓库 check、rustfmt 通过；Rust core23、discovery8通过，`beacon_ports_fall_back_in_order` 在 bind 47610 时 OS10048。随后 netstat 确认旧安装版进程仍占用 UDP47610。保留失败，不宣称本机完整 verify 通过。
- 追加 `cargo test --workspace --all-targets -- --skip beacon_ports_fall_back_in_order`：core / discovery / files / protocol / session / windows_platform 的58项通过，1 filtered；desktop 44 passed / 5 failed / 2 ignored，后续 target not-run。5项均在 `wait_state` 超时：`authenticated_eof_disconnects_without_an_alert_but_auth_and_protocol_failures_still_surface`、`authorized_folders_survive_backend_restart_and_revocation_stays_revoked`、`native_runtime_pairs_windows_macos_and_headset_roles_in_both_directions`、`rejection_before_ready_keeps_a_route_for_an_explicit_fresh_retry`、`wrong_code_preserves_the_display_window_and_five_attempts_limit_new_connections`。随后 `cargo test -p ymlink-desktop --lib runtime::tests -- --test-threads=1` 的33项全部通过（11.07s），含上述5项；保留并发失败，不据此认定根因或修改 Rust。
- 独立只读审查受审实现 `4469f28`（基于 main `1692af1` 的 XAML 差异，blob `0708a73e9e9dcdc4cda5be21e2e37bb6f45501d8`），核对状态优先级、稳定几何、键盘专属 FocusVisualStyle、原生选择 / 无障碍及高对比资源映射，两张原生样本检查无新增问题。无业务 / 协议代码或资源文案变更，不新增镜像样式 setter 的测试。
- `4469f28` 已推送；同源 [CI run 41184260855](https://github.com/forrestzhu/youmu-link/actions/runs/41184260855) 的 Linux checks（Python / check / rustfmt / 不过滤的完整 Rust 测试）已 success，Windows / macOS 构建当时仍在进行，未代填通过。`git diff --check` / 仓库 check 通过；最终记录提交后执行 `check --require-pushed`。
- 后续用户追加完整验证与新独立 CR 见 [YML-027](YML-027-monitor-selection-cr.md)：旧4469f28的run最终因文档推送 cancelled；同生产实现f51e700的run41184493604最终三平台 / 原生构建 / 启动 / 安装器与gitleaks全success，Windows Rust117 /2 ignored /0 filtered、macOS Rust110 /1 ignored /0 filtered、Swift9通过。历史本机失败保留，不再将当时进行中的状态作为当前结论。

## Next

代码与自包含开发版已交付，任务按用户限定的“代码和构建”范围完成；后继完整自动验证与追加独立 CR 已由YML-027关闭。运行 `dist/YM_Link/YM_Link.exe` 自查完整窗口的 hover / 点击 / Tab / 方向键，高对比手动检查 not-run。安装版未覆盖，不将后台 CI 或原生渲染样本代签用户人工验收。
