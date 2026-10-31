# YML-028：显示器卡片预览与名称分区修正

Status: done
Workflow: maintenance
Requires-Device: no
Tests: windows/Tests 生产 BAML / 双语两尺寸 Window.Show / 选择绑定及真实 ABI；完整 harness verify；纯视觉修正不新增镜像 setter 的测试，不改 Rust / 协议

## Scope

用户指出 [YML-026](YML-026-monitor-selection-native.md) 将“整卡淡蓝”误当作期望，实际需要显示器图标预览区与下方名称 / 分辨率有明确分区和细分隔线。修正 WPF 与 HTML demo：只对选中预览区淡蓝高亮，名称区保持白底，中间全宽细线；保留单层外围蓝边、勾选、原生键盘焦点 / 无障碍及高对比资源。用户此前指定实际窗口自己检查，本轮不操作窗口。正在运行的开发版位于 `dist/YM_Link`，修正版单独构建至 `dist/YM_Link-monitor-sections`，不关闭或覆盖它。

## Acceptance

- [x] 两个状态均有明确的图标 / 信息分区和全宽细分隔线，选中只染预览区；原生与 HTML demo 一致。
- [x] 保留固定2px单层外边框、选择 / 勾选 / 内部键盘焦点与资源语义，双语窄布局无新增溢出。
- [x] 新原生样本、Release WPF / 真实 ABI / 独立输出构建有实际结果；新独立 CR 无待修问题。完整门禁结果另记。
- [x] 自查、任务 / STATUS 更新、commit / push main 与严格推送检查闭环，交付前核对最终实际远端。

## Verification

先由用户截图与当前 XAML 复现错误分区，改后运行现有 Release WPF / BAML / ABI，导出两语两尺寸原生样本检查上下色块 / 分隔线。由于标准 build_windows.py 会清空正在运行的 dist/YM_Link，本轮按同一构建流程使用 cargo Release、dotnet Release tests / native ABI、dotnet publish win-x64 self-contained 到独立输出、复制 Rust DLL 与现有图标核验；不增加构建工具选项。运行完整 `harness verify`，新 subagent 做最终 CR，修复属实问题后提交推送。手工 hover / 键盘 / 高对比及完整应用启动 not-run（用户自行检查）；Quest / LAN / 实际采集 not-applicable。

## Evidence

2026-10-31 main 干净，fetch / ff-only 已最新，开工 `ae7084b`。承认上一轮把用户描述的问题误当作需求，当前源码选中 trigger 染整卡且无图标 / 信息分隔，截图与实现一致。正确方向是分区而非整卡染色，旧测试 / CR 的绿色不能证明产品方向正确。本轮不更改旧证据，只以本任务记录修正。只读进程确认用户正运行 `dist/YM_Link/YM_Link.exe`，故使用独立构建目录。

- WPF 改为118px预览行 + 自动填满剩余高度的信息行；后者为白底、底部圆角、顶部1px Hairline。固定2px外边框与内部焦点沿用；预览单独处理 gray hover / pale-blue selected，选中优先于hover。HTML同步同样分区与 forced-colors。继承模板的底色被完整的上下两区覆盖，不影响设备 / 共享行。
- Release现有 `windows/Tests` 退出0：中英资源 / 模型、生产BAML与事件连接器、选择与双语两尺寸原生 Window.Show / 布局 / 下拉、真实DLL并发快照命令与关闭生命周期通过。`dist/evidence/monitor-card-sections/` 导出24张原生样本；主代理抽检中文宽版、英文窄版，独立审查抽检英文两尺寸，均显示上方灰 / 淡蓝、下方白底与全宽细线，布局正常。
- 独立构建复用 `build_windows.run` 的路径脱敏环境，执行 cargo Release 与 dotnet win-x64 self-contained publish 到 `dist/YM_Link-monitor-sections`，复制同源码 Rust DLL、EXE图标与三份自有二进制的构建路径隐私扫描通过。未调用会清空运行目录的标准脚本；首次原生测试用的 Release DLL也已在最终复制前按现有脱敏环境重建。新EXE170496 bytes、managedDLL217088 bytes、RustDLL3361280 bytes。未启动修正版或替换用户正在运行的旧开发版。
- 新独立CR审查基线 `ae7084beba6f053b313acdce5973749f8ed2feb3` 的工作区完整差异；WPF blob `6530400c5ba3832c1bd1b21d9c3ba8378980463e`、HTML blob `5002f87054d3a133e76ed9c89f5a67b993c2ffdf`。检查继承状态、圆角、资源 / 高对比、绑定与footer拉伸，并检查本轮原生样本；未发现属实待修问题，无额外修复 / 复审轮次。HTML差异 / 机械设计扫描通过；未操作用户窗口。
- 完整 `harness verify` 以 `RUST_TEST_THREADS=1` 运行：Python47项 /3 skipped、仓库check与rustfmt通过，Rust core23通过，discovery8通过 /1失败。`beacon_ports_fall_back_in_order` 在绑定47610时 OS10048；只读 netstat 确认用户运行中的旧开发版占用该端口。后续Rust目标 not-run；不关闭用户应用、不重复筛选回归冒充完整通过。此轮未修改 Rust，既有CI绿色不是本次分区视觉验收，新版本CI本轮尚未回读。
- `git diff --check`、仓库check通过；以本轮原生样本、Release / ABI与新CR确认分区修正。手工窗口操作 / 完整启动 / 系统高对比 not-run（用户自行检查），安装版和正在运行的开发版均未覆盖。

## Next

分区修正及独立开发版已交付；用户可先退出原应用，再自行启动 `dist/YM_Link-monitor-sections/YM_Link.exe` 验证实际 hover / 键盘。HTML demo 同步更新，原型旧“整卡淡蓝”方向与其原生实现已由本任务取代；真实设备 / 镜像验收仍按对应任务推进。
