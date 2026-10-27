# YML-001：YM_Link 框架启动（仓库 / harness / 设计 / demo / 发现骨架）

Status: doing
Workflow: feature
Requires-Device: no
Tests: core::beacon (单元：编解码 / 512B / 定长 / 未知字段 / tag_hash)、core::tests::discovery (loopback 集成：Query 单播回复 / tag 隔离 / 冷却 / 自回环 / 端口回退)

## Scope

用户 2026-10-27 授权启动 YM_Link 产品线（游目侧任务 [YM-276](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-276-ym-link-bootstrap.md)）：建立私有 GitHub 仓库、完整 harness 框架、产品 / 架构 / 协议文档、可点击 demo 原型、`ymlink-core` 局域网发现 beacon 骨架与测试。非目标：配对加密实现、文件 / 镜像 lane、Tauri 壳、游目端集成、双机 LAN 真实验收（M2+）。

## Spec

规范来源：[产品定义](../product/YM-LINK.md)、[协议 v0](../engineering/PROTOCOL.md)、[demo 设计](../plans/2026-10-27-demo-design.md)。本轮可观察行为合同：给定同网段两实例（测试允许 loopback），任一方显式 Query，则收到对端单播 Beacon（≤512B CBOR、零身份字段）并进入候选表，1s 内重复 Query 不重复回复；给定不同 tag 或自身回环报文，则静默忽略；给定端口占用，则按 47610→47612 回退绑定。错误恢复：候选 3× 心跳无更新过期；解码失败报文丢弃不崩溃。

## Acceptance

- [x] 私有仓库 `forrestzhu/youmu-link` 建立，框架与文档成文（产品 / 架构 / 协议 / 测试 / 流程），README 声明暂闭源与 GitLab 分发意向。
- [x] harness 可用：YML 编号合同、Tests 字段、evidence 报告绑定、`check --require-pushed` 与 `verify`；`test_harness.py` 回归通过。
- [x] 可点击 demo 原型覆盖桌面与头显两视角（发现 / 配对码 / 屏幕镜像 / 文件浏览与传输），浏览器直接打开可用。
- [x] `ymlink-core` 发现 beacon 骨架：CBOR 编解码（≤512B / deny-unknown / 定长校验）、UDP 广播与监听、同网段过滤、tag 过滤、自回环抑制、Query 单播回复 + 冷却、候选表事件；单元 + loopback 集成测试通过。
- [x] CI workflow 就位：ubuntu / windows 矩阵（macOS 由本地 `harness verify` 覆盖）；2026-10-27 首跑因 GitHub 账户付款 / 额度未启动，已如实记录为 not-run，恢复后重跑补证。
- [ ] M1 评审：用户审阅 demo 与文档后给出结论，修订后本任务归档。

## Verification

1. 仓库 → `gh repo view forrestzhu/youmu-link --json visibility` → PRIVATE。
2. 框架 → `python3 scripts/test_harness.py`、`python3 scripts/harness.py check --require-pushed` → 0 退出。
3. 核心 → `cargo test --workspace`、`cargo fmt --all --check` → 通过；双机 LAN not-run（M2 起，见 TESTING L3）。
4. demo → 浏览器人工核对两视角状态流；不构成视觉合同。
5. CI → push 后 workflow 正常触发（run 40431997238），但三任务均因「账户付款失败 / spending limit」未启动（not-run）；解除后重跑，预期 harness + rustfmt + cargo test 全绿。

## Evidence

- 2026-10-27：`gh repo view forrestzhu/youmu-link --json visibility` 回读 PRIVATE；推送后 `harness check --require-pushed` PASS。本地门禁实测：`cargo test --workspace --all-targets` 15 通过（beacon 单元 8 + loopback 集成 7）、`cargo fmt --all --check` 干净、`python3 scripts/test_harness.py` 16 通过、`python3 scripts/harness.py check` PASS（23 个文本文件）。demo 原型 `node --check` 语法通过，浏览器人工核对两视角状态流。首次修复过程：ciborium 0.2 无 `into_vec`（改 `into_writer` 辅助）、if-addrs `Ifv4Addr` 含 `prefixlen` 且 `broadcast` 已是 `Option<Ipv4Addr>`、临时目录符号链接导致未 resolve 的 ROOT 比较失败——均已修复并保留在对应实现 / 测试中。
- 双机 LAN 与互操作（L3 / L4）not-run：需第二真实设备与游目端实现，M2+ 任务承接。

## Next

推送后跑 `harness check --require-pushed` 与 CI；等待用户 M1 demo 评审结论；M2（Noise 会话 + SPAKE2 配对 + Tauri 壳 + 双机真实验收）就绪后开工。
