# Windows 原生 YM_Link 实施计划

**目标：** Windows 原生入口复用 macOS 已验证的 Rust 发现、加密配对、目录文件、传输与镜像控制逻辑，英文 / 简体中文可切换。

**架构：** WPF / .NET 8 原生窗口以 UTF-8 C ABI 调用 `ymlink-desktop` DLL，网络与权限逻辑继续留在 Rust。Windows Credential Manager 持久化身份，当前用户应用目录用受保护 ACL 保存信任与目录授权。Windows Graphics Capture + Media Foundation H.264 + WASAPI loopback 适配现有媒体输出；协议保持 v3，不复制 klip 源码。

**规格：** [YML-003](../tasks/YML-003-windows-native.md)、[产品定义](../product/YM-LINK.md)、[PROTOCOL](../engineering/PROTOCOL.md)。用户本次已授权实施、测试、两轮 CR 和 main 提交推送。

## 实施与验收

- [x] 平台后端：先补 Windows 身份持久化、目录 ACL、文件路径 / 换源边界与 Windows/macOS 配对角色测试，再实现缺口。复用 `Backend` / `Command` / snapshot；Windows Hello 标识 windows。
- [x] 原生界面：`windows/` WPF、资源与无第三方测试框架的 .NET 测试。发现列表、短码配对、手动 LAN 端点、撤销信任、目录授权、传输、显示器与镜像操作；轮询不覆盖编辑输入，不重复提示同一错误；销毁必须等所有 ABI 调用结束。
- [x] Windows 媒体：`desktop/src/capture/windows.rs` 实现 Capture 接口，WGC、H.264 / AVCC、PCM、有限队列、关键帧恢复、取消与终端故障。编码器合成帧验证与真实采集分开，未经本地操作不开始录屏。
- [x] 构建入口：`scripts/build_windows.py` 生成忽略入库的 `dist/YM_Link`，CLI smoke test 验证生产 DLL / 持久身份启动与退出。补 Windows CI 原生构建与测试。
- [x] 集成验证：harness verify、原生测试、release 构建及窗口启动检查。先查明失败根因再修复，不靠跳过失败用例通过。
- [x] 完成代码后独立 subagent 首轮 CR（平台安全 / 生命周期 / 互通），修复并复核；第二轮新 subagent CR 覆盖修订后全量 diff，修复并复核。记录每轮受审版本 / 发现 / 结果。
- [x] 更新任务、STATUS、架构与构建说明；自查 diff；commit / push main 并 `check --require-pushed`。

## 风险与验证边界

重点检查 Windows 路径前缀 / ADS / junction 越权、文件版本与断点换源、跨平台短码角色、ABI 关闭竞态、WGC 回调 teardown、端口回退与 LAN 防火墙提示。WPF 使用系统字体、标准焦点与文件对话框，布局沿用 macOS 产品信息结构。Windows 媒体采用系统同步 H.264 编码器；其吞吐与延迟按实测，不承诺硬件编码性能。真实 Quest 客户端与第二电脑 LAN 缺失时记 not-run，开发交付不替代实机验收，任务保留 verifying。
