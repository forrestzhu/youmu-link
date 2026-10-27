# 当前进度

更新：2026-10-27。仅记录最近实测事实、未关闭问题与下一步；详细证据见任务。

## 当前制品与服务

| 项目 | 最近实际记录及边界 |
| --- | --- |
| 仓库 | GitHub 私有仓库 `forrestzhu/youmu-link`，暂闭源、无 LICENSE；不对外分发。 |
| 设计与实现基线 | 用户授权基于现有 demo 实现，M1 通过；[YML-001](tasks/archive/YML-001-bootstrap.md) 归档。按 [ADR-0002](engineering/decisions/0002-native-desktop.md) 改为 macOS SwiftUI / AppKit 原生界面 + Rust 后端，取代桌面 Tauri 方案。 |
| Rust 核心 | 已实现 按实际接口子网的 UDP 发现 / 三端口回退与探测、TCP 公告、128 候选 LRU、Noise XX、SPAKE2 六位码、Keychain 身份、信任与撤销、capability 授权目录 / 分页 / range / 版本检测。core 自动测试 37 项通过。 |
| Rust 桌面后端 | C ABI + 统一状态、单设备连接与代际去重、限速 / 取消 / 重连、16KiB 停等 Ack 传输、独立有界控制 / 媒体队列；真实 TCP 单机测试覆盖配对、range、暂停 / 恢复 / 取消迟到 Ack、免码重连、忘记设备与取消配对。 |
| macOS 原生应用 | `dist/YM_Link.app` 已成功构建和签名校验；原生设备 / 镜像 / 共享 / 传输界面与中英资源就位，Swift 测试 1/1 通过。用户手动启动后确认窗口可见，随后 CUA 读取到真实原生窗口与搜索状态；原生目录授权、重启保留、六位码配对与 4 MiB 下载 / SHA-256 / 完成页已实测；镜像链路待系统授权。仅本机 arm64 / macOS 27.0.1 构建实测；macOS 14 / Intel not-run。 |
| 镜像 | Rust ScreenCaptureKit + VideoToolbox 硬件 H.264、系统 PCM 音频与加密分片发送实现就位；真实硬件编码单测通过。完整录屏、权限拒绝恢复、Quest 解码 / 音画同步 / 实测延迟仍 not-run，不填模拟指标。 |
| harness / CI | 仓库检查与 Rust 测试可运行；本轮 `harness verify` 通过（46 Rust 用例通过、1 个硬件测试默认 ignored；另显式运行硬件编码用例通过）。GitHub CI 首跑曾受账户付款 / 额度限制未启动，本轮远端 CI 结果待推送后读取。 |

## 当前工作与待验收

| 工作 | 当前结论 / 下一动作 |
| --- | --- |
| [YML-002 原生 macOS 实现](tasks/YML-002-macos-native.md) | verifying：原生 `.app` 构建与 Apple Development 本地签名完成，继续系统采集验收与交付门禁。 |
| 本地启动 | 用户手动启动已确认成功，CUA 已取得窗口。早期自动启动入口出现 SIGKILL 的原因不明，未据此修改应用生命周期或系统安全策略。 |
| 真实 LAN / Quest | not-run：暂无第二设备 / Quest 端 v2 互操作证据，Rust 测试桩不代替真机。 |

## 下一步

完成原生启动与录屏权限验证，补运行证据；完成最终 `harness verify`、main commit / push 与 `check --require-pushed`。之后推进游目端 v2 协议对接与双机 LAN / Quest 文件播放和镜像验收，见 [ROADMAP](product/ROADMAP.md)。
