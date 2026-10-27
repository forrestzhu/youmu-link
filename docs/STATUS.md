# 当前进度

更新：2026-10-27。仅记录最近实测事实、未关闭问题与下一步；详细证据见任务。

## 当前制品与服务

| 项目 | 最近实际记录及边界 |
| --- | --- |
| 仓库 | GitHub 私有仓库 `forrestzhu/youmu-link`，暂闭源、无 LICENSE；不对外分发。 |
| 设计与实现基线 | 用户授权基于现有 demo 实现，M1 通过；[YML-001](tasks/archive/YML-001-bootstrap.md) 归档。按 [ADR-0002](engineering/decisions/0002-native-desktop.md) 改为 macOS SwiftUI / AppKit 原生界面 + Rust 后端，取代桌面 Tauri 方案。 |
| Rust 核心 | 按实际接口子网的 UDP 发现 / 三端口回退、Noise XX、SPAKE2、Keychain 身份与信任、capability 授权文件服务。core 39 项回归通过（含独立发现向量与 v2 拒绝）；应用协议现为 v3，双方需升级，Noise 承载与原身份保持不变。 |
| Rust 桌面后端 | C ABI + 单设备状态、取消 / 重连、16KiB 停等 Ack 传输；读写与文件工作分别调度，慢磁盘 / socket 不阻塞停止指令。文件工作每连接 4 项、真实 I/O 跨连接全局 4 项；同连接传输 id 不复用，防迟到确认误作用于新任务。 |
| macOS 原生应用 | `dist/YM_Link.app` 已重建、签名校验并打开；Workspace 冗余标题已隐藏，布局核验通过。本轮生产包经两次原生配对验证 v3 镜像查询与首次远端恢复拒绝；测试信任已清理。之前已实测原生目录授权、重启保留与 4 MiB 下载 / SHA-256 / 完成页。Swift 测试 1/1；仅本机 arm64 / macOS 27.0.1 实测，macOS 14 / Intel not-run。 |
| 镜像 | Rust ScreenCaptureKit + VideoToolbox 硬件 H.264、系统 PCM 与加密分片实现就位；双端暂停 / 恢复 / 停止 / 切屏已接入同一状态机，首次开始必须本机确认，对端不请求系统权限。真实硬件合成帧编码单测通过；完整录屏、权限恢复、Quest 解码 / 音画同步 / 延迟仍 not-run。 |
| harness / CI | 本轮 `harness verify` 通过：57 Rust 用例通过、1 个硬件测试默认 ignored；另显式硬件编码用例通过，Swift 1/1。最近回读 `ba65b72` 的 CI run 40452957099，三项因账户付款 / spending limit 未启动（not-run）；未更改账户设置。 |

## 当前工作与待验收

| 工作 | 当前结论 / 下一动作 |
| --- | --- |
| [YML-002 原生 macOS 实现](tasks/YML-002-macos-native.md) | verifying：双端镜像控制、慢 I/O 与迟到确认回归通过，生产包已核验；真实屏幕 / 系统音频采集等待用户确认，Quest 验收未完成。 |
| 本地启动 | 用户手动启动已确认成功，CUA 已取得窗口。早期自动启动入口出现 SIGKILL 的原因不明，未据此修改应用生命周期或系统安全策略。 |
| 真实 LAN / Quest | not-run：暂无第二设备 / Quest 端 v3 互操作证据，Rust 测试桩不代替真机。 |

## 下一步

本轮代码 `3b6329b` 已推送 main，`check --require-pushed` 实跑通过，工作区干净。等待用户确认后验证真实屏幕 / 系统音频采集，再推进游目端 v3 协议对接与双机 LAN / Quest 文件播放、镜像验收，见 [ROADMAP](product/ROADMAP.md)。
