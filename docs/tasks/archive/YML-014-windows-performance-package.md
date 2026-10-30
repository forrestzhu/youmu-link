# YML-014：Windows 最新安装包与性能验收准备

Status: done
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (仅按现有流程构建安装包，无产品代码改动；执行 Windows Release 原生模型 / ABI / 启动检查、许可 / 包载荷与仓库检查)

## Scope

用户在此前 macOS 空闲 CPU 修复后要求先打安装包，待其安装后一起检查性能。本轮在当前 Windows 11 x64 主机构建最新 main 的 Release / 备用 ZIP；安装器复用已通过 CI、与当前 main 产品及构建源码完全一致的输出，保留其真实源码提交号、完整许可及校验和。不自动安装到用户正式位置，不创建公开 Release。macOS 修复与 Windows 的适用范围由只读独立审查核对，Windows 用户安装后的实际性能另验。

## Acceptance

- [x] 最新 main 的 Windows x64 Release 自包含应用与原生 smoke 通过。
- [x] EXE 安装器 / ZIP / SHA-256 已准备，独立核验元数据与完整许可。
- [x] 记录性能修复适用范围、交付路径与后续实际性能验收入口。
- [x] 文档检查与 diff 自查通过；提交 / 推送及严格门禁按仓库交付流程执行。

## Verification

执行 `python3 scripts/build_windows.py --smoke-test`，再按 DISTRIBUTION 收集精确依赖许可并打包备用 ZIP。本机没有 Inno Setup 6，下载并运行便携编译器的命令被自动审批以 `blocked by policy` 拒绝，未执行该命令；改为获取已通过 CI 的安装器，并以 Git diff 核对产品、依赖、脚本、资源及工作流与 main 一致。独立验证 ZIP 的 CRC / BUILD-INFO / 应用与后端二进制、安装器 / ZIP 校验和及图标。只改任务文档，按约定运行 `harness check`；完整 Rust 回归不因纯打包重复运行。本轮未在当前用户执行安装 / 卸载 smoke，保留用户自行安装；该检查已有对应 CI 实际通过记录。真实 Quest / 镜像 / 文件传输性能 not-run。

## Evidence

开工 main 干净，fetch + ff-only 无新增；源码基线 `1671533068f6b08fc10dbe33333efe252addfb98`。主机 Windows 11 专业版 x64 build 26200。开工时未运行 YM_Link，也未检测到当前用户的预览安装记录。

- `python3 scripts/build_windows.py --smoke-test` 退出 0：优化 Rust DLL、WPF 模型 / 本地化 / 真实行模板、真实 DLL 生命周期及排队 / 销毁、win-x64 自包含 Release、应用图标与真实后端启动 / 快照 / 关闭检查通过。没有运行正常交互窗口或采集。
- `collect_licenses.py --output dist/legal --windows-assets windows/YM_Link/obj/project.assets.json` 退出 0：128 个 Rust 依赖、Rust 1.85.0 运行时、2 个 .NET 运行时包许可已收集。
- `package_build.py --platform windows-x64` 退出 0：生成 `dist/packages/YM_Link-0.1.0-windows-x64-1671533068f6.zip`（72,033,664 bytes）及校验和。独立回读 SHA-256 为 `8d4f66fc5664af643956896703a1dc8f4eba4c1b8142b04c1e8f0e64e5183cac`，747 个成员 / 279 个 license 成员、CRC、完整源码 SHA 与平台、EXE / DLL / 自包含框架 / 许可检查通过。首次临时校验脚本用了 Python 3.11 才提供的 `hashlib.file_digest`，当前 Python 3.10 报 AttributeError；改用兼容的 SHA-256 读取后同一包校验通过，无产品或打包失败。
- 安装器来源：[run 41085401188](https://github.com/forrestzhu/youmu-link/actions/runs/41085401188)，源码 `5e85961b4f385209b33f99481e5daf986f418970`。GitHub API 回读 Linux / Windows / macOS 全 success，Windows 原生构建、安装 / 完整载荷 / 启动 / 卸载步骤 success。该提交到当前 main 仅三份文档变化；对 Cargo 清单 / 锁、core、desktop、windows、macos、scripts、assets、工作流的精确双 SHA `git diff --quiet` 退出 0，不将旧 SHA 伪装为当前 SHA。独立只读复核同样确认源码等价；两次构建不声称二进制逐字节一致。
- GitHub 插件回读 artifact `12991612047`，首次本机 `gh run download` 连接停滞，停止本任务的该重复下载进程；改用插件返回的文件下载地址持久化到本机。外层 ZIP 的 124,111,165 bytes / SHA-256 `c6ed61a303df1ab0accc23bcd9436852f1f1fe62f774dea5f659dd7f55e088de` 与 GitHub artifact digest 一致；CRC 与四个安全成员名检查通过后解压到 `dist/packages/`。
- 实际 EXE：`dist/packages/YM_Link-0.1.0-windows-x64-5e85961b4f38-setup.exe`，52,071,069 bytes，SHA-256 `c3984b601c560eaf0021e76902b5857231d0db5529d0d22babfb2b376ab8a15b`；与 CI 随附 `.exe.sha256` 文件及文件名完全一致。PE 文件头 / 正式链环 ICO 检查通过。未签名开发包，Windows 可能显示安全提示。
- 同源 CI ZIP：`dist/packages/YM_Link-0.1.0-windows-x64-5e85961b4f38.zip`，72,039,094 bytes，SHA-256 `0dc12004a415e9ce0fc5dd6ee369b773446efde0dfa24de6f09264be98da5d37`；随附校验和、CRC、747 个成员 / 279 个 license 成员、BUILD-INFO 完整 SHA / 平台 / 版本、自包含依赖与完整许可检查通过；应用 EXE / Rust DLL 均为 AMD64。
- 独立只读 `performance_scope` 审查确认 [YML-013](YML-013-idle-cpu.md) 只改 Swift 状态发布、未改 Rust / Windows。Windows 500ms 轮询串行，行值已有去重，但仍每轮 Render；不能从静态代码或 Mac 的 0.144% 单核 CPU 实测推断 Windows 性能通过。Windows 空闲及长期性能保留待用户安装后实际采样。
- 本轮仅新增任务记录与更新 STATUS，`harness check` 退出 0；自查未修改产品源码。完整本机 Rust 回归 not-run（纯打包 / 文档且对应 CI 已通过）；正常交互窗口、当前用户安装 / 卸载、真实 LAN / Quest / 录屏性能 not-run。

## Next

安装包准备完成并归档。用户安装并完整启动后，先测两个 30s 空闲区间，再切换三页面、刷新显示器、打开 / 取消地址窗口后静置测 30s；记录包 SHA、OS / CPU、累计 CPU 时间差与墙钟差及内存。单核 CPU 口径为 CPU 时间差 / 墙钟差，不直接与任务管理器按逻辑处理器归一化的百分比混比。数小时后再复测覆盖原问题的长期边界；真实镜像与传输按对应任务单独验收。
