# YML-025：显示器卡片选中与悬停 HTML 原型

Status: done
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (仅独立 HTML 视觉原型，不改 Rust、协议或原生应用；浏览器检查鼠标、原生 radio 键盘及窄屏；完整 harness verify 记录既有回归)

## Scope

用户报告 Windows 已选显示器再次 hover 时出现两条边缘线，要求先看 demo HTML。保留截图的显示器 / 音频 / 画质布局，选中卡片整块淡蓝，选中与 hover 共用一条边框；不改 WPF 或已安装应用。原型位于 `design/prototypes/desktop/2026-10-31-monitor-selection/index.html`，无外部依赖。

## Acceptance

- [x] 两块显示器可点击切换，选中卡片整块淡蓝并保留勾选标记。
- [x] 选中 + hover 无额外外围线，未选卡片 hover 有轻微反馈。
- [x] 原生 radio 键盘选择与内部可见焦点可用，窄屏无水平溢出。
- [x] 自包含 HTML 可直接打开；原生应用未修改；实际浏览器和仓库检查结果已记录。

## Verification

浏览器打开原型，检查默认 / hover / 选中 hover / 切换与 Tab、方向键；用窄面板检查响应式。自查 HTML / CSS 状态优先级及无网络请求；运行 `python3 scripts/harness.py verify`，提交推送后 `check --require-pushed`。WPF 构建 / 启动、Quest / LAN 为 not-applicable（未改原生或协议）；HTML 不作为原生缺陷修复验收。

## Evidence

2026-10-31：开工 main 干净，fetch 后 ff-only 已是最新。独立只读检查指出 WPF 选中边框与共享焦点 adorner 可形成双轮廓，hover 自身仅改背景，用户描述的直接触发因素尚未原生复现。此轮只交付讨论用 HTML；设计采用单层卡片边框，键盘焦点在卡片内部名称上显示。

- 浏览器实际打开与截图检查：900×540 双列、373px 与 320px 单列，无水平溢出。点击 RTK1600 后真实 `:hover` 为 true，计算样式为淡蓝 `rgb(234,243,252)`、单层 2px 蓝边，outline / box-shadow 均为 none；HWV6446 为白底。ArrowLeft 实际切回 HWV6446，焦点 / 内部名称 outline / live status 同步更新。最终恢复默认预览尺寸，保留可交互标签页。
- 独立只读审查覆盖现有 WPF 选中 / hover / 焦点样式来源，仅作原型方向依据，不宣称已修复原生问题。原型使用原生 radio / checkbox / select，无库、无外部资源与网络请求。
- `python3 scripts/harness.py verify`：Python 47 项通过 / 3 skipped、仓库 check 与 rustfmt 通过；Rust core 23 单测通过，discovery 8 通过 / 1 失败。失败 `beacon_ports_fall_back_in_order` 在 bind 第一个端口时 OS10048；随后只读确认运行中的 YM_Link 占用 UDP 47610。未关闭应用、未改测试或生产代码；后续 Rust 目标 not-run，不宣称完整 verify 通过。
- `git diff --check`、`python3 scripts/harness.py check` 通过；交付前执行 `check --require-pushed` 核对最终 main。此轮无 WPF / Rust / 采集变更，完整回归端口限制不阻断 HTML 原型交付。

## Next

HTML 原型已完成并在浏览器确认，用户随后明确授权基于 demo 开发；原生实现见 [YML-026](YML-026-monitor-selection-native.md)。
