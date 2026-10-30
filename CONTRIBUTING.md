# Contributing

[简体中文](docs/CONTRIBUTING.zh-CN.md)

Discuss substantial changes in an [issue](https://github.com/forrestzhu/youmu-link/issues); small fixes can go straight to a pull request. Keep changes focused and follow the repository conventions in [AGENTS](AGENTS.md).

Rust owns the shared backend and protocol. The native macOS SwiftUI / AppKit and Windows WPF interfaces call the C ABI. Update [PROTOCOL](docs/engineering/PROTOCOL.md) and its version rules before implementing protocol changes, then add tests. Keep one shared protocol implementation.

Run `python3 scripts/harness.py verify` before submitting. For native interface changes, also run the relevant platform build and tests in the [development guide](docs/engineering/DEVELOPMENT.md). Report the commands, operating system and actual results; mark checks you did not run as `not-run`. Local tests do not establish real LAN or Quest interoperability.

Use synthetic test data. Do not commit user files, real-path logs, identity or trust stores, secrets, SDKs or build outputs. Contributions use [Apache-2.0](LICENSE); preserve third-party licenses and notices, and contribute only material you have the right to submit.
