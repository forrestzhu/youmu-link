# YM_Link 架构

状态：v0.1（2026-10-27）。组件与职责划分；协议细节见 [PROTOCOL](PROTOCOL.md)。

## 组件图

```
┌─ Windows / macOS ──────────────────────────┐      ┌─ Meta Quest 3 / 3S ──────────────┐
│ youmu-link（Tauri 2 壳）                    │      │ 游目 Youmu VR Player              │
│  ┌─ 前端 UI（桌面窗口）────────────────┐    │      │  ┌─ YM_Link 面板（Compose）────┐  │
│  │ 设备列表 / 配对 / 屏幕选择 / 共享目录 │    │      │  │ 发现 / 配对 / 电脑屏幕 /     │  │
│  └──────────────┬──────────────────────┘    │      │  │ 电脑文件 / 传输任务          │  │
│                 │ command / event API        │      │  └──────────────┬─────────────┘  │
│  ┌─ ymlink-core（Rust，tokio）────────▼────┐ │      │  ┌─ ymlink-kotlin（协议实现）──▼─┐  │
│  │ discovery（UDP 广播 + mDNS）             │◄┼─────┼─►│ discovery / session / lane   │  │
│  │ session（Noise XX + SPAKE2 配对）        │ │ LAN │  └──────────────┬───────────────┘  │
│  │ lanes：files / mirror（TCP）            │ │      │                 │ Surface / 流     │
│  │ capture（ScreenCaptureKit / WGC）       │      │  Media3 解码渲染 / 本地存储        │
│  └────────────────────────────────────────┘ │      └──────────────────────────────────┘
└────────────────────────────────────────────┘
```

## 职责划分

- **ymlink-core（桌面）**：全部网络与采集逻辑；UI 只经 command / event API 交互（klip 验证过的分层）。crate 内模块：`beacon` / `discovery`（M0 已建骨架）、`identity` / `pairing` / `session`（M2）、`files` / `mirror` lane（M3 / M4）。
- **ymlink-kotlin（头显）**：在游目 app 内以 Kotlin 实现同一协议（UDP / TCP / Noise / SPAKE2 均有成熟 JVM / Android 库）。**不复用 Rust FFI**：避免交叉编译与 JNI 维护，代价是协议规范 + 互操作测试必须作为唯一真源（见 [ADR-0001](decisions/0001-platform.md)）。
- **连接拓扑**：对等 P2P，双端都监听 TCP（端口写入发现报文），双向拨号用 device_id 大小做 initiator 去重（同 klip，避免双连接）；无中心服务、无云依赖。

## 数据面

- **发现**：UDP 广播 beacon（CBOR、≤512B、60s 心跳）+ 显式 Query 单播回复 + mDNS 辅助（`_youmu-link._tcp.local.`）；身份零暴露，详见 PROTOCOL §2。
- **会话**：所有业务流量走 Noise XX（`Noise_XX_25519_ChaChaPoly_SHA256`），每次连接完整握手，静态密钥 TOFU + keyring；首次配对用 SPAKE2 六位短码。
- **lane 复用**：单一 TCP listener，accept 后按 magic 前缀分流（控制 / 文件 / 镜像），同 klip 的 lane 模型；具体前缀与帧格式在 M2 / M3 实现 PROTOCOL §4-§5 时冻结。
- **文件 lane**：浏览（List / Stat）、流式播放（Range 语义）、传输（分块 + Ack + 断点）。
- **镜像 lane**：采集帧 → 平台硬件编码（H.264/HEVC）→ 帧封装传输 → 头显 Media3 解码 → Surface。编码参数档位与延迟基准 M4 实测定。

## 状态机（连接视角，双端一致）

```
discovered ──connect──► connecting ──noise handshake──► pairing(首连) ──短码确认──► paired
                          │                                │                     │
                          └──────── 公钥已信任则直接进入 ◄────────────────────────┘
paired ──idle 超时──► disconnected ──beacon/重试──► connecting …
```

候选表 `discovered / connecting / paired / failed` 由 `discovery` 模块事件驱动，UI 只读展示。

## 与游目的边界

- 游目仓库负责：YM_Link 面板 UI、播放集成、权限与语言、真机验收；本仓库负责：协议、桌面端、测试桩。
- 协议版本协商：Hello 携带 protocol_version；不兼容版本显式拒绝并提示升级，不静默降级。

## 安全模型摘要

发现零身份；信任 = 静态密钥指纹（keyring 持久化）；首连短码确认（SPAKE2，绑定会话指纹，限速 + 过期）；公钥变化即拒绝并要求重新配对；授权目录白名单是文件 lane 的硬边界；镜像画面不出配对会话。完整规范见 PROTOCOL。
