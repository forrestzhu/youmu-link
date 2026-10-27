# 开源前敏感信息与制品隐私审计

日期：2026-10-27。任务：[YML-004](../tasks/YML-004-opensource-ci.md)。审计基线：`fdaa5a8e97b587735aceb5c0914ef7710986b85a`。

Commit: 24455748a121d23051066ff832ad5a4854c7cc05

后续更新：用户已授权并完成历史邮箱修正，远端 main 的既有提交 author / committer 均为目标邮箱；下文保留当时基线审计事实和原提交编号。旧 → 新 SHA 映射与核验见[历史邮箱修正](2026-10-27-history-email-rewrite.md)。其他历史内容与副本清理边界仍适用。

## 结论与范围

未发现已确认的访问令牌、密码、私钥、带凭据 URL 或用户身份 / 信任状态文件。发现旧 Windows 开发包的 Rust DLL 嵌入了构建机的用户目录、Cargo 源码目录和工作区路径，已修复构建并重打包。当前 demo 设备名、测试公网子网与文档内部参考名称也已泛化。

不能据此宣称整个仓库历史完全不含敏感信息：10 个旧提交仍含非目标公司邮箱，3 个旧 demo 版本仍含个人目录示例；内部参考名称、原测试地址和账户运行条件说明也仍在已推送历史中。没有改写历史、force push 或改变 private 可见性。这些是公开前待用户决定的清理项。

审计包括当前受跟踪源码、全部 5 个本地 refs 的 18 个可达提交、450 个 Git 对象和 245 个唯一文本 blob（2,346,240 bytes），以及本机 Windows x64 开发包。基线无二进制 Git blob / tag。未访问其他仓库的私有源码，不能用本次模式扫描证明所有代码的权利归属；本机未生成 / 检查 macOS 原生二进制。

## 凭据检查

使用官方 Gitleaks 8.30.1 Windows x64，下载归档 SHA-256 为 `d29144deff3a68aa93ced33dddf84b7fdc26070add4aa0f4513094c8332afc4e`。工具、展开副本与报告仅存忽略入库的 `dist/security-audit` / `dist/tools`。扫描不进行在线凭据验证。

显式启用内置默认规则，清除继承的规则配置，使用空 ignore 文件并忽略行内 allow 标记，启用最多 5 层解码 / 2 层归档，输出全量脱敏报告。使用从未签发的合成 token 做阳性控制，检测器正确返回发现；控制值未写入源码或报告正文。

| 范围 | 实际结果 |
| --- | --- |
| 所有 refs / full-history | 18 commits，960,117 bytes scanned，0 findings，exit 0 |
| 基线 HEAD 的独立源码快照 | 876,495 bytes scanned，0 findings，exit 0 |
| 旧 ZIP 安全展开后的文本部分 | 1,698,682 bytes scanned，0 findings，exit 0 |

对 ZIP 文件本身的一次扫描显示 0 bytes，未计作有效证据；表中使用实际展开后的扫描。二进制的用户名 / 路径检查另行进行，不能用凭据扫描替代。

## 已确认问题与修复

旧 `4c2ecdc` Windows ZIP 有 747 个文件，CRC 正常；自有 `ymlink_desktop.dll` 中发现 169 处用户 / Cargo 路径及 4 处工作区路径，来源为 Rust panic / source-location 字符串。未发现身份数据库、信任状态、授权目录配置、PDB 文件或已确认凭据。先前[开源报告](2026-10-27-opensource-ci.md)只检查了归档条目、元数据和原始日志，未覆盖 DLL 内嵌路径，此处补正其边界。

- 新增共享构建环境 helper，使用 Rust 1.85 支持的 `--remap-path-prefix` 映射 home、Cargo、Rustup 与 workspace；工作区映射最后生效，覆盖路径含空格和两种 Windows 分隔符。保留 encoded Rust flags 优先级及原有环境变量。受控构建的环境 flags 优先于 Cargo 配置中的 flags；额外参数应通过 `CARGO_ENCODED_RUSTFLAGS` / `RUSTFLAGS` 传入。
- Windows / macOS 打包前检查自有可执行文件与库的本机前缀，覆盖 UTF-8 / UTF-16LE、ASCII 大小写、斜杠与读取块边界；错误只报告二进制文件名。保留上游许可中的版权姓名、邮箱和公开构建示例，不将它们当作本机泄露删除。
- 用旧 DLL 实跑先得到 RED：打包拒绝并只给出文件名。重新运行 Windows Release / native ABI / WPF / smoke 后 GREEN，打包通过。测试还覆盖 macOS 主二进制的拒绝门禁；实际 macOS 构建仍为 not-run，Swift / 链接器若带入本机路径将被门禁拒绝，尚未证明其发布包已清理。
- demo 设备名改为通用名称；发现测试用 RFC 5737 文档专用子网替代全球分配地址，保留非 RFC 1918 同子网 / 异子网及广播过滤测试语义，生产逻辑与协议不变。当前文档保留独立实现 / 不复制参考源码的要求，并泛化内部名称及账户条件说明。

独立制品审计复核修复后的临时 `fdaa5a8e97b5` ZIP：全部 747 文件中，旧用户名、home / Cargo / workspace、当前主机名 / IP 为 0 命中（UTF-8、UTF-16LE/BE 与分隔符变体）；token / 私钥签名 0，状态 / 信任 / 共享 / 凭据 / key / PDB 文件 0。CRC 通过，464/464 构建文件 SHA-256 与包一致。

首轮临时审计的 Rust DLL SHA-256：`18e8a2146df76b958ec9d4e9ee78b0f46e664d2b6b775774bb6c682dbd608029`。临时 ZIP SHA-256：`f18f6047afdc1a0e35c27b783b0b1fe8dd0140282a6fc21435cd89fe2c64285c`。该 ZIP 在修改未提交时仅用于审计，不能以旧 revision 标签对外分发；正式本机开发包应在提交后按新 SHA 生成并再验 CRC / 内容 / hash。

## 回归与独立审查

`harness verify` 实际通过：Python 37 项中 36 passed / 1 Unix-mode skipped，Rust 76 passed / 2 ignored；仓库记录 / 链接和 rustfmt 通过。真实屏幕 / 音频采集没有在本次执行。

第 1 轮 `privacy_cr1` 受审基线 `fdaa5a8` 后的路径修复树，确认 P2：直接在 UTF-16LE 原始字节替换斜杠会破坏某些中文字符，使隐私门禁漏检。用合成中文用户名及跨 1 MiB 边界样本独立复现 RED，另复现字节 lower 导致不同汉字误匹配。改为完整 encoded 字符的字节正则，仅对 ASCII 字符匹配大小写、对分隔符匹配两种形式，原始文件字节保持不变；新增两项回归 GREEN，并支持混合分隔符。

第 2 轮 `privacy_cr2` 受审 helper blob `4084932f67de5c36ef63744ef58c62f2f3ce9668` / tests blob `c19f514531bbc3725bc094dec8c94abebe800a72`，独立复跑 37 项 Python 测试及 8/8 内存检测夹具，覆盖 Unicode、编码、分隔符和跨块边界，复核构建 / 打包集成与报告限制；首轮 P2 关闭，无新增确定问题。随后根代理从最终源码重跑 Windows Release / WPF / 真实 DLL 并发与生命周期 / startup-snapshot-shutdown smoke，全部通过。审查不代替真实 macOS 或云端 runner 验证。

## 最终实现提交与包复核

实现提交 `24455748a121d23051066ff832ad5a4854c7cc05` 的 author / committer 均为目标邮箱。受审候选源码快照的 Gitleaks 扫描为 896,319 bytes / 0 findings；提交后重新扫描全部 19 个可达提交，为 989,278 bytes / 0 findings。最终 ZIP 安全展开后扫描 1,698,682 bytes / 0 findings，均 exit 0。扫描配置与上述阳性控制一致。

从最终源码构建、重新收集精确依赖许可后生成 `dist/packages/YM_Link-0.1.0-windows-x64-24455748a121.zip`，不分发先前未提交树的临时包。独立制品审计重新检查全 747 个成员：已列出的本机信息与已知 token / 私钥签名 0 命中，状态 / 信任 / 共享 / 凭据 / key / PDB 文件 0 个；CRC 通过，464/464 当前构建文件哈希一致，BUILD-INFO revision 准确、checksum 摘要及文件名正确。

最终 ZIP SHA-256：`67f4ecad735293d6364627f932383c73f36679ec55cdfa10f1689e82bfdc35d9`；最终 Rust DLL SHA-256：`7ff5aa8d9cdbd3cff24db877857eb0748a321b05189a8a316703bce66d3ff1f2`。最终包结论仍限静态扫描和实际 Windows smoke，不能外推到 macOS 包、未扫描外部副本或真实 Quest / LAN 验收。

## 尚未清理的历史与外部副本

历史邮箱并非全部为目标邮箱；完整原值不复制到此报告。当前 Git 配置和后续提交均使用用户指定邮箱。旧包与未提交树的临时审计包共 3 份 ZIP / 3 份 checksum 已从分发输出移入忽略入库的本地审计隔离目录，不上传或对外分发。旧示例路径、内部参考说明和账户条件文本保留在原提交中，仅修改 HEAD 不会抹去它们。历史审查未发现已确认的私密服务地址、原始日志或保密二进制；不能确认原测试地址的真实来源，因此当前改用文档地址。

公开仓库还可能使既有 Actions annotations / logs 可见；应在公开前同时检查远端运行记录、分支 / tags、制品和任何已有副本。当前云端 runner 尚未实际运行，未生成云端开发包。若用户授权重写历史，应处理原作者 / 提交者邮箱、历史个人路径及上述文本，并重新扫描；不能用 mailmap 或新增提交替代实际清理，也不宣称能删除他人已获取的副本。

模式与前缀扫描仅说明检查范围内未命中，不能保证所有未知格式、语义信息或外部副本均不存在敏感内容。详细扫描报告不入库，避免把本机原始信息再次纳入公开历史。
