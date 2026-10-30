# YML-018：Windows 界面与 macOS 视觉对齐

Status: verifying
Workflow: maintenance
Requires-Device: yes
Tests: windows/Tests (生产 WPF 模板 / 选择绑定 / 本地化回归)；不改 Rust / 协议行为，cargo 使用完整门禁回归

## Scope

以当前 macOS ContentView 为视觉权威，优化 Windows WPF 的窗口布局、控件、设备 / 显示器卡片、共享和传输空态，统一标题层级、留白与分段导航。保留 Windows 原生窗口、文件选择、本机共享确认、键盘 / 无障碍、中英资源化及所有业务命令；不引入 WebView 或 UI 库。YML-017 性能排查保持暂停，不恢复性能采样。

## Acceptance

- [x] Windows 使用与 Mac 一致的侧栏设备、顶部标题与分段导航、显示器卡片以及共享 / 传输层级。
- [x] WPF 按钮、输入框、选择器、音频开关与进度条统一，鼠标 / 键盘焦点 / 禁用 / 高对比状态可辨识。
- [ ] 保留配对、设备连接 / 删除、显示器选择、可复制的共享路径与传输操作；中英、窄窗口与长内容可用。
- [ ] Windows 模型 / 模板测试、构建、实际窗口启动 / 视觉检查完成；完整 harness verify、独立审查、diff 与推送门禁有记录。

## Verification

- 对照 macOS ContentView 和 WPF 当前代码确定差异；构建后在真实 Windows 原生窗口检查三页、中英、窄窗口、卡片选择与键盘焦点；截图若可用则检查实际渲染，不以浏览器仿制代验。
- `dotnet run --project windows/Tests/YM_Link.Tests.csproj` 检查真实 WPF 控件模板 / 更新绑定及既有模型 / 本地化；`python3 scripts/build_windows.py` 构建并实际启动；smoke 验证真实 Rust ABI。
- 稳定后 `python3 scripts/harness.py verify`；独立 subagent 审查涉及的 WPF 绑定、状态、键盘 / 高对比与布局。
- 不改协议或采集实现；真实 Quest LAN / 镜像 / 文件传输 not-applicable（沿用 YML-003 待验层），不将 UI 构建代签硬件行为。

## Evidence

2026-10-31 开工 main 干净，fetch 后 ff-only 从 4c1f94b 同步至 2c284ec。当前 Mac 是明确既有视觉权威，不创建新的视觉世界。Windows 现状为默认 WPF 控件、传统 TabItem、下拉显示器加单个大预览、设备选择后独立操作区；Mac 是头部标题 / 分段导航、直接显示器卡片、设备卡片与居中空态。

## Next

Windows CI 运行生产 XAML 实窗渲染 / 模板与构建，下载 PNG 核验；保留真实 Windows 11 用户交互待验边界。

本机完整 `harness verify` 通过：Python47、Rust100 passed /1 ignored、仓库检查及rustfmt。机械样式扫描返回空列表（XAML能力有限，不能代替Windows渲染）。首轮独立审查发现2项P2：headers-only TabControl损失原生Tab自动化语义、高对比下拉高亮不可辨；已改为原生RadioButton分段选择器并补成对选中前景/背景token，待复审及Windows回归。构建与渲染验证由Windows CI运行，本机Darwin无dotnet。
