# YML-023：加密识别连接不占用配对与活动会话

Status: verifying
Workflow: behavior
Requires-Device: yes
Tests: desktop::runtime::tests (Hello-only 静默退出、活动会话 /镜像 /传输保持、Known前取消、双向配对回归、automatic_known_mismatch_pauses_redial_until_manual_pairing_succeeds)；core::tests::beacon（可选能力保留）

## Scope

支持游目 [YM-332](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-332-link-device-identification.md) 在连接前经既有 Noise Hello 获取电脑名称 /系统。保留发现匿名与现有 v4 配对，修复桌面先占 active 再等 Known 导致识别干扰正常会话的边界。YML-022 的界面闪退修复保留。

## Spec

- 桌面通过现有 caps 追加可选 identify，含义是支持 Hello-only 识别后无副作用退出。未声明的旧桌面不被新版 Quest 自动探测。
- Hello pin /expected 校验后，使用 detached trust 快照完成既有 Known 交换，再占 active /停止镜像 /进入配对。仅完成 Hello 后关闭 /超时不发布连接错误，不创建配对窗口或信任，不影响已有 active、镜像与传输。
- 网络等待不持 trust /active mutex；Known 自带5秒超时，主动拨号在该阶段仍响应原 epoch 取消。手动连接真实失败仍给出既有恢复反馈。
- 发现v1、应用v4和所有字段保持，未知 capability 可忽略；正常旧客户端显式连接和双向配对继续有效。core API /存储 /身份不变。

## Acceptance

- [x] Hello-only red→green；同身份首选方向探测不打断 active /镜像 /传输。
- [x] Known等待取消 /撤销、正常配对与旧cap兼容回归通过；完整 harness verify。
- [ ] Windows构建 /真实安装版启动、独立审查与真实Quest LAN识别 /连接 /取消通过。
- [ ] 双端任务 /证据互链，提交推送与 strict。

## Verification

先改 PROTOCOL 的可选cap语义，保留旧冻结向量；修改 /新增 Rust 行为测试观察 red 后最小重排。运行定向 cargo 回归、完整 harness verify、Windows Release /ABI smoke；独立只读审查。Quest3S与Windows11真实LAN查看名称 /系统、连接 /取消且不闪退；未验Mac /其他硬件写not-run，不用loopback代签。

## Evidence

2026-10-31 基于7f56e77，fetch /ff-only已同步。独立只读调查确认 TrustStore::detached、needs_pairing缓存与Known授权后重新校验均现成，无需新增线协议或core API。现有caps允许未知值且最多8项，identify增加后4项；旧桌面Hello会占active，故必须能力门禁，不能无差别自动探测。

失败回归：`hello_only_identification_leaves_idle_state_and_trust_unchanged` 在旧实现观察到错误事件与状态改变；既有 preferred handoff 用真实暂停传输补充 Hello-only 探测，旧实现 active 被取走导致失败。Known 前置后上述两项及原 authenticated EOF /协议错误回归通过，保留 Known 之后认证失败的原反馈。

独立CR 首轮发现两项P2：入站Known未响应取消、等待后实际端点pin可能过时。新增真实TCP回归用重复socket的只读peek确认桌面Known已经发出，再执行取消 /更新实际端点pin /忘记既有信任，确保测试命中等待窗口。依次观察 cancel-inbound 超时red、补入站watch后 pin-outbound 超时red；加入Known之后新信任 /双端点复核后，三种情形全部green。入站watch在Noise之前订阅，active锁内继续校验epoch；网络等待不持trust锁。

复审关闭两P2，追加P3为手动pin冲突反馈遗漏；新增断言先red，再仅对该真实手动失败调用带epoch的dial_error后green，取消 /忘记仍静默。完整门禁首轮被正在运行的安装版占用47610阻断（OS10048），正常关闭空闲窗口后重新执行；下一轮暴露原生产回归“显式断开后不接受自动入站重连”：Known前置使拒绝过晚。将原disconnected门禁也前置并保留active锁内复核后，最终 `RUST_TEST_THREADS=1 python3 scripts/harness.py verify` 退出0，Python47 /3 skipped、Rust109 passed /2 ignored、check /rustfmt通过。未删除或放宽原断开断言，所有首次失败保留。

最终独立只读复审范围 `55eef04..97dacbd9fca57b3b5b856a11c2fc4771c31e44a0`，两项P2及手动pin反馈P3均关闭，未发现剩余可操作代码问题；确认入站watch订阅 /active锁内epoch复核、Known后实际与公告端点pin复核、撤销后的新信任检查及显式断开门禁。2026-10-31 14:15:32（UTC+8）回读 [CI run 41175348703](https://github.com/forrestzhu/youmu-link/actions/runs/41175348703)：同提交Linux /macOS /Windows全部completed /success；Windows于14:15:22完成Rust、WPF /本地化 /ABI /Release /bounded smoke、安装器 /ZIP测试及制品上传。[gitleaks run 41175348707](https://github.com/forrestzhu/youmu-link/actions/runs/41175348707) 同样success。

安装版已更新为 `97dacbd` Release 的 `ymlink_desktop.dll`，SHA-256回读 `f9b03660883542122c82f4f1141d506418daa28c155b701aa817cd37c0a21561`；原Rust DLL备份位于ignored `dist/backups/yml023-97dacbd/`。保留YML-022已修复的managed界面DLL及安装版.NET 8.0.33，更新后已重新启动。上述部署 /启动不等于真实LAN验收；游目YM-332正在原生开发，连接前名称 /平台识别、实际连接 /取消及已有会话保持的Quest3S↔Windows验证仍为not-run。

## Next

2026-10-31 YM-332开发完成后的第一轮独立CR（桌面受审97dacbd、Quest受审d20b03a2上66文件快照，tree SHA-256 `9a7c91f032f1b497470c22010f21fc5ddfbac749f11d2adc7ca8ed49aa3f155b`）发现反向单边信任缺失：Quest保留身份但忘记桌面时，桌面每5秒自动拨入，Known需重新配对后仍重复。新增真实TCP回归先RED（第二次自动拨号仍被accept），再在自动Known确认需配对时按身份暂停重拨；不占active、不创建配对窗口，显式手动连接与正常入站恢复继续可用，认证成功 /忘记时清暂停。定向cargo测试最终1 passed /0 failed /0 ignored，rustfmt通过；同一测试另验证显式四位配对后可再次自动Known连接。

用户要求第一轮问题修复后先commit /push main供另一会话E2E同步，再继续第二轮CR；本次按此顺序交付中间提交。当前修订尚未更新安装版，完整verify /第二轮CR /当前修订真实LAN为pending，既有97dacbd的完整门禁与CI不冒作新修订结果。

第一轮追加复核指出自动暂停分支在post-Known信任锁等待后绕过epoch检查。真实TCP回归以重复socket peek确认桌面已发Known，再持住信任锁并驱动连接future阻塞，取消epoch后释放锁；旧实现真实RED为晚发布`explicit pairing required`并改ready。修复在同一active gate内先核epoch再更新暂停 /反馈，最终 `cargo test -p ymlink-desktop automatic_known_mismatch -- --test-threads=1` 2 passed /0 failed /0 ignored，rustfmt通过。Quest第一轮实现已推送 `348545f9`，对应41项主机 /真实TCP /Rust互操作 /原生渲染无失败或skip，8态fidelity通过；Quest当前语言凭据 /完整E2E仍待另一会话新包验证。

推送第一轮修复后启动新的独立第二轮CR，绑定双端提交 /实际运行包完成Quest3S↔Windows真实LAN名称 /平台识别、连接 /取消及已有会话保持验收；补双端证据互链与最终strict检查。未验真实LAN前保持verifying，不以组件测试、CI或安装版启动代签。
