# YML-024：过滤本机多网卡产生的虚假设备

Status: verifying
Workflow: behavior
Requires-Device: no
Tests: ymlink_core::discovery::tests（本机多网卡 /重叠子网过滤）；core/tests/discovery.rs（loopback /nonce /发现过期）

## Scope

用户只有一台VR，但Windows侧栏出现5个匿名候选。截图中4个地址经本机接口查询确认均属于电脑自身，包括Hyper-V /WSL、以太网与WLAN。修复发现来源过滤，防止本机其他实例 /测试广播成为虚假设备。保留匿名发现、加密身份与既有配对；不修改Quest端或恢复YML-017性能排查。

## Spec

- 默认生产来源过滤拒绝与任一本机接口IPv4相等的地址，即使另一接口处于重叠子网；不同进程nonce不能绕过本机地址排除。
- 真正远端的同子网单播来源继续允许；显式测试配置保留IPv4 loopback，生产仍拒绝loopback。原nonce过滤保留。
- 按[PROTOCOL](../engineering/PROTOCOL.md)发现v1收紧本机接收策略，不新增或修改线字段，不升级发现 /应用版本。共用来源检查也覆盖TCP入站与手动直连；显式loopback测试仍允许。

## Acceptance

- [x] 本机多网卡 /重叠子网回归先red后green，远端与显式loopback回归通过。
- [x] Windows Release构建 /真实ABI与独立审查通过。
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

## Next

代码及现有证据commit /push main后执行strict。待用户允许再次操作窗口时，关闭占端口的安装版补完整无过滤verify /启动smoke，并备份更新后端与实际窗口核验；不从单元测试推断Quest连接通过。YML-024保持verifying，不将未替换的安装版称为已修复。
