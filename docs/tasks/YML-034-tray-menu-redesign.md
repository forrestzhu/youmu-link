# YML-034：Windows 托盘右键菜单重设计（Klip 风格 demo）

Status: doing
Workflow: feature
Requires-Device: no
Tests: not-applicable (本阶段仅交付 HTML 设计 demo，不改产品代码；进入原生实现阶段时按窗口启动核验与 WPF 模型测试补充)

## Scope

把 Windows 托盘右键菜单从 WinForms `ContextMenuStrip`（仅"打开 YM_Link / 退出"两项）重设计为参考 klip 托盘菜单（`klip/src-windows/KlipApp/Shell/TrayMenuWindow.cs`，2026-10-12 demo A Fluent shell）的自绘 WPF 弹窗：圆角卡片、行 hover、状态头、快捷键/说明列。本阶段先交付交互 HTML demo 供用户确认设计方向；原生 WPF 实现另起后续提交。

非目标：本任务不改动 Rust core / 协议；不新增托盘菜单业务动作（是否加入镜像开关等留待用户在 demo 上反馈）；macOS 菜单栏不在本任务内。

## Spec

- 用户入口：任务栏托盘图标右键；左键双击保持恢复主窗口。
- 菜单结构（demo 版，待用户确认）：
  - 顶部状态头：YM_Link 名称 + 连接状态（正在搜索 / 等待连接 / 已连接 + 设备名 / 镜像运行中），状态点颜色沿用主窗口 SuccessInk / Accent 语义。
  - 操作行：打开 YM_Link；退出（底部独立分隔）。
  - 视觉规格对齐 klip：卡片宽 220、圆角 10、行高 30、行圆角 6、hover 面、12pt 正文 + 10.5pt 说明文字；浅色主题为主（当前应用仅浅色），深色作为 Windows 深色任务bar 下的适配方向一并展示。
- 状态变化：状态头随 Snapshot（status / peers / mirror）实时刷新；无可用动作时操作行不降级隐藏。
- 错误恢复：后端不可达时状态头显示"状态待确认"，操作行仍可用。
- 非目标：不加载 WebView；不引入 HTTP 服务。

## Acceptance

- [ ] demo 覆盖至少 4 种连接状态 × 中英双语 × 浅色主题（深色为方向展示），用户可交互（右键托盘图标打开菜单、hover、点击操作行）。
- [x] 已交付：6 状态 × 中英 × 深浅 × 可选镜像开关行，浏览器实测通过（见 Evidence）。
- [ ] 用户确认设计方向后，后续原生实现任务引用本 demo 为视觉基准。

## Verification

- demo：浏览器打开 `design/prototypes/desktop/2026-10-31-tray-menu/index.html`（见其 [README](../../design/prototypes/desktop/2026-10-31-tray-menu/README.md)），人工核验交互与状态切换（无自动化断言；HTML demo 不入 harness 门禁）。
- 原生实现阶段：`dotnet run --project windows/Tests/YM_Link.Tests.csproj` + `python scripts/build_windows.py` + 实际窗口启动核验（届时改写本节）。
- 真实第二设备层：not-applicable（纯视觉 demo）。

## Evidence

- 2026-10-31：demo 初版交付于 `design/prototypes/desktop/2026-10-31-tray-menu/`，Chromium (IAB) 实测通过：右键 / 单击开菜单且定位符合 klip ComputePlacement（可见右缘距锚点 13px、底缘距任务栏 7px）、六状态 × 中英 × 深浅主题、镜像开关行、↑↓/Enter/Esc 键盘导航、退出 toast 与托盘图标自动恢复；菜单定位实测 (222×126) @ (507,342)、taskbarTop 475。截图保存在 ignored 会话工件，不入库。用户反馈待记录。
- 编号说明：创建时本地 main 尚未含远端随后的 YML-031（文件首读）与 YML-032（右键闪退归档），同步发现撞号后改号为 YML-034；033 留给同批并行创建的文件读取吞吐任务改号。

## Next

用户查看 demo 并反馈：菜单项取舍（是否加镜像开关行等快捷动作）、深浅主题取舍、状态头信息密度；确认后建原生 WPF 实现任务并引用本 demo。
