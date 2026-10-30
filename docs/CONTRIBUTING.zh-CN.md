# 参与贡献

[English](../CONTRIBUTING.md)

较大的变更请先在 [Issue](https://github.com/forrestzhu/youmu-link/issues) 中讨论范围；小修可直接提交 PR。保持改动集中，并遵循 [AGENTS](../AGENTS.md) 的仓库约定。

后端协议与业务逻辑共用 Rust，macOS 使用 SwiftUI / AppKit、Windows 使用 WPF，经 C ABI 调用后端。协议变更先更新 [PROTOCOL](engineering/PROTOCOL.md) 及其版本规则，再实现并补测试；不要在前端实现第二套协议。

提交前运行 `python3 scripts/harness.py verify`，原生界面变更另跑[开发指南](engineering/DEVELOPMENT.md)对应平台的构建 / 测试。说明实际命令、系统和验证结果，未运行项写 `not-run`；单机测试不代替真实 LAN / Quest 验收。

使用合成测试数据，不提交用户文件、真实路径日志、身份 / 信任存储、密钥或 SDK / 构建产物。贡献沿用项目的 [Apache-2.0](../LICENSE) 许可证，并保留相关第三方许可与声明；只提交你有权贡献的内容。
