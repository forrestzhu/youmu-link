# Windows 原生实现与两轮 CR

日期：2026-10-27。任务：[YML-003](../tasks/YML-003-windows-native.md)。

Base-Commit: 3a81eade1de1eca03a2616116d688fc70b28e198
Host-OS: Windows 11 Pro x64 10.0.26200
Result: partial — Windows 开发交付、本机验证与两轮 CR 完成；L3 / L4 / L5 not-run

## 范围与实现

Windows WPF / .NET 8 原生界面，中英资源；复用 Rust v3 局域网发现、Noise / SPAKE2、单设备会话 / 重连、授权目录与 range / 传输、镜像控制。Windows 身份放 Credential Manager，状态目录受保护 ACL；Windows 文件 token 绑定同句柄 file ID / ChangeTime，拒绝 DOS 歧义组件。WGC + CPU NV12 + 系统同步 Media Foundation 软件 H.264，WASAPI loopback；未承诺硬件编码或实测 FPS / 延迟。

## 开发回归与问题复现

- 平台 Hello / 配对角色测试先后 RED 于 macos != windows 与 Windows 双方均显示短码；修改实际平台与桌面角色规则后，Windows / macOS / headset-test × 两方向共六场真实 TCP + Noise / SPAKE2 通过。
- Credential Manager 独立测试条目持久化 / 重读 / 损坏拒绝；私有目录 ACL、普通文件拒绝、junction 不可重定向状态或越过共享根通过。测试条目自行清理，不删除生产身份。
- 同大小且创建 / 修改时间相同的新文件，原版本 token 会允许混合读取（RED）；增加文件 ID 与 ChangeTime 后返回 Changed（GREEN）。DOS 设备名原返回 Unavailable，现读取前 Denied。
- 第一次完整 cargo 命令遇到并发构建 EXE 的 Windows OS32 占用，桌面测试未执行；停止并发后发现反复后台启动访问冲突 0xC0000005。新的重复 COM apartment 能力查询稳定复现；WinRT 静态工厂缓存改成作用域内持有，线程 RoUninitialize 前释放，重复查询与 Backend 反复创建通过。
- Windows 错误资源扫描曾暴露 display callback 漏映射，补全类别后通过；实际英文窗口发现共享页残留 Mac 文案，改为 PC。

## 第一轮独立 CR

Reviewer: `cr_round1`（独立上下文，含原生 UI 专项子审查）。受审提交基线见上；受审 staged tree：`3f99d123686ad2c6f529357eb8440d1c37a89aa8`，未提前 commit。

| 等级 | 问题 / 触发 | 修复与核验 |
| --- | --- | --- |
| P1 | ShareItem.Path / TransferItem.Fraction 为只读属性，但 TextBox.Text / ProgressBar.Value 默认双向绑定，首次出现数据行时 WPF 抛 InvalidOperationException | 两处显式 OneWay；实际生产 DataTemplate + 只读行模型的 STA 测试分别 RED 后 GREEN，还验证 Row.Update 后展示更新 |
| P2 | Local 命名 mutex 仅覆盖单会话，同一用户 console / RDP 可并发改同一配置 | Global + SID 跨会话命名；重建后实际第二生产进程 --smoke-test 返回 4，在创建后端前拒绝。真实多会话环境 not-run |

Rust 身份 / ACL / 文件版本、平台配对、WGC / MF / WASAPI、背压、COM / 关闭路径未发现其他确定问题。审查未实际录屏。

## 第二轮独立 CR

Reviewer: `cr_round2`（新的独立上下文）。受审 staged tree：`c8f08f0b4b7db9d7a7f28bf6827db4c9c870add8`。完整改动复核及首轮两项修复检查完成。

发现一项 P2 色彩契约缺口：NV12 使用 BT.601 limited，但编码类型未声明矩阵 / 范围。Microsoft 将未知 matrix 视作 BT.709，HD 接收端默认也可能按 BT.709 解码。实际合成 1280×720 原生编码 SPS 检查 RED：缺少 video color description，得到 None，而应为 limited / matrix 6。为 input / output 设置常规 MF 属性后仍 RED；ICodecAPI 相关颜色选项返回 E_NOTIMPL，不能把 API 设置成功当作修复。

修复：有界 Main profile SPS 解析、VUI 色彩描述规范化，保留其他字段并同时覆盖 avcC 与 in-band SPS。明确 full_range=false / matrix=6，primaries 与 transfer 均为 unspecified=2，不从 YUV 矩阵推断桌面色域。新增纯回归覆盖已有 / 缺失 VUI、错误矩阵与全范围、原生 timing / 转义、逐段截断、非法转义与幂等；实际 1280×720 合成编码检查 SPS 的四项字段 GREEN。首次新增错误消息被 .NET 本地化覆盖检查拦截，补齐 video color 类别后最终构建通过。

`cr_round2` 对修复后的工作文件再次定点复审：确认配置与帧内两个路径、Main SPS / HRD / VUI / 尾位 / emulation-prevention、色域未指定策略和回归断言，判定 P2 resolved，无新增可操作问题。该次为定点复核，不宣称额外完整第三轮 CR。

## 实际验证

- 最终 `python3 scripts/harness.py verify`：exit 0，harness 回归、仓库 / 文档检查、rustfmt 通过；76 Rust passed / 2 ignored（早期 74，追加 SPS 与授权目录重启 / 撤销持久化回归后为 76）。
- `cargo test --locked -p ymlink-desktop native_encoder_emits_avcc_keyframe_without_screen_capture -- --ignored --nocapture`：exit 0，实际系统编码器输出 avcC + AVCC 关键帧；输入为合成像素，未读取屏幕 / 音频。
- `.NET` Release 模型 / 本地化 / 错误类别 / ABI 生命周期测试及 WPF Release 构建通过；CR 新增的实际 STA 数据行模板回归通过。
- 最终 `python3 scripts/build_windows.py --smoke-test`：exit 0，win-x64 自包含未签名开发目录生成；真实 DLL 的队列快照 / 指令、在途完成后销毁、生产后端监听 / 快照 / 干净关闭通过。两轮 CR 修复后重建与启动检查通过；另实测第二实例 exit 4，已有实例不受影响。
- CUA 本机真实窗口：启动 / 显示器枚举、中文→English→跟随系统、镜像未连接禁用、文件 / 传输空态与正常关闭已核验。不提交窗口中的主机名、用户路径或原始日志。

## 限制与下一步

未实际采集屏幕或系统音频；`real_display_and_system_audio_capture_stop_cleanly` 默认 ignored 且须 `YM_LINK_CAPTURE_TEST=1` 显式确认。Quest 型号 / 系统版本：not-run；第二电脑 OS：not-run。没有可用 v3 Quest 客户端 / 双设备验证证据。真实 LAN 发现、防火墙、休眠 / 切网、视频播放 / seek、镜像解码、音画同步与弱网均待实机验收。macOS 构建 / Swift 测试本轮 not-run（Windows 主机，未改原生 Swift）。Windows 10 / ARM64 not-run；未创建安装器、未签名分发、未公开仓库。
