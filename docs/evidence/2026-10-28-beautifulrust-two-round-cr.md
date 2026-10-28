# YML-006：BeautifulRust 重构两轮 CR

日期：2026-10-28。范围为 `ymlink-core` 库 API 与 `ymlink-desktop` 应用 / C ABI；Rust edition 2021，未声明独立 MSRV，本轮使用仓库 CI 同版 rustc 1.85.0。主机 macOS 27.0.1 arm64。

## 审查范围

- 基线：`83dd684375758d3c95ab8bdcb115b5a94fedf143`。
- 原始提交：`d517be04b94c897a61b2c64d2c691c6e915018cc`（`refactor: align non-test Rust code with BeautifulRust rules (no behavior change)`）。
- 规则：BeautifulRust `rust-best-practices` §5（panic / 错误）、§8（FFI / unsafe）、§10（文档）以及 `clippy::unwrap_used`；本轮沿用原任务的非目标，不改公共 API 或协议。
- 第二轮受审对象：原始 diff + `d517be0` 上的修复工作树。`git diff HEAD -- core desktop` 的 SHA-256 为 `f2f6ae001496438faf32383fe6cdbeebc270a32b5698b63ee4e15eac5a820259`；补充修复后的复审快照见第二轮。
- 两轮分别由独立审查 agent `cr_round_one`、`cr_round_two` 执行；主 agent 核验问题、修改代码并运行检查。

## 第一轮：2 P2 / 1 P3

受审提交：`83dd684..d517be0`。以下行号定位原始提交。

| 级别 / 发现 | 证据与影响 | 修复与核验 |
| --- | --- | --- |
| P2：6 处生产 `unwrap()` 漏改 | `desktop/src/runtime.rs:1722,1738,1741,1748,1754,1760`。前面的 `#[cfg(test)]` 只修饰外部 `mod tests`，后面的 `update_shares` 仍由 AddShare / RemoveShare 生产路径调用。按首个 cfg(test) 行截断造成“零残留”误报。规则：`clippy::unwrap_used`。 | 6 处统一改 `expect("lock poisoned")`，保持原有 panic 语义。`cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used` 修前报 6 处错误，修后通过。 |
| P2：新 FFI Safety 合同不完整 | `desktop/src/lib.rs:56,73,99`。`ymlink_string_free` 的“本库返回的指针”范围错误地包含后端 handle；`CString::from_raw` 需要原始字符串分配和长度。snapshot / command 缺少存活及禁止并发销毁约束。规则：BeautifulRust §8。 | 明确字符串仅来自 snapshot / command，分配 / 终止符保持不变、不得并发访问 / 重复释放；handle 存活且调用期间不得销毁，输入字符串可读且调用期间不变。核对 Swift / C# 调用方，未改 ABI / 实现。 |
| P3：新 Errors 文档与实现不符 | `core/src/beacon.rs:136` 列出 encode 不会返回的 InvalidQueryFlag；`discovery.rs:118` 将配置错误误记为 Socket（实际 Bind）；`files.rs:128` 遗漏 Denied；`session.rs:263,296` 遗漏 Crypto / Io / IdentityChanged 传播。规则：BeautifulRust §5 / §10。 | 按实际返回路径更正文档，错误映射保持不变；rustdoc 构建通过。 |

另核对了锁作用域、连接代际 / 取消、文件 I/O permit 生命周期、队列 permit、格式化输出、`Arc::clone`、let-else、`is_none_or` 及分号重写，未发现原始改动引入运行时行为变化。

不成立的候选项：`TrustStore::remember` 的“保留已有 endpoint”文档正确，`remember_endpoint` 中的 `endpoint.or_else(...)` 已实现该行为，未修改。

## 既有验证阻塞修复

父提交 `83dd684` 在独立临时源码快照中重新编译后，同样失败于两个 discovery 单测；目标提交完整测试为 72 passed / 2 failed / 1 ignored。根因是测试 socket 绑定 `0.0.0.0` 后，当前 macOS 即使关闭 `SO_BROADCAST` 仍允许发送广播；单独 socket 实验对比 wildcard 发送成功与 loopback 发送 PermissionDenied，确认测试前提依赖绑定地址。

仅将 `discovery_without_broadcast` 测试 fixture 改为 `UdpSocket::bind((Ipv4Addr::LOCALHOST, 0))`，仍关闭广播，保留全部错误断言及三端口真实 UDP 收包断言；修后 discovery 单测 6/6 通过。生产 `bind_socket`、发送 / 回退策略及协议字段未改，无测试跳过或放宽断言。

## 第二轮

独立审查覆盖原始 diff、首轮修复、相关 Swift / C# 生命周期调用、协议与异步路径。确认首轮三项修复成立，另发现 1 组 P3 文档准确性遗漏：

- `core/src/session.rs:164`：handshake 的本地和远端 Hello 验证都可能返回 IdentityChanged，已有 `forged_device_id_is_rejected_by_authenticated_noise_key` 测试锁定此路径；补全该错误并覆盖本地 Hello / 无效帧。
- `desktop/src/runtime.rs:82`：命令队列关闭也会返回错误；补充 full or closed。
- `core/src/storage.rs:4`：Unix 实现是检查后按路径 chmod，不能保证路径替换必被发现；删除新增文档中 “verifies it stays one / replaced path” 的过强保证，仅描述实际检查和权限设置。
- 主 agent 交叉核验 `core/src/identity.rs:24`：锁定依赖 rand_core 0.6.4 的 `OsRng::fill_bytes` 会在熵源失败时 panic；将 “cannot currently fail” 改为 “never returns Err”，并为 generate / 两个平台的 load_or_create 补 Panics 文档。独立 reviewer 同意。

规则：BeautifulRust §5 / §10 和 Rust API Guidelines C-FAILURE。以上仅改文档。第二轮补充修复后的代码 diff SHA-256：`92298bb2dd2fd93473273a5ba58a0799b5013d445dce06fa65b199a58346c1d4`。独立 reviewer 已对该快照复核：首轮三项和本轮补充项全部关闭，无未关闭的 P1 / P2 / P3；未发现业务行为或协议变化。另独立执行 cargo fmt --check 与 git diff --check，均通过。

## 验证结果

| 检查 | 实际结果 |
| --- | --- |
| `python3 scripts/harness.py verify` | PASS：Python 37/37；Rust 74 passed / 1 ignored；仓库检查、rustfmt 通过。忽略项是原有真实 Mac H.264 硬件编码测试。 |
| `cargo clippy --workspace --lib --bins -- -D clippy::unwrap_used -D clippy::missing_safety_doc -D clippy::missing_errors_doc -D clippy::missing_panics_doc` | PASS；按编译 cfg 覆盖当前主机生产库和 headset_stub，不把测试模块后方代码误排除。 |
| `cargo doc --workspace --no-deps` | PASS，无 rustdoc 警告。 |
| `cargo clippy --workspace --all-targets --all-features -- -W clippy::pedantic` | 退出 0，128 条诊断（包含重复 target），未声称全量 lint 清零。 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | FAIL：既有 `Box<Arc<Output>>`、三处 too_many_arguments 和测试代码 await_holding_lock 诊断；原始与修复 diff 均未新增这些构造。按原任务非目标保留，不添加 lint 屏蔽。 |
| Windows / Linux 运行与真实设备 | 本机 not-run；本轮不声称 Windows 专属代码或真实 Quest / LAN / 屏幕采集通过。 |

正确保留：现有线协议向量、加密会话 / 文件服务 / 取消与重连测试，以及 FFI 防止 unwind 越界和两端包装的释放约定。未引入依赖、全局 lint 配置或公共 API 变更。

## 交付

两轮完成，YML-006 已归档；本报告随修复提交 main。推送前已核对远端 main 为 d517be0、无分支保护 / 必需检查限制；交付门禁为 `python3 scripts/harness.py check --require-pushed`，以实际远端 ref 核验，执行结果在交付消息中报告。
