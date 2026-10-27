# YML-004：Apache-2.0 开源准备、历史身份审计与 GitHub 自动测试打包

Status: verifying
Workflow: maintenance
Requires-Device: no
Tests: python3 -m unittest discover -s scripts -p test_*.py; harness verify; Windows build / legal bundle / package checks; GitHub Actions run diagnostics; history raw commit / tree / parent comparison; independent remote clone email verification; cargo test --locked -p ymlink-core --all-targets; swift test --package-path macos -c release; relocated app localization smoke

## Scope

按用户要求准备 Apache-2.0 开源，核对全部历史 author / committer 邮箱，补齐 GitHub Actions 自动测试与 Windows / macOS 开发包；追加检查源码、所有历史与开发包的敏感信息并修复确认的问题。用户先后明确授权历史公司邮箱修正及必要的精确 lease 推送、将当前已审计仓库公开为开发预览。采用现有原生构建，不引入安装器 / 发布服务；其他历史正文改写、GitHub Release 正式发布及账户计费设置不在本轮自动执行范围。

## Spec

在 main push / PR / 手动触发时执行仓库门禁与三平台 Rust 测试，Windows 补 .NET / ABI / 启动，macOS 补 Swift / 静态链接 / app 构建。受信任 main 构建上传带 SHA / 版本、SHA-256 及第三方许可的开发 ZIP，PR 不上传制品、不接触签名密钥、不自动发布 Release。流程与开源边界见 [实施计划](../plans/2026-10-27-opensource-ci.md)。协议不变。

## Acceptance

- [x] 历史邮箱 / 隐私 / 依赖许可审计有证据，准备 Apache-2.0 文件与元数据；用户授权邮箱修正后，保留本地备份、独立比对原历史并核验远端新历史。
- [x] CI 覆盖 Linux / Windows / macOS 测试与 Windows / macOS 开发包，使用只读权限及固定 action SHA。
- [x] 包内包含项目与第三方许可证、正确原生制品和校验和；本机可运行的验证通过。
- [ ] GitHub runner 实际执行测试 / 构建 / 上传成功；账户阻塞时保留 not-run 与 verifying。
- [x] commit / push main，check --require-pushed 通过。

## Verification

新增许可证收集与打包 stdlib 测试先 RED 后 GREEN；`harness verify` 汇入 scripts 的全部 unittest。Windows 实际 build / legal collect / ZIP / checksum 验证；macOS 构建仅由对应主机 / runner 验证。GitHub 通过 API 回读 Actions 开关、jobs 和 annotations；不把已配置 YAML 当作云端执行成功。

## Evidence

基线 `fb31045` main 干净，fetch / ff-only 无新增。当前 GitHub 仓库 private，Actions enabled=true，workflow active。最近 run 40474543136 三项 job 均未启动，annotation 明确为账户级托管运行条件限制；未改账户设置。16 个历史提交：6 个 author / committer 为目标邮箱，10 个为同一其他邮箱；当前本地配置正确。完整审计和最终验证追加于 [开源准备报告](../evidence/2026-10-27-opensource-ci.md)。

两轮独立 CR 的三项 P2 均经隔离 fixture RED / GREEN 修复与复核。最终本机 Python 29 项中 28 passed / 1 Unix-mode skipped，Rust 76 passed / 2 ignored；Windows 真实 Release / smoke、legal collect、ZIP / SHA-256 独立回读和 actionlint 通过。许可包保留 128 个 Cargo 包、Rust 1.85.0 与两个实际版本 .NET runtime 原文；该轮检查未覆盖二进制内嵌路径，追加审计发现并修复，详见下文。

交付提交 `4c2ecdcc6959cce18f2e48263bb8de4db3e4178d` 已 push，远端门禁通过；GitHub 已识别 Apache-2.0 且仍 private。新 run 40482200859 checks 未启动、Windows / macOS skipped，annotation 仍为账户级托管运行条件限制；实际云端测试 / 打包 / 上传 not-run。本机同提交开发包与 hash 已生成、校验通过。

## 追加敏感信息审计

基线 `fdaa5a8` 的 18 个提交 / 245 个文本 blob 及 Windows 开发包完成独立审计。显式默认规则的 Gitleaks 8.30.1 扫描历史、源码快照与展开包均 0 findings，合成阳性控制正确检测。旧 Rust DLL 内嵌本机路径已经构建 remap 修复，打包新增自有二进制隐私门禁；实际 Windows Release / ABI / smoke 通过。当前文档、演示设备名与发现测试地址已泛化。追加 `harness verify` 为 36 Python passed / 1 skipped，76 Rust passed / 2 ignored。完整范围、确认问题、验证与历史残留见 [敏感信息审计](../evidence/2026-10-27-sensitive-information-audit.md)。原历史和远端运行记录尚未清理，macOS 包未实测，不把 0 findings 当作全局无敏感保证。

隐私修复交付 `24455748a121d23051066ff832ad5a4854c7cc05`；追加两轮独立 CR，首轮 UTF-16LE 中文路径 P2 经 RED / GREEN 修复，次轮复核关闭。提交后 19 个可达提交及新 ZIP 展开内容的 Gitleaks 均 0 findings。最终 Windows 包独立复核全部 747 成员、464/464 应用文件、CRC / revision / SHA-256 通过，本机路径与已知凭据签名 0 命中。3 份旧 / 审计临时 ZIP 及 checksum 已从分发输出隔离，详细 hash 见报告。

## 历史邮箱修正

历史邮箱修正已完成：备份及独立审查通过，20 个既有提交的 author / committer 均为 `jayzero@foxmail.com`，20/20 tree / 姓名 / 日期 / message 不变，16 个 SHA 改变；精确旧 tip lease 推送后远端新 clone 的 20/20 raw commits 一致。原文 SHA 作为改写前证据保留，映射与完整核验见[历史邮箱修正](../evidence/2026-10-27-history-email-rewrite.md)。本轮仅元数据与文档，应用 / 协议代码无变化，运行 harness check，不重复 cargo / 原生构建。

## Next

追加[公开准备复查](../evidence/2026-10-27-public-readiness.md)：基线 `61c072f` 的 21 提交 / 281 个唯一文本 blob fresh 扫描无已确认凭据，许可 / 依赖未发现确定缺件。HEAD 一处内部名称注释已泛化，历史正文 / message 未改。基线 20 runs / 60 jobs 均 steps=0，50 个失败 job 均为账户条件，0 artifacts / releases / tags；不把背景文本视为已确认凭据泄露，公开前决定保留或清理。源码可以开发预览形式开放，Quest 真机、正式签名 / 安装器不作为源码开放的硬前置。

注释 / 文档小修后实际 `harness verify` 为 Python 36 passed / 1 skipped、Rust 76 passed / 2 ignored。用户现已明确授权公开，GitHub 回读 PUBLIC / Apache-2.0，匿名仓库 API / LICENSE 200，私下报告 enabled=true；新增中英 SECURITY / CONTRIBUTING。首个公开后 run 40505699128 的 checks / windows 全部成功并上传 Windows 包；macOS Rust 构建因默认 Xcode 16.4 SDK 过旧失败，已固定镜像中 Xcode 26.3，等待新 run 验证，任务保持 verifying。完整记录见[公开与云端证据](../evidence/2026-10-27-public-launch.md)。未删除旧运行记录或追加历史改写。

公开后第二跑 run 40506880396 证实 Xcode 26.3 / SDK 26.2 / Swift 6.2.4 已解决 Metal 编译问题；Mac 的端口回退回归暴露 rescan 遇不可路由广播即提前返回。已补 2 项确定性真实 UDP 回归，RED / GREEN 与两轮独立 CR 通过；完整门禁 78 Rust passed / 2 ignored、36 Python passed / 1 skipped，协议字段不变。第三跑继续核验 Mac 测试 / 应用构建 / 打包。

第三跑 run40509363905 的 Mac Rust 74 passed / 1 ignored、原生构建 / 链接 / 签名通过，Swift 6 个中文断言因 SwiftPM locale 目录规范化而失败。已修复相对资源定位和显式语言选择，移除 Bundle.module 绝对 fallback，并增加移位应用资源自检 / 标准签名前 debug strip；两轮独立静态 CR 与本机完整门禁通过，第四跑验证原生 GREEN / 包隐私。含 rescan 修复的 Windows 云端包已完整独立下载核验，范围 / hash 见证据。
