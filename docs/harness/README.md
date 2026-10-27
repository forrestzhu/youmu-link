# HARNESS：YM_Link 开发流程

## 适用范围

本流程约束 YM_Link 仓库内的产品 / 工程 / 测试 / 分发工作。无关日常协助只聊天反馈，不建任务、不改 STATUS。与游目联动的头显侧工作走游目仓库 YM 任务，本仓只记录桌面 / 协议侧并互链。

## 工作闭环

1. `git status --short --branch` → fetch + ff-only 同步远端 main；保留用户未提交修改。
2. `python3 scripts/harness.py tasks` 选任务；`python3 scripts/harness.py new <topic> --title "标题" --workflow maintenance|behavior|feature [--requires-device]` 新建。编号含 archive 全局唯一，YML-NNN-topic.md。
3. 状态机 `ready → doing → verifying → done`；外部受限记 `blocked` + 解除条件；done 归 `docs/tasks/archive/`；被取代用 superseded + Superseded-By（不得成环）。
4. 小步提交；行为改动先写测试（red → green）；协议改动先改 [PROTOCOL](../engineering/PROTOCOL.md)。
5. 稳定后 `python3 scripts/harness.py verify`；纯文档改动只跑 `check`；结果与限制记入任务 Evidence。
6. 交付：commit / push main 后 `python3 scripts/harness.py check --require-pushed`，HEAD 必须等于远端 main。不强推。

## 任务字段合同（`check` 强制）

- 头部：`Status` ∈ ready/doing/verifying/blocked/done/superseded；`Workflow` ∈ maintenance/behavior/feature；`Requires-Device` yes/no；`Tests` 必填——cargo 测试目标或 `not-applicable (理由)`。
- 章节：Scope / Acceptance（勾选清单）/ Verification / Evidence / Next 必须；behavior / feature 另需 Spec。
- done：无未勾选项 + Evidence 非空；Requires-Device 的 done 任务必须链接 `docs/evidence/` 报告（字段 `Commit`（40 位提交）/ `Host-OS` / `Result: pass`）。
- 终态任务必须已归档；活动任务不得是终态。

## 按任务大小选择流程

- **maintenance**（文档、依赖、工具、小修）：任务记录 + 相关回归即可。
- **behavior**（行为修复 / 改动）：Spec（预期 / 复现 / 边界）+ 相关回归 + 稳定后 `verify`。
- **feature**（新功能 / 新协议字段）：Spec（用户入口 / 状态 / 错误恢复 / 非目标）+ 计划（复杂时 docs/plans/）+ 测试 + `verify`；涉及双机 / 头显加 `--requires-device` 并按 TESTING L3 出 evidence。

## 命令

```bash
python3 scripts/harness.py check [--require-pushed]   # 仓库 / 任务 / 文档合同
python3 scripts/harness.py verify                     # harness 回归 + check + rustfmt + cargo test
python3 scripts/harness.py tasks [--all]              # 任务清单
python3 scripts/harness.py new <topic> --title "标题" --workflow feature [--requires-device]
python3 scripts/harness.py doctor                     # git / gh / cargo / rustc 就绪检查
```

## 门禁与诚实性

- `check` 只证明结构 / 链接 / 字段合同，不证明命令已执行；实际执行结果只认任务 Evidence。
- 未运行写 `not-run`；单机 loopback 不代签双机 LAN；demo 原型不代签桌面实现。
- 暂闭源约束：不公开仓库、不加 LICENSE、不对外分发；变更需用户明示。
