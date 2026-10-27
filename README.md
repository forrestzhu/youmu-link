# YM_Link

YM_Link 是 [Youmu VR Player](https://github.com/forrestzhu/youmu)（游目）的桌面伴侣应用：安装在 Windows / macOS 电脑上，与 Meta Quest 3 / 3S 上的游目通过局域网互联。

核心能力（按 [ROADMAP](docs/product/ROADMAP.md) 分阶段交付）：

1. **屏幕镜像** —— 把电脑上选定的显示器画面实时镜像到头显内的游目大屏播放。
2. **文件浏览与播放 / 传输** —— 在头显内浏览电脑的文件目录，选择视频直接流式播放，或传输到头显本地。
3. **设备发现与配对** —— 双端在同一局域网内互相发现，经显式配对码确认后建立加密连接；头显侧入口为游目内的 YM_Link 面板。

互联模型参考内部项目 klip 的局域网同步设计（UDP 广播 beacon、动态 TCP 端口、Noise 加密与持久信任），源码与协议独立实现。首次连接须经六位码 SPAKE2 配对；规范见 [PROTOCOL](docs/engineering/PROTOCOL.md)。

## 仓库状态

- 阶段：用户已授权基于 demo 实现，macOS 原生客户端开发中（[YML-002](docs/tasks/YML-002-macos-native.md)）。SwiftUI / AppKit 负责界面，Rust 负责发现、加密配对、授权文件服务、传输与屏幕采集编码；架构见 [ADR-0002](docs/engineering/decisions/0002-native-desktop.md)。Windows 原生界面与游目头显侧 YM_Link 集成尚未实现。
- 验证：Rust core 37 项测试通过；桌面后端已有真实 TCP 的单机配对、文件 range、传输控制与重连集成验证。macOS `.app` 构建和本地开发签名已通过，本机窗口已由用户与 CUA 确认，原生配对与 4 MiB 下载已实测；双机 LAN、真实 Quest 播放 / 镜像与音画同步为 **not-run**，不以测试桩代替设备验收。
- 许可：**暂闭源**（私有仓库，无 LICENSE 文件）。后续计划转 GitLab 分发，并在合适时机开源；在此之前不得公开仓库或对外分发。
- 当前进度：[STATUS](docs/STATUS.md)；文档索引：[docs/README](docs/README.md)。

## 仓库结构

| 路径 | 内容 |
| --- | --- |
| `core/` | `ymlink-core`：UDP 发现、Noise / SPAKE2、身份与信任、授权目录与文件 range 服务 |
| `desktop/` | `ymlink-desktop`：Rust 桌面状态、连接 / 传输、macOS 采集与硬件 H.264 编码、C ABI、头显测试桩 |
| `macos/` | SwiftUI / AppKit 原生界面、目录选择器、中英文资源与 Swift 模型测试 |
| `scripts/` | harness 任务闭环、仓库检查与本地 `.app` 构建 |
| `docs/` | 产品定义、架构、协议、测试策略与任务记录 |
| `design/prototypes/` | 可点击 demo 原型 |
| `.github/workflows/` | CI（仓库检查 + Rust 矩阵测试） |

## 快速开始

macOS 构建目标为 **macOS 14+**。当前只在 **Apple Silicon、macOS 27.0.1、Xcode 27** 开发机验证；尚未在 macOS 14 或 Intel Mac 上运行，不能据此承诺这些环境已通过兼容性验收。需要本机 Rust / Cargo、Python 3.10+ 与 Xcode 的 Swift 工具链。

```bash
# 构建本地开发包（临时签名，未公证；最终构建结果见 STATUS）
python3 scripts/build_macos.py
open dist/YM_Link.app
```

启动后选择共享目录；共享范围仅限用户显式添加的目录。首次镜像由 macOS 请求屏幕录制权限；权限拒绝后按应用提示到系统设置中授权并重新打开应用。FPS / 码率来自实际数据统计，未测量的延迟不显示数值。

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
```

测试桩每次启动生成临时身份，须重新配对；测试后可在桌面删除这些测试设备。下载不会覆盖现有目标，保留匹配的中间文件可续传。镜像输出为 `screen.h264` 与 `system-audio.f32le`，仅证明收到完整编码帧 / 音频包，不证明 Quest 可解码或音画同步。请勿把用户媒体、输出文件或原始路径日志提交到仓库。
