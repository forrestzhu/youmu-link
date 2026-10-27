# Contributing / 参与贡献

较大的变更请先在 [Issue](https://github.com/forrestzhu/youmu-link/issues) 中讨论范围；小修可直接提交 PR。保持改动集中，并遵循 [AGENTS](AGENTS.md) 的仓库约定。

后端协议与业务逻辑共用 Rust，macOS 使用 SwiftUI / AppKit、Windows 使用 WPF，经 C ABI 调用后端。协议变更先更新 [PROTOCOL](docs/engineering/PROTOCOL.md) 及其版本规则，再实现并补测试；不要在前端实现第二套协议。

提交前运行 `python scripts/harness.py verify`，原生界面变更另跑 [README](README.md) 对应平台的构建 / 测试。说明实际命令、系统和验证结果，未运行项写 `not-run`；单机测试不代替真实 LAN / Quest 验收。

使用合成测试数据，不提交用户文件、真实路径日志、身份 / 信任存储、密钥或 SDK / 构建产物。贡献沿用项目的 [Apache-2.0](LICENSE) 许可证，并保留相关第三方许可与声明；只提交你有权贡献的内容。

Discuss substantial changes in an [issue](https://github.com/forrestzhu/youmu-link/issues); small fixes can go straight to a PR. Keep changes focused and follow [AGENTS](AGENTS.md).

Rust owns the shared backend and protocol. The native macOS SwiftUI / AppKit and Windows WPF interfaces call the C ABI. Update [PROTOCOL](docs/engineering/PROTOCOL.md) and its version rules before implementing protocol changes, then add tests.

Run `python scripts/harness.py verify` and the relevant native build / tests from [README](README.md). Report the commands, OS and actual results; mark unrun checks `not-run`. Local tests do not establish real LAN or Quest interoperability.

Use synthetic test data. Do not commit user files, real-path logs, identity or trust stores, secrets, SDKs or build outputs. Contributions use [Apache-2.0](LICENSE); preserve third-party licenses and notices, and contribute only material you have the right to submit.
