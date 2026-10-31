# YML-020：Windows 显示器型号名称识别

Status: verifying
Workflow: behavior
Requires-Device: no
Tests: ymlink-desktop::capture::windows::tests (显示器型号解析 / 活动状态 / 隐私边界)；windows/Tests (双语名称 / 编号回退)

## Scope

修复 Windows 显示器卡片只显示“显示器 1 / 2”的问题，读取对应 GDI 显示源的活动显示器 PnP 型号；当前用户两屏为 HWV6446 / RTK1600。保留现有 DISPLAYn 选择 ID、HMONITOR 与尺寸，不改协议 / C ABI / 采集、音频或暂停的 YML-017 性能任务。完成本机验证并准备含修复的安装包。

## Spec

给定同一电脑上两个活动显示器，界面应显示各自型号 HWV6446 / RTK1600，并保持 1 / 2 号选择及分辨率的对应关系。名称来自 Windows EnumDisplayDevicesW，按 GetMonitorInfoW 的 GDI 显示源匹配，仅读取活动 monitor；只提取接口名中的三字母制造商 + 四位十六进制产品代码，不把完整接口、实例或序列号写入快照。查询失败、空 / 非标准型号回退既有 Display n 并由本地化翻译。原型图标与其他页面不在本次范围。

本机已复现根因：monitors() 将 name 硬编码为 Display n。只读 Windows API 查询返回 DISPLAY1→HWV6446、DISPLAY2→RTK1600，两者 DeviceString 均为 Generic PnP Monitor；WMI 友好名为 QSN-CBB / Monitor，故按用户提供的 PnP 型号命名。采用现有 windows GDI API，无新依赖 / 协议字段。

## Acceptance

- [x] Rust 枚举返回活动屏幕型号，错误 / 无型号回退编号，ID / 尺寸 / 捕获句柄对应不变。
- [x] 先以失败测试固定型号解析及隐私边界，再通过 Rust / WPF 回归和完整 harness verify。
- [x] Release 构建 / 实际窗口启动验证本机两型号及分辨率；不启动镜像 / 音频采集、不改变已有安装。
- [ ] 含修复的安装包与 SHA-256 就绪，文档 / 独立审查 / main 推送门禁闭环。

## Verification

顺序：Rust 解析测试 red→green，补 WPF 双语型号透传 / 编号回退检查；运行 harness verify 与 build_windows.py；用临时状态目录回读真实 DLL 的显示器快照并检查型号、ID、尺寸，实际启动新版窗口核对卡片。独立代理审查 API 映射 / 失败回退 / 隐私边界；CI 原生安装 / 完整载荷 / 启动卸载通过后下载精确提交安装器，独立核对 SHA-256 / BUILD-INFO / 许可。当前用户已有安装，不执行安装 / 卸载 smoke。真实 Quest / LAN / 镜像 / 音频 / 性能 not-applicable（本轮仅名称）。

## Evidence

2026-10-31 开工 main 干净，fetch / ff-only 确认 51f75dc 已是最新。Windows 11 专业版 x64 build 26200；用户截图为两屏通用名称，直接查询已确认两屏型号，与用户游戏中的型号一致。只读独立代理确认名称在 Rust 生成时丢失，后续 snapshot / C ABI / WPF 原样传递，Localizer 仅本地化占位名。

实现只改 Windows 名称获取：当前 MONITORINFOEXW.szDevice 直接传入 EnumDisplayDevicesW，并设置 EDD_GET_DEVICE_INTERFACE_NAME；过滤非活动 monitor，严格解析七字符型号，失败保留 Display n。依据 [Microsoft EnumDisplayDevicesW](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enumdisplaydevicesw)。不新增依赖或协议字段。

- RED：先为原名称回退加解析接点及测试，`cargo test -p ymlink-desktop --lib monitor_model -- --nocapture` 实际失败，None 与预期 HWV6446 不符；异常设备测试通过。GREEN：实现后 `cargo test -p ymlink-desktop --lib monitor_ -- --nocapture` 四项通过，覆盖两型号 / 大小写与前导零、非活动 / 非监视器 / 非法型号 / UTF-16、终止符及查询失败回退。
- 首轮完整 harness verify 与 build_windows.py 并行执行，发现层 `beacon_ports_fall_back_in_order` 在阻占首端口时返回 AddrInUse / 10048；原生生命周期检查也使用同一真实发现端口。待构建检查结束后顺序重跑完整 `harness verify` 退出 0：Python 47 项 / 3 skipped，Rust 107 passed / 2 ignored，仓库检查及 rustfmt 通过；未改发现实现或过滤该测试。
- `python3 scripts/build_windows.py` 退出 0：WPF 双语型号透传 / 编号回退、既有生产 XAML 模板与行绑定、真实 DLL 并发生命周期、自包含 Release 及图标检查通过。
- 临时状态目录的真实 Release C ABI 回读：id=1 / HWV6446 / 3840×2160，id=2 / RTK1600 / 1600×2560。首次诊断过早读取 starting 状态得到空列表；按既有启动约定等待非 starting 后同一 DLL 检查通过，未改启动时序。
- 启动本轮 `dist/YM_Link/YM_Link.exe` 并用 computer-use 前台核对真实窗口：两个型号及上述分辨率均实际显示，首屏已选项正常；未开始镜像或音频采集。开发目录与当前用户安装分离，不覆盖安装。
- 独立代理逐行审查实现与 WPF 测试，未发现需修复的确定性问题：GDI 源匹配不依赖全局顺序、活动过滤、失败回退、UTF-16 / 隐私边界及指针生命周期均成立，原有捕获 ID / HMONITOR 映射不变。克隆屏采用同一 GDI 源的首个有效活动型号；克隆 / 热插拔 / 同型号多屏实际验收 not-run，不把这两屏扩展模式代签所有显示拓扑。

## Next

提交并推送实现以运行同源 CI 安装验收，取回新版安装器 / SHA-256，核验后归档交付。YML-017 性能任务保持暂停。
