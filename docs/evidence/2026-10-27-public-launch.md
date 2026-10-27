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

## SDK 修复后的真实回归

修复提交 `0b29d8f30e1ece0d895549972415d32d06c7e7c6` 触发 [run 40506880396](https://github.com/forrestzhu/youmu-link/actions/runs/40506880396)。Linux 通过，macOS 使用 Xcode 26.3 / 新 SDK 越过 apple-metal 编译；随后 core 的 `beacon_ports_fall_back_in_order` 在 `rescan` 返回 No route to host (65) 而失败。它不是原 SDK 错误的重复。

根因是 rescan 发送循环的 `await?`：第一个不可路由广播立即结束扫描，后面的有效接口 / loopback 没有机会探测。修复为尽力遍历所有目标 × 三端口，任何一次 Query 成功则返回成功，全部失败仍返回 socket 错误；单次 `send_query`、协议字段和公共配置不变。PROTOCOL 仅补充本地发送错误处理说明，不改变线版本。

确定性回归使用真实 UDP socket 并关闭 SO_BROADCAST：广播发送保证失败，之后的 localhost 三端口必须各收到有效 Query，尾部再次失败不覆盖已有成功；另验证全部失败仍报错。旧提前返回实现 RED 为 1 passed / 1 failed（本机错误 10013），修复后新增 2/2 GREEN，core 全目标 46 passed。未修改网络、系统权限或录屏设置；Mac 结果仍需新 run 验证。

网络修复两轮独立 CR：`public_ci_audit` 与 `public_docs` 先后复核 discovery blob `08472e5b42d3cdb3da0a5237740db647cbfe9973`，第二轮同时核对 PROTOCOL blob `b46e455cee3cbb0d70c84acdad779b7f995c7209`，均无确定问题。根代理实际完整 `harness verify` 为 Python 36 passed / 1 Unix-mode skipped、Rust 78 passed / 2 ignored，仓库检查与 rustfmt 通过；macOS 第三次云端结果待实跑。

## 首个 Windows 云端制品独立下载核验

`public_artifact_verify` 完整下载首个 artifact 后核对外层长度 / GitHub digest、内部 ZIP SHA-256 / checksum 文件名及内外两层 CRC，均通过。内层 ZIP SHA-256 为 `e7557e14e563e89199e85b8e21db1c47d74f41d4a69e9e3980e5e3162c769d33`；共 747 文件，BUILD-INFO revision / windows-x64 正确。278 份索引许可存在，128 Rust 包、Rust 1.85.0 标准库和两套 .NET 8.0.32 与实际 runtimeconfig 匹配。

状态 / 密钥 / PDB / 日志 / 媒体文件 0，已知 token / 私钥签名 0；三个自有 PE 对 CI home / workspace 前缀的 UTF-8 / UTF-16LE/BE 扫描均 0，Rust DLL 有 173 处合成路径映射。下载和 JSON 报告仅留在忽略目录，不提交原始日志、签名下载 URL 或构建产物。此结果仅证明首次受审 revision，不冒充后续修复包结果；含 rescan 修复的新 Windows 包另行核验。

## 网络修复后的 Mac 进展

`dd40c6c8cc29b78a532a6ca401e74c234786b41c` 的 [run 40509363905](https://github.com/forrestzhu/youmu-link/actions/runs/40509363905) 已通过 Linux / Windows，Mac Rust 测试、原生应用构建、静态链接与 ad-hoc 签名也通过。随后 Swift 测试有 6 项简体中文断言返回英文，本地化用例失败，打包 / 上传尚未执行。

根因经官方源码确认：SwiftPM 6.2.4 将非 Base locale 转小写，产物为 zh-hans.lproj；原 loader 精确查询 zh-Hans，未找到时转用主机默认偏好，英文 CI 主机因此全部回英文。修复需按实际目录匹配显式语言，不依赖主机语言，并解决同一 Bundle.module 加载器的绝对构建目录 fallback；不能通过改测试期望 / 主机语言或关闭路径门禁绕过。[SwiftPM 资源规范化源码](https://raw.githubusercontent.com/swiftlang/swift-package-manager/swift-6.2.4-RELEASE/Sources/PackageModel/Resource.swift)

## 原生资源修复与待实跑验证

Mac loader 已改为相对 app Resources / host bundle / XCTest anchor 目录查找资源 bundle，不引用含绝对构建目录 fallback 的 Bundle.module。按实际 `.lproj` 名称建立小写索引，显式语言独立于主机偏好，system 才按系统偏好选择。新增大小写 / 主机偏好 / system / 未知 key 与资源随 `.app` 移位的回归；已有 6 项错误断言保留作为真实 RED。

原生入口的 `--verify-localizations` 在创建 DesktopModel 前校验当前 `.app/Contents/Resources` 的实际 bundle 和 en / zh-Hans 文案，然后退出，不创建后端、窗口或采集权限流程。构建器使用系统 strip -S 清除调试记录后签名 / 校验，再复制到唯一临时目录以 30 秒超时运行该模式；它不以二进制字节替换掩盖运行时路径，也不关闭包隐私门禁。

两轮独立 `public_ci_audit` / `public_docs` 静态复核无确定阻断，根代理采纳路径规范化与直接输出安全自检结果的建议。第二轮受审 blob：Models `55055d874ee04c02128012bbf1969ca449275b7e`、App `6e13bdacbf654f8f19893642c00d3df9eb0b7202`、Tests `23575cf622031d64f865ae2b03ce2f4399732426`、build `abe342f7bbb34a668314683c0ab60c1b62d9633e`。本机完整门禁为 Python 36 passed / 1 skipped、Rust 78 passed / 2 ignored；Swift / 移位应用启动 / Mach-O 路径门禁尚待第四跑实证。

## 含网络修复的 Windows 包核验

第三跑 Windows artifact `12737465070` 的外层长度 71,886,420 bytes、GitHub digest `d5a94ec5c3e16467d5df5d52d5b52d15bbc8fa0426e73b96a4ae9b724c8eb51a`、内层 ZIP digest `d5b42fdf9efbeef32990dc3bba80900af3237de199215938eca636faefafb336` 及内外 CRC 全部通过。747 成员、BUILD-INFO revision `dd40c6c8cc29b78a532a6ca401e74c234786b41c` 正确，278 许可文本 / Cargo.lock 精确依赖清单 / 两套 .NET 8.0.32 匹配。

状态 / key / PDB / log / 媒体与已知密钥签名 0；当前项目 CI / home 前缀在自有 PE 中为 0。泛化路径规则的一条候选经 PE Debug Directory / CodeView 结构确认属于微软 apphost.pdb 上游构建元数据，且与首包及官方模板相同，未归类为本项目个人信息泄漏；原路径未复制到报告。本次为静态包核验，运行证据来自对应 Windows job。
