# 首页与多平台安装包（2026-10-30）

任务：[YML-009](../tasks/YML-009-distribution-home.md)。官网实现由 [YM-308](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-308-ym-link-download.md) 跟踪。

## 实现与边界

GitHub README 改为英文优先：功能介绍、独立连接示意图、三步上手、下载入口；中文独立，构建 / 测试桩 / 工程细节移入 [开发指南](../engineering/DEVELOPMENT.md)。官网尚未部署，README 当前下载指向真实可用的[下载指南](../engineering/DISTRIBUTION.md)，不暴露 404 官网地址作为主入口；部署后再切换。

CI 保留 Windows x64 / macOS arm64 ZIP，新增用户级 Windows EXE 安装器、拖入 Applications 的 macOS DMG。完整许可 / 版本 / 源码 SHA / SHA-256 随包保留；DMG 将法律文件放进 App 后重新 ad-hoc 签名。Windows 不提权，不改防火墙，卸载保留应用设置与配对。

## 验证与审查

- 完整 Python 45 passed，定向打包 16 passed，仓库 / rustfmt 通过。
- 本机 `harness verify` 的 Rust 阶段：17 passed / 1 failed 后停止；原因是固定 UDP 47610 被用户已打开的 YM_Link 占用，未关闭用户应用。云端干净环境的完整 Rust 测试通过，不删除本次失败记录。
- 本机 macOS 26.3.1 arm64：实际 DMG 创建 / 压缩 / 挂载 / 签名 / Applications 链接 / 中英资源移位验证通过。初次本机基于已有 app 的包只作为打包验证，不作为本轮源码候选对外分发。
- 独立只读安装器 CR：覆盖打包 / Inno / CI / 回归，未发现必须修复项；独立 16 项回归通过。
- 实际源码候选 `62cf58ca4b0f79da7098b90fdb23c7760f3cbc55` 的 [run41002505749](https://github.com/forrestzhu/youmu-link/actions/runs/41002505749)：Linux / Windows / macOS 全 success。Windows 真实安装、全 payload 字节比对、生产后端启动 / 关闭与卸载通过；macOS DMG 创建 / verify / 只读挂载 / strict signature / 移位资源通过。
- 云端 Windows Server 2025 Datacenter `10.0.26100`，macOS `15.8.2 / 24H232 arm64`、Xcode 26.3；与用户机器或旧系统兼容性验收分开。

## 云端制品独立下载核验

两个真实 artifact 均独立下载，外层 SHA-256 与 GitHub API digest 一致；下列4个制品的 sidecar 全部匹配。Windows ZIP 747 成员 / 279 许可文件、Mac ZIP 288 成员 / 276 许可文件；版本0.1.0、完整 revision62cf 一致。

| 文件 | 字节 | SHA-256 |
| --- | ---: | --- |
| `YM_Link-0.1.0-windows-x64-62cf58ca4b0f-setup.exe` | 52,047,946 | `153f98af6ca1325c08e84bd3cb3009205ba6c38c133cc5812c1258908ad3adb0` |
| `YM_Link-0.1.0-windows-x64-62cf58ca4b0f.zip` | 71,976,810 | `d18fee13432f4cc71d98a7a573434c494e60ae33bd1f4734a292463ef793fca1` |
| `YM_Link-0.1.0-macos-arm64-62cf58ca4b0f.dmg` | 3,920,910 | `1e205624d1b0658f9626aa19cf2253654204d4c58fd7e904cb709cd40a369192` |
| `YM_Link-0.1.0-macos-arm64-62cf58ca4b0f.zip` | 2,483,621 | `8607035ad333e0e775c70f0f183073753fa178548634cc5df3c79bb3d29601f2` |

下载后的 Mac DMG 再次执行 `hdiutil verify`、只读挂载、arm64 / strict codesign / ad-hoc / Applications 链接核验；280 份法律 / 元数据与 ZIP 和 App 内嵌副本逐字节一致，已卸载挂载。Windows 生产 apphost / Rust DLL 为 x64，安装器 PE32 是 Inno 的引导程序，ProductVersion0.1.0、Authenticode 证书表为空。本机未模拟 Windows 安装，以云端实际 smoke 日志为依据。

## 未完成项

2026-10-30 用户明确选择“手动发布开发预览”，此前发布方式阻塞解除。CI artifacts 仍保留 7 天且需要 GitHub 登录；本次另以手动公开 prerelease 提供无需登录的下载。安装器未正式签名，macOS 未公证；真实 Quest 播放 / 录屏 / 声音与旧目标 OS 验收继续按原任务，不能以打包或 CI 代签。

官网三语言首页 / 下载页已在游目仓库实现并经14回归与两轮独立CR；尚未上线，当前 Mac 没有腾讯云部署访问，已向用户询问已有授权访问位置，不请求在聊天传密钥。用户已授权通过 UU 远程检查 Windows 上的现有访问；官网上线和实际 HTTPS 回读继续由 YM-308 推进，任务保留 verifying。

## 手动公开预览与英文公共页面（追加授权）

用户于 2026-10-30 选择手动发布开发预览，并要求贡献指南和安全政策也以英文优先。`20ba100` 已将根目录 CONTRIBUTING / SECURITY 改为英文主文，中文分别移入 `docs/CONTRIBUTING.zh-CN.md` 和 `docs/SECURITY.zh-CN.md`；保留原安全支持边界、私下漏洞入口与许可约定，修正开发指南相对链接。只改文档，不改安全政策或应用行为。

已手动发布 [v0.1.0-preview.20261030.1](https://github.com/forrestzhu/youmu-link/releases/tag/v0.1.0-preview.20261030.1)，Release ID `428704324`，发布时间北京时间 2026-10-30 14:20:14。先以 draft 上传，再核对 8 个文件的大小 / 完整 SHA-256 与已验证 CI 输出一致后公开；`draft=false`、`prerelease=true`、`latest=false`。公开标签精确指向已验源码 `62cf58ca4b0f79da7098b90fdb23c7760f3cbc55`，不是之后的文档提交，不重编译任何制品；没有新增 main 自动发版工作流。

公开资产为上表 4 个 EXE / DMG / ZIP 和各自 `.sha256`。英文 Release notes 明确未签名 / 未公证、真实硬件与系统兼容性边界，并明确要求头显侧是包含 YM_Link application protocol v4 / 四位配对的兼容 Youmu 构建；当前 Meta Store 已发布构建的兼容性未另验证，不能假定可用。

实际回读：

- 已认证 GitHub API：8 个上传资产的服务端 digest / 大小与本地已验证文件全部一致；tag object 为 commit，SHA 精确为62cf。
- 匿名 Release 页面 GET 200，包含正确版本与标题。
- 4 个公开二进制 URL 的匿名 Range GET 206，返回前 4096 字节逐一与本地文件一致。
- 4 个公开 `.sha256` 匿名 GET 200，完整正文与本地 sidecar 一致；下载不需要 GitHub 登录。
- 匿名 API 元数据请求曾收到共享出口 `403 rate limit exceeded`；这不是发布权限或下载失败，未改变任何计费 / 权限设置。随后匿名页面与全部 8 个实际下载链接均通过；认证 API 全文件 digest 验证与匿名下载可达性分别记录。
- 本轮纯文档 `harness check` 72 文本与 `git diff --check` 通过；完整 native / Rust 证据沿用上面已验源码与真实 CI，不重复构建。
