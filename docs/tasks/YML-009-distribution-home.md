# YML-009：英文功能首页、多平台安装包与官网下载衔接

Status: verifying
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (不修改 Rust / 协议；打包工具由 Python 回归及原生平台 CI 安装 / 挂载 smoke 覆盖)

## Scope

将 GitHub 首页改为英文优先、面向功能和下载，附连接示意图及独立中文页；构建 / 测试细节移入开发指南。现有 CI 增加 Windows x64 安装程序与 macOS arm64 DMG，保留 ZIP、许可与 SHA-256。官网侧由 [YM-308](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-308-ym-link-download.md) 跟踪。

用户要求官网首页透出 YM_Link 下载入口。公开 Release 发布方式已向用户询问，未获答复前只生成制品，不自动发布。无应用 / 协议 / Android 功能修改，不将打包视为设备体验验收。

## Acceptance

- [ ] README 英文优先、简洁、有图、功能与预览边界和下载入口清晰；中文与开发文档可达。
- [ ] CI 生成 Windows x64 安装程序与 Apple Silicon DMG，附完整许可和校验和；原生打包及安装 / 挂载验证有实际结果。
- [ ] 官网首页与下载页互链，下载状态与实际发布一致，双方任务互链。
- [ ] 适用回归、自查 / 独立审查、verify 与 main 推送门禁有记录。

## Verification

Python 打包回归；macOS 实际 DMG 构建 / 挂载 / 签名 / 资源验证；云端 Windows 安装 / 卸载 smoke；`python3 scripts/harness.py verify`；自查链接和首页结构；官网验收见 YM-308。未签名 / 未公证及未测平台明确保留。

## Evidence

开工干净 main，fetch / ff-only 同步至 af57f7a。现有 CI 只有双平台 ZIP，7 天 artifacts；无公开安装器。45 项 Python 回归及 repository / rustfmt 通过。本机 verify 的 Rust 阶段在固定 UDP 47610 绑定失败（用户已运行的 YM_Link 占用）；未停止该应用，Rust 完整结果交由干净云端执行。16 项打包回归通过；macOS 26.3.1 arm64 实际 DMG 压缩 / 挂载 / 签名 / 128 项 Rust 许可 / 移位中英资源通过。安装器独立只读 CR 未发现必须修复项；Windows 安装 / 卸载仍待云 CI。

## Next

完成打包回归与官网实现，核对用户发布偏好，执行真实 CI 和对应下载验证后交付。
