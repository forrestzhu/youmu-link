# YM_Link Windows 托盘右键菜单重设计 demo

状态：设计讨论稿（2026-10-31），参考 klip 托盘菜单（`klip/src-windows/KlipApp/Shell/TrayMenuWindow.cs` 2026-10-12 已批准 Fluent shell）的视觉规格，把当前 WinForms `ContextMenuStrip`（仅"打开 YM_Link / 退出"）重设计为自绘 WPF 弹窗方向。由 [YML-034](../../../../docs/tasks/YML-034-tray-menu-redesign.md) 跟踪；用户确认设计方向后才进入原生实现。

## 打开与体验

直接打开 `index.html`，或在仓库根目录运行：

```sh
python3 -m http.server 8768 --bind 127.0.0.1 --directory design/prototypes/desktop/2026-10-31-tray-menu
```

浏览器访问 `http://127.0.0.1:8768/`。无第三方资源与网络依赖；设备名、速率与时间均为合成示例。

- 在模拟任务栏的 YM_Link 托盘图标上右键（或单击）打开菜单；↑ ↓ 选择、Enter 确认、Esc 或点击空白处关闭，与 klip 菜单行为一致。
- 顶部控件仅用于讨论：六种连接 / 镜像状态、菜单中英文、Windows 深浅主题，以及可选的"镜像开关行"方向（klip 同款 32×18 开关，暂停镜像不出主窗口）。
- 点击"打开 YM_Link / 退出"演示对应反馈（Win11 通知样式；退出后托盘图标短暂消失并自动恢复）。

## 设计规格（对齐 klip TrayMenuWindow）

- 卡片：宽 220、圆角 10、深色 `rgba(37,40,45,.94)` / 浅色 `rgba(249,249,250,.94)`、1px 边框 `#45484e` / `#dde0e4`、投影 blur 20 / depth 5 / 30%。
- 行：高 30、水平边距 6、圆角 6、hover `#34373c` / `#ebedf0`；正文 12pt，说明 10.5pt，muted `#8d95a2` / `#949ba7`。
- 状态头：8px 状态点沿用主窗口语义（配对未连接绿点波纹、连接实心绿、异常红），第二行 10.5pt 补充设备 / 速率或恢复指引；异常附"打开主窗口查看详情"。
- 菜单定位：可见右缘在锚点左 12px、底缘不低于任务栏上方 8px（klip ComputePlacement）。
- 状态点颜色在浅色主题沿用应用 `SuccessInk #24733B`；深色主题用 `#6ccb5f` 等价提亮。中英文案进 `windows/YM_Link/Resources/{en,zh-Hans}.json` 资源化。

## 待用户决策

1. 菜单项取舍：是否加入"镜像开关行"（demo 中的可选方向）或其他快捷动作。
2. 深浅主题：当前应用仅浅色；托盘菜单是否跟随 Windows 深色（demo 已给出两套）。
3. 状态头第二行的信息密度（设备名 / fps / 码率）。

10-31 已在 Chromium 实测：右键 / 单击开菜单、六状态切换、中英、深浅、镜像开关、↑↓/Enter/Esc 键盘导航、退出 toast 与图标恢复均通过。未验证真实 WPF 实现、读屏播报与高对比度主题。
