# YM_Link 路线图

状态：v0.3（2026-10-27）。macOS 与 Windows 原生端共用 Rust 后端；未立项候选放文末。

| 里程碑 | 内容 | 完成判据（可观察） | 状态 |
| --- | --- | --- | --- |
| M0 框架启动 | 私有仓库、harness 闭环、产品 / 架构 / 协议 v0、demo 设计与原型、`ymlink-core` 发现 beacon 骨架 + 测试 | `harness verify` 通过；demo 可点击；beacon 编解码与 loopback 集成测试通过 | done 2026-10-27（[YML-001](../tasks/archive/YML-001-bootstrap.md)） |
| M1 demo 评审 | 用户审阅 demo 两视角（桌面 / 头显），确认按其信息架构实现 | 用户“基于当前的 demo 进行实现”明确授权；原生 macOS 先行修订进任务 | done 2026-10-27（YML-001） |
| M2 配对与双机发现 | Noise XX、SPAKE2 六位短码、系统凭据身份、持久信任、macOS / Windows 原生 UI；双机发现 / 配对 / 重连 | 原生应用可运行；真实 LAN 双机证据；`harness verify` 通过 | doing（[YML-002](../tasks/YML-002-macos-native.md)、[YML-003](../tasks/YML-003-windows-native.md)）：Rust 与原生实现已有本机验证；L3 / L4 not-run |
| M3 文件浏览与播放 / 传输 | Rust 授权目录、分页 / range、分块 Ack、暂停 / 取消 / 断点；游目 Media3 对接 | 电脑→头显真实播放 4K 样本；传输中断恢复实测 | doing（YML-002）：文件服务与单机 TCP 集成已有覆盖；Quest 播放及 L3 / L4 / L5 not-run |
| M4 屏幕镜像 | macOS ScreenCaptureKit / VideoToolbox，Windows WGC / Media Foundation / WASAPI；加密媒体分片、系统音频 | 真实头显镜像、音画同步、帧率 / 延迟 / 弱网基准记录 | doing（YML-002 / YML-003）：两端适配已有合成帧编码验证；真实采集 / Quest 与 L3 / L4 / L5 not-run |
| M5 游目端集成 | 游目内 YM_Link 面板、Kotlin 协议实现、Media3 文件播放与镜像解码 | 游目仓库 YM 任务 e2e + 真机佩戴验收 | ready（在游目仓库跟踪） |
| M6 打包分发 | Apache-2.0 源码开发预览已公开，GitHub CI 三平台测试与开发包；后续 Windows 安装器、macOS 签名 / 公证、发行说明 | 源码公开已获用户授权；云端构建、各平台安装与分发验收分别记录 | doing（[YML-004](../tasks/YML-004-opensource-ci.md)）；开发包不等于正式发行，云端结果与未运行项单列 |

## 依赖与风险

- 已有本机记录为 Apple Silicon / macOS 27.0.1 与 Windows 11 专业版 x64 build 26200。macOS 14+、Windows 10 1903+ 为部署目标；macOS 14 / Intel、旧 Windows / ARM64 尚未运行验证。Windows 当前为 CPU 转换与系统同步软件 H.264，不推断其实际延迟 / 吞吐。
- M2 起需要第二台真实设备。单机 TCP 与 Rust 头显测试桩只能覆盖 L2，不能代替同网段双机 LAN、头显 Kotlin 互操作或佩戴体验。
- M3 / M4 的桌面实现、媒体编码包接收与头显真实播放分开记录；未实测不填入吞吐、延迟或音画同步达标结论。
- 游目端集成节奏以游目仓库任务为准；协议先冻结规范，再实现两端并补 L4 对拍。
- 原创代码采用 Apache-2.0，仓库已于 2026-10-27 按用户授权公开为开发预览；历史邮箱已修正，后续历史改写与正式发行仍由用户明确决定，不因配置 CI 自动发布正式 Release。

## 未立项候选

- 镜像画面裁剪 / 缩放布局、多显示器同屏。
- 电脑端“投到这里”反向发起（头显浏览时主动唤醒桌面端）。
- 传输限速与队列管理 UI 精细化。
