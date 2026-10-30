# YM_Link

**把电脑上的内容，带进你的私人影院。**

YM_Link 是 Youmu VR Player（游目）的 Windows / macOS 桌面伴侣，与 Meta Quest 3 / 3S 通过局域网连接。

[**下载开发预览**](https://youmu.forrestzhu.com/zh/ym-link/#download) · [安装说明](engineering/DISTRIBUTION.md) · [English](../README.md) · [游目官网](https://youmu.forrestzhu.com/)

![电脑与 Quest 通过加密局域网连接，授权目录后在游目中浏览影片；屏幕镜像仍为实验功能。](assets/ym-link-overview.svg)

- **电脑片库，头显直接看。** 只共享你选择的目录，在游目内浏览、直接播放，或传输到头显稍后观看。
- **把桌面放上大屏。** 选择显示器进行镜像，可附带系统音频。镜像仍属实验功能，真实采集、Quest 播放与音画同步继续验证中。
- **四位配对，局域网加密。** 同网发现、首次短码确认、保存信任后重连；YM_Link 无需账号或云端中继。

头显需使用支持 **YM_Link 协议 v4** 的游目构建；当前 Meta 商店版本兼容性未验证。

安装并打开两端 → 连接同一局域网 → 在游目打开 YM_Link，输入四位码 → 电脑添加共享目录。

头显需使用支持 YM_Link 协议 v4 的游目构建；当前 Meta 商店版本的兼容性尚未验证。

当前为**开发预览**：Windows x64、Apple Silicon Mac（目标 macOS 14+），尚未正式签名 / 公证。Intel Mac、Windows ARM64 与旧版目标系统未完成兼容性验收；外部媒体来源适用游目的使用权益要求。实际构建与安装说明见[下载指南](engineering/DISTRIBUTION.md)；Quest 影片播放仍需进一步硬件验证，实测边界见 [STATUS](STATUS.md)。

[反馈问题](https://github.com/forrestzhu/youmu-link/issues) · [开发指南](engineering/DEVELOPMENT.md) · [文档索引](README.md) · [Apache-2.0](../LICENSE) · [第三方许可](../THIRD_PARTY_NOTICES.md)
