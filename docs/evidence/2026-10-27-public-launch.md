# Apache-2.0 公开开发预览与首次云端执行

日期：2026-10-27。任务：[YML-004](../tasks/YML-004-opensource-ci.md)。公开基线：`7d324b572cd0c568db7d0f7c162f7e6e7374f9a7`。

## 用户授权与公开回读

用户在历史个人标识、当前邮箱及剩余公开范围说明后明确要求“开源吧”。本轮将现有已审计仓库设为 public，没有追加历史正文改写、删除旧 Actions 记录或变更游目端仓库可见性；本机备份 / SDK / 缓存仍为忽略项，不上传。源码公开为开发预览，不自动发布正式 GitHub Release、不更改账户计费设置。

GitHub API 回读仓库 `forrestzhu/youmu-link` 为 PUBLIC，licenseInfo 为 Apache License 2.0。通过未携带凭据的 HTTP 请求，仓库 API 与 raw LICENSE 均实际返回 200，private=false，Apache-2.0 原文可公开获取。

已启用 GitHub 私下漏洞报告，`PUT /private-vulnerability-reporting` 后 GET 回读 enabled=true。新增中英 SECURITY / CONTRIBUTING，并同步 AGENTS / README / 产品定义 / ROADMAP 的公开开发预览状态；不承诺尚未验收的 Quest、系统兼容性、签名 / 公证或支持时限。

## 首次公开后 CI

手动触发 main 的 [run 40505699128](https://github.com/forrestzhu/youmu-link/actions/runs/40505699128)，受测提交为公开基线。此次 runner 实际执行，之前 private 仓库的账户级阻塞未阻止本次运行；不以此概括账户整体计费状态。

| Job | 实际结果 |
| --- | --- |
| checks / Ubuntu 24.04 | Python 回归、仓库检查、rustfmt 与 Rust 测试全部通过。 |
| windows / Windows 2025 x64 | Rust、WPF / 本地化 / 真实 DLL ABI、Release、startup / snapshot / shutdown smoke、精确许可收集、ZIP / SHA-256 和上传全部通过。 |
| macos / macOS 15 arm64 | 架构断言通过；首次 Rust 构建在 apple-metal 0.10.3 的 Swift bridge 编译失败，后续应用构建 / 上传未执行。 |

Windows artifact 为 `YM_Link-windows-x64-7d324b572cd0c568db7d0f7c162f7e6e7374f9a7`，id `12734757832`，外层 transport ZIP 为 71,886,368 bytes，GitHub 摘要 `474ac4b97753cd64d7ac2ae7ca872d2b5b7145e027327e10866faecd12fc61dc`。该摘要属于外层 artifact，不冒充内部开发 ZIP 的 checksum；本地独立内容核验另行记录。

## macOS SDK 根因与修复

实际失败为 Swift 找不到 MTLTensorDataType / Extents / Descriptor / Usage、makeTensor、MTLSamplerReductionMode 和 lodBias。锁定 apple-metal 的 Swift bridge 引用了 macOS 26 才引入的 API，运行时 availability 不能让旧 SDK 解析这些类型；镜像默认 Xcode 16.4 只有 macOS 15.5 SDK。[Apple API 文档](https://developer.apple.com/documentation/metal/mtltensordatatype)

同一 macos-15 arm64 官方镜像已安装 Xcode 26.3 / macOS 26.2 SDK。因此仅在 macos job 层设置 `DEVELOPER_DIR=/Applications/Xcode_26.3.app/Contents/Developer`，覆盖首次 cargo test 与所有后续 Swift / Rust 构建，并在测试前记录 Xcode / SDK / Swift 版本；不更换 runner、不升级依赖、不改变 macOS 14 部署目标。编译工具链选择不等于旧系统或实际采集验收。[官方镜像清单](https://raw.githubusercontent.com/actions/runner-images/main/images/macos/macos-15-arm64-Readme.md)

本次失败为真实 RED，修复后的 GREEN 以新 workflow 实跑结果为准；本地 Windows 无法代替 Mac 验证。`actionlint` 与 `harness check` 已通过。首轮独立 `public_ci_audit` 复核 job 级环境、编译 / 链接工具链一致性和根因，受审 workflow blob 为 `562531b0588da9a155ce89b969af1e0d56d0e404`，无确定问题；第二轮独立 `public_docs` 复核环境作用域、原生构建继承、权限 / 上传条件及开发预览文档，actionlint / check / diff 均通过、无确定问题；受审 workflow blob 相同。后续实跑结果追加记录。

## 仍未覆盖

真实 Quest v3、双机 LAN、录屏 / 系统音频、延迟 / 弱网 / 音画同步仍待设备验收。Windows / macOS 开发 ZIP 的成功不代表正式签名、安装器、公证、Intel 或所有旧系统兼容性通过。旧历史个人演示 / 内部背景和远端旧记录仍按公开时状态保留，不宣称所有外部副本已清理。
