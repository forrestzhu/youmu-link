# YML-018：Windows 界面与 macOS 视觉对齐

Status: blocked
Workflow: maintenance
Requires-Device: yes
Tests: windows/Tests (生产 WPF 模板 / 选择绑定 / 本地化回归)；不改 Rust / 协议行为，cargo 使用完整门禁回归

## Scope

以当前 macOS ContentView 为视觉权威，优化 Windows WPF 的窗口布局、控件、设备 / 显示器卡片、共享和传输空态，统一标题层级、留白与分段导航。保留 Windows 原生窗口、文件选择、本机共享确认、键盘 / 无障碍、中英资源化及所有业务命令；不引入 WebView 或 UI 库。YML-017 性能排查保持暂停，不恢复性能采样。

## Acceptance

- [x] Windows 使用与 Mac 一致的侧栏设备、顶部标题与分段导航、显示器卡片以及共享 / 传输层级。
- [x] WPF 按钮、输入框、选择器、音频开关与进度条统一，保留焦点 / 禁用 / 高对比颜色映射。
- [x] 保留配对、设备连接 / 删除、显示器选择、可复制的共享路径与传输操作；中英、窄窗口与长内容可用。
- [x] Windows 模型 / 模板测试、构建、生产XAML Window.Show / client渲染检查完成；完整 harness verify、独立审查与diff有记录。
- [ ] Windows11用户机完整应用交互、默认1120宽窗、实际键盘 / 系统高对比补验。

## Verification

- 对照 macOS ContentView 和 WPF 当前代码确定差异；构建后在Windows runner的原生Window.Show样本中检查三页、中英、窄窗口、卡片选择与焦点契约，Windows11实际键盘另验；截图若可用则检查实际渲染，不以浏览器仿制代验。
- `dotnet run --project windows/Tests/YM_Link.Tests.csproj` 检查真实 WPF 控件模板 / 更新绑定及既有模型 / 本地化；`python3 scripts/build_windows.py` 构建并实际启动；smoke 验证真实 Rust ABI。
- 稳定后 `python3 scripts/harness.py verify`；独立 subagent 审查涉及的 WPF 绑定、状态、键盘 / 高对比与布局。
- 不改协议或采集实现；真实 Quest LAN / 镜像 / 文件传输 not-applicable（沿用 YML-003 待验层），不将 UI 构建代签硬件行为。

## Evidence

2026-10-31 开工 main 干净，fetch 后 ff-only 从 4c1f94b 同步至 2c284ec。当前 Mac 是明确既有视觉权威，不创建新的视觉世界。Windows 现状为默认 WPF 控件、传统 TabItem、下拉显示器加单个大预览、设备选择后独立操作区；Mac 是头部标题 / 分段导航、直接显示器卡片、设备卡片与居中空态。

## Next

blocked：Windows11用户机当前不可连接；解除条件是Windows电脑上线后使用f9df8d3新版安装包，补完整code-behind交互 /默认1120窗口 /键盘与高对比验收。CI自动层与原生渲染已通过；不恢复YML-017性能采样。

本机完整 `harness verify` 通过：Python47、Rust100 passed /1 ignored、仓库检查及rustfmt。机械样式扫描返回空列表（XAML能力有限，不能代替Windows渲染）。首轮独立审查发现2项P2：headers-only TabControl损失原生Tab自动化语义、高对比下拉高亮不可辨；已改为原生RadioButton分段选择器并补成对选中前景/背景token，待复审及Windows回归。复审确认两项关闭，新增测试无确定性编译问题；完整按键遍历和系统高对比渲染仍待人工。构建与渲染验证由Windows CI运行，本机Darwin无dotnet；首轮fixture尾空格失败、第二轮并发取消及最终viewport修复见[验证报告](../evidence/2026-10-31-windows-ui-alignment.md)。

最终f9df8d3的Windows103 passed /2 ignored、WPF模型 /行绑定 /两语两尺寸24状态、真实ABI、Release自包含 /smoke及安装 /启动 /卸载均通过。24 PNG回读client为1008×681、844×541各12张，主代理及独立审阅共抽检12张，无新增视觉阻断。Mac同源Rust100 passed /1 ignored、Swift9及DMG /ZIP通过；API状态显示存在延迟，逐项日志与产物已核验。安装包校验及交付见验证报告。
