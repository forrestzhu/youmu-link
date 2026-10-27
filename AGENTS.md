# YM_Link 开发约定

## 适用范围与记录门禁

- 本仓库推进 YM_Link 桌面伴侣应用的产品设计、实现、缺陷排查、测试验收与分发准备；仅在同一聊天、工作区或电脑上的无关操作不创建 YML 任务、不更新 `docs/STATUS.md`。
- 混合请求只记录 YM_Link 相关部分；影响验证的外部事件只在对应任务记录必要影响与结果。
- 与游目（Quest 端）联动的任务在[游目仓库](https://github.com/forrestzhu/youmu)以 YM 任务跟踪端侧工作，本仓库记录桌面 / 协议侧，双方互链。

## 产品与范围

- YM_Link：Windows / macOS 桌面应用，连接 Meta Quest 3 / 3S 上的游目；产品全称 Youmu VR Player 桌面伴侣 YM_Link。
- 先读 [当前进度](docs/STATUS.md)、[产品定义](docs/product/YM-LINK.md) 和当前任务；用 [文档索引](docs/README.md) 定位材料。
- 阶段目标见 [ROADMAP](docs/product/ROADMAP.md)；未立项的想法先进 ROADMAP 候选，不直接动代码。
- 本仓库暂闭源：不公开、不对外分发、不添加 LICENSE；转 GitLab 分发与开源时机由用户决定后更新本条。

## 工作闭环

1. 运行 `git status --short --branch`，保留用户已有修改；开工先 fetch 远端 main 并 ff-only 同步。
2. 用 `python3 scripts/harness.py tasks` 选已有任务；新建用 `python3 scripts/harness.py new <topic> --title "标题" --workflow maintenance|behavior|feature`；需要真实头显或第二台电脑验收时加 `--requires-device`。
3. 任务状态 `ready → doing → verifying → done`；受外部条件限制记 `blocked` 与解除条件；done 归 `docs/tasks/archive/` 并更新链接；被新设计取代用 superseded 并声明 Superseded-By。
4. 小步修改，先复现再修复；协议与发现等行为改动必须带 cargo 单元 / 集成测试，任务 `Tests:` 字段声明目标或写明不适用原因。
5. 代码稳定后运行 `python3 scripts/harness.py verify`（harness 回归 + 仓库检查 + rustfmt + cargo test）；只改文档时只跑 `check`。记录实际结果与限制，未运行写 `not-run`。
6. 自查 diff；证据集中在任务或 `docs/evidence/` 报告；`docs/STATUS.md` 简述结果、限制和下一步。
7. 默认直接在 `main` 开发并 commit / push；交付前运行 `python3 scripts/harness.py check --require-pushed` 确认 HEAD 等于远端 main。不强推，保留用户修改。

## 技术边界

- 技术方向：后端统一使用 Rust（`ymlink-core` 协议核心、`ymlink-desktop` 桌面状态与平台适配）；macOS 界面使用 SwiftUI / AppKit，Windows 使用 WPF / .NET 8，两者经 C ABI 读取快照与发送命令，不引入 WebView。现行决策见 [ADR-0002](docs/engineering/decisions/0002-native-desktop.md) 与 [ADR-0003](docs/engineering/decisions/0003-windows-native.md)，[ADR-0001](docs/engineering/decisions/0001-platform.md) 保留历史。Quest 端在游目内以 Kotlin 实现同一协议，协议以 [PROTOCOL](docs/engineering/PROTOCOL.md) 为唯一规范，双端互操作测试必须覆盖。
- 互联模型参考内部项目 klip 的已验证设计（UDP 广播 beacon、CBOR 报文、Noise 加密、SPAKE2 短码配对）；借鉴设计思想，**不复制 klip 源码**，实现独立编写。
- 发现报文不携带设备身份；身份与信任只在加密层交换。新协议字段先改 PROTOCOL 文档并升版本，再写两端实现。
- 不引入 HTTP 服务框架、不做常驻云依赖；公网穿透 / 中继如需引入必须先立 ADR。
- 屏幕采集走平台 API（macOS ScreenCaptureKit / Windows Graphics Capture），解码能力与延迟按真实硬件验证，不从平台名称推断。
- 不记录用户文件路径、不把原始日志入库；UTF-8、LF；不提交 SDK、缓存、构建产物、签名密钥。
- 桌面 UI 文案中英资源化；macOS 资源位于 `macos/Sources/YM_Link/Resources/{en,zh-Hans}.lproj/Localizable.strings`，Swift 负责本地展示，业务状态来自 Rust。demo 原型文案可用中文硬编码。
- Windows 中英资源位于 `windows/YM_Link/Resources/{en,zh-Hans}.json`；`python3 scripts/build_windows.py` 构建忽略入库的 `dist/YM_Link`，`dotnet run --project windows/Tests/YM_Link.Tests.csproj` 验证原生模型 / 本地化；改 Windows 界面补构建与实际窗口启动核验。当前 x64 自包含开发目录不等于安装器 / 签名分发就绪。
- macOS 本地构建运行 `python3 scripts/build_macos.py`，产物为忽略入库的 `dist/YM_Link.app`；目标 macOS 14+，临时签名不等于分发就绪。改动原生界面时补跑 Swift 测试和 `.app` 构建 / 启动核验；部署目标与实测系统分别记录，不推断未测试的 macOS / Intel 兼容性。

## Code Review Rules

- 按任务风险与用户要求确定审查范围和轮次；要求独立 subagent / 多轮时，记录各轮受审提交、发现、核验与复审结果。
- 检查异步竞态与任务取消、socket 生命周期与端口回退、广播风暴与回环过滤、限速 / 冷却、大文件分块与断点、跨平台 API 差异（macOS / Windows）。
- 协议改动核对双端实现与 PROTOCOL 一致性、版本兼容矩阵与降级行为。
- 涉及真实设备的验收记录头显型号 / 系统版本与电脑系统版本，不以单机 loopback 测试代替双机 LAN 验收。
