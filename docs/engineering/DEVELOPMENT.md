# Development guide

Build, test and protocol tools for contributors. For the product and downloads, see the [project homepage](../../README.md).

## 本地构建与运行

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

macOS 构建目标为 **macOS 14+**。当前只在 **Apple Silicon、macOS 27.0.1、Xcode 27** 开发机验证；尚未在 macOS 14 或 Intel Mac 上运行，不能据此承诺这些环境已通过兼容性验收。需要本机 Rust / Cargo、Python 3.10+ 与 Xcode 26+ 的 Swift 工具链；锁定的 Metal bridge 编译需要 macOS 26 SDK，CI 选择已安装的 Xcode 26.3。较新的编译 SDK 不等于旧系统兼容性已验收。

```bash
# 构建本地开发包（临时签名，未公证；最终构建结果见 STATUS）
python3 scripts/build_macos.py
open dist/YM_Link.app
```

启动后选择共享目录；共享范围仅限用户显式添加的目录。首次镜像由 macOS 请求屏幕录制权限；权限拒绝后按应用提示到系统设置中授权并重新打开应用。FPS / 码率来自实际数据统计，未测量的延迟不显示数值。

应用协议现为 v4，双方都需升级；发现 v1、Noise 承载与既有身份保持不变。本机首次开始镜像后，可在任一端暂停、恢复、停止或切屏。对端不能首次开始录屏；停止、断线或采集失败后需要本机重新开始。

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

打开 demo 原型：浏览器直接打开 [`design/prototypes/desktop/2026-10-27-ym-link-demo/index.html`](../../design/prototypes/desktop/2026-10-27-ym-link-demo/index.html)。

## 头显协议测试桩

`headset_stub` 是 Rust 加密协议测试端，用于读取授权目录、range、下载和接收镜像数据；它不包含 Quest UI、Media3 播放或头显显示。桌面生产后端拒绝 loopback 地址；单机人工调试可使用本机局域网 IPv4，仍只算单机验证。

先在 Mac 启动 YM_Link，并查看动态 TCP 监听端口：

```bash
lsof -nP -a -c YM_Link -iTCP -sTCP:LISTEN

# 替换地址和端口；连接后在终端输入桌面显示的四位码
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
