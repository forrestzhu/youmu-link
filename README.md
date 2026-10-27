# YM_Link

YM_Link 是 [Youmu VR Player](https://github.com/forrestzhu/youmu)（游目）的桌面伴侣应用：安装在 Windows / macOS 电脑上，与 Meta Quest 3 / 3S 上的游目通过局域网互联。

核心能力（按 [ROADMAP](docs/product/ROADMAP.md) 分阶段交付）：

1. **屏幕镜像** —— 把电脑上选定的显示器画面实时镜像到头显内的游目大屏播放。
2. **文件浏览与播放 / 传输** —— 在头显内浏览电脑的文件目录，选择视频直接流式播放，或传输到头显本地。
3. **设备发现与配对** —— 双端在同一局域网内互相发现，经显式配对码确认后建立加密连接；头显侧入口为游目内的 YM_Link 面板。

互联模型参考了内部项目 klip 的局域网同步设计（UDP 广播 beacon + mDNS 辅助 + Noise 加密 + 短码配对），协议独立实现；规范见 [PROTOCOL](docs/engineering/PROTOCOL.md)。

## 仓库状态

- 阶段：M0 框架启动（2026-10-27）。harness 任务闭环、产品 / 架构 / 协议文档、demo 原型与 `ymlink-core` 局域网发现骨架已就位；桌面 UI 壳、配对加密、文件与镜像 lane 按路线图推进。
- 许可：**暂闭源**（私有仓库，无 LICENSE 文件）。后续计划转 GitLab 分发，并在合适时机开源；在此之前不得公开仓库或对外分发。
- 当前进度：[STATUS](docs/STATUS.md)；文档索引：[docs/README](docs/README.md)。

## 仓库结构

| 路径 | 内容 |
| --- | --- |
| `core/` | `ymlink-core` Rust 协议核心（发现 beacon、后续配对 / 文件 / 镜像 lane） |
| `scripts/` | harness 脚本（任务闭环与仓库检查） |
| `docs/` | 产品定义、架构、协议、测试策略与任务记录 |
| `design/prototypes/` | 可点击 demo 原型 |
| `.github/workflows/` | CI（仓库检查 + Rust 矩阵测试） |

## 快速开始

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
