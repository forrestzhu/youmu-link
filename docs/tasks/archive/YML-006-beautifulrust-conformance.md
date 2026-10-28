# YML-006：Rust 后端按 BeautifulRust 规范清理（业务逻辑不变）

Status: done
Workflow: maintenance
Requires-Device: no
Tests: python3 scripts/harness.py verify; cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used -D clippy::missing_safety_doc -D clippy::missing_errors_doc -D clippy::missing_panics_doc; cargo doc --workspace --no-deps

## Scope

按 BeautifulRust 规则清理共享 / macOS 非测试 Rust：panic 原因、公共 API 错误 / FFI 安全契约、Arc::clone / let-else / is_none_or 等等价惯用法。原始提交为 `d517be04b94c897a61b2c64d2c691c6e915018cc`，基线为 `83dd684375758d3c95ab8bdcb115b5a94fedf143`。

按用户追加要求完成至少两轮独立 CR 并修复属实问题；同时修复阻塞本机门禁的 discovery 测试 fixture。生产行为、错误映射、公共 API 与协议字节不变。

非目标：Windows 专属 capture 实现、公共 API 形状调整、函数拆分、平台有界强转、既有锁中毒恢复策略、全量 pedantic lint 清零。测试 unwrap 豁免；未引入依赖或全局 lint 配置。

## Spec

maintenance：运行时改动为等价惯用法 / panic 说明；文档准确描述现有行为。测试 fixture 绑定 loopback，保留广播发送失败与三个回退端口收包断言，不放宽断言、不跳过测试。

## Acceptance

- [x] 本机编译的共享 / macOS 生产库和 headset_stub 无裸 unwrap，以编译语义 Clippy 检查覆盖，不按 cfg(test) 行界截断。
- [x] 公共 Result / FFI 文档按实际返回路径、指针来源和生命周期修正。
- [x] 至少两轮独立 CR；首轮 2 P2 / 1 P3、第二轮 1 P3，均修复并复核关闭。
- [x] harness verify 通过；业务行为与协议不变。

## Verification

- `python3 scripts/harness.py verify`：PASS，Python 37/37，Rust 74 passed / 1 ignored，仓库检查与 rustfmt 通过。
- Tests 字段所列定向 Clippy：PASS；原有全量 lint 告警仍保留，见报告限制。
- `cargo doc --workspace --no-deps`：PASS，第二轮文档修正后再次验证通过。
- 真实 Quest / 双机 LAN / 录屏音频 not-run，本轮不作设备验收结论。

## Evidence

- [两轮审查、修复与验证报告](../../evidence/2026-10-28-beautifulrust-two-round-cr.md)记录受审提交、修复代码快照、每轮发现与关闭核验。
- 初次清理按 cfg(test) 行界截断，误将生产 `update_shares` 中 6 处 unwrap 排除，原“零残留”结论已纠正；定向 Clippy 修前报 6 处错误、修后通过。
- 两项 discovery 失败在独立临时源码快照重新编译的父提交上复现，确认为既有测试环境假设；仅修改测试 socket 绑定，解除门禁阻塞。
- `cargo clippy --workspace --all-targets --all-features -- -D warnings` 仍因既有 Box<Arc<Output>> / 参数数量 / 测试锁分析诊断失败；不声称全量 lint 清零。

## Next

本次审查与清理已完成。完整 lint 配置、API 形状及 Windows 专属风格清理若另行立项，再单独验收；真实设备工作继续按 YML-002 / YML-003 推进。
