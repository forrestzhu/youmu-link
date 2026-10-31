# YML-032：Windows 任务栏右键闪退

Status: done
Workflow: behavior
Requires-Device: no
Tests: dotnet run --project windows/Tests/YM_Link.Tests.csproj -c Release; cargo test --workspace --all-targets --locked (CI)

## Scope

用户安装73cb59a后打开YM_Link，右键任务栏图标时闪退。修复实际WPF异常及对应编译模板覆盖，不将同时发生的右键直接认定托盘根因。保留共享 /信任与原文件读取修复；不用Computer Use，不要求重复人工复现。

## Spec

给定已有配对设备，生产BAML首次创建待连接行、连接状态切回已配对、行重建或窗口隐藏 /恢复时，小圆圈动画不得因冻结的RenderTransform导致未处理异常。保留动画显示条件、系统减少动画设置、隐藏时停止及托盘打开 /退出语义。不改协议v4 /Rust后端或产品布局。

## Acceptance

- [x] 保存本次异常类型 /栈摘要，编译后配对行回归取得同类RED→GREEN。
- [x] 初始配对 /状态切换 /行重建 /隐藏恢复 /减少动画与中英回归通过。
- [x] 两轮独立CR属实发现修复；按用户顺序R1修复后先推main，再R2。
- [x] 同源Windows构建 /CI /安装包核验，更新用户安装版并验证启动；未执行真实右键不冒记通过。

## Verification

先补生产App资源 /MainWindow BAML的初始已配对行自动回归；解除测试窗口的backend Loaded事件，屏幕外 /不激活 /不进入任务栏，不操作用户应用。运行原生测试 /Release构建，稳定后适用完整门禁与三平台CI。Windows .NET运行时与实际安装包绑定；用户系统事件只保留ignored原始副本，入库仅无媒体路径 /身份的摘要。真实鼠标右键not-run（用户不用Computer Use），用生产模板状态回归与安装启动覆盖确认问题层。

## Evidence

2026-10-31从673584b干净main fetch /ff-only，无新提交。系统20:38:41的事件1026：.NET8.0.32 XamlParseException，内层InvalidOperationException“无法对不可变对象实例上的(0).(1)进行动画处理”；栈为Storyboard.ProcessComplexPath → StyleHelper → FrameworkTemplate → VirtualizingStackPanel。事件1000匹配安装版进程，Rust DLL SHA `d2f99f7615daf803dfb235a576a51ac66382e1ebb99e909cbcf17097d2b82172`与73cb59a包相符。

独立只读调查定位唯一复杂动画路径为MainWindow的PairedRipple.ScaleX /ScaleY；Forms托盘菜单无WPF动画。既有编译BAML测试仅放connected行，已配对动画只在loose XAML下检查，漏了生产模板首次实例化。冻结 /clone的具体时序待RED确认，不以静态猜测签根因。

本机Windows11 Pro 25H2 build26200.9168：red-compiled-01 /green-compiled-01初稿因fixture继承disabled PeerList，断言为无动画，不能计产品RED或否定修复。启用合成列表后red-compiled-02出现与用户同样的XamlParse /immutable `(0).(1)`及ProcessComplexPath栈。唯一产品调整为将ScaleTransform从局部Ellipse.RenderTransform移入该Ellipse内联Style的Setter，使目标初始化先于同Style的动画触发；保留原尺寸 /ScaleX /ScaleY /Opacity时间线、系统动画开关和隐藏清理。

R1独立yml032_cr1查出P1：测试用new App会被WPF构造函数排队Startup，在Dispatcher泵中可能创建另一真实窗口 /托盘 /后端；改回原有普通Application加载App资源，仅MainWindow使用生产BAML，并断开fixture Loaded后端事件，断言只有一个窗口且backend loaded=false。P2：宿主关闭动画可令回归假绿；编译fixture覆盖动画资源为true，并显式断言有效偏好，保留禁用 /恢复 /最小化覆盖。两项复审关闭，最终XAML blob `964dd780bd0b3e21e8f6faf3b677a8cb0adaec8f`、测试 `93e94cb0f12b2c6846be30c79450403674128ba6`。隔离后重新撤回产品修复，red-isolated-03仍取得相同异常；恢复后windows-build-02完整GREEN，退出0。未用夹具初稿的绿结果代签隔离。

`python scripts/harness.py verify`退出0，Python47项 /3 skipped、仓库check /rustfmt、Rust120 passed /2 ignored /0 filtered。最终 `python scripts/build_windows.py --smoke-test`退出0，发布BAML初始配对 /状态 /重建与隐藏 /动画开关、旧中英布局 /托盘 /关闭回归、真实ABI及Release发布 /startup snapshot shutdown通过。本地共享运行时8.0.12，仅开发构建，不替换用户安装的8.0.32；同源CI包继续验证实际8.0.32。原始日志留ignored `dist/evidence/yml032/`。

框架依据：[WPF Application构造代码](https://raw.githubusercontent.com/dotnet/wpf/v8.0.0/src/Microsoft.DotNet.Wpf/src/PresentationFramework/System/Windows/Application.cs)将Startup排到Dispatcher，不能以不调用Run隔离；[Storyboard文档](https://learn.microsoft.com/en-us/dotnet/desktop/wpf/graphics-multimedia/storyboards-overview)说明复杂路径与Freezable克隆语义。实际缺陷以生产BAML RED→GREEN为依据，未将鼠标右键本身当根因。

受审修复8789d24622fe7bf937f0b1f370cfe81f81ec1ad5在R1修复 /复审后先push main；随后新独立yml032_cr2审查673584b..8789d24全部四文件，0项可执行发现，确认冻结对象初始化、动画生命周期、fixture启动隔离和强制动画覆盖，无产品回归发现。R2仅静态只读，未冒记实际8.0.32安装验收。旧安装754文件已完整备份并逐项验证，三份用户JSON摘要已保存；当前等待同源CI41203019626完整包，不混用本地8.0.12运行时。

同源[CI41203019626](https://github.com/forrestzhu/youmu-link/actions/runs/41203019626)三平台及gitleaks最终全success。Windows job124542621902完整Rust120 /2 ignored /0 filtered、修订后的生产BAML初始配对 /重建、原中英布局 /托盘 /X与显式Exit、真实ABI /Release /startup和安装卸载smoke均通过；实际自包含包为.NET8.0.32。已通过GitHub connector取回artifact13043268774，外包SHA `43399fe1d692e2244d49665f7a5cfcaa3f6837557d46355240a1ae8c309deefe`等于API digest，4成员 /双sidecar /CRC /747文件 /BUILD-INFO精确8789d24通过。安装器52,124,382bytes，SHA `ef3291b58fc0ccc60bd47505192330898a8ca842fbbe1420f8170c7f956496ad`。独立包审0阻断；确认新managed BAML已改变，非旧资源，保留旧安装额外7文件而不删除无关载荷。

当前无安装版进程时，以既有路径 /静默 /NOCLOSEAPPLICATIONS更新；安装退出0，747个预期文件逐项相符，三份用户JSON摘要不变。安装版smoke退出0，随后CLI正常打开，实际主窗口句柄 /TCP /UDP监听存活，按启动时刻过滤系统1000 /1026，新增匹配YM_Link闪退事件0。新managed DLL SHA `16dfc108db56d29954ebefc35a7cddad7891ba79a910cc34937bad67ce743ed2`。原始包 /部署 /live记录留ignored `dist/packages/ci-8789d24-verified/`，旧完整备份保留；全程不用Computer Use，不操作用户右键，不索要重复复现。

## Next

实现、相同异常的生产BAML RED→GREEN、两轮独立CR、完整门禁 /CI与同源安装更新 /实际窗口启动已完成，done归档。真实Shell鼠标右键未由agent执行（用户禁止Computer Use），不冒称实际点击验收；当前应用保持打开，用户配置与旧安装备份保留。
