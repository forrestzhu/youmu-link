# YML-003：Windows 原生客户端与 Rust 局域网互联、文件及镜像适配

Status: verifying
Workflow: feature
Requires-Device: yes
Tests: cargo test --workspace --all-targets; dotnet run --project windows/Tests/YM_Link.Tests.csproj; Windows build / startup smoke test

## Scope

参考 macOS 与最近提交，在 Windows 实现原生 WPF 界面、Rust 平台身份与采集适配；复用发现、Noise / SPAKE2 配对、信任重连、授权目录与传输逻辑。支持英文 / 简体中文。用户要求开发后至少两轮独立 subagent CR，修复属实问题后 commit / push main。头显 Kotlin 客户端、安装器与公开分发不在本次范围。

## Spec

最初沿用 PROTOCOL v3；现行四位配对已由 [YML-008](archive/YML-008-four-digit-pairing.md) 升为 [PROTOCOL 应用 v4](../engineering/PROTOCOL.md)。设计与实施见 [Windows 计划](../plans/2026-10-27-windows-native.md)。Windows WPF 仅负责展示、系统目录选择与本机镜像确认，经 C ABI 调用共用 Rust 后端。未配对不得访问目录或镜像；首次镜像必须本机确认；后台恢复不得再次请求系统权限。私钥放 Windows Credential Manager，状态保存在当前用户应用目录并限制 ACL。Windows Graphics Capture / Media Foundation / WASAPI 负责画面、H.264 与可选 PCM 音频。不得将单机协议桩视为 Quest 验收。

## Acceptance

- [x] Windows 原生应用构建、启动，发现 / 配对 / 重连与目录授权持久化自动测试通过。
- [x] 镜像 Windows 采集适配、协议媒体输出、取消与故障回归通过；中英文资源完整。
- [x] 至少两轮独立 CR 完成，属实问题修复与复核；harness verify 通过。
- [x] commit / push main，check --require-pushed 通过。
- [ ] 真实 Windows + Quest LAN 发现、配对、目录播放与镜像 / 音频验收（缺设备时保留 not-run）。

## Verification

先为 Windows 身份存储、桌面配对角色与文件边界补回归，再实现平台适配。稳定后运行 `python3 scripts/harness.py verify`、原生 .NET 测试、`python3 scripts/build_windows.py` 与应用 smoke test。实际启动核验原生窗口，真实录屏 / 音频另列证据。真实 Quest 及第二电脑 LAN 独立记录设备 / OS 与结果，不从 loopback 推断。

## Evidence

开工 main 干净，`git fetch origin main` / `git merge --ff-only origin/main` 无新增；基线 `3a81ead`。macOS 最近修复为 `d9130b8`。开工发现 Windows 身份仅 macOS 支持、Hello 平台硬编码 macos、采集非 macOS stub，确认为三个平台缺口。

- RED / GREEN：Credential Manager 持久化 / 损坏拒绝、Windows / macOS 双方向短码角色回归、同大小 / 时间换源续传拒绝、DOS 设备名路径拒绝均先失败后修复。junction 越界与 ACL 回归通过。
- 首次完整 cargo 尝试遇到测试 EXE 被并发构建占用（OS32，未执行桌面测试）；停止并发运行后发现反复启动的 WinRT 缓存 use-after-apartment 崩溃（0xC0000005）。连续新线程查询单测稳定 RED，改作用域内工厂后 GREEN；不掩盖初始失败。
- `python3 scripts/harness.py verify`：退出 0，74 Rust passed / 2 ignored；harness 回归 / 仓库结构 / rustfmt 通过。`native_encoder_emits_avcc_keyframe_without_screen_capture -- --ignored --nocapture` 退出 0，实际系统 H.264 合成帧编码通过，未录屏。
- `python3 scripts/build_windows.py --smoke-test`：退出 0，Release 自包含 win-x64 构建、真实 DLL 队列读写 / 销毁测试、生产后端监听 / 快照 / 关闭检查通过。实际主机 Windows 11 专业版 x64 build 26200、Rust 1.85、.NET SDK 8.0.405。
- 原生 CUA 已核验实际 Release 窗口、显示器枚举、中文 / 英文切换、镜像未连接禁用、文件与传输空态、恢复系统语言并正常关闭；修正英文共享页残留 Mac 文案。未授权任何用户目录、未实际采集屏幕 / 音频。
- CR 第 1 轮受审基线 `3a81ead` / staged tree `3f99d123686ad2c6f529357eb8440d1c37a89aa8`；独立 reviewer 确认 P1 数据行双向绑定异常、P2 Local mutex 跨会话失效。修复与第二轮结果继续追加。
- 两轮结果见 [Windows 实现与两轮 CR](../evidence/2026-10-27-windows-two-round-cr.md)：首轮两项全部修复，第二轮受审 tree `c8f08f0b4b7db9d7a7f28bf6827db4c9c870add8` 发现 SPS 色彩描述 P2，经实际 HD 合成编码 RED / GREEN 和定点复审关闭。最终 `harness verify` 76 passed / 2 ignored；显式实际编码检查通过，.NET STA 数据行 / 本地化 / 真实 ABI 生命周期、最终 Release 构建 / smoke 通过。目录授权经实际后端添加→重启恢复→撤销→重启不再恢复的回归通过。
- 交付代码 `d9b54fa6ba7dd911412dd0c9db4afe4ea0d77f11` 已 commit / push main，`harness check --require-pushed` 实跑通过；本地开发包为 `dist/YM_Link/YM_Link.exe`，未签名 / 未对外分发。
- 2026-10-30 从 `5677cdc` ff-only 同步到 `efe6f56` 后在 Windows 11 Pro x64 build 26200 复验，见 [Windows 最新代码回归](../evidence/2026-10-30-windows-regression.md)。修复测试桩先等 stdin、后发 PairRequest 导致桌面无显示码即超时的问题，真实子进程 RED / GREEN；最终完整 Rust 103 passed / 2 ignored、Python 36 passed / 2 skipped、WPF 模型 / 本地化 / 数据行 / ABI、Release / smoke、系统 H.264 合成编码通过。真实 WPF 中英切换、四槽与前导零、取消、加密 Shares 查询及正常断开无误报通过；临时信任清理至 0。未改产品协议、桌面运行时或 WPF 实现；Windows↔Quest LAN 与真实采集仍 not-run。

## Next

开发交付完成，任务保留 verifying：真实 Windows + Quest LAN 发现 / 配对、文件播放 / seek、镜像 / 音频同步及弱网未验收；解除条件为可连接的 v4 Quest 客户端与第二台设备。旧 Windows / ARM64、macOS 构建复跑 not-run（本轮主机为 Windows，未改 macOS 界面）。
