# YML-002：macOS 原生客户端与 Rust 安全互连、文件和镜像实现

Status: verifying
Workflow: feature
Requires-Device: yes
Tests: core::tests::discovery、core::tests::session、core::tests::files、core::tests::protocol_vectors、desktop 状态与 C ABI 测试、Swift 构建；LAN / Quest 另记设备报告

## Scope

用户要求基于当前 demo 实现，参考已验证的局域网互连设计，后端 Rust、前端原生，优先 macOS。本任务承接 M2–M4 桌面侧；见 [计划](../plans/2026-10-27-macos-native.md) 与 [ADR-0002](../engineering/decisions/0002-native-desktop.md)。不复制参考项目源码。Quest 端由游目仓库承接，保留互操作与真机验收要求。

## Spec

给定同 LAN 对端，用户在原生设备列表连接，经 Noise XX 与 SPAKE2 六位码授权后交换业务数据；受信设备匹配静态密钥才可重连。授权目录支持分页浏览、range 和断点传输，越界请求拒绝。显示器经系统授权后可开启 / 停止屏幕与音频采集，未经配对不发送。断线停止镜像，文件操作等待恢复。权限拒绝、配对失败与超时有明确 UI。持续发送拥塞先降实际分辨率，保留用户档位上限；以 [PROTOCOL](../engineering/PROTOCOL.md) v0.5 为准。

## Acceptance

- [x] 原生 `.app` 可构建并通过本地开发签名校验，设备 / 镜像 / 共享 / 传输界面读取 Rust 状态。
- [x] 原生窗口实际启动通过（用户手动打开后确认，随后 CUA 取得真实窗口）；完整配对 / 文件 / 镜像 UI 联调继续。
- [x] 安全会话、配对、信任 / 遗忘 / 重连和发现回退有测试证据。
- [x] 受限目录、range / 暂停与断点恢复有测试证据。
- [x] 用户追加的两轮独立 subagent CR 已完成，6 项属实问题修复并复核关闭；证据见[本轮报告](../evidence/2026-10-27-native-two-round-cr.md)。
- [ ] Rust 真实采集 / 编码 / 加密发送屏幕与系统音频，权限失败可恢复；真实弱网降分辨率后可继续解码。
- [x] `harness verify`、原生构建与独立审查通过；已有 main 提交与交付门禁证据（`712cabd` + `ba65b72` + `7614ae3`），本轮提交后再核验远端一致性。
- [ ] LAN 与 Quest 文件播放 / 镜像 / 音画同步报告完成。

## Verification

1. Rust → `cargo test --workspace --all-targets` → 错码、过期、换钥、越界、range 和 loopback 通过。
2. 原生 → `python3 scripts/build_macos.py` → 可启动 `.app`，不含 WebView。
3. 本地门禁 → `harness verify` → PASS；交付 → `check --require-pushed` → HEAD 等于远端 main。
4. UI / 采集 → 实际观察与授权后真实帧 / 字节统计；未授权不声明通过。
5. L3 / L4 / L5 → 第二设备及 Quest → not-run，等待设备与游目端实现。

## Evidence

- 用户追加至少两轮 subagent CR，按完整实现基线 `7dce375..8e41ea0` 开展首轮独立分区审查，修复后再做第二轮；轮次、发现、核验及交付证据集中于[本轮 CR 报告](../evidence/2026-10-27-native-two-round-cr.md)。
- 本轮两轮 CR 交付代码 `d9130b8` 已推送 main，`check --require-pushed` 实跑 PASS；6 项发现全部修复并复核。原生应用已重建 / 打开、70 Rust / 2 Swift 与显式硬件编码通过；真实采集与 Quest 验收未完成，YML-002 保持 verifying。
- 2026-10-27 联调条件实查：使用本机已有 SDK 的 `adb devices -l`，当前连接设备 0；只读核对相邻游目 `app/src/main` 与 YM-276，尚无可运行 YM_Link v3 客户端，头显集成仍属后续任务。未安装 / 启动 Android 包、未修改游目仓库。录屏确认尚无用户答复，未采集画面或音频。解除条件：用户确认本机采集测试并完成系统授权；Quest / 第二设备具备匹配 v3 的端侧实现与连接条件后，继续 L3–L5 验收。
- 完成范围复核定位产品体验原则“弱网先降分辨率”尚缺：旧 capture 只丢帧 / 补关键帧。本轮先修订规范 v0.5（无新增字段，应用仍 v3），补持续背压检测与降低实际采集 / 编码分辨率；录屏与 Quest 验收仍待外部条件。
- 弱网续轮回归：尺寸 cap 先 RED（960 上限仍生成 1920），修复后 GREEN；运行时策略先 RED（满队列两窗口未触发），修复后 GREEN。最终 `harness verify`：59 passed、0 failed、1 ignored；显式硬件合成帧编码 1 passed，视频计数实际为 1 次尝试 / 0 丢弃。真实采集、自动重启后帧 / 音频与 Quest 弱网解码仍 not-run。
- 弱网续轮审查（`b5260fe` + 工作树）发现 fatal 媒体包可能被背压分支跳过、以及旧代际初始化错误可能丢失两项 P2。改为共享授权链的独立首错信号：fail / stop 同锁同步；Err / JoinErr 在释放 capture gate 前记录首错，UI 仍按代际更新；恢复 / 切屏共用链，仅有效的本机 Start 更新。隔离测试永久占用 capture gate，即使断言失败也不能录屏；移除保护或不保存旧启动错误时均实际 RED，补齐后 GREEN。原 reviewer 限定复审确认两条时序关闭。
- 故障边界补齐后的完整 `harness verify`：62 passed、0 failed、1 ignored；显式硬件编码 1/1，Swift 1/1。原生包已重建与签名校验，窗口重新打开成功；未触发真实录屏 / 音频或权限申请。此处不将策略单测或合成编码作为实际弱网体验证据。
- 弱网代码 `092e514` 已推送 main，`check --require-pushed` 实跑 PASS；任务保持 verifying，等待真实采集和设备验收。
- 续轮差距核对：产品 F1 要求双端暂停 / 停止 / 切屏，上一实现仅本机开始 / 停止。本轮先将协议文档升 v0.4、应用 Hello / 业务 v3，补齐本机与已配对对端控制及授权边界；录屏确认仍待用户回复，不用单元测试代替实际采集。
- 本轮独立只读复核（基线 `7614ae3` + 工作树）：镜像控制未发现新 P1 / P2；检查首次开始 / 权限请求限制、过期请求、连接代际、stop / error 解除授权与 capture gate 保留到真实 teardown。文件 actor 复核发现同连接同 id 重开会受旧 Ack 干扰，规范先明确同连接 id 不复用、最多 1024 个，重连后仍可原 id 续传，再补回归修复。
- 本轮最终 `harness verify` PASS：57 Rust passed、0 failed、1 ignored；`swift test --package-path macos` 1/1；显式合成像素硬件编码 1/1。新增回归实际验证慢 catalog / 真实 TCP 写反压下 Stop 转发、文件工作上限与取消后真实 I/O 仍持额度、退休 id 迟到 Ack / 重开拒绝、失效本地控制不影响新任务。独立复核的 ID 复用 P2 已先 RED 后 GREEN，随后根代理回读实现与回归。
- 当前 v3 生产 `.app` 重建与现有 Apple Development 签名校验通过，CUA 打开成功。截图确认功能栏不再有 Workspace 三行占位；同机实际网卡地址经两次原生弹窗配对，MirrorQuery 返回 v3 / stopped 与实际显示器，首次远端 Resume 返回 ok=false 且状态仍 stopped，无录屏或授权弹窗。两条临时信任已由原生“移除配对”清理，设备数回到 0。本次仍为同机控制链路验证，不代替真实采集或双机 LAN。
- 本轮代码提交 `3b6329b` 已推送 `origin/main`；`python3 scripts/harness.py check --require-pushed` 实跑 PASS，确认 HEAD 等于实际远端 main，工作区干净。真实采集 / 设备验收未完成，任务保持 verifying。

- 可审查主提交 `712cabd` 与镜像 P2 修复 `ba65b72` 均已推送 main，`check --require-pushed` 实跑 PASS。独立复核确认旧流 running 发布竞态关闭。新增固定向量由手工 CBOR 与标准库 SHA-256 独立生成，`cargo test --offline -p ymlink-core --test protocol_vectors` 1 passed；未要求跨语言 map 键顺序相同，未宣称 Kotlin 互操作已通过。
- 最新已回读 CI run `40452957099`（受审代码 `ba65b72`）：三个任务均未启动，annotations 明确为 GitHub 账户级托管运行条件限制；属于 not-run，不标测试失败或通过，未修改账户设置。

- 原生移除配对菜单与共享目录“移除”按钮已实测：测试信任和共享目录配置均回到空集合，测试设备 / 样本授权已清理。

- 原生生产包联调：通过真实系统目录选择器授权隔离的 4 MiB 样本目录；重启后仍可见。使用当前实际企业网段地址与 Rust 测试对端完成原生弹窗六位码配对、Noise/SPAKE2 授权与文件下载，4,194,304 字节 SHA-256 与源一致；`.part` 与 checkpoint 均已清理，原生传输页显示 sample.bin 已完成与 100%。这是同机使用实际网卡地址的自连验证，不是第二设备 LAN / Quest 验收。
- 用户 Workspace 三行反馈：原生 segmented Picker 增加 labelsHidden()，重启后截图确认冗余标题不再占位；主包 InfoPlist 中英用途资源补齐，系统语言下原生 UI 可正常显示中文。显示器硬件名称精确化按用户指示暂留小项。
- 实际企业网段修复：旧 RFC1918 判定拒绝当前实际配置的企业网段；先增纯测试得到明确 RED，再改为有效接口 + 连续非零掩码的同子网单播判定。文档先升 v0.3（报文仍发现1/业务2），TCP 入站、手动拨号与测试对端复用同一 is_lan_source；新增3项过滤回归 GREEN。
- 回归环境记录：原生 app 运行时会占用 UDP 47610，使固定端口回退用例报 AddrInUse；在正常退出 app 后全量测试通过。一次中途完整门禁仅因 root 两处新接入调用未 rustfmt 失败；格式化后重新运行 harness verify 全绿。

- 2026-10-27：main 工作区干净；fetch + ff-only 同步；本机 macOS 27.0.1 / Xcode 27.0 / Swift 6.4 / Rust 1.85。
- `cargo test --offline --workspace --all-targets`：46 passed、0 failed、1 ignored（硬件编码默认不在通用 CI 跑）；core 37 项覆盖发现 / 身份 / PAKE / 文件边界，桌面 9 项覆盖 C ABI、真实 TCP 整体流程、媒体格式与测试桩安全落盘。
- `cargo test --offline -p ymlink-desktop --lib hardware_encoder_emits_avcc_keyframe_without_screen_capture -- --ignored`：1 passed，实际 VideoToolbox require-hardware 生成 avcC + AVCC 关键帧；无录屏或屏幕权限申请，不能代替屏幕采集验收。
- `swift test --package-path macos`：1 passed，检查 Rust snapshot 字段、中英文资源与错误回显边界。`swift build --package-path macos -c release` 与 `build_macos.py` 完成；包资源、执行权限、系统动态依赖与 `codesign --verify --strict` 检查通过。使用现有 Apple Development 身份做本地开发签名，未公证、未对外分发。
- `python3 scripts/harness.py verify`：PASS（harness 回归、仓库合同、rustfmt、workspace 测试）；最终提交后交付门禁另记。
- 首次构建失败记录：release 过程宏报 E0463；与 debug 直接 dlopen 对照确认 release Mach-O 的 LINKEDIT string pool 未对齐，符合 [Rust #157750](https://github.com/rust-lang/rust/issues/157750)。只对 release build-override 设置 strip=false 后，在保留 macOS14 部署目标的相同脚本中构建通过；未更换工具链或改系统安全设置。
- 依赖拉取首次 120s 截止超时；关闭 Cargo HTTP multiplexing 并限定后续请求低速 / 总时限后完成，避免重复等待同一无输出进程。
- 独立审查（受审基线 `7dce375` + 本任务工作树）：会话审查验证 Noise remote static / Hello pin、双向 Known、PAKE 绑定、总期限、取消半帧、信任落盘；运行时审查发现全局 trust 锁跨配对等待、旧连接 / 镜像代际回写、迟到 Ack / 本机控制导致断连、媒体反压、慢磁盘阻塞、断线恢复状态和手动断开门控。均在共享入口修复，并以桌面真实 TCP 用例复核配对 / range / pause-resume-cancel / 迟到 Ack / 重连 / 断开拒入 / 忘记后重配 / 取消配对。测试桩旧流半包另留重组回归。最终专项受审提交 `712cabd`：独立 reviewer 发现镜像旧任务 running 控制消息可迟到覆盖新流（P2）；将代际检查 / running 发布与状态提交置于同一同步门禁，并用非阻塞发送消除 await 空窗，补 `mirror_start_announcement_rejects_old_generations_and_full_control_queues` 回归实际通过；独立 reviewer 回读同步门禁与回归后确认原 P2 时序已关闭，未扩大复审范围。
- 启动观察：早期 CUA 自动入口两次 spawn 后约 11s 以 LSExitStatus=9 结束，终止者未知；不据此认定应用崩溃。用户手动启动后明确确认能看到窗口，CUA 随后成功取得原生 AX 窗口：设备真实空态、3 个实际显示器、镜像 / 共享 / 传输标签与系统权限入口可见。Keychain 初始化与 Rust 监听已进入搜索态。原先自动入口异常保留为观察事实，未改系统安全设置。
- 用户反馈：显示器通用名称不准确，先记小项；要求优先推进整个业务逻辑。另指出 Workspace 标签被挤为三行，按原生 Picker 隐藏冗余标签修复。
- 自动安全检查拒绝通过 Terminal UI 做启动诊断，原因仅为“安全原因”；未绕过该限制。只记录上述净化结论，不提交原始系统日志。
- 真实双设备 LAN / Quest、屏幕与音频采集、权限恢复、旧系统 / Intel 兼容性、体验指标均 not-run。

## Next

原生窗口 / Keychain / 配对 / 文件主链路已实测；等待用户确认录屏与系统权限，继续真实屏幕 / 音频采集和 LAN / Quest 联调。当前保持 verifying 与总目标 active，缺少这些证据不标 done。
