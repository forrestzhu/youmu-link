# YML-008：四位配对码与双端配对体验

Status: done
Workflow: behavior
Requires-Device: yes
Tests: Rust99 passed /1硬件ignored、Python38、Swift6、Windows原生CI；Quest1147的22E2E /171语言 /192视觉；新app双方向真实LAN原生组件各1/1，详见Evidence链接。

## Scope

用户明确要求YM_Link从六位改四位，并指定先做HTML demo讨论，定稿后逐项开发 /修复、测试验证、至少两轮独立subagent CR并修复属实问题，最后commit /push main。此任务保留完整目标，用户已于2026-10-29明确确认当前demo并授权原生落地，进入双端实施。覆盖Mac /Windows桌面显示码与Quest同面板输入，保留品牌入口、取消重试与单设备候选。

## Spec

设计来源：design/prototypes/desktop/2026-10-29-four-digit-pairing/。沿用Youmu经典薄荷空间面板与桌面原生视觉，电脑显示四位、Quest四格大按键输入，含前导零；输入在当前面板。依10-29用户新反馈，第四位输入后立即自动配对，移除手动配对按钮；提交中禁用数字 /删除，左下角取消保持可用。四格以52px目标宽度居中排在键盘上方、空间不足可等比收缩；说明 /电脑信息在左侧，减轻边框与重复状态。不常驻解释“输完自动配对”，只由顶部呈现真实配对进度。错码在当前输入盘轻晃 /提示后自动清空，允许直接重输；同一有效逻辑配对窗口沿用电脑码与最初截止，重试不续期，重新开始 /真实过期另取新码；同名新会话不保留旧输入。过期 /限速 /断线与混合版本给可操作反馈。demo是合成状态，不连接网络，不代替原生或XR验证。

仅修改YM_Link配对码：旧Wi-Fi helper与私密PIN协议不变。实施前更新协议与跨版本策略，不能只改UI、截短 /补零旧码或静默放宽长度；保留现有300秒、每连接一次及每IP限速门禁，评估四位码熵变化。确认后在两仓建立实际实现与设备验收链。

### 已确认的实施约束（2026-10-29）

- 应用Hello /认证 /业务统一升v4；发现仍v1，Noise magic /prologue与PAKE domain保留transport-v2。新旧应用在加密Hello拒绝，不尝试六位回退或同时接受两种长度。现行规范以已更新的`PROTOCOL.md` v0.6 /应用v4为准。
- 四位码使用操作系统随机源在0..10000均匀取值，格式化保留前导零；双端严格只收4个ASCII数字。码空间从1000000变10000，不能称强度不变。保留Noise /SPAKE2 /双向确认、单调300秒期限、单连接一次尝试、每IP每60秒最多5次新配对；该限额只约束同一IP与当前进程，不宣称是全网或跨重启限额。五次在线独立尝试的成功概率上界为5/10000=0.05%，不同于原六码的0.0005%。
- 应用版本与身份存储版本分离。Android既有`youmu-link-v3.identity`、AAD、`youmu.link.v3` alias、JSON v=3保持；桌面`device-identity-v2`凭据与trust.json保持。不能全局替换v3 /v2而重建密钥或清空信任。仅双方Known均成立才免码；单端忘记须重新四位配对，pin不符继续拒绝。
- 错码原位重输不得改成同一SecureSession多次SPAKE2：每次失败关闭旧TCP /Noise /PAKE，新尝试经同IP限速门禁。原生需新增绑定同一远端静态key的内存逻辑配对窗口，协商剩余期限，保留首次300秒截止，取消 /过期销毁；不能通过重新连接重置计时。当前四位实现已增加独立逻辑窗口及请求 /剩余期限协商，继续真实互操作与取消代际回归，不把源码完成当真机通过。
- UI承诺的过期 /限速必须来自实际会话事实。v4认证需稳定拒绝码与有界冷却信息，并在等待用户输入期间能处理对端拒绝；保持每方向单一reader，不能为提示增开并发读取。输入期限届满要发布EXPIRED，不能混同用户取消。世代 /取消 /提交与落盘前均核对当前会话，迟到回调不能恢复已取消连接。
- 新版端的版本拒绝提示须覆盖用户从Quest和桌面两方向发起；只对当前可关联连接发布，避免无关或迟到入站错误覆盖已连接状态。已交付旧端只核对拒绝 /不修改信任，不能宣称能补上旧二进制缺少的提示。原生中英与全部Android声明语言使用资源key，未知机器错误映射通用提示。

### 兼容验收矩阵（预期；实际结果见Evidence）

| 双端 /情形 | 可观察预期 |
| --- | --- |
| v4↔v4，双方未信任 | 电脑显示4位，Quest原位逐键输入；0427等前导零经真实SPAKE2成功，仅成功后保存信任。 |
| v4↔v4，升级前双方已信任 | 身份 /公钥 /信任摘要保持，同一profile升级后Known免码重连，不消费新配对尝试。 |
| v4↔v4，任一端忘记 /pin变更 | 单方Known不能放行；忘记后四位重配，pin变更拒绝，不静默换身份。 |
| v4↔v3，任一拨号方向，已信任或未知 | 发现可见、加密Hello拒绝；新版端显示更新提示，不出现错误码长输入、不进配对 /业务，不修改既有信任。 |
| v4Hello之后认证 /业务错版本或未知字段 | 严格关闭，未授权不得into_split /开放文件或镜像，不能降级继续。 |
| 空 /3位 /5位 /6位 /全角数字 /空格 /非数字 | 核心严格拒绝非法长度 /字符，不截短 /补零；UI前三位不提交，第四位自动提交且忽略提交中的额外输入，前导零保持字符串。 |
| 错码 /过期 /取消 /重试 /同名新会话 | 不保存新信任、不开放业务；仅错码原位轻晃后清空，重输使用新PAKE会话并沿用逻辑窗口截止；过期 /断线另提示，新旧回调隔离。 |
| 同IP第6次新配对、返回列表再连接、换端口重连 | 限额在显示新码前生效；冷却提示可恢复，换端口不绕过、列表仍同一匿名设备。 |

### 检查与制品依赖

Rust先补session真实TCP四位 /旧版Hello两方向 /严格码长 /Known迁移 /限速回归，再补desktop运行时取消与错误状态；Swift /WPF核对4格 /输入上限 /前导零 /中英文错误映射与实际窗口。Android覆盖LinkSession /LinkClient /LinkControls /Rust互操作及e2e/CoreVideoFlowE2ETest的真实逐键配对路径；维持文件seek、镜像流代际与GPU /PCM证据，不把fixture成功替代原生Mac。

游目`scripts/build_link_peer.py`已固定双轮审核的四位core提交`feaa9552955704acfce259a35f43694408fbac56`并重建；`main.rs`公开合成码、业务v、fixture启动校验、Python回归及Android主机 /设备参数同步。最终harness须证明对应core /peer输入及二进制摘要，拒绝旧core /旧binary，不手写receipt绕过。原生Mac真实LAN另验；最终受控profile装包 /双SHA /语言E2E绑定新Android输入，保留正式包与两包清理边界。候选稳定后做至少两轮独立实施CR，记录候选 /发现 /修复 /复审，再完成最终检查与两仓main推送。

已认证会话普通TCP退出或传输超时以设备已断开和任务待恢复呈现，不再弹出全局“操作未完成”；未认证连接失败、协议 /加密 /存储错误仍提示。

## Acceptance

- [x] 可点击HTML覆盖发现 /四位输入 /前导零 /删除 /取消 /提交 /成功 /错误重连 /过期 /限速 /断线 /版本不兼容；宽窄与中英可看。
- [x] 用户讨论并明确确认demo，确认结论记录后再原生实施。
- [x] Rust /macOS /Windows /Kotlin /资源 /工具 /协议与测试统一四位，混合版本明确提示，真实Mac↔Quest双方向组件互通通过；Windows↔Quest LAN另验。
- [x] 适用完整测试、当前设备链与语言 /视觉验证通过，失败与限制如实记录；真实录屏 /声音 /XR未代签。
- [x] 实施后至少两轮独立subagent CR，绑定候选，属实问题复现 /修复 /复审闭环。
- [x] 两仓main实现commit /push及严格门禁完成。

## Verification

采用的验收链：HTML阶段先做JS语法、内置浏览器宽窄 /中英 /主流程与有界截图审阅；用户确认后，原生协议 /身份 /取消 /重试 /版本回归先红后绿，覆盖Swift /WPF与Android输入，再跑完整verify、视觉合同、当前制品E2E /语言矩阵及真实Quest↔Mac双方向组件。各阶段独立CR分开计数，新增断开反馈和打包问题各自复现 /修复 /两轮复核。预期为严格四位、第四位自动提交、错码原位重输、存储身份保持与正常结束无误报；具体实际结果 /失败 /限制见Evidence与报告。提交前check /diff自查，push后require-pushed。

## Evidence

最终当前执行见[新app双方向原生与构建重链报告](../../evidence/2026-10-29-four-digit-native-relink.md)：6d613d0 /SHA80f02a6…，Quest /Mac发起各1/1通过，正常结束无误报；Rust99 /Python38 /Swift6、原Android22E2E /171语言 /192视觉、各轮独立CR及失败保留详见报告与下列时间线。真实录屏 /声音 /XR不在本次通过范围，继续既有YML-002 /003与游目对应任务。

开工两仓main均干净，fetch无新提交；游目80d1fba1、桌面5f0b7fb。当前生产仍六位，未发现四位讨论稿或已确认结论。现有10-27demo是六位的历史来源，不据其推断新方案已批准。只读盘点已区分旧helper /私密PIN /发现六字段，确认现有v3严格校验无长度协商；当前盘点不是实施后CR。

2026-10-29 HTML阶段：`node --check` 通过；impeccable detect 单次结果 `[]`。Codex内置浏览器实际点击通过：0–3位不可提交 /4位可提交、删除补齐、2568成功、错误0000→清空并换0427→前导零成功、提交后立即取消不被迟到回调覆盖、已连接返回列表保持连接、断开及取消重连保留信任并免码重连、连续5次取消新连接后第6次限速、限速返回列表仍不能绕过、过期无输入入口 /旧码失效展示、混合版本提示、端口连续变化5次仍1行 /设备计数1。过期边界的deadline检查由源码复审核实，未用实际等待300秒代替该说明。

中英分别检查1440×1000与390×844，根页面均无横向溢出；数字键宽屏65px /窄屏64px。临时viewport已恢复1280默认，默认也无横向溢出；浏览器warn /error为空。截图为`artifacts/yml008/pairing-{zh,en}-{wide,narrow}.jpg`，属于设计合成状态。持久语义标签与live区域含逐位数字；真实读屏播报 /XR /射线 not-run。

独立只读设计审查：候选为本目录工作树。首轮发现截止时间竞态、返回列表绕过限速、首次重试复用首码、读屏无法复核数字及取消重连清除信任；已核实并修正，随后同审阅者复审无新实质阻断。限速 /取消 /重试 /信任路径另有上述浏览器实际交互。本轮是设计稿review，不能计入原生实施后至少两轮独立CR。

HTML与资源之外未修改原生源码 /协议，不重建或替换头显 /Mac包；原生全套测试当前阶段not-run。设计稿仍待用户明确确认，最终commit /push遵从用户“都完成后”顺序，当前不将提案当完整实现交付。

本阶段最终仓库门禁实际通过：桌面61文本文件；游目882文本文件及当前Android语言凭据。两仓diff空白检查通过。新增设计稿 /说明已自查；门禁不等于构建或设备验收。已提供具体设计确认选项，尚未收到结论，未关闭完整目标。

后续自主准备轮只读核对当前源码后补齐以上迁移约束与矩阵，未改原生：Rust `session.rs`的Hello严格v3 /六码校验、`runtime.rs`的每IP5/60s与仅主动手动拨号发布握手错误；Android `LinkSession.kt`的v3 /六码及等待输入的独立截止、`LinkClient.kt`的TimeoutCancellationException归取消分支；`LinkStore.kt`的存储v3密钥 /AAD /信任；构建peer的旧pin与E2E现有main /binary校验。发现的提示传播 /过期风险为源码核查结论，尚未写red或声称真机复现。现有业务行为仍为六位，设计确认门禁不变。

只读代理独立核对同一80d1fba1 /5f0b7fb原生基线，确认存储 /入站错误发布 /输入超时 /限速等待等风险，并补充真实v3持久身份在同applicationId升级与重启、Known撤回竞态、旧对端raw Hello真正上网的矩阵；已纳入以上计划。本轮为实施准备审查，不是实施后CR，也没有测试执行结果。更新后桌面仓库check再次实际通过61文本文件。

等待门禁复核：可点击稿与截图仍在，预览服务的原执行会话81259实际仍运行；两仓原生源码回读仍为v3 /六位。连续三轮均未收到用户定稿结论，独立设计验证与协议准备已完成，不能把自动目标续跑视作设计批准。当前blocked只因用户指定的先定稿后开发顺序；不是完整目标完成，也不是构建 /设备故障。

用户随后反馈demo打不开：重新检查发现8768无监听、原81259句柄已不存在，所以上述运行记录不代表当前可用。已按原目录重新启动仅127.0.0.1监听的独立预览进程；父命令退出后lsof确认监听，Codex内置浏览器实际刷新加载标题 /四格 /码2568，数字2输入再删除可用。已保留预览标签页，未把open_in_codex的queued返回当作已显示证明。实际截图`artifacts/yml008/pairing-preview-restored.jpg`。两仓fetch后无远端新增，原生源码未改。

另生成单文件离线副本`artifacts/yml008/ym-link-pairing-offline.html`，SHA256 `e88d1150ec2d33b3d29058aacdcb034802f2bfd14ba314c316dd01952b12875f`：CSS内联、JS置body末尾，来源内容保持；只读代理核查无外部请求或相对资源。内置浏览器不允许file协议，因此离线副本仅提供本机保存备用，未声明其在内置浏览器运行通过；正常入口继续使用已恢复的HTTP预览。此轮解决访问问题，不构成用户设计定稿。

10-29用户讨论追加：四位输完直接自动配对，并要求优化附图的UI。已按新指令替代原“显式点击配对”提案：第四位只在addDigit的3→4转换自动提交，先同步verifying；移除Pair按钮 /Enter提交 /旧焦点引用，数字长按重复事件不重复填码。四格归到键盘正上方、说明与电脑信息集中左侧，柔和填充减轻线框，取消占键盘左下角并在提交期间可用；缩短主面板无效留白，去掉底部重复等待提示。中英说明与讨论清单同步，原生代码 /协议未改。

新稿实际浏览器验证：三位仍等待且无Pair按钮，删除可回退；2568第四位后立刻“正在配对”，数字禁用 /取消可用、读屏语义为“已输入2 5 6 8，正在自动配对”，随后无需其它点击成功。0000自动报错→重连显示0427并清空→键盘逐位自动成功，Enter未重复提交。第四位后鼠标取消及Escape取消，追加数字 /五次端口更新后仍停留发现页且一条设备，旧回调未恢复连接。提交前后keypad的x /y相同。

新版中英1440×1000 /390×844均无横向溢出，数字盘目标高66 /64px。只读设计复审未发现实质交互缺陷；审阅者提出1001px英文提示可能换行导致移动的假设，实际pair-entry 220.5px、ready /busy状态区均52px且keypad y均447.5，未复现，未据猜测改CSS。临时viewport已恢复，warn /error为空，截图`artifacts/yml008/pairing-auto-{zh,en}-{wide,narrow}.jpg`。这是设计审查及网页验证，不计原生两轮CR或真机通过。

JS语法及本轮最终harness check实际通过；静态资源URL加入内容指纹避免浏览器沿用旧版缓存。离线副本已同步为自动配对稿，当前SHA256 `dd70c48acfad0391df5a55083e6bace3d056220a6ca8df629fb21abdf973b9db`，取代上方手动配对稿副本；其file协议验证边界不变。

10-29继续讨论：按用户“每个数字框太宽”与“不要展示输完自动配对的解释”两条反馈，四格改为flex:0 1 50px并居中，保留高度 /数字字号；删除常驻自动说明与演示页尾操作教程，电脑说明也删除自动机制的复述。准备 /确认阶段均不插入额外行，真实进度只在顶部显示。只读尺寸核查确认220px容器中可收至47.5px /格，非实测结论；实际内置浏览器1440宽测四格各50px、高66px，无横向溢出；英文窄面板各50px且无溢出，中英可见文本均无自动机制说明。2568逐键输入后自动连接实际通过，截图`artifacts/yml008/pairing-narrow-slots.jpg`。旧浏览器控制句柄失效后用同一内置浏览器新标签验证，viewport reset已返回；原生未改。JS语法 /仓库check通过，离线副本当前SHA256 `f90092cd3eeb0ef82ac17f4db18b480ec1b9d8cdb98ee004b976db40f662d5ec`，替代上版。

10-29用户再要求键盘收窄、四格间隔稍大且整体同宽：仅改CSS，pair-entry最大宽340→280px；code-slots以space-between分配间隔且gap最小12px，单格50px目标保留。内置浏览器实际测键盘280px、四格各50px、间隔26.664px，上下左右外沿误差均0，无横向溢出；英文窄面板同样误差0、数字键宽86.664px。只读几何审查确认220px容器时可收缩为格46px /间隔12px /数字键66.7px，此项是源码推导，未代签实测。JS及原生未改，自动配对复用上一轮结果。截图`artifacts/yml008/pairing-aligned-widths.jpg`；复用现有用户预览标签，无新增浏览器页。离线副本已同步，当前SHA256 `5bf67ee4888547a739c67feb7b485edf6545ad0756b1c7e911aaf8f6f715de85`；本轮仓库check通过。

10-29用户要求码框再稍宽 /间隔稍小：单格目标50→52px；保持整体280px，实际浏览器测间隔24px、左右对齐误差均0。仅CSS微调，JS /原生不变；截图`artifacts/yml008/pairing-slot-balance.jpg`，离线副本当前SHA256 `6e3b5a6c1886bdf5b306efaaba3c971fe3e0442dc851d2d7f5e9db745c34f145`，仓库check通过。


10-29用户纠正错码体验：不跳独立失败 /过期页面，四格轻晃、提示后自动清空可重输。已把error并入原输入盘，CSS横向±5px /320ms反馈，520ms后自动清空并恢复输入，错误文字留到下一位；只改变四格transform，错误文案绝对定位在既有22px间隔内。电脑示例码及原始截止不变，每次准备重试计入原5/60s限额；真实过期与限速仍分别反馈，cancel /epoch继续取消旧回调。原生 /现行PROTOCOL未改，不将demo的逻辑窗口当成真实SPAKE2支持。

实际浏览器输入0000：标题仍为“输入配对码”，观察到code-reject动画（系统reduceMotion=false），错误数字随后清空、数字键重新启用、错误文字仍在；键盘文档y在准备 /错误 /清空三态均655.4375。Mac码始终2568，随后直接输入2568自动成功，新输入时错误文字移除。连续5次错误后阻止第6次准备并显示冷却；错误反馈中取消后新会话输入0，五次端口更新后仍保留0，旧回调未清新输入。英文窄面板错误提示无横向溢出、提示底缘与数字盘顶缘相接不重叠，warn /error为空。截图`artifacts/yml008/pairing-inline-error.jpg`为清空后的原位提示。

独立只读代理以真实demo.js摘要`0215af944942df76dfd2e1c87cff3638ab7da32aae92b2d1cdd7df74104c0454`运行Node vm虚拟时钟 /DOM stub，10/10通过：错码→清空→正确、确认 /反馈期间取消、旧排队回调隔离、初始次数 /第5与第6 /列表绕过 /冷却恢复、输入 /确认 /清空跨截止、前导零和重复数字。只证明状态机 /HTML字符串；不是浏览器动画 /真实读屏或原生CR。减少动态效果的保留文字 /状态色路径已源码核对，未修改系统设置做实测。JS语法及仓库check通过；离线副本已同步，当前SHA256 `46cfc3ba3739be33b42a082daf9611c51cf51bad13403da085ec0152efc36d3d`。

2026-10-29用户明确“ok，可以了，基于我们的demo进行开发和落地”，确认门禁解除。已冻结三份源到游目design/contracts/youmu-link-pairing/demo与approved-demo.json；本轮实施前Android安全ff至dc099297，旧两处文档笔记已恢复、恢复stash保留于ignored备份；桌面仍5f0b7fb。开始先红后绿及v4规范更新，设计复审不计本次两轮实施CR。

10-29原生实施中途证据：Rust v4真实TCP四位 /前导零 /严格长度 /Known /v2与v3双方向拒绝16/16通过；两个额外回归去掉修复后实际red，分别复现缺resume字段被当fresh、最终Accepted缓冲后越过截止仍落信任，恢复修复后green。桌面首轮36单元 /8 stub及wire通过（硬件1 ignored）；最新统一active门、pre-Noise取消epoch、带window_id/attempt的Pair和独立EnterTarget路由改动待整轮回归与新负例。macOS首轮.app构建通过；Swift新增测试的首次两失败为新窗口合成fixture沿用旧错误，已修正fixture，待重跑；Windows本机不具备运行环境，最终由现有Windows CI构建 /模型 /窗口smoke回读，未标通过。

Android在真实socket与当前候选Rust对端上14项定向通过，包含错码新连接恢复、旧窗口deadline中断较长新socket、取消入站ready、错码后的拨号失败可恢复；其后整合了最新demo原文案及workspace去重，当前新测试 /fidelity运行中。中途只读审阅发现并修正取消 /缓存 /迟到发布竞态，不计最终两轮实施CR。批准三文件SHA不变，24份浏览器参考已完成；语义skin和组件窄宽触控尺寸适配在冻结前经独立静态复核，详情见Youmu合同reference_adaptations。

开发互操作夹具新增显式host-only candidate路径，机器记录core源码摘要；device E2E拒绝candidate与非reviewed pin（9项Python回归通过）。最终须在两轮实施CR后更新受控core pin、默认重建再跑设备；不使用candidate签发设备凭据。另一Youmu工作区的fixture占用发现端口时保留完整失败日志，不停止其任务，先完成独立回归。

## Next

四位原生实现、既有信任保持、错码重试、视觉 /语言 /设备链和额外两轮增量CR已完成；新app双方向原生组件与正常结束无误报通过。构建修复6d613d0三平台CI（含macOS实际重链回归）已全success；本任务以原生组件范围完成并归档，纯记录提交后执行最终推送严格门禁。真实录屏 /声音 /XR按既有对应任务另验。

2026-10-29当前原生验证：Rust完整98项通过 /硬件编码1 ignored、Python37、Swift模型5及.app构建通过；Android9项输入＋24捕获及24组fidelity通过。第1轮全新独立实施CR完成3项P2并复核：恢复实际认证期限收紧到旧窗、入站版本错误active门原子发布、WPF busy门不吞4th/cancel。真实TCP更长ready恢复窗与pre-Noise Disconnect在独立源码副本移除对应修复后实际red，当前完整cargo green；Windows模型增加busy第四位保留提交机会与取消串行准入断言，尚须Windows CI执行。第2轮已冻结新候选并由全新审阅者独立进行；当前Mac锁定，CUA明确无法启动 /观察窗口，GUI与真实录屏不冒称通过。Android完整主机 /最终包 /设备链进行中。

最终独立实施CR两轮已闭环：第1轮3项P2修复后复核通过；第2轮全新审阅者绑定cr2候选`f41002a6e8dc3035e3e712295c2e379b90e23f3db0af2a5f2f10824094b481b3`（Android候选`ffc4d839b5d0f47d87e61d1aaa7ec3b498dbc5307a3738f71cd5eb04a48bac47`），发现Swift组合grapheme比较会把4 Character /6 byte误提交并锁死attempt。独立实际red输出nonASCII submitted=true、corrected ASCII=false；改精确Character.asciiValue后组合标记 /VS /全角三例均不误提交，同attempt 0427可提交，独立复现及Swift6/6通过。Models增量SHA `4a750f2e0f56ffa21c43433d5bf43bdfbdf45fe3ee2afc885b1826a70cee4e2f`、Tests `0f15d650dabedfdb1acaa02de796a1cfc80adb0481fea371c778701331bb9c98`；审阅者明确无剩余实质发现。工具子审阅17项通过。最终.app重新构建通过，Mac当前锁定，GUI /真实屏幕采集 /Windows窗口及设备链未由CR冒称通过。Android完整2040项零失败 /错误 /跳过，双lint0错误及APK已构建；为当前双输入重新冻结最终制品和192视觉捕获正在进行。

当前代码实现`feaa955`已push main /严格门禁通过，[CI40814254018](https://github.com/forrestzhu/youmu-link/actions/runs/40814254018)的Linux /macOS /Windows三jobs均success，Windows原生模型 /资源 /ABI /release构建及实际窗口smoke通过。Quest3S1147 /21已受控成对安装0删除，正式包与既有加密v3身份保持，22/22 E2E /9地区171语言、双SHA /双编译输入指纹与192视觉通过。真实LAN生产core合成对端的错码重连、0427第四位自动配对、固定逻辑窗口、Known重连、文件seek /合成镜像GPU /PCM全部通过；不是实际桌面GUI或录屏通过。

当前外部阻塞：初次Mac锁屏；解锁后当前.app已通过CUA实际打开 /观察窗口，但后台等待已有身份的SecKeychainFindGenericPassword。进程采样核实钥匙串等待，内置控制对SecurityAgent明确安全拒绝，未读取密码、未重建身份或绕过授权。解除条件为用户在本机自行处理钥匙串系统提示后，继续真实Mac↔Quest原生组件配对 /免码重连；佩戴射线 /双眼仍由方便时实际观察。Android实现cb92c118已push，交付记录见游目本轮报告。

10-29用户已处理钥匙串授权，本机当前.app真实进入searching。原样1147与原生Mac的Quest发起组件1/1通过，实际四位码GUI、配对 /共享目录 /Known重连及双SHA回读通过。但正常disconnect /close后弹出全局连接错误；真实TCP新回归已red，error_id由0错误增至1。当前补最小桌面修复：已认证Io/Timeout仅转已断开 /任务待恢复，其他错误保留；新回归同时检查未认证EOF及已认证错误应用版本仍显示错误。修后需增量双轮独立复核、app重建及原生复验，首轮1/1不能替新包签通过。

正常断开增量修复：runtime SHA071cd61dd9c1f9cfca690fbe68e3603ddf04b75fcd470dcd3258730b08f3ffdd，tests SHA4c7813f6dcacdf47e747aa4de048488f0e04277badd3f60cdcef316292980e6f。新增量第1 /2轮分别由原implementation_cr_round1 /round2独立复核，无实质发现；明确只覆盖当前代际已认证Io/Timeout静默，文件 /镜像 /协议 /加密 /存储及未认证失败反馈仍保留。真实TCP red/green与transfer waiting断言已核对，未动态等待130秒超时，transfer waiting使用合成状态记录。完整99 Rust passed /1 hardware ignored、Python37、Swift6、当前app构建通过；首完整固定UDP端口被旧Mac应用占用的失败保留，释放实际进程后复跑通过。当时打包产物SHA256为256cedcb1b3769b482a649c2f566a98350a5b20fc9a978fce6f9e78c6f9b27d0；后续核查确认它复用了旧Swift可执行文件，没有链接进本次Rust修复，不能称修复包。本机现为macOS26.3.1 /25D2128 arm64；初次原生1/1不为这份新app签无弹窗通过。Mac又锁屏，已请求解锁以做新包双方向原生配对 /Known重连 /正常结束无弹窗复验，当前blocked仅此外部条件。Android1147与reviewed core输入均未变。

修复实现1d10677的[CI40840443478](https://github.com/forrestzhu/youmu-link/actions/runs/40840443478)已实际回读Linux /macOS /Windows全success，Windows原生模型 /ABI /release构建 /窗口smoke通过，gitleaks40840443558 success。当前新app原生双方向无弹窗复验仍因Mac锁屏not-run，不以旧包首轮1/1或CI代替；解锁问题已通过异步问题请求用户处理。无额外源码变化，后续仅补设备证据。


用户解锁后续验发现打包链复用旧Swift可执行文件：256ced…并未包含已通过回归的Rust修复。现已修build_macos.py强制指定包clean后重链，新增真实archive替换回归并接入macOS CI；真实red→green、完整Python38 /Swift6 /app实际Linking通过，额外两轮独立只读CR各自重跑2项通过，候选3份SHA及边界集中[本轮原生与重链报告](../../evidence/2026-10-29-four-digit-native-relink.md)。新app SHA80f02a6…启动再次等待系统钥匙串身份访问，已请求用户自行处理，当前blocked解除条件随此更新；双方向原生无弹窗仍未验，不能使用旧包成功或旧CI代签。


用户处理新app钥匙串提示后，6d613d0 /SHA80f02a6…于16:56和16:57的Quest发起、Mac原生按钮发起各1/1通过：四位GUI、paired /Known reconnected、共享目录1；两次结束后CUA均显示一条可信设备已断开 /等待连接，无全局错误弹窗。双APK前后SHA不变，逐轮临时配对经原生菜单清理，最终设备0；forward /instrumentation /Android临时身份均0，正式APK /既有加密v3身份保持。原生组件与构建细节集中[本轮报告](../../evidence/2026-10-29-four-digit-native-relink.md)。钥匙串外部阻塞已解除；真实录屏 /声音 /XR另验。
