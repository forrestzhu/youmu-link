# macOS 原生 YM_Link Implementation Plan

**Goal:** 根据 demo 实现 macOS 原生桌面伴侣，Rust 统一承载后端，与游目头显经局域网互连。

**Architecture:** SwiftUI / AppKit 展示设备、镜像、共享目录和传输界面。C ABI 连接 Rust 状态管理器，复用发现模块，Noise + SPAKE2 保护业务流量；无 HTTP。klip 仅作架构参考。

**Tech Stack:** SwiftUI、AppKit、Rust、tokio、CBOR、snow、spake2、Keychain、ScreenCaptureKit、VideoToolbox。

## 实施顺序

1. 更新 ADR / 协议与 [YML-002](../tasks/YML-002-macos-native.md) 行为合同；用户“基于 demo 实现”的指令记为 M1 通过。
2. 修正全部候选端口广播，增加 TCP 端口公告与主动重扫；回归覆盖端口回退与超长报文拒绝。
3. `core/src/identity.rs` / `session.rs` 和 `core/tests/session.rs`：系统身份、Noise 握手、SPAKE2 会话绑定、信任持久化、超时限速。真实 TCP 集成测试覆盖错码 / 身份变化拒绝。
4. `core/src/files.rs` 与测试：授权目录、受限路径、分页、range 与断点、文件变化检测；禁止穿越 / 符号链接越界。先冻结协议再实现。
5. `desktop/`：连接状态、监听、配对、重连、文件传输、Rust macOS 采集编码与真实统计；`macos/` 原生窗口并行实现。
6. `scripts/build_macos.py` 构建 `.app`，补齐 Info.plist 隐私用途与本地开发签名；Rust 头显测试桩用于 loopback 验收。
7. 运行 `cargo test`、`swift build` 和本机 UI 核验，稳定后 `harness verify`；独立审查安全与生命周期并修复有证据的问题。
8. 更新 STATUS / 任务 / 使用说明；main commit / push，执行 `check --require-pushed`。第二设备 / Quest 未验收时保持任务 verifying 与总目标 active。
9. 产品完成范围复核补弱网控制：连续背压窗口降低实际采集 / 编码尺寸并重建新流，用户画质作为上限；策略与尺寸边界自动回归，实际采集 / 弱网效果仍需授权和设备验证。

## 可观察验收

- 原生 `.app` 可启动，空态和错误真实，目录选择可用，数据来自 Rust。
- 独立进程经 Noise + 六位码建立信任，错码失败，重启重连无需重输，遗忘后重配。
- 未配对不能读取文件或订阅屏幕；越界路径拒绝；range 与断点有可运行测试。
- 真实采集经系统权限后送入硬件编码和加密会话；不授权时有恢复入口；未测量不显示延迟数字。
- 头显播放、镜像、音频与双设备弱网恢复需 Quest 端和硬件验收，保留在总目标中。
