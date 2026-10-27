# YM_Link 测试策略

状态：v0.1（2026-10-27）。分层真机 / 真机命令随里程碑补充；本文定义分层与门禁，不预填未实测指标。

## 分层

| 层 | 范围 | 工具 / 命令 | 门禁 |
| --- | --- | --- | --- |
| L1 单元 | beacon 编解码、过滤规则、tag / nonce、候选表、配对状态机（纯逻辑） | `cargo test --workspace`（CI 矩阵 ubuntu / windows / macos） | 每次改动相关回归；`harness verify` 全量 |
| L2 集成（单机 loopback） | 双 socket 发现交互：Query 单播回复、冷却、tag 隔离、自回环抑制、端口回退 | `cargo test` 内 `#[tokio::test]`（`allow_loopback` 显式测试配置） | 同上；不依赖真实网卡 |
| L3 双机 LAN | 真实两台设备（macOS ↔ Windows / Linux 测试桩）发现、配对、重连、防火墙 | 手动清单 + `docs/evidence/` 报告（Commit / Host-OS / Result 绑定） | M2 起里程碑必须；单机通过不得标记互联通过 |
| L4 互操作 | ymlink-core ↔ ymlink-kotlin（游目端）协议对拍：golden beacon、握手、lane 消息 | 双仓 CI 各自 golden 夹具 + 联调任务真机 | 协议版本变更必须 |
| L5 体验基准 | 镜像延迟 / 帧率 / 音画同步、传输吞吐、弱网降级 | M4 / M3 定基准后固化脚本与样本 | 里程碑验收 |

## 现有覆盖（M0）

- L1：`core/src/beacon.rs` 编解码 / 512B 上限 / 定长校验 / 未知字段拒绝 / tag_hash。
- L2：`core/tests/discovery.rs` loopback 双实例：Query→Beacon 单播回复、tag 隔离、冷却去重、自回环忽略、端口回退绑定。
- demo 原型：浏览器人工核对，不是自动化合同；将来桌面 UI 若从原型还原，按当期任务决定是否建立 visual contract。

## 门禁命令

```bash
python3 scripts/harness.py verify          # harness 回归 + 仓库检查 + rustfmt + cargo test
python3 scripts/harness.py check --require-pushed   # 交付：HEAD == 远端 main
```

## 证据纪律

- 真机 / 双机结果只记 `docs/evidence/` 报告并从任务 Evidence 链接；报告绑定提交哈希与两侧系统版本。
- 未运行的层写 `not-run`；环境限制（无第二设备、CI 缺平台）如实记录并转 blocked 条件，不以单机结果代签。
