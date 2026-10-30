# Windows 最新代码回归

- Date: 2026-10-30（北京时间）。
- Commit: 同步 `5677cdc..efe6f566730bc895baa4619e13c60d5c043e1691`；本报告所在提交追加测试桩修复、回归与记录，产品运行时 / 协议 / WPF 源码保持该基线。
- Host-OS: Windows 11 Pro x64，10.0.26200；Rust 1.85.0，.NET SDK 8.0.405。
- Result: 本机自动门禁、原生窗口与单机加密组件通过；真实 Windows↔Quest LAN、录屏 / 音频 / 播放体验 not-run。
- Task: [YML-003](../tasks/YML-003-windows-native.md)，现行四位协议见 [YML-008](../tasks/archive/YML-008-four-digit-pairing.md)。

## 同步与最近变化

开工 `git status --short --branch` 干净，main 位于 `5677cdc`。实际执行 fetch 与 ff-only 合并至 `efe6f56`，53 个文件变化。主要行为变化为应用 v4 四位配对 / 原位错码重试与限速、匿名发现候选端口轮换去重、正常已认证断开不再发布全局操作错误；另有 Rust 清理、gitleaks CI 和 macOS 外部 Rust archive 强制重链修复。Windows 模型 / 文案 / 四槽界面也已有对应实现。

## 发现与修复

首次用实际 Release WPF 和 `headset_stub shares` 联调，测试桩输出等待四位码，但窗口没有配对面板，5 秒后显示超时。根因为 CLI 在 `needs_pairing` 后先等待 stdin，直到取得输入才调用 `authorize(Enter)`；v4 桌面须先收到 PairRequest 才会协商窗口并显示码。该问题位于测试工具，不据此认定 Windows 产品配对失败。

新增 `desktop/tests/headset_stub.rs`，使用 Cargo 提供的实际二进制、随机 loopback TCP 端口与生产 core 对端：保持 stdin 未写入，要求收到 PairRequest，再发送 PairReady、输入合成的前导零码、完成 SPAKE2、Shares 与 Bye。修前在等待 PairRequest 的有界断言真实 RED；改为 `authorize_enter` 持有输入 future 后 GREEN。没有放宽协议、延长握手超时、加入自动信任或修改产品后端。

独立只读增量 CR 覆盖 stub / 新 integration test / README 及关联认证生命周期，无新增实质发现；发现 README 首段既有“六位码”遗漏，已同步为四位。对端拒绝 / 取消仍由生产 `authorize_enter` 处理，CLI 错误退出；测试异常由 RAII kill / wait 回收子进程。

受审基线为 `efe6f56`，修后 stub 源码 SHA-256 为 `8372c73957066cb3c755aa1b0ae8455eb72e6ffae19154248a8b8e4dc4f11136`，新增 integration test 为 `cb63863973ec9bc953989f56f89b78bd4fdcfe8dca48ce9552554bd4c284e3a0`；复审后只追加文档记录及上述 README 旧文案修正。

## 实际验证

| 检查 | 结果与边界 |
| --- | --- |
| `python3 scripts/harness.py verify` | 修前基线 Rust 102 passed / 2 ignored；修后完整 Rust 103 passed / 2 ignored。Python 共 38 项，36 passed / 2 skipped（macOS 专用）；仓库检查、rustfmt 通过。 |
| `python3 scripts/build_windows.py --smoke-test` | 两次 Release 自包含 win-x64 构建退出 0；WPF 模型 / 本地化 / Getter-only 实际行模板、真实 DLL 并发队列 / 销毁、生产后端启动 / 快照 / 关闭通过。smoke 本身不打开窗口。 |
| 显式系统编码检查 | `cargo test --locked -p ymlink-desktop native_encoder_emits_avcc_keyframe_without_screen_capture -- --ignored --nocapture`，1 passed，真实 Media Foundation 合成帧 H.264 / SPS 检查；没有读取屏幕或系统声音。 |
| 实际 Release WPF 窗口 | CUA 观察中文启动、3840×2160 显示器枚举、刷新 / 重新搜索、未连接时镜像禁用、英文镜像 / 文件共享 / 传输空态、回到系统中文、空地址回车错误提示与关闭提示、正常关闭。 |
| 实际 WPF + 修后测试桩 | 走本机 LAN IPv4（同一电脑，非双机 LAN）。四槽显示与倒计时、取消后面板消失；新会话显示前导零码，正确输入后加密认证和 Shares 查询退出 0。窗口最后显示 1 条已配对 / 已断开设备，无全局错误提示。原生输入码模式的第四位自动提交 / 摇动 / 清空由 WPF 模型及 Rust 运行时自动回归覆盖，本轮未用实际窗口重复该输入角色。 |
| 测试状态清理 | 窗口正常退出后，仅对新增、名称与平台均匹配测试桩的 1 个信任条目调用生产 C ABI forget；原有信任基线 0，清理后 0，现有系统身份保留。语言恢复跟随系统；没有授权用户媒体目录。 |

格式化后的首次定向复跑遇到人工联调仍在使用测试桩 EXE（OS error 5），未执行该次测试；CLI 退出后最终完整门禁包含新测试并通过。门禁前已退出 WPF，避免占用固定发现端口。完整日志保留于忽略入库的 `artifacts/windows-2026-10-30/`，配对码、用户路径与原始媒体不入库。

首次窗口 / 配对核验的 Rust DLL SHA-256 为 `d334767bd35cf035fa5ca3491ea007032224a13e01283ad9da3a32f6b27b401b`。测试桩修复后最终重建同一产品源码，模型 / ABI / smoke 再次通过；最终包再启动确认进入搜索、设备数为 0、显示器与中英资源正常，随后正常关闭。最终本机开发目录绑定如下，不当作已签名安装器：

| 文件 | 最终 SHA-256 |
| --- | --- |
| YM_Link.exe | `5ce627c11f8975388888bfe8f6e589ef392939ddfe2fbc25c740e0fa42df93f8` |
| YM_Link.dll | `3203e4f52c9d96908f3df20cec504ea5d6393b72d2770a22f0b0e60b3840acea` |
| ymlink_desktop.dll | `1eb5416dc984e6194b607b8fc0dfee947b2c753e168aa49d18dfe76943cb8779` |

## 未覆盖边界

ADB 实际设备列表为空；未验证 Windows↔Quest 的广播 / 防火墙、双方向配对与 Known 重连、真实文件播放 / seek、录屏 / 系统音频、Quest 解码 / 音画同步、弱网与 XR。没有运行要求 `YM_LINK_CAPTURE_TEST=1` 的真实采集测试。Windows 10 / ARM64、macOS 复跑、安装器 / 签名分发均 not-run；YML-003 保持 verifying，不能以本机回归代签设备验收。
