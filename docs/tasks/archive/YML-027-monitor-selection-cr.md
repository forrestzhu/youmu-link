# YML-027：显示器卡片验证与用户追加独立 CR

Status: done
Workflow: maintenance
Requires-Device: no
Tests: windows/Tests 生产 BAML / 选择绑定 / 双语两尺寸原生渲染、Release ABI 和自包含构建；CI 不筛选的 cargo workspace / all-targets 与 smoke；本轮不改协议

## Scope

用户追加要求对 [YML-026 原生显示器卡片](YML-026-monitor-selection-native.md) 做好测试验证，开发后启动新的独立 subagent CR，核验并修复属实问题，最终 commit / push main。受审基线为 `1692af1`，受审实现 `4469f28`，开工 main `f51e700`。保留用户此前明确的“窗口由我检查”，不激活 / 关闭 / 操作应用，不覆盖安装版；以现有自动测试和独立 CI 补足代码 / 构建验证。

## Acceptance

- [x] 当前源码的 Release WPF / 本地化 / 生产 BAML / 原生渲染 / 真实 DLL ABI / 自包含构建有新实际结果。
- [x] 核验同实现三平台 CI 的完整 Rust、构建 / smoke 与 gitleaks；明确保留本机已知端口限制及历史失败，不用筛选回归冒充完整通过。
- [x] 新独立 subagent 完成 CR，记录受审提交、发现；属实问题修复 / 验证 / 复审，无问题则如实记录。
- [x] 最终自查、任务 / STATUS 更新、commit / push main 与 require-pushed 门禁闭环；交付前执行严格门禁核对最终提交与实际远端。

## Verification

运行 `python3 scripts/build_windows.py` 并用 `YML_UI_EVIDENCE_DIR` 导出既有 Window.Show 测试样本；检查代表性的两语 / 两尺寸镜像卡片。读取 `f51e700` 的 GitHub Actions 三平台完整日志 / steps、制品结果和 gitleaks。若有 CR 问题，先核验，再以最小修复和针对性回归验证，并补适用完整门禁；纯记录修改跑 `harness check`。实际用户窗口 hover / 键盘 / 系统高对比 not-run（用户自己检查），Quest / LAN / 真实采集 not-applicable（本轮纯 UI 验证）。最后 commit / push，`harness check --require-pushed`。

## Evidence

2026-10-31：开工 main 干净，fetch / ff-only 已最新。已启动全新的独立审查 subagent 和只读 CI 证据审计 subagent；不复用上一轮 CR 结论代替本次审查。`f51e700` 的 gitleaks 已 success，repository-checks 的 Linux checks 已 success，原生平台仍在运行；`4469f28` 的旧 run 被后续文档推送取消，保留 cancelled，不计作完整成功。

- 本机新运行 `python3 scripts/build_windows.py` 退出0：Rust Release、生产 BAML 事件连接器、原生选择 / 布局 / 下拉回归、中英资源 / 状态模型、真实 DLL 并发快照 / 命令 / drain / 重复 dispose / 关闭拒绝、自包含 win-x64 publish 与 EXE 图标验证全部通过。`dist/evidence/monitor-selection-cr/` 导出24张原生 Window.Show 样本，主代理抽检本轮中文1024×720 / 英文860×580的 mirror 样本，整卡白 / 淡蓝与单层蓝边正确。
- 用户追加的新独立 CR：受审范围 `1692af1a859614b06d6c8832264ed4511e59787c..f51e700f6a894ee35774fc35896126f7a9f80505`、代码提交 `4469f28`。检查继承样式状态优先级、固定边框 / padding、显示器局部原生 FocusVisualStyle、ListBox 选择与无障碍名称、系统高对比映射，并抽检两张代表性的原生样本；未发现需要修复的问题，因此无属实缺陷修复或复审轮次。未将旧审查结果算作本次 CR。
- 本轮只读 netstat 仍确认旧安装版占用 UDP47610；遵守用户“窗口由我检查”，不关闭应用换取本机端口回退测试通过。本机此前完整门禁的端口失败及追加并发测试5项超时 / 随后串行runtime33通过仍保留在 YML-026，不用本轮 WPF / 构建成功掩盖。完整无筛选 Rust 与启动 smoke 由同源独立 CI 检查。
- `git diff --check`、`python3 scripts/harness.py check` 通过。人工 hover / Tab / 方向键 / 高对比和完整应用启动本轮 not-run（用户自行检查），不以原生样本或 CR 代签。
- 同实现 `f51e700f6a894ee35774fc35896126f7a9f80505` 的 [repository-checks run41184493604](https://github.com/forrestzhu/youmu-link/actions/runs/41184493604) 最终 conclusion=success，checks / windows / macos 三个 job 均 success；[gitleaks run41184493563](https://github.com/forrestzhu/youmu-link/actions/runs/41184493563) 同 SHA success。主代理核对终态、步骤与解码 job 日志，不仅采用审计 subagent 的状态结论。
- Linux checks job124488520489：Python47项、仓库 baseline、rustfmt、`cargo test --workspace --all-targets --locked` 共104 passed / 0 failed / 0 ignored / 0 filtered。Windows job124488709123：同一完整 cargo 命令的10个 suite 合计117 passed / 0 failed / 2 ignored / 0 filtered；原生模型 / 本地化 / BAML / 两语两尺寸渲染 / ABI / 图标 / Release / 启动 snapshot 与 clean shutdown smoke、安装器完整载荷 / 选项 / 静默不启动 / 启动关闭 / 卸载清理、ZIP / SHA 与制品上传全通过。
- macOS job124488709086：10个 Rust suite 合计110 passed / 0 failed / 1 ignored / 0 filtered；Swift9项零失败，archive→Swift 重链接回归、应用资源 / 图标 / 静态链接 / ad-hoc签名、DMG / ZIP 载荷验证与上传通过。以上保留既有显式硬件采集 ignored，不代替真实屏幕 / 系统音频或用户机器交互验收。
- CI 三份制品清单已核对，均 expired=false：Windows UI（725274 bytes）、Windows x64（124199601 bytes）、macOS arm64（6716644 bytes）。这里只证明 CI 成功生成 / 上传，不宣称本机下载、覆盖安装、签名分发或公开 Release。本地原始日志和 PNG 保持忽略入库，仅将必要结果记录在本任务。

## Next

新 CR 无属实待修问题，生产源码与完整通过的 `f51e700` 相同，本次交付提交只补记录。用户按既定选择自行检查实际窗口；后续真实连接 / 镜像仍由对应任务验收，不重复推断。
