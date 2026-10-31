# YML-021：安装完成启动与桌面快捷方式

Status: done
Workflow: behavior
Requires-Device: no
Tests: not-applicable (不改 Rust 或协议；原生 Inno 配置由 CI 编译及实际安装验证，扩展现有 Windows smoke 检查快捷方式 / 静默不启动 / 卸载清理，运行 Python 回归与仓库检查)

## Scope

用户在 YML-020 显示器修复安装包准备期间追加要求：安装完成后可立即打开，并提供创建桌面快捷方式的选项，参考 Klip 安装体验。使用现有 Inno 原生 Tasks / Icons / Run，不新增自定义 UI、依赖或管理员权限。最终交付一个同时含显示器型号修复和这两项选项的 Windows x64 安装包，不覆盖当前用户安装，不发布公开 Release。

## Spec

完成页“立即打开”默认勾选，可取消；只在用户结束交互安装时打开，静默 / verysilent 安装不打开。新增默认勾选且可取消的“创建桌面快捷方式”安装任务，写入当前用户桌面；后续安装沿用 Inno 的用户任务选择。开始菜单快捷方式保持，卸载清理安装器创建的两处快捷方式并保留设置 / 配对。

只读核对 Klip 与 Klip2 的 scripts/build-windows-app.mjs：MUI_FINISHPAGE_RUN 默认勾选、静默不启动；原先没有桌面快捷方式。本任务沿用其完成页行为，桌面任务为本次新增默认选择。使用 Inno 内置可本地化消息，不自行拼接桌面路径。

## Acceptance

- [x] 原生安装任务 / 完成页启动选项就位，均可取消且静默不自启。
- [x] CI 实际安装默认创建桌面 / 开始菜单快捷方式，目标正确；取消桌面任务时无桌面快捷方式。
- [x] 两种安装均完整载荷 / 原生启动 / 卸载通过，快捷方式及卸载记录清理通过。
- [x] 独立审查 / 文档检查与最终安装包 SHA-256 通过，YML-020 修复一并保留；提交 / 推送及严格门禁按仓库交付流程执行。

## Verification

本机 Python 回归 / harness check / diff 自查；独立审查 Inno 默认勾选 / Tasks 绑定 / skipifsilent、路径及 Shell 检查。CI 编译后在一次性 Windows 用户运行增强 smoke，两轮安装（默认 / TASKS空）核对完整载荷、Shell .lnk 的真实目标、未自动启动、卸载清理。已安装用户及已有快捷方式时拒绝 smoke，保持本机设置；交互完成页的实际勾选 / 取消 / 点击完成启动在本机未安装前记 not-run，不以静默 smoke 代签。原生应用 / Rust 沿用 YML-020 同源代码完整回归，后继完整 CI 再验证。

## Evidence

2026-10-31 基于 ffe9ce56ec543f7bf738a404d215926a876f3b42，工作区干净；此前 YML-020 三平台 CI run 41152416780 已全部 success，Windows107 passed /2 ignored、WPF / ABI / Release与原安装验收通过。当前追加仅改安装器和验收脚本，不触及显示器修复或采集 / 性能。

实现依据 Inno 官方 Run / Tasks / Icons 文档，采用 postinstall nowait skipifsilent 和受 desktopicon 任务控制的 userdesktop 快捷方式；无需 Pascal 自定义完成页。原生配置是低影响可逆改动，不写静态字符串断言；实际行为由原生 CI 安装检查验证。

本机 `python3 -m unittest discover -s scripts -p 'test_*.py'` 退出 0，47 项 /3 skipped；`harness check`（91 个文本文件）与 `git diff --check` 通过。独立代理使用 Windows PowerShell 5.1.26100.9168 执行生产生成的检查脚本，仅把 Desktop / Programs / 注册表检查目标替换为临时目录 / 链接；带空格 / 单引号路径、嵌套数组及比较 / quoting 通过，不触碰真实快捷方式、注册表或应用。原生应用 / Rust 与刚通过完整门禁及三平台 CI 的 ffe9ce5 一致，不重复运行会争用当前用户发现端口的生命周期检查，后继 CI 完整重验。

独立审查未发现确定性阻断：Tasks绑定、postinstall默认勾选、skipifsilent、TASKS空及失败时 finally 卸载均符合预期。验收边界：进程检查为存活快照，非进程启动历史追踪；失败会尝试卸载但不主动结束应用，异常传播后不声称清理断言通过。交互完成页、升级任务选择记忆的实际操作 not-run，依据 Inno 原生语义，不以静默测试代签。

最终源码 `57a9e0d206634982b51f3a92c4fed0d357a65c85` 的[CI run 41153337802](https://github.com/forrestzhu/youmu-link/actions/runs/41153337802) 三平台全 success。独立代理回读 Windows job 124394765058：Rust107 passed /2 ignored、四项型号回归、WPF / 双语 / ABI / Release及后端 smoke 均通过；增强安装脚本的完成消息只在默认 / TASKS空两轮的载荷、真实 .lnk 目标、注册项、无存活应用、显式启动 / 退出及卸载清理全部通过后打印，实际日志已确认该消息。交互式 Finish 人工点击仍未覆盖。

GitHub connector 取回 artifact 13023824493，外层 124,168,206 bytes；实际 SHA-256 与服务端 digest 一致：`ae46605bfa998d850183098ec8e9a8ff1f1069a36ed6851ed0a91fe3ad3c8ae8`。安全提取恰好四个预期文件到忽略入库的 `dist/packages/ci-57a9e0d/`，外层 / 内层 CRC 均通过：

| 文件 | 大小（bytes） | SHA-256（与 sidecar 一致） |
| --- | --- | --- |
| `YM_Link-0.1.0-windows-x64-57a9e0d20663-setup.exe` | 52,083,482 | `b0482b4515d6fe2cca9430c52a71091c6caf5b51efb9ef8f4ceee4207f2912fe` |
| `YM_Link-0.1.0-windows-x64-57a9e0d20663.zip` | 72,083,722 | `191359ece8067199bab5dde89054696957493e94d483c6ddfe701b5701e89336` |

ZIP 747 个成员 /279 个依赖许可文件，BUILD-INFO 完整源码 SHA、windows-x64、0.1.0 与预期一致；应用、自包含运行库及根许可齐全，LICENSE / NOTICE 与仓库逐字节一致。安装器 PE 图标与正式 ICO 一致，Authenticode 回读 NotSigned。未覆盖当前用户安装或发布 GitHub Release；本机已有安装，安装选项实际交互留待用户自行安装，不把 CI 静默验收称为人工交互通过。

独立代理再次回读下载后的实际文件，外层 / 内层 hash、sidecar、CRC与逐字节成员比对全部通过；无重复或越界 ZIP 成员。根三份许可与提交一致，inventory 的278份文本完整非空，128个Rust依赖名称 /版本与Cargo.lock精确对应，Rust运行时1.85.0；两个.NET运行时包均8.0.33，与包内deps精确对应。独立解析安装器和产品EXE的7帧图标均与正式ICO一致；精确git diff确认ffe9ce5至57a9e0d仅安装脚本与文档变化，Rust /WPF产品源码不变。最终归档后的 `harness check`（91个文本文件）与diff检查通过；提交 /推送后以 `harness check --require-pushed` 执行严格交付门禁。

## Next

含 YML-020 型号修复和本任务安装选项的新 setup.exe 已就绪，向用户交付本机路径。保留未签名开发包边界与未执行的完成页 / 升级交互验收；YML-017 性能任务保持暂停。
