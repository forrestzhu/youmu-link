# YML-NNN：任务名称

Status: ready
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (填写不涉及可自动验证行为的具体理由；涉及时改为 cargo 测试目标，如 core::tests::discovery)

## Scope

一个具体结果、影响范围、非目标、依赖及必要授权。优先续写原任务；新任务用 `harness new` 分配包含 archive 的唯一编号。

## Spec

maintenance 可删除本节；behavior 写预期 / 复现与边界，feature 写用户入口、状态变化、错误恢复和非目标。采用“给定…当…则…”或同等可观察描述；复杂功能才链接独立计划 / 设计，不重复现行规格。协议行为以 [PROTOCOL](../../engineering/PROTOCOL.md) 为准并写明版本。

## Acceptance

- [ ] 一项可观察结果；每项对应下面的检查或人工观察。

## Verification

实现前列出验收项 → 测试 / 命令 → 预期结果；区分相关回归、稳定后的适用完整检查（`harness verify`）和真实第二设备 / 头显层。无须运行的层写 `not-applicable (原因)`；应跑而未跑写 `not-run`。结构检查不证明这些命令已经执行。

## Evidence

实际命令、退出状态、结论和限制；行为改动记录 red / green。需要第二设备 / 头显的任务链接绑定当前提交 / 系统版本的 docs/evidence 报告。续写时保留第一次失败及其原因，不用复跑成功抹去不稳定性。

## Next

下一可执行动作；用户要求 CR 时在 Evidence 记录各轮范围 / 发现 / 核验 / 复审；blocked 写解除条件，verifying 写待验项与准备入口。done 仅在所有验收有证据后归档；被新设计取代可用 superseded 并声明 Superseded-By，Evidence 说明替代原因、替代链不得成环；保留未验收项，不当作通过。
