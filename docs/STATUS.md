# 当前进度

更新：2026-10-28。仅记录最近实测事实、未关闭问题与下一步；详细证据见任务。

用户授权的历史邮箱修正已完成：20 个既有提交的 author / committer 均统一为目标邮箱，20 个文件树不变，16 个提交号变化；旧记录中的提交号见[旧 → 新映射](evidence/2026-10-27-history-email-rewrite.md)。

## 当前制品与服务

| 项目 | 最近实际记录及边界 |
| --- | --- |
| 仓库 | GitHub public `forrestzhu/youmu-link`，2026-10-27 用户明确授权 Apache-2.0 开发预览；匿名 API / LICENSE 200、私下漏洞报告 enabled=true。历史邮箱已统一，未追加历史正文 / 旧运行记录清理，不自动发布正式 Release。 |
| 设计与实现基线 | 用户授权基于现有 demo 实现，M1 通过；[YML-001](tasks/archive/YML-001-bootstrap.md) 归档。按 [ADR-0002](engineering/decisions/0002-native-desktop.md) 改为 macOS SwiftUI / AppKit 原生界面 + Rust 后端，取代桌面 Tauri 方案。 |
| Rust 核心 | UDP 发现 / 三端口回退、Noise XX、SPAKE2、系统身份与信任、capability 授权文件服务。rescan 单路由失败不阻断其余探测，新增 2 项真实 UDP 回归；核心 Windows 46 / Mac 41 项通过。应用协议仍 v3，未改变线字段，真实 Quest 互操作待验收。 |
| Rust 桌面后端 | C ABI + 单设备状态、取消 / 重连、16KiB 停等 Ack 传输；读写与文件工作分别调度，慢磁盘 / socket 不阻塞停止指令。文件工作每连接 4 项、真实 I/O 跨连接全局 4 项；同连接传输 id 不复用，防迟到确认误作用于新任务。 |
| macOS 原生应用 | macOS 15.7.9 arm64 / Xcode 26.3 云端：Rust 74 passed / 1 ignored，Swift 4/4、静态链接、ad-hoc 签名、移位应用中英资源无交互启动与 ZIP 通过；最终 Mac 包独立核验 288 成员 / 138×2 文案 / 许可 / 签名完整性 / 路径门禁通过。此前 Mac 27.0.1 手工配对 / 4 MiB 下载为历史证据；新版人工窗口交互、Mac 14 / Intel not-run。 |
| Windows 原生应用 | WPF / .NET 8 + Rust DLL，自包含 win-x64 开发目录 `dist/YM_Link`；中英切换、设备 / 镜像 / 目录 / 传输入口，Credential Manager 身份与私有 ACL。Windows 11 Pro x64 build 26200 实际构建 / 启动 / 干净关闭通过；原生数据行模板、ABI 生命周期与跨平台配对回归通过。Windows 10 / ARM64、安装器 / 签名分发 not-run。见 [YML-003](tasks/YML-003-windows-native.md)。 |
| 镜像 | Rust ScreenCaptureKit + VideoToolbox 硬件 H.264、系统 PCM 与加密分片、双端控制、持续背压降分辨率实现就位。首次开始必须本机确认；独立故障信号跨暂停 / 恢复保存，采集或初始化失败不能自动重启，对端不请求系统权限。真实硬件合成帧编码单测通过；真实采集 / 弱网切档、权限恢复、Quest 解码 / 音画同步 / 延迟仍 not-run。 |
| Windows 镜像 | Rust WGC / D3D11、CPU NV12 转换与系统 Media Foundation 软件 H.264、WASAPI loopback，复用 v3 AVCC / PCM 输出与双端控制。合成 HD 编码及实际 SPS 色彩标记检查通过；修复 WinRT 工厂缓存跨 apartment 销毁造成的反复启动崩溃。真实屏幕 / 系统音频采集及性能 not-run，不推断硬件编码性能。 |
| harness / CI | 最终实现 627b943 的 run40514312079 三平台全部 success；Linux Python / 仓库 / rustfmt / Rust，Windows Rust / WPF / ABI / Release / smoke，Mac Rust / Swift / 移位启动 / 签名均通过。Windows x64 / Mac arm64 ZIP、完整许可及 SHA-256 已上传，两个最终云端包均完整下载独立核验。本机门禁 78 Rust passed / 2 ignored、36 Python passed / 1 skipped；制品保留 7 天，不自动发布正式 Release。 |
| gitleaks 防泄漏 CI | `.github/workflows/gitleaks.yml` 于 push(main) / PR / 手动触发，SHA 锁定 gitleaks-action v2.3.9 全量历史扫描；本地 26 commits 无泄漏，Actions run 40620888348 首跑通过，见 [YML-005](tasks/archive/YML-005-ci-gitleaks.md)。 |

## 当前工作与待验收

| 工作 | 当前结论 / 下一动作 |
| --- | --- |
| [YML-002 原生 macOS 实现](tasks/YML-002-macos-native.md) | verifying：双端控制、传输边界与弱网策略自动回归通过；真实屏幕 / 系统音频采集等待用户确认，弱网切档与 Quest 验收未完成。 |
| [YML-003 Windows 原生实现](tasks/YML-003-windows-native.md) | verifying：两轮 CR 的三项发现全部修复并复核；最终门禁 76 Rust passed / 2 ignored，另实际系统编码器 / SPS 检查、WPF 数据行 / 本地化 / ABI、Release 构建与启动通过。实际 Quest / LAN、文件播放与屏幕 / 音频仍待设备，证据见 [Windows 实现与两轮 CR](evidence/2026-10-27-windows-two-round-cr.md)。 |
| [YML-004 开源与 CI](tasks/archive/YML-004-opensource-ci.md) | done 并归档：Apache-2.0 公开开发预览、匿名可读、私下漏洞入口、中英安全 / 贡献说明、三平台真实 CI 与双平台开发包全部闭环。CI 发现的 SDK / 扫描早退 / 中文资源问题均修复并经两轮审查和实际 GREEN，见[公开与云端证据](evidence/2026-10-27-public-launch.md)。 |
| [用户追加两轮 CR](evidence/2026-10-27-native-two-round-cr.md) | 完成：首轮 3 P2 / 1 P3、第二轮 2 P3，全部修复并复核关闭；覆盖端点 pin、采集 teardown、传输历史 / 连接交接、重复错误事件和测试桩换源续传。 |
| 本地启动 | 用户手动启动已确认成功，CUA 已取得窗口。早期自动启动入口出现 SIGKILL 的原因不明，未据此修改应用生命周期或系统安全策略。 |
| 真实 LAN / Quest | not-run：本轮用现有 SDK 的 ADB 实查为 0 个连接设备；相邻游目源码尚无 YM_Link v3 客户端，YM-276 仍是桌面启动 / 头显设计范围。暂无第二设备或 v3 互操作证据，Rust 测试桩不代替真机。 |

## 下一步

源码与 CI 开发包已完成；继续真实 Quest / LAN、录屏音频与交互验收，需要 v3 客户端和设备。正式签名 / 公证 / 安装器按 M6 后续推进，见 [ROADMAP](product/ROADMAP.md)。
