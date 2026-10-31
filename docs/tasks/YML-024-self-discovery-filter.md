# YML-024：过滤本机多网卡产生的虚假设备

Status: verifying
Workflow: behavior
Requires-Device: no
Tests: ymlink_core::discovery::tests（本机多网卡 /重叠子网过滤）；core/tests/discovery.rs（loopback /nonce /发现过期）

## Scope

用户只有一台VR，但Windows侧栏出现5个匿名候选。截图中4个地址经本机接口查询确认均属于电脑自身，包括Hyper-V /WSL、以太网与WLAN。修复发现来源过滤，防止本机其他实例 /测试广播成为虚假设备。保留匿名发现、加密身份与既有配对；不修改Quest端或恢复YML-017性能排查。

## Spec

- 默认生产来源过滤拒绝与任一本机接口IPv4相等的地址，即使另一接口处于重叠子网；不同进程nonce不能绕过本机地址排除。
- 有效报文提交前复核当前接口，覆盖心跳间隔内新增本机地址；无效报文与冷却Query不触发枚举，枚举失败拒绝。
- 真正远端的同子网单播来源继续允许；显式测试配置保留IPv4 loopback，生产仍拒绝loopback。原nonce过滤保留。
- 按[PROTOCOL](../engineering/PROTOCOL.md)发现v1收紧本机接收策略，不新增或修改线字段，不升级发现 /应用版本。共用来源检查也覆盖TCP入站与手动直连；显式loopback测试仍允许。

## Acceptance

- [x] 本机多网卡 /重叠子网回归先red后green，远端与显式loopback回归通过。
- [x] Windows Release构建 /真实ABI与独立审查通过。
- [x] 用户追加CR的动态接口P2修复、失败回归与独立复审关闭。
- [ ] 完整无过滤harness verify与Windows启动smoke通过。
- [ ] 更新本机安装版后实际窗口无本机虚假候选。

## Verification

先运行新增cargo单元回归观察本机来源误放行，再最小修复来源校验；完整verify串行运行，避免占用发现端口。随后Windows Release /真实ABI /smoke，实际窗口回读本机候选。该任务验证本机误发现排除，无须第二设备；Quest真实连接、录屏 /音频不在本任务中代签。独立只读审查核对所有调用者与跨平台边界。

## Evidence

2026-10-31 开工main干净，fetch /ff-only同步97dacbd。本机接口确认截图中的172.22.64.1、172.22.192.1、192.168.1.3和192.168.31.67均为本机；四行同一50778端口，该监听已退出。现存安装版TCP59205 /UDP47610。代码排除同一nonce，但不同进程nonce仍按同子网接收；测试后端也会向LAN广播，构成可复现的来源。未声称确定历史50778的进程身份。旧匿名候选按现有180秒过期。

新增两个来源回归均实际red：本机172.22.64.1被放行，重叠子网的本机10.1.1.2被放行。加入全接口本机地址排除后green。独立只读审查发现1项P2：接口枚举预先丢弃/32等无有效广播子网的接口，可能经另一宽子网重新放行。修复为保留全部IPv4用于自身排除，仅生成定向广播时校验有效子网；扩充/32重叠用例，新增无效广播目标回归先red后green。复审关闭P2，无新增发现；现有nonce、loopback和远端同子网语义保留。定向discovery lib全9项通过。

首次发现集成8/9通过，beacon_ports_fall_back_in_order因安装版占用47610报OS10048。首次完整verify也命中同一占端口，另暴露本任务PROTOCOL相对链接错误（已修复），Python门禁因此失败；未改动或跳过原端口回退断言来伪造完整通过。用户Esc停止窗口自动化，之后未继续操作应用或关闭安装版；最终安装版替换 /窗口回读not-run，保留该限制。

修正链接后Python47项（3 skipped）、harness check、rustfmt /diff检查通过。`RUST_TEST_THREADS=1 cargo test --workspace --all-targets -- --skip beacon_ports_fall_back_in_order` 退出0：111 passed /2 ignored /1 filtered；明确仅排除被安装版占端口阻断的既有回退用例，此结果不等同完整verify。

`python3 scripts/build_windows.py --smoke-test` 完成Release Rust /WPF自包含目录构建、生产BAML /双语布局 /真实DLL生命周期ABI验证。最终启动smoke退出4，经App.xaml.cs核对为当前安装版持有用户级单实例mutex，未进入后端启动，不是本次发现修复异常；不关闭原应用、不绕过单实例。构建产物在忽略入库的dist/YM_Link，未覆盖安装目录。macOS本机构建 /真实Quest LAN not-run，线格式 /信任格式不变。

### 用户追加独立CR（2026-10-31）

用户要求独立subagent审查已推送的8fdf71489b2e00d8ca8df9e97933947240990a1b；开工fetch /ff-only同步至171741340095e024fb01c7f935a01b0863ccd185，保留其他任务的重连修复。新代理self_discovery_cr只读检查该提交与最新main交互，发现1项P2：接收仍使用最长60秒的接口缓存，在已有/16子网内动态新增本机/32时可发布虚假候选，并因旧180秒过期保留。根代理核实属实。

为无真实LAN依赖地验证边界，将原报文准入校验机械提取为accepted_packet，生产继续使用原接口函数，测试仅替换该私有参数。新增实际报文回归在旧校验下red：合成来源10.1.1.4被放行（true而期望false）。随后在CBOR /nonce /tag /discoverable及Query冷却通过后复核当前接口，修后Query和Beacon均拒绝新增/32，同时远端仍通过、空接口失败关闭。非法报文、同nonce /错tag、隐藏公告、来源拒绝与冷却Query的惰性枚举回归通过；定向discovery lib共11项通过。

复审范围为1717413上的当前修复diff；原代理确认P2关闭、无新增发现，接口锁不跨枚举，不改事件 /广播 /过期生命周期。有效同tag Beacon仍逐包枚举，不将早期拒绝误称全局UDP限速。没有修改Quest线协议或操作安装版应用。

本轮修复后完整verify再次执行：Python47（3 skipped）、仓库检查与rustfmt通过；Rust唯一运行失败仍为既有beacon_ports_fall_back_in_order，安装版PID38052占用47610，OS10048。为继续验证其余目标，单独运行明确过滤该用例的workspace /all-targets：116 passed /2 ignored /1 filtered、退出0；未将该过滤结果当作完整verify通过。保留用户之前Esc停止窗口操作的约束，未关闭安装版或规避单实例。

最终`python3 scripts/build_windows.py`退出0：Release Rust /WPF自包含构建、真实DLL生命周期 /并发快照ABI、生产BAML /双语布局通过。此次未重复启动已知受单实例阻断的smoke，未覆盖安装目录。修复仅改Rust发现准入与工程规范，Windows /macOS原生UI不变；最终原生窗口实显及Quest真实LAN保持not-run。

## Next

代码及现有证据commit /push main后执行strict。待用户允许再次操作窗口时，关闭占端口的安装版补完整无过滤verify /启动smoke，并备份更新后端与实际窗口核验；不从单元测试推断Quest连接通过。YML-024保持verifying，不将未替换的安装版称为已修复。
