# YML-016：修复 Windows 共享目录路径显示前缀

Status: done
Workflow: behavior
Requires-Device: no
Tests: not-applicable (仅 Windows 展示层格式化；以 WPF 模型和真实行模板回归验证，无 Rust / 协议行为改动)

## Scope

修复 Windows 已授权目录直接显示规范化路径系统前缀的问题。仅调整行的展示值，保留 Rust 快照、配置、目录授权与文件访问路径；不改协议和 macOS。

## Spec

Windows 添加目录后，Rust `canonicalize` 产生带扩展命名空间前缀的路径；`ShareItem.Path` 直接返回原值，导致已授权目录泄露系统表示。

给定扩展盘符绝对路径，显示为普通盘符路径；给定扩展 UNC 路径，显示为标准双反斜杠网络路径。普通路径、中文、空格、长路径内容与卷 GUID / 设备命名空间保留。原始 `Share.Path` 不变，新增和重启恢复的目录使用同一展示逻辑。

## Acceptance

- [x] 盘符及 UNC 目录展示去掉系统前缀，原始数据不变。
- [x] 普通、中文、长路径与特殊命名空间边界回归通过，行模板初始 / 更新展示通过。
- [x] 完整门禁、Windows 构建与实际窗口核验完成，记录限制。

## Verification

- `dotnet run --project windows/Tests/YM_Link.Tests.csproj`：先观察扩展路径展示断言失败，再验证模型边界与真实 WPF 模板通过。
- `python3 scripts/harness.py verify`：仓库 / harness / rustfmt / Rust 回归通过。
- `python3 scripts/build_windows.py`：Release 自包含目录、WPF 与真实 ABI 回归通过；再实际启动核验文件页。
- 第二设备 / Quest：not-applicable（本地展示修复，无协议变化）。

## Evidence

- 开工工作区干净；fetch 后 ff-only 同步至 `4c1f94b`。
- 根因：`desktop/src/runtime.rs` 保存 canonical 路径；Windows `ShareItem.Path` 与真实模板原样展示该值。
- Red：`dotnet run --project windows/Tests/YM_Link.Tests.csproj` exit 1，新增路径展示断言在原实现上失败；修复后相同命令 exit 0，模型与真实 WPF 初始 / 快照更新模板通过。
- `python3 scripts/build_windows.py` exit 0：Release 自包含目录、WPF、真实 DLL 排队 / 销毁 ABI 回归通过。
- 首轮 `python3 scripts/harness.py verify` exit 1：Python 47 项（3 skipped）、仓库与 rustfmt 通过；Rust `beacon_ports_fall_back_in_order` 被运行中的旧版占用 UDP 47610 阻断（AddrInUse）。已正常关闭空闲旧版，确认发现端口释放，待完整复跑；保留首轮失败。
- 独立 subagent 审查：基线 `4c1f94b` 与本次 Windows 三文件工作区 diff，无 actionable findings；确认展示转换不改变路径与授权 ID，特殊命名空间保留。
- 释放端口后完整 `python3 scripts/harness.py verify` exit 0：Python 47 项（3 skipped）、仓库 / rustfmt、Rust 103 passed / 2 ignored；包含首轮失败的端口回退测试，不过滤测试。
- Windows 11 Pro 25H2 x64 build 26200.9168：本地 Release EXE `--smoke-test` exit 0；用 computer-use 启动明确 `process:` 路径的本地修复版，并以进程路径核验，文件共享页真实已有目录显示不再带系统前缀。启动器首次将裸 EXE 路径解析为已安装旧版，明确进程路径重启后确认是本地修复版；未覆盖已安装目录。
- 通过 computer-use 无障碍文本对照旧版 / 新版实际共享行；现有 `shares.json` 启动前后 SHA-256 相同，目录与授权无需迁移。未把用户目录路径、原始树或截图入库。
- GPU 截图捕获失败（0x887A0005），改用键盘导航与实际窗口无障碍值核验；截图视觉验收 not-run。UNC 由模型 / WPF 模板验证，真实网络共享挂载 not-run。本修复不涉及 Quest / LAN / 屏幕采集，不追加这些验收。

## Next

已归档；按 main 记录提交并推送，交付前执行 `harness check --require-pushed`。本地修复版在 `dist/YM_Link/YM_Link.exe`，当前已启动；已安装版本与公开 Release 未替换，后续随正常分发更新。
