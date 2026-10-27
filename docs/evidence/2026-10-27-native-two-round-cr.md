# macOS / Rust 实现：用户要求的两轮独立 CR

本报告承接用户“启动 subagent 至少两次 CR，修复属实问题后 commit / push main”的要求；任务为 [YML-002](../tasks/YML-002-macos-native.md)。

## 审查范围与轮次

- 完整改动基线：`7dce375`（原型审查完成）至 `8e41ea07b672df8eda2b5fa5fb7f24c75af643ea`，含 macOS 原生客户端、Rust 协议 / 文件 / 运行时 / 采集、构建与测试。
- 第 1 轮已完成，三个独立只读 subagent：`cr1_core` 审发现 / 身份 / 会话 / 文件；`cr1_runtime` 审运行时 / C ABI / 采集；`cr1_native` 审 Swift / 本地化 / 构建 / 测试对端。均审上述同一提交，发现 3 个 P2、1 个 P3，无 P1。
- 第 2 轮已完成：全新 subagent `cr2_security_runtime` 审 core / runtime / C ABI / 传输，`cr2_capture_native` 审采集 / Swift / 测试对端 / 构建。初始受审为 `8e41ea0` + 首轮修复工作树，代码 diff SHA-256 为 `ed39693319601e448bc8366577ce41ca6e85e61e715fbefd78329c07c8baa1cb`。确认首轮问题关闭，新增 2 个 P3，修复后由原审查者限定复核关闭；不将同轮并行分区当作两轮。

## 发现与核验

| 编号 | 级别 / 来源 | 核验与修复 |
| --- | --- | --- |
| R1-C1 | P2 / `cr1_core` | 手动拨号端点已绑定 A，但未知 B 用另一 Hello.listen_port 绕过 pin。真实 TCP / Noise 回归在原实现明确失败；现主动连接另校验实际拨号端点，公告端点仍校验并用于回拨。 |
| R1-R1 | P2 / `cr1_runtime` | 停止输出后，delegate / encoder fatal 与 stop / drain 返回错误被丢弃。纯回调回归在原实现获得 None，明确 RED；现停止媒体但继续保留共享首错，清理错误同样记录，同一授权链重启拒绝。仍执行 handler 移除、drain、invalidate、release，capture gate 保持到 drop 后。 |
| R1-R2 | P2 / `cr1_runtime` | 长任务暂停后，128 次短任务挤掉长任务视图但保留服务器 job。真实 TCP 回归原实现明确失败；现仅淘汰 completed / cancelled / failed，暂停任务可继续从 16KiB 偏移发送。若 128 条全未完成，明确拒绝新 job；取消等待任务可释放记录且连接保持可用。 |
| R1-N1 | P3 / `cr1_native` | 相同异步错误第二次被文本去重吞掉。Swift 原文本逻辑 4 个断言失败；Rust 错误事件不递增时回归也失败。现 snapshot.error_id 每次全局错误递增，Swift 按 id 消费，同 id 轮询不重复，相同文本的新 id 可再次提示。该字段仅为本地快照合同。 |
| R2-R1 | P3 / `cr2_security_runtime` | 首选连接接替旧连接，旧 cleanup 因代际不符跳过，暂停任务不能取消。审查者隔离复现、根代理再次真实 TCP RED（paused != waiting）。现交接持 active 锁迁移未完成任务为 waiting；Open / Ack / read / expire / action / 错误写入共用 active→state 与代际门禁，阻止旧 actor 迟到改新状态。回归 GREEN，原审查者 fresh 重跑 1/1 并回读后确认关闭。 |
| R2-N1 | P3 / `cr2_capture_native` | Rust 测试桩断点缺少来源身份，相同 share / path / version / size 可跨来源拼接。已认证 remote.device_id 贯穿下载并保存至 checkpoint schema 3；拒绝旧格式 / 缺来源 / 换来源，且保持 part 和 metadata 不变。缺比较时实际 RED；修复后 bin 4/4，原审查者独立重跑 4/4 并回读确认关闭。 |

根代理核对了全部调用路径、diff 和实际回归输出。完整两轮共 3 个 P2、3 个 P3，均已修复与复核，未关闭 CR 发现 0。R2-R1 的测试由原审查者补入，其生产代码由根代理修复；其余两轮审查均只读。最终代码工作树 diff SHA-256 为 `74e092aa2799936b132413b1545d04ea7ef9216bbeb328a288caacd0068a2058`，与 handoff 限定复核一致；stub 来源修复也已由对应审查者关闭，此后该文件未改。

## 验证

- 首轮修复后 `harness verify` PASS：69 Rust passed、0 failed、1 ignored；第二轮修复后最终 PASS：70 passed、0 failed、1 ignored，含 core 39、desktop lib 23、stub 4、wire 4。
- 显式硬件合成帧编码 1/1，通过；不采集屏幕、不申请权限。
- 根代理 `swift test --package-path macos` 2/2；本地 Apple Development 签名构建与严格签名校验通过，最终 `.app` 已由 CUA 打开并读取原生窗口。仅本机 arm64 / macOS 27.0.1 验证，旧系统 / Intel not-run。
- 生产原生包实际提交同一无效地址两次，分别看到相同的中文失败提示；关闭后重复轮询未重弹。结束后重启应用，回到无临时设备的搜索态。未发送网络连接或采集请求。
- 修复提交 `d9130b8` 已推送 `origin/main`；根代理实跑 `check --require-pushed` PASS，HEAD 等于实际远端 main，工作区干净。检查与两轮 CR 不能替代下列真实设备边界。

## 边界

本轮不触发屏幕 / 音频采集或系统授权，不以单机测试代替 Quest / 双机 LAN。真实采集、弱网效果与 Quest 解码 / 音画同步仍待授权及设备条件；显示器精确名称按用户指示暂缓。
