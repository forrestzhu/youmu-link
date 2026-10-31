# YML-019：最新代码 Windows 安装包交付

Status: done
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (仅交付既有 CI 安装包，无产品或打包代码改动；核对源码一致性、CI 安装验收、完整产物校验和与许可证，执行 harness check)

## Scope

用户要求基于最新代码打一个安装包；当前为 Windows 11 x64 主机。取回已通过 CI 的最新 Windows x64 安装器 / ZIP，核对其源码与当前 main 的产品、依赖、资源和打包逻辑一致，保留真实源码版本及校验和，并提供本机安装入口。本轮不覆盖当前用户的既有安装，不恢复 YML-017 性能排查，不发布 GitHub Release。

## Acceptance

- [x] fetch / ff-only 同步 main，核对最新安装包与 main 的产品及打包源码一致。
- [x] EXE / ZIP 与各自 SHA-256 保存到本机，独立回读元数据、校验和与完整许可。
- [x] 核验同源 CI 构建、安装 / 启动 / 卸载结果，记录当前用户未执行安装的边界。
- [x] 更新 STATUS，文档检查 / diff 自查通过；提交 / 推送及严格门禁按仓库交付流程执行。

## Verification

通过 git diff 核对源码差异；核验 run 41101011409 的源码 SHA、各平台任务与 Windows 打包步骤；使用 GitHub connector 下载 Windows artifact，并校验服务端 artifact digest、各独立文件 SHA-256、ZIP CRC、BUILD-INFO 完整源码 SHA / 平台、应用运行库与许可。仅文档改动运行 harness check；完整 Rust / WPF 回归沿用同源已成功 CI。本机安装 / 卸载 not-run：已有同 AppId 安装记录，安装 smoke 会拒绝且不能隔离当前用户状态；真实 Quest / 镜像 / 性能不属于本次打包验收。

## Evidence

2026-10-31 开工 main 干净，fetch / ff-only 确认最新 HEAD 为 `3892a22382bb8b0c4037ae02eb1b8bd758a576ac`。本机实际系统为 Windows 11 专业版 x64 build 26200。安装包真实源码为 `f9df8d3c6f89066ce561e98b400cd03d19457aa3`；到当前 main 仅三份文档和 Windows 测试中的两行注释变化，产品 / 依赖 / 资源 / 打包脚本 diff 为空。[run 41101011409](https://github.com/forrestzhu/youmu-link/actions/runs/41101011409) 的 Linux / Windows / macOS 与 Windows 构建 / 打包验收步骤全部 success；本轮直接回读 API 结果，不只沿用历史文档。

独立只读代理核对既有流程及状态风险：本机没有标准位置 / PATH 中的 ISCC，且 HKCU 已有同 AppId 卸载键。本轮取回同源 CI 原始安装器，不在当前用户执行安装 / 卸载 smoke。

通过 GitHub connector 下载 artifact `12998448730` 并保存至忽略入库的 `dist/packages/ci-f9df8d3/`。外层 `artifact.zip` 为 124,201,429 bytes，实际 SHA-256 与 GitHub digest 一致：`4aea4ced4bf0541c578d1b812c6f2c5b198ff42bb152e28903953678495cbea2`，CRC 通过且恰含四个预期文件。内层独立回读：

| 文件 | 大小（bytes） | SHA-256（与 sidecar 一致） |
| --- | --- | --- |
| `YM_Link-0.1.0-windows-x64-f9df8d3c6f89-setup.exe` | 52,118,106 | `a63bd9e5cf0c1169bf0e4681412ba9ad74d15dac68ad89aa3730fe2406289629` |
| `YM_Link-0.1.0-windows-x64-f9df8d3c6f89.zip` | 72,082,321 | `f341140b52d8f26a224badb53d05f3272319e7f6b07f10cb9418fbbd1c68ffdd` |

ZIP 747 个成员、279 个依赖许可文件，CRC 通过；BUILD-INFO 完整源码 SHA / `windows-x64` / `0.1.0` 与预期一致；应用、Rust DLL、自包含 .NET / WPF 运行库及 LICENSE / NOTICE / THIRD_PARTY_NOTICES 齐全，LICENSE / NOTICE 与当前仓库逐字节一致。`Get-AuthenticodeSignature` 回读 EXE 为 NotSigned，保持现有未签名开发包边界，未进行签名或公开 Release 发布。

本轮再回读同源 Windows job `124219168375` 实际日志：Rust 103 passed / 2 ignored，原生图标 / Rust 启动关闭 smoke 通过，明确输出 `Verified Windows install, complete payload, native startup/shutdown and uninstall.`。这证明上述原始 CI 产物的安装验收，本机用户安装 / 卸载仍为 not-run。

独立代理对下载后的实际文件重新计算外层 / 内层 hash、CRC、已提取文件逐项 hash 与安装器 PE 图标，全部通过。许可证 inventory 的 128 个 Rust 外部依赖与 Cargo.lock 完全一致，Rust 1.85.0、两个 .NET 8.0.32 runtime pack 及其引用的 278 份许可文本完整非空，三个根许可文件均与仓库一致；再次确认产品 / 依赖 / 资源 / 打包 / 工作流与 main 无差异。

`python3 scripts/harness.py check` 退出 0（89 个文本文件）；`git diff --check` 退出 0，任务 / STATUS diff 已自查。本轮未新增或修改实现，不重复构建同源产物；最终提交 / 推送后执行 `harness check --require-pushed` 作为交付门禁。

## Next

安装包与校验和已就绪，向用户交付本机 `dist/packages/ci-f9df8d3/` 下的 setup.exe 入口，由用户自行安装；YML-018 Windows11交互与YML-017暂停的性能任务保持原状态。
