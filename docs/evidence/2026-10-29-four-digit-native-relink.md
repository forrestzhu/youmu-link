# 四位配对：Mac原生续验与构建重链

- Commit: `1d10677be30e9c67db763f8c4675fbada4820987`（正常断开修复）；后续构建工具候选摘要见下表。
- Host-OS: macOS26.3.1 /25D2128 arm64；Quest3S Android14 /API34 /incremental4069240019900610。
- Result: partial，重新链接后的app等待本机处理钥匙串提示，尚未签双方向原生通过。
- Task: [YML-008](../tasks/YML-008-four-digit-pairing.md)。

## 真实发现与修复

用户处理既有身份的钥匙串提示后，Quest1147与原生Mac首次配对、共享目录查询、Known免码重连1/1通过；但正常断开误弹全局连接错误。真实TCP回归先复现已认证EOF使error_id从0增至1，再只将当前代际已认证Io/Timeout呈现为已断开、任务待恢复。未认证EOF与已认证错误业务版本仍报错；加密 /存储 /文件 /镜像错误分支保留。transfer waiting断言使用合成任务状态，未等待真实130秒超时。

正常断开修复完整99 Rust passed /1硬件ignored、Python37、Swift6通过，额外两轮独立只读CR均无实质发现。固定UDP端口被旧原生app占用导致的首轮失败保留，释放已确认进程后完整复跑通过。[同一修复三平台CI](https://github.com/forrestzhu/youmu-link/actions/runs/40840443478)全部success，Windows原生模型 /ABI /release /窗口smoke通过；gitleaks也通过。

其后本机打包命令成功却仍误弹。核查发现Rust archive在15:59更新，Swift可执行文件仍为11:47；SHA `256cedcb1b3769b482a649c2f566a98350a5b20fc9a978fce6f9e78c6f9b27d0`的app复用了旧后端，不能作为修复包。该产物首次原生发现20秒超时，下一次自动发现、配对 /Known重连1/1通过但仍误弹；期间没有点击重新搜索，超时原因未证实。失败与旧包成功均保留，不归因于新runtime失败。

构建脚本现在先构建Rust，再清理明确的Swift package生成物并重新链接。真实C archive回归只将原生返回值1改2、Swift源码不变；旧脚本第二次仍输出1而实际red，修后两次实际Linking并输出1、2。不是mock调用顺序断言。macOS CI新增同一回归；Linux明确平台跳过，原门禁保留。

## 构建候选与独立CR

| 文件 | SHA-256 |
| --- | --- |
| scripts/build_macos.py | `24dff33b6fd018551d545ce40d57e2bf02a16514adf8df36b8159628b8a56e95` |
| scripts/test_build_macos.py | `f960028d3ea41639e352df5cd5f665ed58b2fdf345b07ade53d2339591d9af84` |
| .github/workflows/repository-checks.yml | `06ed31d2c74fd21a6387c09b7eabbeb6f8f75b7736a54bd899e2cedb42b46ebb` |

两名独立只读审阅者分别复核以上三份字节、生产构建顺序、清理边界、真实red /green与CI接入，均未发现实质问题，各自独立重跑2项测试通过。清理仅限指定Swift包生成物，不触及Rust archive /源码 /其它工作区。此增量双轮与原四位实现、正常断开修复各自的两轮CR分开记录。

修后完整Python38、Swift6、app构建 /静态依赖 /签名 /移位资源验证通过；本机16:44实际编译及Linking，最终app可执行文件SHA为`80f02a6d4a4a465cb4b135d2a84e3b95de3449086c3d10f206d1cd1f82e487fd`。Rust源码保持已通过99项的字节，Android1147双APK与reviewed core输入不变；未因此重跑Android完整2040项。

记录稳定后`python3 scripts/harness.py verify`整轮再次通过：Python38、Rust99 passed /1硬件ignored、rustfmt及仓库检查。日志为ignored `artifacts/yml008/native/native-relink-verify-final.log`。AGENTS /HARNESS任务合同及现有构建入口保持适用，TESTING补真实重链命令与边界，CI调用同一测试文件，不另设跳过门禁路径。

## 原生复验边界

本次app保留已有系统身份，启动再次等待SecKeychainFindGenericPassword；进程采样已证实，内置控制禁止操作系统SecurityAgent。已请求用户在本机处理提示，不读取密码、不重建身份、不绕过授权。处理后按Quest发起、Mac发起分别验证四位GUI /配对 /共享目录 /Known重连 /正常结束无全局错误，并清理本次临时配对。

组件测试使用生产LinkClient、真实LAN与原生Mac app，仅经私有USB通道代输入GUI观察到的四位码；不启动真实录屏或系统音频。结果不代签VR射线、双眼舒适、真实声音、镜像性能或Windows↔Quest LAN。原始日志与产物仅留ignored artifacts，配对短码不入库。
