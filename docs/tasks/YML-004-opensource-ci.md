# YML-004：Apache-2.0 开源准备、历史身份审计与 GitHub 自动测试打包

Status: verifying
Workflow: maintenance
Requires-Device: no
Tests: python3 -m unittest discover -s scripts -p test_*.py; harness verify; Windows build / legal bundle / package checks; GitHub Actions run diagnostics

## Scope

按用户要求准备 Apache-2.0 开源，核对全部历史 author / committer 邮箱，补齐 GitHub Actions 自动测试与 Windows / macOS 开发包。采用现有原生构建，不引入安装器 / 发布服务。仓库转 public、已推送历史改写、GitHub Release 公开发布及账户付款设置不在本轮自动执行范围。

## Spec

在 main push / PR / 手动触发时执行仓库门禁与三平台 Rust 测试，Windows 补 .NET / ABI / 启动，macOS 补 Swift / 静态链接 / app 构建。受信任 main 构建上传带 SHA / 版本、SHA-256 及第三方许可的开发 ZIP，PR 不上传制品、不接触签名密钥、不自动发布 Release。流程与开源边界见 [实施计划](../plans/2026-10-27-opensource-ci.md)。协议不变。

## Acceptance

- [x] 历史邮箱 / 隐私 / 依赖许可审计有证据，准备 Apache-2.0 文件与元数据；保留已推送历史。
- [x] CI 覆盖 Linux / Windows / macOS 测试与 Windows / macOS 开发包，使用只读权限及固定 action SHA。
- [x] 包内包含项目与第三方许可证、正确原生制品和校验和；本机可运行的验证通过。
- [ ] GitHub runner 实际执行测试 / 构建 / 上传成功；账户阻塞时保留 not-run 与 verifying。
- [ ] commit / push main，check --require-pushed 通过。

## Verification

新增许可证收集与打包 stdlib 测试先 RED 后 GREEN；`harness verify` 汇入 scripts 的全部 unittest。Windows 实际 build / legal collect / ZIP / checksum 验证；macOS 构建仅由对应主机 / runner 验证。GitHub 通过 API 回读 Actions 开关、jobs 和 annotations；不把已配置 YAML 当作云端执行成功。

## Evidence

基线 `fb31045` main 干净，fetch / ff-only 无新增。当前 GitHub 仓库 private，Actions enabled=true，workflow active。最近 run 40474543136 三项 job 均未启动，annotation 明确为账户付款失败 / spending limit；未改账户设置。16 个历史提交：6 个 author / committer 为目标邮箱，10 个为同一其他邮箱；当前本地配置正确。完整审计和最终验证追加于 [开源准备报告](../evidence/2026-10-27-opensource-ci.md)。

两轮独立 CR 的三项 P2 均经隔离 fixture RED / GREEN 修复与复核。最终本机 Python 29 项中 28 passed / 1 Unix-mode skipped，Rust 76 passed / 2 ignored；Windows 真实 Release / smoke、legal collect、ZIP / SHA-256 独立回读和 actionlint 通过。许可包保留 128 个 Cargo 包、Rust 1.85.0 与两个实际版本 .NET runtime 原文；未把开发工具或本机路径加入制品。

## Next

完成提交推送并回读新 run。真实 runner 通过需要解除账户阻塞，或用户批准公开后再实跑验证；不保证可见性变更自动解除所有账户限制。开源前由用户决定是否统一历史邮箱、清理历史示例路径与公开仓库；本次只泛化 HEAD 的示例目录，不重写历史。
