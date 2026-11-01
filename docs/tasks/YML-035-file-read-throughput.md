# YML-035：文件读取吞吐：请求排队、句柄缓存、32K 块与单次写帧

Status: verifying
Workflow: behavior
Requires-Device: yes
Tests: core::tests::files::{streaming_reads_reuse_one_handle_until_the_file_changes_or_the_share_goes, request_limits_bound_allocation_and_noise_frame_size, read_supports_seek_resume_eof_and_rejects_changed_files}; desktop::runtime_tests::{pipelined_range_reads_are_all_served_and_reuse_one_open_handle, file_queue_overflow_rejects_only_the_excess_request, file_work_queues_beyond_four_slots_and_cancelled_requests_do_not_emit_chunks, cancelling_file_waiters_keeps_io_slots_until_disk_work_finishes, pipelined_loopback_throughput_report (ignored benchmark)}; desktop::tests::native_backend::largest_transfer_and_file_chunks_fit_an_encrypted_frame

## Scope

游目用户在 Wi-Fi 下经 YM_Link 播放约 5 GB 影片 2 倍速即卡顿，起播也慢。桌面侧原因：`Read` 单块上限 16 384 B；每块重新 open 文件、两次元数据校验、再 close；`serve()` 超过 4 个在途文件请求立即回 `unavailable`，响应经 32 项控制队列 `try_send`，满了直接断连；长度前缀与密文分两次写出。配对的 Quest 端任务为游目 [YM-335](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-335-link-stream-throughput.md)（流水线预读 16 × 32 KiB、旧桌面自动回退 16 K、无装箱 CBOR 解码）。

本任务：`MAX_READ_BYTES` 16 384 → 32 512（最坏 2 B/项编码连同 `file_response` 外层仍 <65 519）；传输块独立为 `TRANSFER_CHUNK_BYTES = 16 384`，§4.3 停等语义不变；`FileCatalog` 缓存已校验文件句柄（≤8，30 s 空闲释放，每次读取前后仍以同句柄 metadata 核对版本，每 1 s 按路径重开核对同名替换，撤销共享即丢弃）；`serve()` 把文件请求排进 32 项 FIFO，取得控制队列 permit 后才开始磁盘工作，写出阻塞时暂停读取而非断连；`SecureWriter` 一次写出长度 + 密文，`SecureReader::recv_raw` 供快速 / 部分解码。协议规范升 v0.7（[PROTOCOL §3.1 / §3.3 / §4.2 / §4.3](../engineering/PROTOCOL.md)），报文字段与应用版本 v4 不变。非目标：不改传输停等窗口、不改镜像、不手写 CBOR 编码器。

## Spec

- 给定已授权连接连续发送 16 个 `read`（各 32 512 B）而不等待，则全部以 `read` 回复、按 `id` 匹配，期间 catalog 只保留 1 个打开句柄；文件被追加后下一次读返回 `changed` 且句柄数归零。
- 给定磁盘被占住，4 个槽位满时第 5 个请求排队等待而不是回 `unavailable`；排队超过 32 个请求的才回 `unavailable`，其余在磁盘空闲后全部服务。
- 给定被取消的传输 job 的读取仍在磁盘上进行，其结果不发出 chunk（既有行为保留）。
- 给定同名替换（rename 覆盖，新 inode），默认 1 s 内下一次读取返回 `changed`；测试夹具以零间隔核对保持"替换即 changed"可观察。
- 单元：`Read.length` 1–32 512 合法，0 / 32 513 / 越界 offset 为 `invalid_request`；最坏编码 `FileResponse::Read` 与 `Message::TransferChunk`（16 384）均 <65 519。

## Acceptance

- [x] 流水线 16 读全部服务、单句柄复用、追加后 `changed`（`pipelined_range_reads_are_all_served_and_reuse_one_open_handle`）。
- [x] 排队语义：第 5 个等待，第 37 个 `unavailable`，其余全部服务（`file_work_queues_beyond_four_slots…`、`file_queue_overflow_rejects_only_the_excess_request`）。
- [x] 句柄缓存生命周期：复用 / 过期 token 不缓存 / 变化驱逐 / 撤销共享清空（`streaming_reads_reuse_one_handle…`）；rename 替换仍 `changed`（`read_supports_seek_resume_eof_and_rejects_changed_files`）。
- [x] 回环基准：深度 1 → 38 MB/s，深度 4 / 16 → 72 / 69 MB/s（Windows 11 x64，release，`recv_raw` 客户端）。
- [ ] 真机：Windows 安装版 + Quest 3S（YM-335 构建）同一 Wi-Fi 播放 ≥5 GB 影片 2 倍速不卡顿、起播明显加快；macOS 构建同样验证。

## Verification

- `cargo test -p ymlink-core`、`cargo test -p ymlink-desktop` → 预期全部通过（新增 3 项 + 修改 2 项）。
- `cargo test --release -p ymlink-desktop pipelined_loopback_throughput_report -- --ignored --nocapture` → 打印三档深度的 MB/s（基准，不设阈值）。
- `cargo fmt --all -- --check`、`cargo clippy --all-targets` → 预期不新增告警（基线 8 条保留）。
- `python scripts/harness.py verify` → 预期通过。
- 设备层：Windows / macOS 安装版各自与 Quest 实测（not-run 见 Evidence）。

## Evidence

- 2026-10-31 Windows 11 Pro x64（已合并 origin/main 至 e7141ca 之后的 8 个提交）：`cargo test -p ymlink-core` 23 + 9 + 11 + 1 + 16 + 3 passed；`cargo test -p ymlink-desktop` 51 passed / 3 ignored（含新基准）+ 4 + 1 + 4 passed。红 → 绿：句柄缓存首版让 `read_supports_seek_resume_eof_and_rejects_changed_files` 的 rename 替换读出旧内容（旧 inode 元数据未变），加入每 1 s 按路径重开核对、夹具零间隔后通过。
- 回环基准（release，64 MiB，`isolated_file_server_raw`）：depth 1 = 38.1 MB/s（1173 块/s），depth 4 = 72.4 MB/s，depth 16 = 68.7 MB/s。先前用 `recv::<Message>` 的客户端只测到 12 MB/s，瓶颈是 ciborium 逐项解码 uint 数组（≈2.7 ms / 32 KiB），与桌面无关；ciborium 编码 32 KB 约 265 µs（≈120 MB/s），手写编码器 61 µs，暂不引入。
- clippy：新增 1 条 OR-pattern 提示已改为区间，基线 8 条未动。`cargo fmt` 已运行。
- `harness verify`：见下方记录。
- 真机联调（Windows 侧）：CI `repository-checks` 三平台通过（run 41261968031），`YM_Link-windows-x64-af3b56ab53ce-setup.exe`（SHA-256 4ce308d9…，校验一致）已覆盖安装至本机 `I:\my_program_files\YM_Link`（原 73cb59a → af3b56a），安装后正常启动。Quest 侧配对任务 YM-335：全量 22 例 E2E + 180 项本地化矩阵在 Quest 3S 通过（含 link 之旅，钉住 v4 对端即验证了旧桌面回退路径）。macOS 构建 not-run（需用户 Mac）。
- 用户原网络 2 倍速 / 起播主观体验：not-run，待用户佩戴反馈（两端均已装新版：Quest 主包 20261101.1112 + 桌面 af3b56a）。
- CR（用户要求 ≥3 轮独立 subagent 只读审查，逐轮修复后复审）：
  - 第 1 轮：属实并修复——排队溢出的 `unavailable` 在控制队列满时经 `try_send` 会断连，改为先 `try_reserve_owned`，预约不到则继续排队；`send_inner` 长度校验移到 nonce 递增前；补目录锁串行化注释。未采纳：`MAX_FILE_WORK` 实际并行化（回环 70 MB/s 已达标，留待实测）。
  - 第 2 轮：属实并修复——上游同步遗漏：工作树落后 origin/main 8 提交（含 e7141ca Windows 首读版本号修复，与句柄缓存同域），已 stash→ff-only→pop 干净合并并全量复测通过；回压下 `waiting` 队列对不断发请求却不读响应的对端无上界，加 `MAX_FILE_BACKLOG` 硬上限后断连。
  - 第 3 轮：无 P0/P1/P2，结论 Ship。5 条 P3：云占位（OFFLINE/RECALL）文件首读会多一次 `changed` 重开、NTFS LastAccess→ChangeTime >1h 级联可能中途一次 `changed`（均为安全方向失败且早于本改动，如现网出现再记入 PROTOCOL）；Pong 在控制队列满时仍走 `try_send`（仅洪泛对端可达，视为预期）；`MAX_FILE_BACKLOG` 断连路径未单独测试（安全网，已手推有界）；测试 panic 时临时目录泄漏（与既有风格一致）。另确认：本机 discovery 测试 `beacon_ports_fall_back_in_order` 偶发失败是正在运行的旧版 YM_Link 进程占用 UDP 47610 所致，与改动无关。复查确认：Windows 首读 token 与句柄缓存无新增风险、eof/parse 断言在所有边界一致、serve() 计数与 permit 全部配对；回环基准复测 47/94/93 MB/s。

## Next

- 待用户佩戴实测 2 倍速与起播（两端新版已就位；头显现连 PC 热点 FORREST-PC_WIFI，同二层可直连）。macOS 用 `python3 scripts/build_macos.py` 后同样实测。
- 若实测仍受桌面 CPU 限制，再评估手写 uint 数组编码（4× ciborium）与多线程编码。
