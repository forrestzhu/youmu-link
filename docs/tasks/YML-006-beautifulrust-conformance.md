# YML-006：Rust 后端按 BeautifulRust 规范清理(业务逻辑不变)

Status: verifying
Workflow: maintenance
Requires-Device: no
Tests: cargo test(与干净 HEAD 基线对比,无新增失败;清理不改行为,不新增测试面)

## Scope

以 [BeautifulRust](https://github.com/morganlinton/BeautifulRust)(本地 clone 于 `~/my_workspace/personal/BeautifulRust`)的 rust-best-practices 规则与 lints 配置为基准,清理 `ymlink-core` / `ymlink-desktop` 非测试 Rust 代码的机械违规:

- 非测试代码 `unwrap()` → `expect("原因")`(锁中毒 / join_next / 队列保留槽 / 平台句柄 / FFI 兜底),涉及 runtime.rs、capture.rs、lib.rs、headset_stub.rs。
- core(库 crate)公共 `Result` 函数补 `# Errors`(beacon/discovery/files/identity/session/storage 共 22 处),FFI unsafe 函数补 `# Safety`(5 处),可 panic 的 `Capture::start` 补 `# Panics`。
- `Arc::clone(&x)` 显式风格、冗余闭包、`Vec` 上多余 `.to_vec()`、`format!` 收集、`map_or(true,..)`→`is_none_or`、let-else、分号一致性、`clone_from` 等惯用法修正;部分由 `cargo clippy --fix --lib` 机械应用。

非目标(记录不改):测试代码内的 unwrap(BeautifulRust 明确豁免);平台互操作的有界 `as` 强转(CoreMedia/SCK/win32,已由长度或上限守卫);公共 API 形状调整(needless_pass_by_value、多余 Result 包装、unused async 等,会破坏调用方);Windows 专属 `capture/windows.rs`(本机无法编译核验);函数拆分 / 参数分组(too_many_lines / too_many_arguments);既有 `unwrap_or_else(PoisonError::into_inner)` 的锁中毒恢复点为刻意设计,仅改为函数指针形式;`screencapturekit::prelude::*` 框架 prelude 豁免。业务逻辑、协议字节、错误语义全部不变。

## Spec

maintenance,无行为规格;约束为 diff 仅含 panic 消息、属性、文档注释与等价惯用法重写,测试相对干净 HEAD 无新增失败。

## Acceptance

- [x] 非测试代码无裸 `unwrap()`(BeautifulRust unwrap_used 规则)。
- [x] core 公共 `Result` 函数与 FFI unsafe 函数文档齐全(# Errors / # Safety / # Panics)。
- [x] cargo 测试相对干净 HEAD 无新增失败;rustfmt 干净。

## Verification

- 非测试 unwrap 残留 → clippy(BeautifulRust lint 集)按 cfg(test) 行界核对 runtime.rs / capture.rs / lib.rs / headset_stub.rs 非测试段 → 0 残留。
- 行为不变 → `cargo test --workspace --no-fail-fast` → 除 2 个既有环境失败外全绿(见 Evidence)。
- 格式 → `cargo fmt --check` → 干净。
- 稳定后完整检查 → `python3 scripts/harness.py verify` → 见 Evidence。
- 真实设备层: not-applicable (纯清理,无协议字节改动;protocol_vectors 测试锁定报文未动)。

## Evidence

- BeautifulRust lint 基线:`cargo clippy --all-targets -W clippy::pedantic -W clippy::unwrap_used …`(完整命令见任务提交),首跑非测试代码 570+ unwrap/34 项其他违规;终跑非测试段仅剩记录在非目标里的类别(强转 / 函数长度 / API 形状)。
- 行为基线:`git stash` 后在干净 HEAD(83dd684)复跑 `cargo test -p ymlink-core --lib discovery::tests`,`query_targets_report_failure_when_no_probe_succeeds` 与 `query_targets_survive_unreachable_broadcast_and_probe_every_port` 同样失败——本机网络允许 UDP 广播 send 成功,测试假设失败,属既有环境依赖失败,与本次改动无关。
- `cargo test --workspace --no-fail-fast`:除上述 2 项外全部通过(core 集成 / protocol_vectors / desktop runtime_tests / native_backend)。
- `python3 scripts/harness.py verify`:FAIL 于 `cargo test --workspace --all-targets`,失败项即上述 2 个既有环境失败;harness 回归与仓库检查、rustfmt 通过。限制:在修复该环境依赖测试(建议单独立任务)前,verify 在本机无法全绿。

## Next

- 建议新任务处理 discovery 两个环境依赖测试(send 对广播地址可成功的假设),解除本机 verify 阻塞;之后本任务可直接归档。
- 可选后续(用户决策):将 BeautifulRust `lints/lints.toml` 以 `[workspace.lints]` 落进 Cargo.toml 并在 CI 启用 `-D warnings`(需先处理测试内 unwrap 的豁免策略)。
