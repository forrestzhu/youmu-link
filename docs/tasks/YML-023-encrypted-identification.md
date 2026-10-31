# YML-023：加密识别连接不占用配对与活动会话

Status: doing
Workflow: behavior
Requires-Device: yes
Tests: desktop::runtime::tests (Hello-only 静默退出、活动会话 /镜像 /传输保持、Known前取消、双向配对回归)；core::tests::beacon（可选能力保留）

## Scope

支持游目 [YM-332](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-332-link-device-identification.md) 在连接前经既有 Noise Hello 获取电脑名称 /系统。保留发现匿名与现有 v4 配对，修复桌面先占 active 再等 Known 导致识别干扰正常会话的边界。YML-022 的界面闪退修复保留。

## Spec

- 桌面通过现有 caps 追加可选 identify，含义是支持 Hello-only 识别后无副作用退出。未声明的旧桌面不被新版 Quest 自动探测。
- Hello pin /expected 校验后，使用 detached trust 快照完成既有 Known 交换，再占 active /停止镜像 /进入配对。仅完成 Hello 后关闭 /超时不发布连接错误，不创建配对窗口或信任，不影响已有 active、镜像与传输。
- 网络等待不持 trust /active mutex；Known 自带5秒超时，主动拨号在该阶段仍响应原 epoch 取消。手动连接真实失败仍给出既有恢复反馈。
- 发现v1、应用v4和所有字段保持，未知 capability 可忽略；正常旧客户端显式连接和双向配对继续有效。core API /存储 /身份不变。

## Acceptance

- [ ] Hello-only red→green；同身份首选方向探测不打断 active /镜像 /传输。
- [ ] Known等待取消 /撤销、正常配对与旧cap兼容回归通过；完整 harness verify。
- [ ] Windows构建 /真实安装版启动、独立审查与真实Quest LAN识别 /连接 /取消通过。
- [ ] 双端任务 /证据互链，提交推送与 strict。

## Verification

先改 PROTOCOL 的可选cap语义，保留旧冻结向量；修改 /新增 Rust 行为测试观察 red 后最小重排。运行定向 cargo 回归、完整 harness verify、Windows Release /ABI smoke；独立只读审查。Quest3S与Windows11真实LAN查看名称 /系统、连接 /取消且不闪退；未验Mac /其他硬件写not-run，不用loopback代签。

## Evidence

2026-10-31 基于7f56e77，fetch /ff-only已同步。独立只读调查确认 TrustStore::detached、needs_pairing缓存与Known授权后重新校验均现成，无需新增线协议或core API。现有caps允许未知值且最多8项，identify增加后4项；旧桌面Hello会占active，故必须能力门禁，不能无差别自动探测。

失败回归：`hello_only_identification_leaves_idle_state_and_trust_unchanged` 在旧实现观察到错误事件与状态改变；既有 preferred handoff 用真实暂停传输补充 Hello-only 探测，旧实现 active 被取走导致失败。Known 前置后上述两项及原 authenticated EOF /协议错误回归通过，保留 Known 之后认证失败的原反馈。

独立CR 首轮发现两项P2：入站Known未响应取消、等待后实际端点pin可能过时。新增真实TCP回归用重复socket的只读peek确认桌面Known已经发出，再执行取消 /更新实际端点pin /忘记既有信任，确保测试命中等待窗口。依次观察 cancel-inbound 超时red、补入站watch后 pin-outbound 超时red；加入Known之后新信任 /双端点复核后，三种情形全部green。入站watch在Noise之前订阅，active锁内继续校验epoch；网络等待不持trust锁。复审与完整门禁待记录。

复审关闭两P2，追加P3为手动pin冲突反馈遗漏；新增断言先red，再仅对该真实手动失败调用带epoch的dial_error后green，取消 /忘记仍静默。完整门禁首轮被正在运行的安装版占用47610阻断（OS10048），正常关闭空闲窗口后重新执行；下一轮暴露原生产回归“显式断开后不接受自动入站重连”：Known前置使拒绝过晚。将原disconnected门禁也前置并保留active锁内复核后，最终 `RUST_TEST_THREADS=1 python3 scripts/harness.py verify` 退出0，Python47 /3 skipped、Rust109 passed /2 ignored、check /rustfmt通过。未删除或放宽原断开断言，所有首次失败保留。

## Next

按顺序执行：Rust失败回归→Known前置→Rust验证→Quest有界识别与界面→两端构建和真机验收。证据与每次失败保留在各自任务。
