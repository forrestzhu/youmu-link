# YM_Link 测试策略

状态：v0.2（2026-10-27）。自动测试、本机原生运行与真实设备验收分层记录；不预填未实测指标。

## 分层

| 层 | 范围 | 工具 / 命令 | 门禁 |
| --- | --- | --- | --- |
| L1 单元 | beacon / 业务编解码、大小与字段校验、tag / nonce、文件边界、C ABI 空指针 / 参数、Swift 模型解码 | `cargo test --workspace --all-targets`；`swift test --package-path macos` | 每次改动相关回归；完整门禁见下文 |
| L2 集成（单机） | 双 socket 发现；真实 TCP Noise / SPAKE2；桌面授权文件 / range / 传输控制 / 重连 / 取消；测试桩断点与媒体组包 | `core/tests/`、`desktop/src/runtime_tests.rs`、`desktop/tests/native_backend.rs`、测试桩单测；loopback 只在显式测试配置允许 | 自动通过只证明单机逻辑，不证明 LAN 或头显可用 |
| L3 双机 LAN | 真实两台设备的发现、配对、重连、端口 / 防火墙、休眠与网络切换 | 手动清单 + `docs/evidence/` 报告，绑定提交及双端型号 / 系统 | M2 起里程碑必须；当前 not-run |
| L4 互操作 | Rust ↔ 游目 Kotlin：golden beacon、Noise / SPAKE2、文件与媒体帧对拍、错误版本拒绝 | 双仓一致性夹具 + 联调任务真机 | 协议版本变化必须；当前 Kotlin 侧尚未集成，not-run |
| L5 体验基准 | Quest 文件播放 / seek、镜像延迟 / FPS / 音画同步、吞吐与弱网恢复 | M3 / M4 固化样本、脚本和佩戴记录 | 里程碑验收；当前 not-run |

CI workflow 配置 Ubuntu 24.04 / Windows 2025 / macOS 15 三平台测试；Windows 补 Release / WPF / ABI / smoke，macOS 补 Swift 测试、Rust 静态链接与 `.app` 构建。公开后 run40514312079 已实际跑通三平台测试与 Windows x64 / Mac arm64 开发包。Mac 固定 Xcode 26.3 / SDK 26.2，另验证签名后的 `.app` 复制到临时目录仍能无交互加载中英资源；这不替代真实窗口 / 设备验收。结果与两平台包独立核验见[公开与云端证据](../evidence/2026-10-27-public-launch.md)。

Windows CI 另运行 WPF Release 构建与 .NET 控制台测试；无交互屏幕采集，不将 CI 构建视为原生窗口验收。

## 现有覆盖与边界

- `core/src/beacon.rs`：8 项单测，CBOR 编解码、512B、字段 / 版本 / 长度与 tag_hash。
- `core/tests/discovery.rs`：9 项 loopback 集成，Query→Beacon、tag 隔离、冷却、自回环、过期、端口回退、独立 TCP 端口公告、超长 UDP 拒绝。
- `core/tests/session.rs`：10 项真实 TCP 集成，Noise 双向传输、SPAKE2 对码 / 错码 / 过期、双方 Known 门控、冒用旧 id 拒绝、撤销信任、帧限制、取消读帧、私有信任存储与 detached 副本、v2 Hello 的明确升级拒绝。
- `core/tests/files.rs`：6 项集成，目录授权 / 撤销、分页 / 大目录预算、range / seek / 续传 / EOF、文件变化、越界路径 / symlink / FIFO 拒绝。
- `core/src/discovery.rs`：5 项接口 / 企业网段 / 特殊地址与有界缓存回归；`core/tests/protocol_vectors.rs`：1 项独立发现报文固定向量。core 共 39 项，本机结果见任务。
- 桌面 runtime 真实 TCP 集成已有通过记录：配对、授权目录、Stat / range、传输暂停 / 恢复 / 取消、取消后迟到 Ack、重连和配对取消。业务 wire 另测未知字段与最大消息大小；测试桩另测下载中间文件与媒体分片重组。
- 镜像控制回归覆盖首次远端恢复拒绝、暂停模型下切屏 / 重复暂停、停止后恢复拒绝、旧代际 running 公告、有界控制队列；慢磁盘 / 慢 socket 回归验证停止指令仍可转发，取消等待者后全局真实 I/O 上限保持。合成暂停模型不调用采集 API，不能代替真实暂停 / 恢复录屏。
- 弱网控制回归使用真实有界 channel 验证单窗口不切档、连续拥塞触发、恢复窗口清零、视频丢弃比例阈值与有限尺寸下限；profile 回归覆盖按源比例缩小、偶数尺寸、不放大与按像素面积降低目标码率。上述测试不证明实际 ScreenCaptureKit 切档、接收端解码或弱网体验通过。
- macOS 原生 Swift 模型测试、`.app` 最终构建、启动与交互结果以当前 [STATUS](../STATUS.md) 及 [YML-002](../tasks/YML-002-macos-native.md) 的实际证据为准。自动协议测试不代替窗口启动、目录选择或系统权限操作。
- `hardware_encoder_emits_avcc_keyframe_without_screen_capture` 是显式忽略的硬件编码检查，使用合成像素，不采集屏幕、不请求屏幕录制；即使通过，也只证明当前 Mac 的 VideoToolbox 编码，不证明屏幕采集或 Quest 解码。
- demo 原型用于页面 / 交互参考，不是视觉合同，也不把原型中的模拟性能数字作为实现验收标准。

## 门禁命令

```bash
# harness 回归 + 仓库检查 + rustfmt + workspace Rust 测试
python3 scripts/harness.py verify

# 原生构建与本地临时签名；随后验证窗口实际启动
python3 scripts/build_macos.py
swift test --package-path macos
open dist/YM_Link.app

# 单独诊断核心 / 桌面集成
cargo test --locked -p ymlink-core --all-targets
cargo test --locked -p ymlink-desktop native_backend_pairs_serves_ranges_controls_transfer_reconnects_and_cancels

# 显式硬件检查：合成像素到硬件H.264，不抓屏
cargo test --locked -p ymlink-desktop hardware_encoder_emits_avcc_keyframe_without_screen_capture -- --ignored --nocapture

# 交付：已提交的main等于实际远端main
python3 scripts/harness.py check --require-pushed
```

真实加密测试桩的 CLI 和动态 TCP 端口定位见 [README](../../README.md)。捕获与下载样本只留本机，不提交原始媒体或路径日志。

## macOS 本机核验

本机 Apple Silicon / macOS 27.0.1 / Xcode 27 的结果单独记录；macOS 14 是部署目标，旧系统与 Intel 验证为 not-run。`.app` 需检查界面启动、空态、配对取消、目录选择 / 撤销、显示器枚举、录屏权限拒绝与恢复、开始 / 停止镜像和任务控制。

真实屏幕与系统音频采集需系统授权。未取得权限或设备不足时记录限制，不伪造包数、帧率、延迟或同步效果；不能仅凭 Info.plist、部署版本或成功编译推断系统兼容性。

## Windows 本机核验

本次实际主机为 Windows 11 专业版 x64 build 26200、Rust 1.85 / .NET SDK 8.0.405。Windows 10 1903+ 是 WGC 部署目标，旧系统 / ARM64 未验证。界面核验覆盖窗口启动、空态、显示器枚举、中英文切换、重新扫描与关闭；配对 / 文件 / 控制另由真实 TCP 自动测试证明单机逻辑。

- `core/tests/windows_platform.rs`：受保护私有 ACL、重开状态目录、普通文件拒绝、无需 symlink 权限的 junction 拒绝与 capability 越界测试。
- `core/tests/files.rs`：Windows DOS 设备 / 歧义路径拒绝，同大小 / 相同创建及修改时间换源仍使版本 token 失效。
- `core::identity::windows_tests`：独立测试凭据创建 / 读取保持同一身份，损坏数据 fail-closed，清理只删除测试条目。
- runtime 真实 TCP 配对覆盖 Windows / macOS / headset-test 与双向发起角色；反复 Backend 创建与销毁，检测 WinRT 生命周期回归。
- `capture::windows::tests`：NV12 / 尺寸边界、AnnexB→AVCC / avcC、背压关键帧与故障保留、反复新 COM apartment 查询。合成帧原生编码与实际采集分别显式 ignored，默认门禁不录屏。

```powershell
dotnet run --project windows/Tests/YM_Link.Tests.csproj -c Release
python3 scripts/build_windows.py --smoke-test
cargo test --locked -p ymlink-desktop native_encoder_emits_avcc_keyframe_without_screen_capture -- --ignored --nocapture
```

真实采集用例 `real_display_and_system_audio_capture_stop_cleanly` 还要求显式 `YM_LINK_CAPTURE_TEST=1`，会实际读取屏幕与系统音频，未经本机确认不运行；本轮 not-run。WASAPI 端点静音时可无音频包，不能据此认定音画同步已验证。CPU 编码路径的实际分辨率 / FPS、真实 LAN / Quest 播放和弱网验收分别记录，不从合成帧推断。

## 证据纪律

- 真机 / 双机结果只记任务链接的 `docs/evidence/` 报告，绑定 Commit、双端设备型号与系统版本、执行命令、Result 和未覆盖边界。
- 未运行的层写 `not-run`，需要设备的任务在真实验收前保持未完成；自动测试不能代签 L3 / L4 / L5。
- 不把环境变量原始输出、配对短码、私钥、用户路径或原始媒体日志写入仓库；只留必要统计与脱敏结论。

## 本机原生 app 与固定端口用例

完整门禁前正常退出本机运行的 YM_Link：发现回退集成用例需要占用三个固定 beacon 端口，运行中的真实 app 会造成 AddrInUse。原生 UI 端到端验证在自动门禁后另行启动，不把这个端口冲突误记为协议回归。
