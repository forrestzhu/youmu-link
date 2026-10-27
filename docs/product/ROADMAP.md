# YM_Link 路线图

状态：v0.2（2026-10-27）。用户已授权基于 demo 开始实现，先做 macOS 原生界面 + Rust 后端；未立项候选放文末。

| 里程碑 | 内容 | 完成判据（可观察） | 状态 |
| --- | --- | --- | --- |
| M0 框架启动 | 私有仓库、harness 闭环、产品 / 架构 / 协议 v0、demo 设计与原型、`ymlink-core` 发现 beacon 骨架 + 测试 | `harness verify` 通过；demo 可点击；beacon 编解码与 loopback 集成测试通过 | done 2026-10-27（[YML-001](../tasks/archive/YML-001-bootstrap.md)） |
| M1 demo 评审 | 用户审阅 demo 两视角（桌面 / 头显），确认按其信息架构实现 | 用户“基于当前的 demo 进行实现”明确授权；原生 macOS 先行修订进任务 | done 2026-10-27（YML-001） |
| M2 配对与双机发现 | Noise XX、SPAKE2 六位短码、Keychain 身份、持久信任、macOS SwiftUI / AppKit 原生 UI；双机发现 / 配对 / 重连 | 原生 `.app` 可运行；真实 LAN 双机证据；`harness verify` 通过 | doing（[YML-002](../tasks/YML-002-macos-native.md)）：Rust / 原生实现推进；L3 / L4 not-run |
| M3 文件浏览与播放 / 传输 | Rust 授权目录、分页 / range、分块 Ack、暂停 / 取消 / 断点；游目 Media3 对接 | 电脑→头显真实播放 4K 样本；传输中断恢复实测 | doing（YML-002）：文件服务与单机 TCP 集成已有覆盖；Quest 播放及 L3 / L4 / L5 not-run |
| M4 屏幕镜像 | macOS ScreenCaptureKit / VideoToolbox、加密媒体分片、系统音频；Windows 采集随后实现 | 真实头显镜像、音画同步、帧率 / 延迟 / 弱网基准记录 | doing（YML-002）：macOS 采集 / 编码 / 发送实现推进；真实 Quest 与 L3 / L4 / L5 not-run |
| M5 游目端集成 | 游目内 YM_Link 面板、Kotlin 协议实现、Media3 文件播放与镜像解码 | 游目仓库 YM 任务 e2e + 真机佩戴验收 | ready（在游目仓库跟踪） |
| M6 打包分发 | Windows 原生界面与安装器、macOS 分发签名 / 公证、GitLab 渠道、更新说明、开源评估 | 用户确认分发就绪与开源决定；各平台构建及安装验收 | ready；本地临时签名 `.app` 不等于完成分发 |

## 依赖与风险

- 当前开发机是 Apple Silicon / macOS 27.0.1。macOS 14+ 是部署目标，macOS 14 与 Intel 尚未运行验证；Windows 原生 UI 和采集适配另行推进。
- M2 起需要第二台真实设备。单机 TCP 与 Rust 头显测试桩只能覆盖 L2，不能代替同网段双机 LAN、头显 Kotlin 互操作或佩戴体验。
- M3 / M4 的桌面实现、媒体编码包接收与头显真实播放分开记录；未实测不填入吞吐、延迟或音画同步达标结论。
- 游目端集成节奏以游目仓库任务为准；协议先冻结规范，再实现两端并补 L4 对拍。
- 本仓库保持私有，不对外发布；GitLab 分发与开源时机仍由用户决定。

## 未立项候选

- 镜像画面裁剪 / 缩放布局、多显示器同屏。
- 电脑端“投到这里”反向发起（头显浏览时主动唤醒桌面端）。
- 传输限速与队列管理 UI 精细化。
