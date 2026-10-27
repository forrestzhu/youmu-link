# ADR-0002：macOS 原生界面与 Rust 后端

日期：2026-10-27。状态：接受。用户要求基于 demo 实现，先做 macOS。本条取代 ADR-0001 的桌面 Tauri 决策；头显 Kotlin 与协议双端实现决策保留。

## 决策

- macOS 使用 SwiftUI / AppKit 原生窗口、控件和文件选择，不使用 WebView。Windows 原生界面随后单独实现。
- 网络、发现、身份、信任、配对、目录授权、文件读取、传输和屏幕采集均在 Rust。Swift 仅经 C ABI 读取状态与发送命令。
- `ymlink-core` 为跨平台协议核心；`ymlink-desktop` 管理桌面状态、系统适配与 C ABI；macOS Swift Package 链接 Rust 静态库。无本地 HTTP 服务、无前端框架。
- 借鉴 klip 的全部候选 UDP 端口广播、系统分配 TCP 端口、加密后交换身份、短码配对与持久信任设计。源码独立编写，不复制 klip 实现。
- 长期 Noise 静态密钥存 macOS Keychain；信任元数据与用户授权目录存本机应用数据目录，原子替换并限制访问。少量设备不引入 SQLite；达到数据库查询需求再迁移。
- ScreenCaptureKit 采集、VideoToolbox 编码使用 Rust 系统绑定；屏幕权限依赖系统授权，不伪造采集成功或延迟指标。

## 验证边界

Rust 测试、原生构建和本机运行验证在开发机执行。双机 LAN、Quest 解码、音画同步与实际延迟另留设备证据；loopback 不能代替。
