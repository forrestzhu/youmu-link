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

## 2026-10-31 安装版完整更新

用户要求更新后由其手动打开。同步至 `cf6337f`，确认其相对 `355871e9add81d1bd35c22cb9716dddf0a40b9ae` 仅有文档变化；取回该产品提交的成功 [CI run 41188794396](https://github.com/forrestzhu/youmu-link/actions/runs/41188794396) Windows 完整制品，未使用本机 SDK 默认的旧运行库。外层 artifact SHA-256 与 GitHub digest 一致（`8f8baab8f09265f50f40e57f5d3bc3516eb43c4d0c16fc2882b0a1a275e39a6f`），外层 /内层 ZIP CRC、两份载荷 sidecar、源码 /平台元数据、PE 图标与根许可文件核验通过。安装器 SHA-256 `b86168b68ca9f164e40bda5ebfaf6d1adb4e57fa699532458c6564702408660f`；完整载荷含747文件及自包含 .NET /WPF 8.0.32。

确认应用已退出后，完整备份旧安装目录754文件并逐一核对摘要，使用上述安装器沿用原目录静默覆盖（不重启、不自动启动、不主动关闭其他应用），安装器退出0。全部747个载荷文件与安装后逐字节一致；安装版 `--smoke-test` 退出0，真实 backend startup /snapshot /shutdown 通过。更新前后用户状态文件集合与摘要均保持；原路径、配对与共享配置不迁移。最终应用保持退出，由用户自行打开。忽略入库的原始制品、部署回执及备份分别位于 `dist/packages/ci-355871e-verified/` 与 `dist/backups/yml023-355871e-full/`。

安装后 managed DLL SHA-256 `a2f05192527fccb72229f65f8d2adbb7d7a73f2def32193dfbbdb5f1140c19f9`，Rust DLL `c183859d7728b62d0eb5ad6c8859e4ef4010ab44c3cbfb66a9e08f35cc22556f`。包含此前连接 /单边信任重拨修复、自发现过滤以及最新显示器分区与托盘功能；X /Alt+F4 仅收起托盘，实际退出使用托盘菜单。启动检查不代替实际安装版与 Quest 的 LAN 验收，任务继续 verifying。

此前经用户明确批准的防火墙操作已成功：仅为安装版程序添加 Private /LocalSubnet /TCP 入站允许规则 `Youmu.YM_Link.PrivateLan.Tcp.YML023`，不修改公共网络或系统默认策略；实际 Quest 直连仍待验证。此前两次 Native 尝试未取得识别成功，保留失败记录；第二次目标实际为另一目录的临时预览，不将其当作已放行安装版的复验结果。

## 追加两轮 CR 与主机验证

2026-10-31 YM-332开发完成后的第一轮独立CR（桌面受审97dacbd、Quest受审d20b03a2上66文件快照，tree SHA-256 `9a7c91f032f1b497470c22010f21fc5ddfbac749f11d2adc7ca8ed49aa3f155b`）发现反向单边信任缺失：Quest保留身份但忘记桌面时，桌面每5秒自动拨入，Known需重新配对后仍重复。新增真实TCP回归先RED（第二次自动拨号仍被accept），再在自动Known确认需配对时按身份暂停重拨；不占active、不创建配对窗口，显式手动连接与正常入站恢复继续可用，认证成功 /忘记时清暂停。定向cargo测试最终1 passed /0 failed /0 ignored，rustfmt通过；同一测试另验证显式四位配对后可再次自动Known连接。

用户要求第一轮问题修复后先commit /push main供另一会话E2E同步，再继续第二轮CR；本次按此顺序交付中间提交。当前修订尚未更新安装版，完整verify /第二轮CR /当前修订真实LAN为pending，既有97dacbd的完整门禁与CI不冒作新修订结果。

第一轮追加复核指出自动暂停分支在post-Known信任锁等待后绕过epoch检查。真实TCP回归以重复socket peek确认桌面已发Known，再持住信任锁并驱动连接future阻塞，取消epoch后释放锁；旧实现真实RED为晚发布`explicit pairing required`并改ready。修复在同一active gate内先核epoch再更新暂停 /反馈，最终 `cargo test -p ymlink-desktop automatic_known_mismatch -- --test-threads=1` 2 passed /0 failed /0 ignored，rustfmt通过。Quest第一轮实现已推送 `348545f9`，对应41项主机 /真实TCP /Rust互操作 /原生渲染无失败或skip，8态fidelity通过；Quest当前语言凭据 /完整E2E仍待另一会话新包验证。

按用户要求完成推送后，新独立subagent第二轮CR受审桌面24269ac、Quest348545f9，结束81/81差异 /依赖摘要一致，发现1项P2：已信任Quest换IP且单边信任丢失，新匿名路由expected=None绕过ID暂停。真实TCP保存旧127.0.0.2、发现实际127.0.0.1，旧实现重复accept先RED；统一内存身份→host暂停关联后，在tick选择和dial TCP前同时抑制已解析匿名host。另端口刷新到第二真实listener先RED再GREEN，保留手动配对、正常入站及成功 /Forget清理。最终runtime串行33 passed /0 failed /0 ignored、rustfmt通过；第二轮主审独立复核关闭该P2及端口刷新边界，0项新属实问题。双轮受审提交 /发现 /实际验证见[汇总](https://github.com/forrestzhu/youmu/blob/main/docs/evidence/2026-10-31-link-device-identification-review.md)。

24269ac的[CI](https://github.com/forrestzhu/youmu-link/actions/runs/41180395541) Linux /macOS success，Windows安装器卸载smoke未移除exe导致失败（此前Rust /WPF /Release步骤通过）；不记三平台全绿。新修复CI /完整verify与真实设备仍待验，安装版未再替换。

最终产品提交171741340095e024fb01c7f935a01b0863ccd185的[CI run41181100201](https://github.com/forrestzhu/youmu-link/actions/runs/41181100201)回读completed /success，Linux、Windows、macOS三job全部success；Windows原安装器卸载smoke复验通过，保留上一轮失败记录。[gitleaks run41181100187](https://github.com/forrestzhu/youmu-link/actions/runs/41181100187)同提交success。本机构建 `cargo build --locked -p ymlink-desktop --lib --release` 退出0，Rust DLL SHA-256 `c06dd224ab4292c3405995de8086232b3578ca495f6ff85849f76271cac233f4`。未替换仍在使用的97dacbd安装版；当前修订实际Quest LAN仍not-run，Quest完整主机full-host-02及另一会话新包E2E继续执行，不以CI /本机Release代签。

后续安全ff同步另一会话7ae881c发现过滤，a830d75的[三平台CI41181994786](https://github.com/forrestzhu/youmu-link/actions/runs/41181994786)与[gitleaks41181994816](https://github.com/forrestzhu/youmu-link/actions/runs/41181994816)均success；本轮runtime两文件仍与独立复审SHA一致。再次Release构建退出0。实读安装版进程已退出、14:09后YM_Link匹配崩溃事件0；备份并核对原97dacbd DLL到ignored `dist/backups/yml023-a830d75/`，仅更新Rust DLL，回读SHA `448b1c487cf115477f539ea3e02d9ffb20de5ebf7931090b639eab615a8e36cc`。managed55组件SHA及.NET8.0.33回读保持；本次未启动更新后的应用，以免干扰另一会话模拟器Windows网络夹具。

Quest完整主机原进程exit0（2257非skip通过 /26 SMB环境skip、双lint /双APK /release Kotlin完成），另会话当前同0c51输入的完整22E2E /180语言已独立重算与原字节同步机器凭据；不将windows-lan-relay合成Rust对端当实际WPF安装版。待共享网络夹具释放后补当前安装版↔Quest3S直接LAN、取消 /重启稳定恢复与自有临时信任清理，仍保持verifying。

## 2026-10-31 实际 LAN 发现排查

用户手动打开上述安装版后要求不使用 Computer Use；本轮仅使用CLI /ADB /JDWP与仪器测试。核对实际进程路径、源包摘要和监听端口，Quest3S固定serial在线，未操作并行模拟器或其他会话的夹具。安装版信任基线0；运行冻结private1532双APK（与YM-332已核验E2E相同），`native-lan-03`在20秒发现等待处失败，保留原始失败与设备清理证据，临时Android目录剩余0。

发现当前安装程序仅有此前授权的TCP入站规则；有效冻结v1 Query从Quest向三个UDP端口发送均无回复，修正CLI探测参数后TCP连接成功。准备仅放行安装程序 /Private /LocalSubnet /Inbound UDP47610–47612、EdgeTraversal Block的新规则脚本（SHA-256 `10ae3c4f662c6b182bb4c7752b570e4cb05f68f440c45f64e3d925fcb7e4318f`）。两轮新独立只读复核均0项问题，AST /隔离mock核对正确范围与拒绝范围偏差；用户点击管理员确认后脚本退出0，精确范围回读通过，不修改原TCP规则或其他策略。随后同一Query即获得157字节有效Beacon，公告端口与安装版实际TCP listener一致，缺失UDP入站是本轮发现阻塞的已验证原因。

`native-lan-04`发现已通过，但在加密名称 /系统等待处超时；原失败保留。以相同应用UID进行真实TCP与UDP探测均通过，Noise首包也得到96字节响应帧，不把首包响应冒作完整认证。随后冻结APK的CLI调试运行已完成识别 /Windows类型 /配对前无信任 /取消，但等待人工配对码超时，不能记完整pass。新一轮无调试器 `native-lan-05`同样完成上述识别与取消，live_port精确绑定安装版；等待人工四位码180秒后超时，本轮退出1，未完成认证、Known及重启恢复。终态复核无活动instrumentation、自有Android临时目录0、USB转发已清理，桌面信任仍0且文件摘要与基线一致；安装版原进程继续运行，自启动以来匹配Application1000 /1026崩溃事件0。04的识别超时原因尚未单独定位，不因后续识别成功删除该记录。

## Next

当前两轮独立CR与属实问题修复 /复审已经完成，实际LAN识别 /配对前取消的部分检查已执行。待用户方便人工报码时重新发起NativeDeviceTest，完成认证、Known、6500ms断线稳定与重启恢复；同时保留并继续确认04的首次识别超时。全过程遵守不用Computer Use，仅CLI /ADB及必要人工操作；完整链路未通过前保持verifying，不以部分阶段、组件测试或CI代签。
