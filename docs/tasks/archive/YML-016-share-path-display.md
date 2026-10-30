# YML-016：修复 Windows 共享目录路径显示前缀

Status: done
Workflow: behavior
Requires-Device: no
Tests: not-applicable (仅 Windows 展示层格式化；以 WPF 模型和真实行模板回归验证，无 Rust / 协议行为改动)

## Scope

修复 Windows 已授权目录直接显示规范化路径系统前缀的问题。仅调整行的展示值，保留 Rust 快照、配置、目录授权与文件访问路径；不改协议和 macOS。

## Spec

Windows 添加目录后，Rust `canonicalize` 产生带扩展命名空间前缀的路径；`ShareItem.Path` 直接返回原值，导致已授权目录泄露系统表示。

给定扩展盘符绝对路径，显示为普通盘符路径；给定扩展 UNC 路径，显示为标准双反斜杠网络路径。普通路径、中文、空格、长路径内容与卷 GUID / 设备命名空间保留。目录组件以点 / 空格结尾时保留扩展前缀，避免复制后被普通 Windows 路径规则规范化为另一个目录。原始 `Share.Path` 不变，新增和重启恢复的目录使用同一展示逻辑。

## Acceptance

- [x] 盘符及 UNC 目录展示去掉系统前缀，原始数据不变。
- [x] 普通、中文、长路径与特殊命名空间边界回归通过，行模板初始 / 更新展示通过。
- [x] 完整门禁、Windows 构建与实际窗口核验完成，记录限制。
- [x] 用户追加独立 CR 的属实发现修复并经回归、完整门禁与复审关闭。

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
- 用户追加独立 CR：受审 `4c1f94b9e63919eee1ce9ac9714d68d32a0bbd19..95839215af98dce19914e2733cfb2685fc648f02`。subagent 发现 1 P2：通过普通 junction 选择到尾点 / 尾空格目录时，直接去前缀会使复制路径指向另一个目录。独立临时夹具的原生 junction / final-path API / 文件 ID 实验证实尾点目录与普通同名目录不同；夹具已清理。未操作用户数据，实际选择窗口与额外 Rust 探针 not-run；源码确认规范路径会经过既有 AddShare / catalog 接受并进入快照。
- 属实发现修复：两类转换前先检查路径组件尾点 / 尾空格，需扩展命名空间保持原义时保留原值。新增 6 项盘符 / UNC / 叶子与父目录边界；`dotnet run --project windows/Tests/YM_Link.Tests.csproj -c Release` 修复前 exit 1、修复后 exit 0，包含原有真实 WPF 行模板。
- 复审：同一独立 subagent 审查 `9583921` + 本次 `MainWindow.xaml.cs` / `Program.cs` 工作区 diff（本修复提交的代码差异），确认 P2 关闭，无新增 actionable findings；独立运行 Release 测试程序集与 `git diff --check` 均通过。
- CR 修复后 `python3 scripts/harness.py verify` exit 0：Python 47 项（3 skipped）、仓库 / rustfmt、Rust 103 passed / 2 ignored；`python3 scripts/build_windows.py --smoke-test` exit 0，Release 自包含目录、真实 WPF 模板、DLL ABI 与实际启动 / 退出 smoke 通过。
- 最终构建已通过 computer-use 明确进程路径启动，并回读实际主窗口；现有共享配置校验值仍与启动前相同。切换文件共享页回读时用户按物理 Esc 停止 Computer Use，立即停止后续窗口输入；本轮最终构建的文件页回读 not-run，不将此前版本的行回读写成此次验收。路径行为由本轮模型 / 真实 WPF 模板回归验证。

## Next

修复与追加 CR 闭环，归档后 commit / push main，并执行 `harness check --require-pushed`。本地最终修复版已启动；窗口自动操作已按用户 Esc 停止。已安装版本与公开 Release 未替换。
