# 当前进度

更新：2026-10-27。仅记录最近实测事实、未关闭问题与下一步；详细证据见任务。

用户授权的历史邮箱修正已完成：20 个既有提交的 author / committer 均统一为目标邮箱，20 个文件树不变，16 个提交号变化；旧记录中的提交号见[旧 → 新映射](evidence/2026-10-27-history-email-rewrite.md)。

## 当前制品与服务

| 项目 | 最近实际记录及边界 |
| --- | --- |
| 仓库 | GitHub private `forrestzhu/youmu-link`，原创代码已准备 Apache-2.0、NOTICE 与第三方声明；历史邮箱修正已授权完成，公开与历史路径 / 内部说明清理仍待用户决定，不自动发布 Release。 |
| 设计与实现基线 | 用户授权基于现有 demo 实现，M1 通过；[YML-001](tasks/archive/YML-001-bootstrap.md) 归档。按 [ADR-0002](engineering/decisions/0002-native-desktop.md) 改为 macOS SwiftUI / AppKit 原生界面 + Rust 后端，取代桌面 Tauri 方案。 |
| Rust 核心 | 按实际接口子网的 UDP 发现 / 三端口回退、Noise XX、SPAKE2、Keychain 身份与信任、capability 授权文件服务。core 39 项回归通过（含独立发现向量与 v2 拒绝）；应用协议现为 v3，双方需升级，Noise 承载与原身份保持不变。 |
| Rust 桌面后端 | C ABI + 单设备状态、取消 / 重连、16KiB 停等 Ack 传输；读写与文件工作分别调度，慢磁盘 / socket 不阻塞停止指令。文件工作每连接 4 项、真实 I/O 跨连接全局 4 项；同连接传输 id 不复用，防迟到确认误作用于新任务。 |
| macOS 原生应用 | `dist/YM_Link.app` 已重建、签名校验并打开；Workspace 冗余标题已隐藏，布局核验通过。本轮生产包经两次原生配对验证 v3 镜像查询与首次远端恢复拒绝；测试信任已清理。之前已实测原生目录授权、重启保留与 4 MiB 下载 / SHA-256 / 完成页。Swift 测试 1/1；仅本机 arm64 / macOS 27.0.1 实测，macOS 14 / Intel not-run。 |
| Windows 原生应用 | WPF / .NET 8 + Rust DLL，自包含 win-x64 开发目录 `dist/YM_Link`；中英切换、设备 / 镜像 / 目录 / 传输入口，Credential Manager 身份与私有 ACL。Windows 11 Pro x64 build 26200 实际构建 / 启动 / 干净关闭通过；原生数据行模板、ABI 生命周期与跨平台配对回归通过。Windows 10 / ARM64、安装器 / 签名分发 not-run。见 [YML-003](tasks/YML-003-windows-native.md)。 |
| 镜像 | Rust ScreenCaptureKit + VideoToolbox 硬件 H.264、系统 PCM 与加密分片、双端控制、持续背压降分辨率实现就位。首次开始必须本机确认；独立故障信号跨暂停 / 恢复保存，采集或初始化失败不能自动重启，对端不请求系统权限。真实硬件合成帧编码单测通过；真实采集 / 弱网切档、权限恢复、Quest 解码 / 音画同步 / 延迟仍 not-run。 |
| Windows 镜像 | Rust WGC / D3D11、CPU NV12 转换与系统 Media Foundation 软件 H.264、WASAPI loopback，复用 v3 AVCC / PCM 输出与双端控制。合成 HD 编码及实际 SPS 色彩标记检查通过；修复 WinRT 工厂缓存跨 apartment 销毁造成的反复启动崩溃。真实屏幕 / 系统音频采集及性能 not-run，不推断硬件编码性能。 |
| harness / CI | 交付 `4c2ecdc` 配置三平台测试与 main 开发 ZIP；固定 action SHA、只读 token、制品保留 7 天。隐私修复 `2445574` 的本机门禁 Rust 76 passed / 2 ignored、Python 36 passed / 1 Unix-mode skipped；最终 Windows Release / native ABI / smoke / legal / ZIP / hash 与全包独立隐私复核通过。此前 run 40482200859 checks 因账户级托管运行条件限制未启动，Windows / macOS skipped、上传 not-run；未改账户设置。 |

## 当前工作与待验收

| 工作 | 当前结论 / 下一动作 |
| --- | --- |
| [YML-002 原生 macOS 实现](tasks/YML-002-macos-native.md) | verifying：双端控制、传输边界与弱网策略自动回归通过；真实屏幕 / 系统音频采集等待用户确认，弱网切档与 Quest 验收未完成。 |
| [YML-003 Windows 原生实现](tasks/YML-003-windows-native.md) | verifying：两轮 CR 的三项发现全部修复并复核；最终门禁 76 Rust passed / 2 ignored，另实际系统编码器 / SPS 检查、WPF 数据行 / 本地化 / ABI、Release 构建与启动通过。实际 Quest / LAN、文件播放与屏幕 / 音频仍待设备，证据见 [Windows 实现与两轮 CR](evidence/2026-10-27-windows-two-round-cr.md)。 |
| [YML-004 开源与 CI](tasks/YML-004-opensource-ci.md) | verifying：邮箱修正与 Apache-2.0 准备完成，仍 private。新[公开准备复查](evidence/2026-10-27-public-readiness.md)未见凭据 / 许可硬障碍；已泛化 HEAD 遗漏注释。历史演示 / 内部 / 账户背景及旧 Actions 记录的公开范围待决定；基线 20 runs / 60 jobs 全部未执行步骤，CI / 开发包实跑仍待验证。完整门禁 36 Python passed / 1 skipped、76 Rust passed / 2 ignored。 |
| [用户追加两轮 CR](evidence/2026-10-27-native-two-round-cr.md) | 完成：首轮 3 P2 / 1 P3、第二轮 2 P3，全部修复并复核关闭；覆盖端点 pin、采集 teardown、传输历史 / 连接交接、重复错误事件和测试桩换源续传。 |
| 本地启动 | 用户手动启动已确认成功，CUA 已取得窗口。早期自动启动入口出现 SIGKILL 的原因不明，未据此修改应用生命周期或系统安全策略。 |
| 真实 LAN / Quest | not-run：本轮用现有 SDK 的 ADB 实查为 0 个连接设备；相邻游目源码尚无 YM_Link v3 客户端，YM-276 仍是桌面启动 / 头显设计范围。暂无第二设备或 v3 互操作证据，Rust 测试桩不代替真机。 |

## 下一步

公开前决定是否保留历史演示 / 内部 / 账户背景及旧 Actions 记录，不接受公开的部分先清理；当前保持 private，邮箱已修正。可按开发预览开放源码，再实际验证三平台 CI / 开发包；账户条件若仍阻塞，按实际诊断处理。真实 Quest / LAN、录屏音频验收仍需 v3 客户端和设备，见 [ROADMAP](product/ROADMAP.md)。
