# ADR-0001：技术栈与双端实现策略

日期：2026-10-27。状态：**部分 superseded**（同日）。桌面 Tauri、未使用的签名密钥与 SQLite 基线条款由 [ADR-0002](0002-native-desktop.md) 和当前 [PROTOCOL](../PROTOCOL.md) 取代；头显 Kotlin 双实现与互操作门禁仍接受。下文保留原决策历史。

## 背景

YM_Link 需要一个 Windows / macOS 桌面应用（发现 / 配对 / 文件 / 屏幕采集），并与 Meta Quest 3 / 3S 上的游目（Kotlin / Meta Spatial SDK / Media3）互联。内部项目 klip 已用 Rust + tokio 裸 UDP / TCP + CBOR + Noise 验证过同类局域网互联的依赖面与可行性。

## 决策

1. **历史决策，已 superseded：桌面端 Tauri 2 壳 + Rust 核心（`ymlink-core`）**。原定全部网络与采集逻辑在 Rust crate，UI 只经 command / event API 交互。现改为 macOS SwiftUI / AppKit + Rust C ABI，见 ADR-0002。
2. **头显端：在游目内用 Kotlin 实现同一协议**（`ymlink-kotlin`），不做 Rust FFI / 交叉编译。
3. **协议规范（PROTOCOL.md）为唯一真源**，双端各自实现，互操作测试（L4 golden 夹具对拍 + 真机联调）为必过门禁。
4. 原依赖基线向 klip 已验证组合对齐：tokio（net / time / sync / rt / macros）、ciborium（CBOR）、sha2、if-addrs、snow（Noise）、spake2、keyring、ed25519-dalek / x25519-dalek（M2 起）、rusqlite（信任态，M2 起）。其中 Ed25519 签名与 SQLite 为 superseded 候选，当前使用 X25519 Noise 身份、私有 JSON 信任元数据和 cap-std 授权目录；实际依赖以 Cargo manifest 为准。不引入 HTTP 框架 / WebSocket 的约束保留。

## 备选与理由

- **Electron**：内存与包体大，屏幕采集仍要原生模块，放弃。
- **Kotlin Compose Desktop**：单语言栈诱人，但 Windows 屏幕采集 / 硬件编码生态弱、UDP 组播控制需自包装，放弃。
- **Flutter Desktop**：采集与网络同样走 FFI，UI 收益不抵桥接成本，放弃。
- **头显端复用 Rust（JNI / 交叉编译）**：避免双实现，但引入 NDK 工具链、构建复杂度与崩溃面；协议面（UDP / TCP / Noise / SPAKE2）在 JVM/Android 均有成熟库，双实现的互操作成本可控且换来两侧各自原生可调试。

## 后果

- 双端实现偏差风险由 L4 互操作门禁承接：golden 报文夹具入库、协议版本协商显式拒绝不兼容。
- 桌面 UI 技术栈已由 ADR-0002 改为平台原生界面；不再选择 Tauri 前端框架。
- 屏幕采集平台 API（ScreenCaptureKit / Windows Graphics Capture）在 M4 前先做 spike，若任一平台受阻则该平台镜像降级方案另立 ADR。
