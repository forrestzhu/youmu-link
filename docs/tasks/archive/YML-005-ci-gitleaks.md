# YML-005：添加 gitleaks CI 防泄漏检查

Status: done
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (仅新增 GitHub Actions 工作流，不涉及可自动验证的 Rust 行为；等效检查为本地 gitleaks 全量扫描 + Actions 实跑)

## Scope

新增 `.github/workflows/gitleaks.yml`：push 到 main、所有 pull_request 和手动触发（workflow_dispatch）时用 gitleaks-action（CR 后升级到 v3.0.0）做密钥/隐私泄漏扫描，失败即阻断。fetch-depth: 0 拉全量历史，但 action 按事件加 `--log-opts`：push/PR 为增量扫描新提交，仅 workflow_dispatch 全量历史（本地 `gitleaks git` 已做一次全量基线，dispatch 可随时全量兜底）。非目标：不做自定义规则、不扫描组织内其他仓库、tag push 不单独触发（与既有 repository-checks 基线一致，tag 打在已扫过的 main commit 上）。

## Spec

maintenance 任务，无行为规格。

## Acceptance

- [x] 仓库存在 gitleaks CI 工作流，覆盖 push(main) 与 pull_request。
- [x] gitleaks 对当前仓库全量历史扫描无泄漏（本地等效验证，CI 首跑预期绿）。

## Verification

- 工作流存在且触发条件正确 → 读文件确认 → push/PR 均触发。
- 全量历史无泄漏 → 本地 `gitleaks git --redact .`（与 action 同引擎）→ exit 0, no leaks found。
- `harness verify` → not-applicable (无 Rust/文档行为改动，跑 `harness check` 仓库基线)。

## Evidence

- `brew install gitleaks` 安装 gitleaks 8.x（2026-10-28）。
- `gitleaks git --no-banner --redact -v .` → exit 0；26 commits scanned, ~1.06 MB, no leaks found。限制：本地引擎版本与 action 内置版本可能略有差异，以 CI 首跑为准。
- 首版 gitleaks.yml（浮动 tag / ubuntu-latest）随 276d27b 推送，Actions run 40620620580 success (12s)。
- 复查发现与既有 repository-checks.yml 风格不一致，改为 SHA 锁定 action（checkout v7.0.1 同哈希、gitleaks-action v2.3.9=ff98106）、persist-credentials: false、fetch-depth: 0、timeout 10m、concurrency 复用既有模式。
- 锁定版 41b3e9b 的 Actions run 40620888348 success (11s)。第二轮回查：push 事件实际带 `--log-opts=-1` 增量扫描，该 run 只扫了 41b3e9b 单个 commit，原"全量历史扫描通过"表述失实，已修正；全量基线以本地 `gitleaks git`（26 commits，exit 0）为准，另补 workflow_dispatch 全量 run 作 CI 侧全量证据。

### CR 记录（两轮独立 subagent）

- 第一轮（受审 5677cdc..8b0e85d）：4 项发现。#1 Important——gitleaks-action@v2 上游宣告 EOL（2026-09-16 起 Node 20 从 runner 移除），经 `git ls-remote` 核实 v3.0.0=e0c47f4f、上游 README 确认日期与一行迁移，**属实已修**：pin 升 v3.0.0。#2 Minor——`GITLEAKS_ENABLE_COMMENTS` 默认 true，只读 token 下泄漏时评论 API 调用会失败，经上游 README 核实默认值，**属实已修**：显式设 false 保持最小权限。#3 Minor——tag push 不触发，与基线一致且 tag 打在已扫 commit 上，**接受不修**。#4 Minor——Scope 漏 workflow_dispatch 一词，**已补**。
- 第二轮（受审 5677cdc..aeb53c7）：复核第一轮修复全部通过（v3.0.0 SHA、env 传法、个人仓库免 license、`GITLEAKS_ENABLE_COMMENTS: false` 字符串匹配语义均核实源码/日志）。新发现 3 项。#1 Important——文档声称"全量历史扫描"与实际不符：action 对 push/PR 事件加 `--log-opts` 增量扫（经 run 40621106829 / 40622898952 日志核实均为 `--log-opts=-1`），仅 dispatch 全量，**属实已修**：Scope/Evidence 改为增量口径并补 dispatch 全量 run。#2 Minor——STATUS.md 仍写 v2.3.9 未随 aeb53c7 更新，**属实已修**。#3 Minor——cancel-in-progress:true 会取消增量扫描造成该批 commit 漏扫，**属实已修**：本工作流改为 false（排队不取消），偏离基线一行并有注释说明理由。
- 两轮 CR 修复随 612c768 推送；workflow_dispatch run 40624080000 success，日志确认无 `--log-opts`、31 commits scanned（全量历史），CI 侧全量证据补齐。限制：未核 GitHub 分支保护是否将本 check 设为 required（未设时"阻断"仅体现为 run 变红）。

## Next

无（done 归档）。
