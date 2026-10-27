# ADR-0003：Windows 原生界面与 Rust 平台适配

日期：2026-10-27。状态：接受。用户授权参考 macOS 实现 Windows，业务后端尽量 Rust 复用、前端 native，中英文先行。

## 决策

- Windows 使用 WPF / .NET 8 原生窗口、控件、目录选择与键盘焦点，不使用 WebView。沿用 macOS 的设备 / 镜像 / 授权目录 / 传输信息结构。
- 复用 `ymlink-core` 与 `ymlink-desktop`。Rust 额外输出 cdylib；WPF 经现有 UTF-8 C ABI 命令与快照调用。所有 ABI 调用串行放到 UI 线程外，SafeHandle 销毁等待在途调用；每用户单实例防止配置竞争。
- 私钥放 Windows Credential Manager（不回退明文）；信任与目录配置保存在当前用户 LocalAppData 中的 Youmu / YM_Link。Rust 以目录句柄设置当前用户 / SYSTEM 的受保护继承 ACL，拒绝 reparse 根。远端文件始终经 capability 目录句柄读取。
- Windows Graphics Capture 以 Win32 显示器互操作与 free-threaded frame pool 采集；D3D11 读回并转换 NV12，系统 Media Foundation 同步 H.264 编码器输出 AVCC，WASAPI loopback 输出 48 kHz / stereo / f32le。当前为 CPU 编码与转换；不把系统编码器等同硬件编码。硬件 MFT 优化按实测再立项。
- Native frontend 仅在本机确认后首次开始镜像。暂停、重连、切屏与队列背压等控制继续由 Rust 统一处理，Windows 接入不增加协议字段，不升级 v3。
- 本地构建默认生成 win-x64 自包含开发目录 `dist/YM_Link`，包括 .NET 运行时；不提交 SDK / 缓存 / 产物，不创建安装器或对外分发。

## 兼容与验证边界

WGC 显示器互操作目标 Windows 10 1903+；本次构建为 x64。实际 OS、编码器测试、原生窗口验证以 [YML-003](../../tasks/YML-003-windows-native.md) 为准，未测试系统与 ARM64 不推断通过。双机发现、防火墙、Quest Kotlin、文件播放、真实采集音画同步与弱网体验独立验收；自动测试与合成帧不代替真机。
