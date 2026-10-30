# YML-013：修复 macOS 空闲界面持续高 CPU 占用

Status: blocked
Workflow: behavior
Requires-Device: no
Tests: not-applicable (Rust 协议与后端行为不变；Swift 发布去重回归、完整 harness verify 与本机进程 CPU / 原生界面核验)

## Scope

定位并修复用户本机 YM_Link 空闲时持续约一个核心的 CPU 占用；覆盖 macOS 快照发布与实际布局热点，不改协议、信任、共享目录或录屏授权。

## Spec

- 给定后台快照内容没有变化，定时读取不能反复发布相同的 SwiftUI 状态；配对输入也不能因无配对的轮询持续发出变更。
- 真实状态、配对倒计时 / 重试、错误事件、传输进度变化仍按现有 500ms 读取周期及时到达界面。
- 修前保留真实 CPU 与线程采样；修后在本机同类空闲场景测量并检查窗口可用性，不以测试通过推断真实性能。

## Acceptance

- [x] 真实采样定位占用线程及根因，保存脱敏证据。
- [x] 相同快照不重复发布，真实变化与配对 / 错误语义有 Swift 回归。
- [x] 完整 harness verify、Swift 测试、app 构建与去重版本机启动 / CPU 对比通过。
- [ ] 最终签名版经用户处理系统钥匙串弹窗后完整启动并复测 CPU（外部解除条件）。
- [x] 用户追加至少两轮 subagent CR、属实问题修复 / 复审、记录、main 推送与 strict 门禁完成。

## Verification

对运行进程用 ps / top 与 5 秒 sample 建立基线；Swift 回归先 red 再修复 green。实现稳定后运行 harness verify、Swift 测试、build_macos.py；原生窗口交互与空闲 CPU 多次采样。真实 Quest / 镜像性能不属于本次空闲 CPU 修复验收，不以单机代签。

## Evidence

开工 main 干净，fetch / ff-only 已同步。用户进程持续约 2.5 小时，ps 98.5% CPU，top 两次增量为 81.2% / 97.2%。5 秒 sample 中主线程持续 SwiftUI GraphHost.flushTransactions / RootGeometry.sizeThatFits / AttributeGraph；两个 Tokio worker 几乎全程 park，后台快照队列只有 2 / 3871 样本。原始采样只保留在本机临时文件，不入库。宿主 macOS 26.3.1 (25D2128) arm64。

新增 Swift 同步发布回归先 red（20 次相同输入产生 20 次快照 / 20 次配对输入发布，共 5 项断言失败），快照及输入值去重后 green。完整 Swift 9/9 通过，含同文本新 errorId、倒计时 / 阶段 / attempt、传输进度及重复错码 / 新窗口的延迟清除边界；app 构建、静态链接 / 签名 / 移位中英文资源核验通过。

单变量 A/B：保留原生分段 Picker 与 500ms 轮询，只消除无效发布。原版重启后暂降到约 5–8%（说明持续 98% 包含运行态布局因素）；修复版首个 30.01s 累积 CPU 仅 0.05s，平均 0.167%。后续 5 秒 sample 的主线程 4278/4278 样本全部在 runloop 等待，原先分段控件尺寸测量热点消失。原始问题采样该路径约 73% 主线程样本，证据不支持扩大为替换原生控件或修改 Rust。三页面切换、刷新显示器、地址 sheet 打开 / 取消实际窗口均通过。

独立审查：cpu_audit 检查 Rust 循环与完整 Swift diff；swiftui_audit 定位分段控件热点并复核输入 / 错误语义。两者无剩余 actionable findings，建议的桥接边界测试已补充并通过。长期数小时与真实镜像性能尚未测，不从本次空闲结果推断。

交互后第二个 30.02s 窗口累计 CPU 0.07s、平均 0.233%。RUST_TEST_THREADS=1 完整 harness verify 通过：Python47、Rust100 passed / 1 ignored、rustfmt 与仓库检查；Swift9/9。修复版已复制回用户原来的本机 app 启动位置，旧 bundle 备份至 ignored artifacts/yml013；复制前后可执行文件 SHA-256 同为 6c1e0da33ad80a7dbbda524fa5d0a6669ceac417a2b36ace6d9b6cb3119dc560，签名校验通过，并通过原生窗口重新启动。

用户追加要求：修复后至少两轮独立 subagent CR，修复属实问题后 commit / push main。正式第1轮 cr_round1 初审基线 2ca69d6c86d2974eba8ee9f7f6292d0e04e9a90b，三个 Swift 文件 diff SHA-256 0a70bd7cde1b5fff883f54cdd25871f49b1949650e3c8cb2e7a5bc8db02e0256。审查质疑用户编辑事件去重可能影响原生 TextField 的过滤回写；临时最小 SwiftUI 窗口对照证实原版和去重 setter 都有既有草稿边界，同值发布也不能修复，故不将该假设记为本次引入或已修复缺陷；恢复 editPairing 原实现，删去只能证明通知、不能证明原生修复的临时测试，收敛为后台轮询去重。第1轮对最终指纹 19e5d6ac3ab5163a6f21d292a1a86e1418aae3923f412ea83ef79a3f5ece3cd4 收尾复核通过，无剩余可操作发现；最终 Swift 9/9 通过。原生 probe 已退出，不入库。

正式第2轮 cr_round2 已顺序完成并通过，独立审查最终同一指纹，无可操作发现，覆盖跨队列 / 生命周期、快照比较与真实状态、配对计时 / 迟到任务、测试与性能证据。原位置三个连续30s CPU均值 0.266% / 0.166% / 0.832%，总90.11s累计CPU0.38s、平均约0.422%；并非用户最初异常进程的数小时长期复验。

最终收敛版本 Swift9/9 与 app 重建通过。最终完整 verify 曾遇既定 UDP 发现端口 AddrInUse；即时检查无仍占用，端口释放后单次完整重跑通过 Python47 / Rust100 passed、1 ignored、rustfmt / 仓库检查，保留首次失败。最终 app 构建 SHA-256 为 7ed31b267c60e1c5afc14368fb1acede9a3f1969d121f811aef765683387cdf4；在原启动位置更新可执行文件并保留既有资源，重新签名后本机 SHA-256 为 bbe1bd3030c85ee9826ba34cb9ad05a0c9d85522d44fb08207c606ac99c92682，签名严格校验通过并已启动。

详细采样与两轮 CR 见[性能报告](../evidence/2026-10-30-idle-cpu.md)。

最终签名后的 app 需要 macOS 钥匙串重新允许读取已有身份：原生窗口仍 starting，sample 证实 Rust keyring get_secret 等待系统 SecurityAgent；启动等待时测到的0.1%不算完整运行的性能证据。自动化工具禁止操作 SecurityAgent，已请求用户手动处理，未读取密码或绕过保护。

实现提交 9da3eda6a264886e3c12e7affb8ae412b13d79ca 已推送 main，harness check --require-pushed 通过。推送前远端同步并行图标提交 a438c0b，原推送因远端前进被拒；仅重放本地未推送提交并合并 STATUS 两行，保留双方记录，无强推。重放后受审代码 diff SHA-256 仍完全一致，原位置 app 的图标资源与最新 main 一致。代码与两轮 CR / 完整门禁已完成；任务只因最终本机系统授权等待而保留 blocked，不提前归档。

## Next

解除条件：用户在 macOS 系统弹窗允许 YM_Link 读取已有钥匙串身份。随后读取主窗口确认完成启动，再测完整运行的 CPU；补充结果、将任务 done 归档并更新链接。若用户暂不处理，保留当前系统提示与任务，不把等待期低CPU计入验收。
