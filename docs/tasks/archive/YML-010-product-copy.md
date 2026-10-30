# YML-010：产品界面文案克制与可理解性约束

Status: done
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (仅补充产品界面文案约束与任务记录，不修改应用、协议或可执行行为)

## Scope

将用户要求的克制文案原则写入 AGENTS，约束 YM_Link 面向最终用户的产品界面：内容应帮助当前操作，图标与直觉交互优先，必要限制保持准确且易懂。仅修改开发约定和本任务 / STATUS 记录；不修改现有界面、网站、README、安全政策或发布状态。

## Acceptance

- [x] AGENTS 以三条简短约束明确文案用途、图标 / 按需帮助、必要限制与适用边界。
- [x] 约束保留无障碍名称、歧义操作标签及真实安全 / 隐私 / 费用 / 兼容限制，不将工程交付解释放入产品界面。
- [x] 文档检查与 diff 自查通过，任务完成归档并在 STATUS 简述。

## Verification

逐条对照用户要求并自查 `git diff`，确认没有扩大到工程文档或删减真实限制；运行 `python3 scripts/harness.py check`，预期任务字段、归档与文档链接检查通过。Rust / Swift / Windows 测试、构建与设备验收：not-applicable（没有代码、界面或协议行为改动）。

## Evidence

开工 main 干净；`git fetch origin main` 与 `git merge --ff-only origin/main` 均退出 0，已与远端同步。`harness tasks` 无适合的活动文档任务，使用 `harness new` 创建本维护任务。

`python3 scripts/harness.py check` 在 verifying 阶段及 done 归档后均退出 0：73 个文本文件的仓库记录、任务合同与文档链接检查通过；`git diff --check` 退出 0。逐条自查 diff，确认仅增加产品界面约束及本任务 / STATUS 记录，三个边界均保留：无障碍与歧义标签、真实限制、工程文档与用户明确文字；未修改界面或现有工程事实。

## Next

已完成并归档；后续 YM_Link 界面改动按 AGENTS 执行，具体界面修改与体验验收另随对应任务记录。
