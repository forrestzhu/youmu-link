# 源码公开准备复查

日期：2026-10-27。任务：[YML-004](../tasks/archive/YML-004-opensource-ci.md)。基线：`61c072f15ac2ef8c553218bc0e2ac4c642710e0d`，fetch / ff-only 无新增，开工工作区干净。仅复查公开准备并修正当前注释 / 文档，不改变仓库可见性、不改写历史正文或删除远端记录。

## 结论

未发现新的已确认凭据泄露或阻止桌面源码公开的确定许可缺件。邮箱修正已完成，当前源码及许可证具备作为开发预览公开的基础。剩余公开前决策是：是否接受历史演示信息、内部工程背景与账户运行背景一同公开；不接受的部分需先清理。CI 云端实跑仍未完成，它影响自动测试 / 开发包验收，不应与源码公开本身混为一个技术前置条件。

真实 Quest / LAN、macOS CI 构建、录屏音频和正式签名分发的未完成项应继续明确标为 not-run；可以在开发预览阶段保留，不把当前版本宣传为已验收的完整头显产品。

## 本次实查

独立 `public_history_readiness` 审查 21 个 main 提交：42 个 author / committer 邮箱字段均为目标邮箱；515 个可达对象、281 个唯一 UTF-8 文本 blob（2,591,863 bytes），无二进制 blob。Gitleaks 8.30.1 fresh main / full-history 为 21 commits / 1,002,648 bytes / 0 findings / exit 0，未发现需要先轮换的已确认凭据。此结果仍不保证未知格式或所有外部副本均无敏感信息。

| 剩余历史内容 | 基线范围 | 判断 / 下一步 |
| --- | --- | --- |
| 看似个人用户名的绝对目录示例 | demo.js 的 3 个唯一 blob、16 个提交树，HEAD 已无 | 演示数据，无法证明是真实目录；公开前决定是否保留，不等同为凭据泄露。 |
| 带个人名字的演示设备名 | 同一 demo.js 的 4 个 blob、18 个提交树，HEAD 已无 | 同属公开前隐私取舍。 |
| 内部参考项目名称 / 工程背景 | 43 个 blob、11 个路径，21 个提交树；首个提交 message 也有名称 | 名称或设计借鉴不证明保密源码被复制；若决定去除名称，须覆盖代码注释、文档历史及首个 message。 |
| 账户运行条件的具体说明 | 32 个 blob、8 个路径、17 个提交树，HEAD 已泛化 | 不是访问凭据，是否公开该运行背景由维护者决定；远端 annotations 另列于下。 |
| 全球分配地址的测试子网 | discovery.rs 的 1 个 blob、14 个提交树，HEAD 已替换 | 仅在测试 fixture 中，来源不明，不能认定为真实内部拓扑；不希望公开时纳入历史清理。 |

本轮发现 `core/src/beacon.rs` 当前注释还有内部名称 / 参考端口残留，已仅泛化注释；常量、线协议与运行行为不变。修正 README / ROADMAP 的历史邮箱待处理旧描述、TESTING 的两平台 CI 旧描述，并明确 Windows C++ Build Tools / SDK 前置依赖。没有顺带改写历史内容。

## 许可与依赖

独立 `public_license_readiness` fresh `cargo metadata --locked --offline` 检查 128 个外部 crates：均有许可元数据和可发现原文，无 GPL / LGPL / MPL 声明。Apache-2.0 LICENSE、NOTICE、第三方声明与 workspace SPDX 齐全，GitHub API 已识别 Apache-2.0。当前 legal bundle 对应 128 crates、Rust 1.85.0 标准库 / runtime 和两个 .NET 8.0.12 runtime packs；本轮未重新打包。

无受跟踪的自定义二进制、图片、字体或媒体资产；原生只引用系统字体 / SF Symbols，Swift 无远程依赖、WPF 无 PackageReference。项目声明参考架构独立实现、未复制参考项目源码。本次没有检查他人的私有源码或所有权协议，不能单凭改邮箱 / 模式扫描证明完整权属；也没有发现具体相反证据或需要新增授权的资产。

## GitHub 与 CI

基线 API 回读：仓库仍 private、个人所有、当前连接有 admin 权限、未 archived / disabled；仅 main、无 tags / forks / releases / Actions artifacts。Actions enabled=true，默认 workflow token 为 read、不能批准 PR；仓库 Actions secrets / variables / environments 均为 0。当前 workflow 固定 action SHA、只读 token、无秘密变量引用，外部 PR 不上传制品，不使用 self-hosted runner。

基线全部 20 个 workflow runs / 60 个 jobs 的 steps 都为 0。50 个失败 job 的 annotations 均指向账户级托管运行条件，其余为 skipped；没有测试 / 构建 / 上传实际执行证据。annotations 的已知 token / 私钥和本机路径模式为 0 命中，但包含账户运行背景。最新基线 run 为 40495620077。

一次路径模式初筛把 HTTPS URL 当作盘符路径，已用边界反例纠正并重新核验全部失败 annotations；该误报不作为隐私发现记录。只保存忽略入库的分类统计，不提交原 annotations 或原始日志。

公开后 Actions 历史 / logs 也会公开，所以应先决定这些无执行的旧记录是否保留；本轮只审核，不删除。[GitHub 可见性说明](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility)

标准 GitHub-hosted runners 在公开仓库的计算使用免费；制品 / 缓存存储等仍有适用规则。可在公开后实际触发三平台流程验证，不能保证现有账户限制仅靠改可见性就解除；若仍被拦截再处理账户条件。macOS 15 runner、Rust 静态链接与本机路径门禁尤其仍待实际构建。[GitHub Actions 计费说明](https://docs.github.com/en/billing/concepts/product-billing/github-actions)

## 开发预览阶段可保留的限制

- Quest v3 集成与真实 LAN / 播放 / 镜像 / 同步未验收，游目端仓库仍私有。桌面源码、协议与 Rust 测试桩可独立使用，不要求同时公开游目；必须保留当前产品边界。
- Windows 安装器 / 正式签名、macOS 公证 / Intel / 旧系统测试未完成，影响正式发行，不阻止源码预览公开。
- 仓库暂无 SECURITY / CONTRIBUTING 文件，建议补一个私下漏洞报告渠道及简短贡献说明；属于项目维护准备。当前 private-vulnerability-reporting API 返回 404，不能当作已启用；该功能供公开仓库启用，公开后应回读确认。[私下漏洞报告说明](https://docs.github.com/en/code-security/how-tos/report-and-fix-vulnerabilities/configure-vulnerability-reporting/configure-for-a-repository)
- 根 README 主要为中文，UI 已有英文 / 简体中文资源；英文 README 可以后补，不是现有源码许可或构建的硬阻碍。

## 建议执行顺序

1. 决定历史演示 / 内部 / 账户背景的公开范围；需要去除时在备份后做定点历史清理，保持已统一的邮箱，复扫全部 refs 和提交 message，并更新 SHA 映射。
2. 决定旧 Actions 记录是否保留；若清理，先留下必要脱敏证据。此前改写产生的旧 commit 直链 / cache 与已有副本也应单独核对，不能把新 main 干净等同为远端旧对象全被删除。
3. 明确转 public 后更新仓库状态 / 安全报告入口，实际运行三平台 CI、检查包内容与校验和。失败按实际诊断处理，不把配置视为通过；不自动发布正式 Release。

本轮实际 `harness verify` 通过：Python 36 passed / 1 Unix-mode skipped，Rust 76 passed / 2 ignored，记录 / 链接与 rustfmt 通过。仅注释 / 文档修正，不重建分发包；真实设备、macOS 原生构建与云端运行保持 not-run。

最终独立复核：`public_history_readiness` 核对历史分类 / 计数、当前名称泛化和副本边界，`public_license_readiness` 核对许可结论、公开 / 发行边界与文档修正，两者均无新增阻断问题。`harness check` / `git diff --check` 通过；当前更改仅文档及一条代码注释。
