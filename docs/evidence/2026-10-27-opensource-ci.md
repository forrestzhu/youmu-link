# 开源准备、历史身份与 GitHub CI

日期：2026-10-27。任务：[YML-004](../tasks/YML-004-opensource-ci.md)。基线：`fb310450d50a4af86a0818a3c66ba9fb6bb72b56`。

## 历史与公开范围

本地非浅克隆，fetch main / tags 后远端仅 main、无 tags，所有本地 refs 共 16 个历史提交。author 与 committer 分别统计，均为 6 个 `jayzero@foxmail.com`、10 个其他同一公司邮箱。当前 Git user.email 与最近两个 Windows 提交均为目标邮箱。

需处理的旧提交为 `3a81ead`、`d9130b8`、`8e41ea0`、`057ae81`、`092e514`、`b5260fe`、`3b6329b`、`7614ae3`、`ba65b72`、`712cabd`。完整原始邮箱未另复制到报告。mailmap 只影响部分展示，不会删除原提交邮箱。本次未改写历史、未 force push。

独立只读审计遍历 214 个 blob / 2,194,843 bytes，均为文本；未发现常见私钥 / 访问 token / 带凭据 URL 模式、SDK、缓存、二进制产物或原始日志。模式扫描不保证不存在所有敏感信息。当前 demo.js 中一处看似个人目录的示例路径已泛化；该路径仍存在于 3 个历史版本。内部参考项目名称出现在 11 个路径 / 40 个历史 blob，文字说明为架构借鉴、独立实现，没有发现显然复制的外部源码。是否保留公开名称由公开前内容审阅决定。

## 许可证与依赖

按用户提出的方向准备 Apache-2.0 原文、NOTICE、workspace 元数据与第三方说明；仓库仍 private。Apache-2.0 允许商用和闭源衍生，保留原许可与相关声明，并含贡献者范围的专利授权：[许可证原文](https://www.apache.org/licenses/LICENSE-2.0)。

`cargo metadata --locked --offline` 审计 128 个外部 crates，均声明宽松许可；未发现 GPL / LGPL / MPL 或缺失文本的外部包。保留 BSD-3-Clause、LLVM exception、Unicode-3.0 等具体许可，打包不将它们重新声明为 Apache。Swift 无远程包，WPF 无 PackageReference。Windows 自包含 .NET Core / Windows Desktop 运行时另收集实际恢复版本的 MIT 与 third-party notices。

## GitHub 现状与验收

Actions enabled=true，现有 workflow state=active。最近旧 run [40474543136](https://github.com/forrestzhu/youmu-link/actions/runs/40474543136) 三项 jobs 的 steps=[]，failure annotation 明确表示近期账户付款失败或 spending limit 需调整；没有实际执行测试。该状态与 YAML 里的测试失败分开记录，不更改账户付款设置。

公开仓库使用标准 GitHub-hosted runners 的计算免费，但存储 / 制品等规则仍按账户适用政策：[GitHub Actions 计费](https://docs.github.com/en/billing/concepts/product-billing/github-actions)。转 public 会公开代码、历史与 Actions 日志：[可见性影响](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/managing-repository-settings/setting-repository-visibility)。本次不因计费阻塞擅自公开。

## 实现与本机验证

- Actions 固定完整 SHA，Rust 固定为仓库的 1.85.0，Linux / Windows / macOS 测试；仅 main push / main 手动触发上传开发包，PR 只测试打包不上传。只读 token、checkout 不保存凭据、7 天制品保留，不自动发布 Release。
- Windows 包验证自包含运行时必需文件；macOS 显式链接 Rust `.a` 并移除 modulemap 的动态 autolink，防止 `cdylib` 加入后优先链接工作区 dylib；`otool` 拒绝非系统 / 非 Swift runtime 依赖。macOS 构建与 Unix 模式 ZIP 实测本轮 not-run（Windows 主机），不能因配置成功宣称已通过。
- `python3 -m unittest discover -s scripts -p test_*.py`：29 项，28 passed / 1 Unix-mode skipped。新增收集 / 打包测试先见缺实现 RED 后 GREEN；harness verify 现汇入全部 scripts unittest。Rust 全门禁 76 passed / 2 ignored；原生 Windows Release / 真实 ABI / 数据行 / 中英资源 / bounded smoke 均 exit 0。
- `collect_licenses.py` 实际收集 128 个 Cargo 包、Rust 1.85.0 标准库 / 运行时许可证和 2 个实际恢复版本的 .NET runtime packs。Rust 安装组件本身包含 LICENSE-APACHE、LICENSE-MIT、COPYRIGHT，minimal 工具链无需安装 rust-docs；原文按字节保留，生成索引不记录本机路径。
- Windows 实际 ZIP 生成成功，独立回读通过全包 CRC、SHA-256、应用 / runtime 必需文件、项目 LICENSE、128 crates / Rust runtime / .NET 索引与 notices；没有把 SDK、缓存、工作区状态或原始路径日志加入包。
- 官方 actionlint 1.7.12 校验 workflow exit 0；工具 ZIP 对照官方 SHA-256 校验，仅放忽略入库的 dist/tools。`git diff --check`、仓库门禁通过。

## 独立审查

基线 `fb310450`。第 1 轮 `ci_review1` 覆盖全部实现，确认 P2：收集器接受重定向至仓库的 dist，可清理非 dist 目录。在完整隔离的 Windows junction fixture 先 RED，再要求 resolved dist 等于字面 workspace/dist，修改 / 删除前拒绝；保护原文件的回归 GREEN。

第 2 轮 `ci_review2` 受审 staged tree `cb0e7d31dfb1a0cf92f1678c2e6a16653be0b8f6`，复核全量并确认两项 P2：

1. 打包器仅检查 is_symlink，不识别 Windows junction，可带入 build / legal 目录外的文件。两个入口实际隔离 junction fixture 都先 RED；改成迭代遍历，在下降 / 读文件前检查 reparse、symlink 和实际包含边界，也避免环路预遍历，回归 GREEN。
2. Cargo 图外静态嵌入的 Rust 标准库 / 运行时许可未随包。增加精确 sysroot 的原文、版本索引与缺文件拒绝回归，GREEN。

第 2 轮 reviewer 独立回跑 29 项 Python 测试、检查实际 legal bundle 与两个定点修复，判定全部关闭，无剩余确定问题。审查不代替 macOS 或 GitHub runner 实际执行；新 run 结果提交后补齐。
