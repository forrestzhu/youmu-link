# 文档 / 源码索引

按需读取，避免每轮扫描整个工作区。历史材料在 `docs/history/`（如建立）与任务 archive，不自动作为新需求。

| 主题 | 文档 |
| --- | --- |
| 当前进度与未结验收 | [STATUS](STATUS.md) |
| 产品定义（YM_Link） | [product/YM-LINK](product/YM-LINK.md) |
| 路线图与里程碑 | [product/ROADMAP](product/ROADMAP.md) |
| 系统架构 | [engineering/ARCHITECTURE](engineering/ARCHITECTURE.md) |
| 互联协议规范 | [engineering/PROTOCOL](engineering/PROTOCOL.md) |
| 测试策略 | [engineering/TESTING](engineering/TESTING.md) |
| 技术决策记录 | [engineering/decisions/](engineering/decisions/) |
| 当前桌面技术栈 | [ADR-0002：macOS 原生界面与 Rust 后端](engineering/decisions/0002-native-desktop.md) |
| Windows 原生技术栈 | [ADR-0003：Windows 原生界面与 Rust 平台适配](engineering/decisions/0003-windows-native.md) |
| macOS 实施顺序与验收合同 | [plans/2026-10-27-macos-native](plans/2026-10-27-macos-native.md) |
| macOS 构建、启动与头显测试桩 | [开发指南](engineering/DEVELOPMENT.md) |
| 开发流程与 harness | [harness/README](harness/README.md) |
| harness 改进提案 | [harness/IMPROVEMENT](harness/IMPROVEMENT.md) |
| 打包与安装器 | [DISTRIBUTION](engineering/DISTRIBUTION.md) |
| 应用图标与正式矢量 | [资源与再生](../assets/app-icon/README.md)、[原生 / CI 验收](evidence/2026-10-30-app-icon.md) |
| 任务记录 | [tasks/](tasks/)（归档在 `tasks/archive/`） |
| 当前 macOS 实现任务 | [YML-002](tasks/YML-002-macos-native.md) |
| 当前 Windows 实现任务 | [YML-003](tasks/YML-003-windows-native.md)、[实施计划](plans/2026-10-27-windows-native.md) |
| 开源与自动测试打包 | [YML-004](tasks/archive/YML-004-opensource-ci.md)、[实施计划](plans/2026-10-27-opensource-ci.md)、[历史 / 许可 / CI 报告](evidence/2026-10-27-opensource-ci.md)、[敏感信息审计](evidence/2026-10-27-sensitive-information-audit.md)、[历史邮箱修正与 SHA 映射](evidence/2026-10-27-history-email-rewrite.md) |
| gitleaks 防泄漏 CI | [YML-005](tasks/archive/YML-005-ci-gitleaks.md) |
| 源码公开剩余项 | [公开准备复查](evidence/2026-10-27-public-readiness.md) |
| 公开状态与实际云端执行 | [公开开发预览与首次 CI](evidence/2026-10-27-public-launch.md)、[SECURITY](../SECURITY.md)、[CONTRIBUTING](../CONTRIBUTING.md) |
| 框架与 demo 启动记录 | [YML-001（归档）](tasks/archive/YML-001-bootstrap.md) |
| demo 原型 | [../design/prototypes/](../design/prototypes/) |

游目（Quest 端）仓库：https://github.com/forrestzhu/youmu （私有）。头显侧 YM_Link 面板与播放集成在游目仓库内实现和验收。
