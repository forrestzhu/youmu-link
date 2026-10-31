# 当前进度

更新：2026-10-31。仅记录最近实测事实、未关闭问题与下一步；详细证据见任务。

YM-332已按用户要求先推第一轮修复，再完成第二轮CR及属实问题复审，33项runtime回归通过。已合入另一会话7ae881c，a830d75三平台完整CI /gitleaks通过。原安装版进程退出后，97dacbd Rust DLL已备份并更新为当前Release（managed55与.NET8.0.33保持），尚未重启；另一会话Quest22E2E /180语言已核验，实际WPF↔Quest LAN待共享Windows网络夹具释放后验证。详情见[YML-023](tasks/YML-023-encrypted-identification.md)。

用户授权的历史邮箱修正已完成：20 个既有提交的 author / committer 均统一为目标邮箱，20 个文件树不变，16 个提交号变化；旧记录中的提交号见[旧 → 新映射](evidence/2026-10-27-history-email-rewrite.md)。

## 当前制品与服务

| 项目 | 最近实际记录及边界 |
| --- | --- |
| 仓库 | GitHub public `forrestzhu/youmu-link`，2026-10-27 用户明确授权 Apache-2.0 开发预览；匿名 API / LICENSE 200、私下漏洞报告 enabled=true。历史邮箱已统一，未追加历史正文 / 旧运行记录清理，不自动发布正式 Release。 |
| 设计与实现基线 | 用户授权基于现有 demo 实现，M1 通过；[YML-001](tasks/archive/YML-001-bootstrap.md) 归档。按 [ADR-0002](engineering/decisions/0002-native-desktop.md) 改为 macOS SwiftUI / AppKit 原生界面 + Rust 后端，取代桌面 Tauri 方案。 |
| Rust 核心 | UDP 发现 / 三端口回退、Noise XX、SPAKE2、系统身份与信任、capability 授权文件服务。rescan 单路由失败不阻断其余探测，新增 2 项真实 UDP 回归；核心 Windows 46 / Mac 41 项通过。当前应用协议v4 /四位配对，发现v1与transport-v2保持；1147 Quest真实LAN core链已通过，原生Mac当前补正常断开反馈回归。 |
| Rust 桌面后端 | C ABI + 单设备状态、取消 / 重连、16KiB 停等 Ack 传输；读写与文件工作分别调度，慢磁盘 / socket 不阻塞停止指令。文件工作每连接 4 项、真实 I/O 跨连接全局 4 项；同连接传输 id 不复用，防迟到确认误作用于新任务。 |
| macOS 原生应用 | macOS 15.8.2 arm64 / Xcode 26.3 云端：Rust100 passed/1 ignored、Swift6/6、静态重链接2/2、新图标/静态链接/ad-hoc签名/移位中英资源/DMG完整载荷通过（最新链环a438c0b）。本机26.3.1链环图标独立副本About实显通过；此前镂刻版About/Finder实显保留；此前288成员/138×2文案/许可/签名/路径独立包核验保留。此前 Mac 27.0.1 手工配对 / 4 MiB 下载为历史证据；新版人工窗口交互、Mac 14 / Intel not-run。 |
| Windows 原生应用 | 10-30 同步 `efe6f56` 后，Windows 11 Pro x64 build 26200 的完整 Rust 103 passed / 2 ignored、Python 36 passed / 2 skipped、WPF 模型 / 本地化 / 行模板 / ABI、Release 自包含目录与 smoke、系统 H.264 合成编码通过。实际窗口中英 / 显示器 / 四位前导零 / 取消、单机加密 Shares 查询与正常断开无误报通过；修复 CLI 配对协商前等待 stdin 的测试桩缺陷，临时信任清至 0。真实 Windows↔Quest LAN / 采集、Windows 10 / ARM64、安装器 / 签名 not-run。[报告](evidence/2026-10-30-windows-regression.md)、[YML-003](tasks/YML-003-windows-native.md)。 |
| 镜像 | Rust ScreenCaptureKit + VideoToolbox 硬件 H.264、系统 PCM 与加密分片、双端控制、持续背压降分辨率实现就位。首次开始必须本机确认；独立故障信号跨暂停 / 恢复保存，采集或初始化失败不能自动重启，对端不请求系统权限。真实硬件合成帧编码单测通过；真实采集 / 弱网切档、权限恢复、Quest 解码 / 音画同步 / 延迟仍 not-run。 |
| Windows 镜像 | Rust WGC / D3D11、CPU NV12 转换与系统 Media Foundation 软件 H.264、WASAPI loopback，复用 v3 AVCC / PCM 输出与双端控制。合成 HD 编码及实际 SPS 色彩标记检查通过；修复 WinRT 工厂缓存跨 apartment 销毁造成的反复启动崩溃。真实屏幕 / 系统音频采集及性能 not-run，不推断硬件编码性能。 |
| harness / CI | 97dacbd的[run41175348703](https://github.com/forrestzhu/youmu-link/actions/runs/41175348703)三平台及[gitleaks](https://github.com/forrestzhu/youmu-link/actions/runs/41175348707)全success；Linux /Windows /Mac完整测试、Windows WPF /ABI /Release /smoke /安装器 /ZIP、Mac Swift /签名 /DMG /ZIP及制品上传通过。本机串行完整门禁Python47 /3 skipped、Rust109 passed /2 ignored通过。此前公开制品的独立下载核验保留；用户追加授权后已手动发布 [开发预览](https://github.com/forrestzhu/youmu-link/releases/tag/v0.1.0-preview.20261030.1)，8个公开资产 digest / 大小及匿名下载回读通过，无 main 自动发版。见[分发记录](evidence/2026-10-30-distribution-home.md)。 |
| gitleaks 防泄漏 CI | `.github/workflows/gitleaks.yml` 于 push(main) / PR / 手动触发，SHA 锁定 gitleaks-action v3.0.0；push/PR 增量扫新提交、dispatch 全量历史（run 40624080000 扫 31 commits 通过），本地 26 commits 全量基线无泄漏，见 [YML-005](tasks/archive/YML-005-ci-gitleaks.md)。 |

## 当前工作与待验收

| 工作 | 当前结论 / 下一动作 |
| --- | --- |
| [YML-027 显示器卡片追加验证 / CR](tasks/archive/YML-027-monitor-selection-cr.md) | done：用户追加的新独立CR无待修问题，本机新Release / WPF / BAML / ABI /24张原生样本检查通过。f51e700的run41184493604三平台与gitleaks全success；Linux Rust104、Windows117 /2 ignored、macOS110 /1 ignored均0 filtered，Swift9、原生启动smoke / 安装卸载 / DMG与ZIP通过。未操作用户窗口或覆盖安装版，保留历史本机失败和旧CI cancelled。 |
| [YML-026 显示器卡片原生落地](tasks/archive/YML-026-monitor-selection-native.md) | done（代码 / 构建）：4469f28 实现整卡淡蓝 / 单蓝边、白底未选卡片及内部键盘焦点；Release 自包含构建 / ABI、生产 BAML / 双语两尺寸24张原生样本通过。用户明确窗口由自己检查，安装版未覆盖；开发版在 `dist/YM_Link`。本机端口占用 / 并发超时历史保留，后继三平台完整CI与新独立CR已由YML-027验证通过。 |
| [YML-025 显示器选择 HTML 原型](tasks/archive/YML-025-monitor-selection-demo.md) | done（原型）：整卡淡蓝选中、单层蓝边，选中 hover 不叠线；点击 / 原生 radio 方向键、900px 双列及 373 / 320px 单列浏览器检查通过。用户已授权原生落地，见YML-026。仓库 check / Python47通过，完整 verify 的 discovery 端口回退测试被运行中应用占 UDP47610 阻断，保留失败，不代签原生修复。 |
| [YML-024 本机虚假设备与新安装包](tasks/YML-024-self-discovery-filter.md) | verifying：本机多网卡修复8fdf714及动态新增/32的CR修复7ae881c均已推送；新增回归green、复审关闭。同产品源码a830d75的run41181994786三平台全success，Windows117 /2 ignored /0 filtered、Release /ABI /启动及两轮安装卸载通过；52,113,676 bytes新安装器及SHA /ZIP已取回至dist/packages/ci-a830d75/并核验，待用户沿用原目录覆盖安装与设备列表实显。未替换现有安装版，本机占端口失败保留，未发布公开Release。 |
| [YML-023 加密连接前识别](tasks/YML-023-encrypted-identification.md) | verifying：Hello-only不接管active；追加取消 /撤销 /pin及双向单边信任、匿名新IP /端口刷新恢复回归均RED→GREEN，两轮新独立CR属实问题已关闭。1717413三平台CI /gitleaks全success，本机runtime33项及Rust Release通过。安装版仍为97dacbd，保留修复后的界面DLL和.NET8.0.33；游目[YM-332](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-332-link-device-identification.md)原生已接入并推送，完整主机 /新包E2E继续执行，实际Quest3S↔Windows LAN待验。 |
| [YML-022 VR 连接闪退](tasks/YML-022-windows-connection-crash.md) | verifying：系统记录与编译BAML回归复现Button→DockPanel；EventSetter修复55eef04已推送，回归 /独立CR /Release /ABI smoke与本机安装版实际启动通过。保留原组件备份与.NET8.0.33，后继Rust更新见YML-023。首轮Rust bind OS10055，串行完整门禁107 passed /2 ignored通过；待真实VR重连。Quest3S已通过ADB在线，名称 /系统由YM-331 Demo推进至YM-332原生开发，UDP继续匿名。 |
| [YML-021 安装后启动与桌面图标](tasks/archive/YML-021-installer-launch-shortcut.md) | done：完成页立即打开与桌面快捷方式选项均默认勾选 / 可取消，静默安装不自启。57a9e0d 三平台 CI success，两轮真实安装（默认 /取消桌面任务）的完整载荷 / .lnk 目标 / 进程检查 / 原生启动 / 卸载清理通过；52,083,482 bytes 安装器已下载至 `dist/packages/ci-57a9e0d/`，SHA-256 / CRC / 元数据 / 许可 / 图标通过。保留 YML-020 修复，未签名；用户安装及交互完成页 /升级记忆 not-run，未覆盖既有安装。 |
| [YML-020 Windows 显示器名称](tasks/archive/YML-020-windows-display-names.md) | done：已修复后端固定 Display n，按各 GDI 源的活动 monitor 提取 PnP 型号；本机实际窗口 / Release C ABI 已显示 HWV6446（3840×2160）及 RTK1600（1600×2560）。四项新回归、本机完整门禁（Rust107 /2 ignored、Python47 /3 skipped）、独立审查及 ffe9ce5 /57a9e0d 两次三平台 CI 通过；最终安装包合并 YML-021 已取回核验。首轮并行检查端口占用保留记录，未恢复性能排查。 |
| [YML-019 Windows 安装包取回](tasks/archive/YML-019-latest-windows-package.md) | done（历史交付）：已取回 f9df8d3 同源 CI 的 Windows x64 EXE / ZIP，与当时 main 产品 / 依赖 / 打包代码一致；本机及独立代理完整 SHA-256、ZIP CRC / 元数据 / 279 份依赖许可 / 安装器图标通过。安装器52,118,106 bytes、未签名，位于忽略入库的 `dist/packages/ci-f9df8d3/`；后继显示器修复和安装体验由 YML-020 /021 更新。 |
| [YML-018 Windows UI 对齐](tasks/YML-018-windows-ui-alignment.md) | 实现 /CI通过，用户机验收blocked：按Mac对齐侧栏、原生分段导航、显示器卡片与三页空态，统一WPF控件。f9df8d3 Windows103 /2 ignored、生产XAML双语两尺寸24状态 /12图抽检、真实ABI、Release /安装启动卸载通过；新版包已生成。待Windows11电脑上线补默认宽窗 /实际键盘 /高对比；YML-017性能排查保持暂停。[记录](evidence/2026-10-31-windows-ui-alignment.md)。 |
| [YML-017 Windows 性能排查](tasks/YML-017-windows-idle-performance.md) | blocked（用户要求暂停，待实际连接VR后通知继续）：安装版激活后两段约61s平均0.87%单核；后继开发版恢复前台30s约67.73%单核（16线程归一化4.23%）。两次热线程栈经过UIA同步派发，观察干扰尚未排除；对照被并行任务关闭进程打断。已停止测量 / 窗口操作，未改产品代码，不宣称根因或真实性能验收通过。[报告](evidence/2026-10-31-windows-idle-performance.md)。 |
| [YML-016 共享路径显示](tasks/archive/YML-016-share-path-display.md) | done：普通盘符 / UNC 展示修复 `9583921`；用户追加独立 CR 的 1 P2（尾点 / 尾空格目录复制歧义）已修复，6 项边界回归 red→green、复审关闭。本轮完整门禁（Rust103 /2 ignored、Python47 /3 skipped）、WPF / Release / ABI / smoke 通过，最终主窗口已启动、配置校验不变；文件页回读时用户 Esc 停止自动化，本轮最终文件页回读 not-run。原始路径与授权保留，未替换已安装版本或公开 Release。 |
| [YML-015 GitHub 首页图](tasks/archive/YML-015-readme-hero.md) | done：内置 imagegen 生成暖白 / 蓝金 3D 设备示意图，第二轮增大辅助文字并加深实验说明；中英 README 已接入。850px 明暗背景与独立审阅通过，390px 缩略图细节需打开原图；完整替代文本与原 SVG 保留。实现 dd22349 已推送且 strict 门禁通过，未改应用或官网。 |
| [YML-014 Windows 安装包](tasks/archive/YML-014-windows-performance-package.md) | done（安装包准备）：10-31 本机最新 main Release / WPF / 真实 ABI / 启动 smoke、许可与 ZIP 校验通过；已取得并校验 run41085401188 的 Windows x64 EXE（源码5e85961，与当前main产品 / 依赖 / 资源 / 打包脚本一致，仅文档不同），CI 安装 / 启动 / 卸载通过，完整 SHA 与许可保留。待用户自行安装后实测空闲 CPU / 内存及交互后回落；未以 Mac 修复或 CI 代签 Windows 性能。 |
| [YML-013 空闲 CPU](tasks/archive/YML-013-idle-cpu.md) | done：重复发布引发SwiftUI持续布局已修复（9da3eda），两轮独立CR / 完整门禁通过。用户允许钥匙串后，最终签名版完整启动，含页面交互后静置的三段共90s平均CPU约0.144%，5s采样主线程全部正常等待，三页面响应通过；Python47 / Rust100（1 ignored）/ Swift9、构建与签名核验保留。数小时 / 真实镜像性能另验。[报告](evidence/2026-10-30-idle-cpu.md)。 |
| [YML-012 原标与Link](tasks/archive/YML-012-link-logo.md) | done：右侧加宽留白蓝金链环已正式采用。资源/原生/安装器与a438c0b三平台CI通过；本机受占端口两项在CI无过滤通过，About/GitHub实际显示已确认。官网YM-320已上线并通过三语桌面/窄屏及25目标TLS回读；未替换公开Release。见[记录](evidence/2026-10-30-linked-app-icon.md)。 |
| [YML-011 应用图标](tasks/archive/YML-011-app-icon.md) | done（历史镂刻版，视觉由YML-012后继替换）：用户选定第4个“镂刻”；正式SVG/PNG/ICNS/ICO、Mac/Windows/安装器及GitHub品牌引用完成。本机Python47/Rust100/Swift6、About/Finder实显及d6ccceb三平台CI/安装器通过；官网三语页眉已上线并通过25目标正常TLS回读。未替换既有公开Release。见[验收](evidence/2026-10-30-app-icon.md)。 |
| [YML-010 产品界面文案](tasks/archive/YML-010-product-copy.md) | done：AGENTS 补充三条克制文案约束，优先简洁操作、图标与按需帮助；保留无障碍、真实限制及明确适用边界。仅文档修改，harness check 与 diff 自查通过；后续界面改动按此执行。 |
| [YML-009 首页与安装包](tasks/archive/YML-009-distribution-home.md) | done：README / CONTRIBUTING / SECURITY英文优先并保留中文；Windows安装器 / Mac DMG及三平台真实CI、手动[公开预览](https://github.com/forrestzhu/youmu-link/releases/tag/v0.1.0-preview.20261030.1)已交付。[官网品牌下载](https://youmu.forrestzhu.com/ym-link/)已上线、首页可达，YM标识 / 平台图标、两轮CR / 24路由正常TLS字节核验 / Chrome实际DMG下载通过。[记录](evidence/2026-10-30-distribution-home.md)。main不自动发版。 |
| [YML-008 四位配对](tasks/archive/YML-008-four-digit-pairing.md) | **原生实现与组件验证通过**：四位v4、原位错码清空、自动提交与信任保持；Android1147的22E2E /171语言 /192视觉通过。正常断开误报及SwiftPM复用旧后端均已修，完整99 Rust /38 Python /6 Swift、真实重链与各增量双轮独立CR通过。6d613d0新app双方向原生配对 /Known重连 /共享目录各1/1，结束无误报，临时配对清理至0；[证据](evidence/2026-10-29-four-digit-native-relink.md)。真实录屏 /声音 /XR与Windows↔Quest LAN另验。 |
| [YML-007 配对与重复候选](tasks/archive/YML-007-pairing-discovery-regression.md) | done（桌面归并与原生配对）：同IP端口变化改为替换匿名行，认证清理历史行并保留可信身份；4项新回归、完整37 Python /78 Rust /1 ignored、Swift4与本机app构建启动通过。初轮端口占用 /超时保留；同一实现CI三平台单次重跑全success（首轮Mac容量取消保留）；Quest2339→原生Mac1/1真实组件配对 /Known重连通过，临时身份已清理；XR /实际录屏声音另验。[报告](evidence/2026-10-28-pairing-discovery-regression.md)。 |
| [YML-002 原生 macOS 实现](tasks/YML-002-macos-native.md) | verifying：双端控制、传输边界与弱网策略自动回归通过；真实屏幕 / 系统音频采集等待用户确认，弱网切档与 Quest 验收未完成。 |
| [YML-003 Windows 原生实现](tasks/YML-003-windows-native.md) | verifying：10-30 最新四位实现的 Windows 门禁 / 构建 / 原生窗口与单机配对通过，测试桩调用顺序缺陷已修复并经真实子进程回归、独立 CR；[最新证据](evidence/2026-10-30-windows-regression.md)。此前两轮 CR 的三项发现全部修复并复核，见 [Windows 两轮 CR](evidence/2026-10-27-windows-two-round-cr.md)。实际 Quest / LAN、文件播放与屏幕 / 音频仍待设备。 |
| [YML-004 开源与 CI](tasks/archive/YML-004-opensource-ci.md) | done 并归档：Apache-2.0 公开开发预览、匿名可读、私下漏洞入口、中英安全 / 贡献说明、三平台真实 CI 与双平台开发包全部闭环。CI 发现的 SDK / 扫描早退 / 中文资源问题均修复并经两轮审查和实际 GREEN，见[公开与云端证据](evidence/2026-10-27-public-launch.md)。 |
| [用户追加两轮 CR](evidence/2026-10-27-native-two-round-cr.md) | 完成：首轮 3 P2 / 1 P3、第二轮 2 P3，全部修复并复核关闭；覆盖端点 pin、采集 teardown、传输历史 / 连接交接、重复错误事件和测试桩换源续传。 |
| [YML-006 BeautifulRust 清理](tasks/archive/YML-006-beautifulrust-conformance.md) | done 并归档：两轮独立 CR 的 2 P2 / 2 P3 已修复并复核关闭（生产 unwrap 漏扫、FFI Safety、错误 / panic 文档）；仅修测试 socket 绑定解决既有 UDP 门禁失败。Python 37/37、Rust 74 passed / 1 ignored、定向 Clippy / rustdoc 通过；既有全量 lint 告警保留，业务 / 协议不变。见[两轮 CR 报告](evidence/2026-10-28-beautifulrust-two-round-cr.md)。 |
| 本地启动 | 用户手动启动已确认成功，CUA 已取得窗口。早期自动启动入口出现 SIGKILL 的原因不明，未据此修改应用生命周期或系统安全策略。 |
| 真实 LAN / Quest | 应用v4已与游目Kotlin集成；Quest3S1147与生产Rust core互操作 /文件seek /合成GPU和PCM通过，新Mac原生app双方向四位配对 /Known重连 /共享目录各1/1通过。原生结束无错误弹窗，临时配对清理至0；真实录屏 /声音 /XR与Windows↔Quest LAN尚未通过，见[YML-008](tasks/archive/YML-008-four-digit-pairing.md)。 |

## 下一步

Windows最新桌面回归 /三平台CI与安装版更新已完成，Quest3S已通过ADB在线；继续游目YM-332原生开发，再以最新双端实际运行包验收连接前识别、真实连接 /取消及已有会话保持。真实LAN、文件播放与录屏 /音频仍按对应任务验收，不以设备在线、本机协议桩、CI或合成编码代签。

YML-008四位实现、新app双方向原生组件与正常结束无误报均已通过，系统钥匙串阻塞已解除；6d613d0三平台CI及新增实际重链回归均已通过，最终记录按main严格门禁交付。真实录屏 /声音 /XR与Windows↔Quest LAN继续按对应任务验收，不能用组件或CI代签。

YML-007候选归并已交付，四位后继YML-008也已完成新app双方向组件复验。安装器 CI 已由 YML-009 验证；正式签名 /公证与公开发行按M6后续推进，见[ROADMAP](product/ROADMAP.md)。
