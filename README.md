# YM_Link

YM_Link 是 [Youmu VR Player](https://github.com/forrestzhu/youmu)（游目）的桌面伴侣应用：安装在 Windows / macOS 电脑上，与 Meta Quest 3 / 3S 上的游目通过局域网互联。

核心能力（按 [ROADMAP](docs/product/ROADMAP.md) 分阶段交付）：

1. **屏幕镜像** —— 把电脑上选定的显示器画面实时镜像到头显内的游目大屏播放。
2. **文件浏览与播放 / 传输** —— 在头显内浏览电脑的文件目录，选择视频直接流式播放，或传输到头显本地。
3. **设备发现与配对** —— 双端在同一局域网内互相发现，经显式配对码确认后建立加密连接；头显侧入口为游目内的 YM_Link 面板。

互联模型参考已验证的局域网同步设计（UDP 广播 beacon、动态 TCP 端口、Noise 加密与持久信任），源码与协议独立实现。首次连接须经六位码 SPAKE2 配对；规范见 [PROTOCOL](docs/engineering/PROTOCOL.md)。

## 仓库状态

- 阶段：macOS 使用 SwiftUI / AppKit（[YML-002](docs/tasks/YML-002-macos-native.md)），Windows 使用 WPF / .NET 8（[YML-003](docs/tasks/YML-003-windows-native.md)）；Rust 共用发现、加密配对、授权文件服务、传输与镜像控制，采集编码按平台适配。决策见 [ADR-0002](docs/engineering/decisions/0002-native-desktop.md) / [ADR-0003](docs/engineering/decisions/0003-windows-native.md)。游目头显侧 YM_Link v3 集成与真实设备验收仍待推进。
- 验证：Windows 本轮 76 项 Rust 回归、WPF 数据行 / 中英资源 / ABI 生命周期、Release 构建与启动检查通过；合成 HD 实际 H.264 编码与 SPS 检查另行通过。桌面后端覆盖真实 TCP 单机配对、文件 range、传输控制、重连与授权目录重启持久化。macOS 之前已有 `.app`、原生配对及 4 MiB 下载证据，本轮未在 Windows 复跑 macOS 构建。双机 LAN、真实 Quest 播放 / 镜像与音画同步为 **not-run**，不以测试桩代替设备验收。
- 许可：原创代码采用 [Apache-2.0](LICENSE)，第三方依赖保留各自许可，见 [THIRD_PARTY_NOTICES](THIRD_PARTY_NOTICES.md)。历史邮箱已按维护者授权修正，当前 GitHub 仓库仍为 private；历史示例 / 内部背景的处理和公开时点仍由维护者决定，不自动发布 Release。
- 当前进度：[STATUS](docs/STATUS.md)；文档索引：[docs/README](docs/README.md)。

## 仓库结构

| 路径 | 内容 |
| --- | --- |
| `core/` | `ymlink-core`：UDP 发现、Noise / SPAKE2、身份与信任、授权目录与文件 range 服务 |
| `desktop/` | `ymlink-desktop`：Rust 桌面状态、连接 / 传输、macOS / Windows 采集编码、C ABI、头显测试桩 |
| `macos/` | SwiftUI / AppKit 原生界面、目录选择器、中英文资源与 Swift 模型测试 |
| `windows/` | WPF 原生界面、中英文资源、模型 / 本地化 / ABI 生命周期测试 |
| `scripts/` | harness 任务闭环、仓库检查、macOS `.app` / Windows 开发目录构建 |
| `docs/` | 产品定义、架构、协议、测试策略与任务记录 |
| `design/prototypes/` | 可点击 demo 原型 |
| `.github/workflows/` | CI（仓库检查 + Rust 矩阵测试） |

## 自动测试与开发包

GitHub Actions 在 main push、PR 或手动触发时运行 Linux / Windows / macOS Rust 测试；Windows 补 WPF / ABI / bounded smoke，macOS 补 Swift、静态 Rust 链接与临时签名 `.app` 构建。真实录屏、音频和 Quest 用例不在无人值守 CI 中运行。

成功的 main push / 手动 main 构建会上传 Windows x64 自包含 ZIP 与 macOS arm64 ZIP，文件名包含版本和提交 SHA，附 SHA-256 与完整许可证，Actions artifacts 保留 7 天。PR 执行包完整性验证但不上传。制品为未正式签名 / 未公证的开发包，不是安装器或正式发行。

本地构建后可复用同样的收集与打包步骤：

```powershell
python3 scripts/collect_licenses.py --output dist/legal --windows-assets windows/YM_Link/obj/project.assets.json
python3 scripts/package_build.py --platform windows-x64
# macOS 构建后省略 --windows-assets，并改为 --platform macos-arm64
```

开源前隐私审查见[敏感信息审计](docs/evidence/2026-10-27-sensitive-information-audit.md)，当前剩余项见[公开准备复查](docs/evidence/2026-10-27-public-readiness.md)。源码开源与 CI runner 实际执行状态见 [开源与 CI 报告](docs/evidence/2026-10-27-opensource-ci.md)。当前账户级托管运行条件限制曾阻止任务启动；workflow 已配置不代表云端任务已成功。

## 快速开始

Windows 需要 Rust MSVC 工具链、Visual Studio C++ Build Tools、Windows SDK、.NET 8 SDK 与 Python 3.10+。当前构建 win-x64，WGC 显示器采集目标 Windows 10 1903+；本机验证为 Windows 11 专业版 x64 build 26200，其他系统 / ARM64 not-run。

```powershell
# 默认自包含 .NET 运行时的本地未签名开发目录；检查生产 DLL 启动与关闭
python3 scripts/build_windows.py --smoke-test
./dist/YM_Link/YM_Link.exe

# 单独原生模型 / 中英文 / 生命周期检查
dotnet run --project windows/Tests/YM_Link.Tests.csproj
# 加 --debug --framework-dependent 可缩短开发构建；需要 .NET 8 Windows Desktop Runtime
```

Windows 窗口可切换系统语言 / English / 简体中文，设备面板显示 TCP / UDP 端口并提供手动 IPv4:端口连接。发现不到时先核对同一局域网、访客网隔离与系统对应用的专用网络访问权限；应用不会自动改防火墙。选择“授权文件夹”后才开放该目录，未配对设备不可访问。录屏首次开始弹出本机确认，系统音频可关闭。当前 Windows 使用系统软件 H.264 编码与 CPU 缩放；不把配置档位当作实际性能，不承诺硬件编码、Quest 播放或延迟达标。开发目录不是安装器或已签名分发包。

macOS 构建目标为 **macOS 14+**。当前只在 **Apple Silicon、macOS 27.0.1、Xcode 27** 开发机验证；尚未在 macOS 14 或 Intel Mac 上运行，不能据此承诺这些环境已通过兼容性验收。需要本机 Rust / Cargo、Python 3.10+ 与 Xcode 的 Swift 工具链。

```bash
# 构建本地开发包（临时签名，未公证；最终构建结果见 STATUS）
python3 scripts/build_macos.py
open dist/YM_Link.app
```

启动后选择共享目录；共享范围仅限用户显式添加的目录。首次镜像由 macOS 请求屏幕录制权限；权限拒绝后按应用提示到系统设置中授权并重新打开应用。FPS / 码率来自实际数据统计，未测量的延迟不显示数值。

应用协议现为 v3，双方都需升级；发现 v1、Noise 承载与既有身份保持不变。本机首次开始镜像后，可在任一端暂停、恢复、停止或切屏。对端不能首次开始录屏；停止、断线或采集失败后需要本机重新开始。

持续发送拥塞时会逐步降低实际分辨率和目标码率，用户所选画质保留为上限；最低尺寸仍持续拥塞会提示关闭系统音频或重连。控制策略已具备自动回归，实际弱网采集 / Quest 解码效果尚待硬件验证。

```bash
# 仓库检查（任务 / 文档 / 链接合同；不构建 Rust）
python3 scripts/harness.py check

# 完整本地门禁：harness 回归 + 仓库检查 + rustfmt + cargo test
python3 scripts/harness.py verify

# 任务闭环
python3 scripts/harness.py new <topic> --title "标题" --workflow feature
python3 scripts/harness.py tasks
```

打开 demo 原型：浏览器直接打开 `design/prototypes/desktop/2026-10-27-ym-link-demo/index.html`。

## 头显协议测试桩

`headset_stub` 是 Rust 加密协议测试端，用于读取授权目录、range、下载和接收镜像数据；它不包含 Quest UI、Media3 播放或头显显示。桌面生产后端拒绝 loopback 地址；单机人工调试可使用本机局域网 IPv4，仍只算单机验证。

先在 Mac 启动 YM_Link，并查看动态 TCP 监听端口：

```bash
lsof -nP -a -c YM_Link -iTCP -sTCP:LISTEN

# 替换地址和端口；连接后在终端输入桌面显示的六位码
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT shares

# 使用 shares 返回的 SHARE_ID；路径相对于该共享根
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT list SHARE_ID
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT stat SHARE_ID sample.mp4
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT range SHARE_ID sample.mp4 0 1024
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT download SHARE_ID sample.mp4 ./download.mp4

# 配对后在桌面开始镜像；接收60秒，可选保存到一个新的目录
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT mirror 60 ./mirror-output

# 查看实际状态；控制仅限本机已开始的镜像
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT mirror-info
cargo run --locked -p ymlink-desktop --bin headset_stub -- 192.168.1.10:PORT mirror-control pause
# 其它控制：resume、stop、display DISPLAY_ID
```

测试桩每次启动生成临时身份，须重新配对；测试后可在桌面删除这些测试设备。下载不会覆盖现有目标，保留匹配的中间文件可续传。镜像输出为 `screen.h264` 与 `system-audio.f32le`，仅证明收到完整编码帧 / 音频包，不证明 Quest 可解码或音画同步。请勿把用户媒体、输出文件或原始路径日志提交到仓库。

下载 checkpoint schema 3 绑定已认证的来源设备身份；更换来源或使用旧 checkpoint 时会明确拒绝并保留原文件，不能混合续传。该本地格式版本独立于网络应用协议。
