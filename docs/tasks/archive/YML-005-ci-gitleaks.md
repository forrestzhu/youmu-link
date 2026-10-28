# YML-005：添加 gitleaks CI 防泄漏检查

Status: done
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (仅新增 GitHub Actions 工作流，不涉及可自动验证的 Rust 行为；等效检查为本地 gitleaks 全量扫描 + Actions 实跑)

## Scope

新增 `.github/workflows/gitleaks.yml`：push 到 main 和所有 pull_request 时用 gitleaks-action@v2 对全量历史（fetch-depth: 0）做密钥/隐私泄漏扫描，失败即阻断。非目标：不做自定义规则、不扫描组织内其他仓库。

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
- 锁定版 41b3e9b 的 Actions run 40620888348 success (11s)，全量历史扫描通过，验收全部满足。

## Next

无（done 归档）。
