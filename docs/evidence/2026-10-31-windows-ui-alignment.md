# Windows 界面与 macOS 对齐

Implementation: bbdf01a；验证工具修复至f9df8d3
Host: 本机 macOS；Windows 原生构建 / WPF 渲染由 GitHub windows-2025 runner 执行（首轮 /第三轮实际Microsoft Windows Server2025 Datacenter 10.0.26100）
Result: implemented / automated checks passed；Windows11用户机交互待验

## 设计依据与实现

现有 macOS `ContentView.swift` 是本次明确的视觉权威。Windows 原界面使用默认 WPF Button / TabItem / ComboBox / CheckBox，标题在内容页内重复，显示器下拉框与单个大预览分离，设备连接按钮独立堆叠，空态左对齐。此次保留原生 WPF / Rust C ABI 和所有协议 / 采集实现，统一到 Mac 的结构：

- 245px 侧栏、正式蓝金应用图标、设备卡片与卡片内连接 / 断开，可信设备可使用原生上下文菜单忘记。连接 / 配对中仍可断开；命令使用所属行 ID，不依赖其他行的选中状态。
- 标题 / 副标题与原生 RadioButton 分段选择器位于同一头部；窄于1080px时导航移到标题下方，最小窗口860×580。三页只显示当前一页，保留数据和控件状态。
- 正文28px边距、显示器卡片最小约210px、106px图形区与选中边框；音频用开关、画质用圆角下拉框。共享目录 / 传输卡片和居中空态遵循 Mac 的层级与留白。
- 统一表面、文字、边框和蓝色强调token；按钮 / 输入 / 选择器 / 进度条替换默认旧式chrome。保留原生焦点样式与自动化名称；高对比使用系统颜色，选中前景 / 背景成对映射。

标题资源只在构造、页选择和语言切换时更新，不加入500ms快照重复布局逻辑；不新增常驻动画。YML-017性能排查仍按用户要求暂停，本次不以视觉更新宣称CPU问题解决。

## 验证方法与边界

`harness verify` 本机通过（Python47，Rust100 passed /1 ignored、仓库检查及rustfmt）。XML解析和diff检查通过。Impeccable扫描空列表仅是机械扫描结果，原生XAML支持有限，不能代替实图。

扩展现有STA回归，解析生产 MainWindow.xaml 和 App.xaml 的真实WPF控件 /样式，仅去除x:Class与code-behind事件，注入合成Peer / Display / Share / Transfer，在Windows执行Window.Show，再对其实际client区域执行RenderTargetBitmap。最终外窗1024×720 /860×580适配托管桌面，图片排除OS标题栏与边框，实际client像素尺寸以PNG回读为准。两种locale ×两种外窗 ×镜像 /共享 /传输 /共享空态 /传输空态 /配对错误，共24张。完整1120默认宽窗、人工键盘 /高对比另待Windows11用户机复验。样本不是真实用户设备或路径，不连接身份 /网络 /采集。图片只保存在CI短期artifact与忽略入库的本地artifacts目录。

回归保留getter-only共享路径与进度更新绑定；新增卡片选择、共享长路径换行、进度、RadioButton互斥 /标题 /恰一页可见、窄窗头部、ComboBox真实Popup开合 /选项更新、连接中取消及焦点属性检查。它不能证明完整真实code-behind交互、人工Tab键遍历、屏幕阅读器体验或系统高对比视觉；这些保留Windows11用户机复验。合成配对错误只证明布局，不代签真实Quest配对。

## 独立审查

首轮受审：2c284ec后本次工作树（合入bbdf01a）。发现2项P2：

1. headers-only TabControl移除PART_SelectedContentHost损失原生Tab自动化children及焦点目标；改用与Mac Picker更接近的原生RadioButton分段选择器与同级可见页。
2. 高对比中下拉选中 /高亮背景映射WindowBrush导致不可区分；增加选中 /高亮前景背景token并映射Highlight /HighlightText。

复审确认两项关闭，未发现其他确定性命令ID /初始化 /显示选择问题；测试新增代码无明显编译缺陷。Windows首轮编译通过，复审无新增问题。

## Windows CI

[首轮41096696208](https://github.com/forrestzhu/youmu-link/actions/runs/41096696208)：Linux /Mac通过，Windows Rust与C# /XAML编译通过；完整布局回归发现合成路径样本末段带尾空格，因此ShareItem按既有语义正确保留canonical路径、与测试期望不符。9152f80去除样本尾空格，未修改路径实现。首轮镜像图及共享失败图已下载，发现RenderTargetBitmap仅捕获内容Grid未含Window底色，且CI工作区限制实际Window宽度；补内容底色与固定内容viewport渲染，避免图像透明黑底及尺寸标注不准确。上述失败保留，不当产品回归通过。

第二轮9152f80的运行41097531908在最终viewport修复ced5c09推送后被并发策略取消；不计完整通过。第三轮[41097977465](https://github.com/forrestzhu/youmu-link/actions/runs/41097977465)的Windows Rust103 passed /2 ignored、C# /XAML编译通过，但首张PNG仍受HWND父级裁剪，透明像素断言失败，未计完整通过；6a8a33a改为对同一树脱离HWND后离屏渲染，先保存PNG再断言以保留诊断图。该修复仅测试工具，未改变产品布局。后续Mac任务在新提交触发时可被并发取消；Mac产品源码 /依赖与第一轮成功任务一致。

第四轮[41099557254](https://github.com/forrestzhu/youmu-link/actions/runs/41099557254) Windows编译通过，首张固定viewport PNG成功保存，但WPF文字抗锯齿产生alpha254（首个点111,27），被过严alpha255断言拒绝；脱离Window后ComboBox /CheckBox出现绘制缺失，该离屏路径未采用。f9df8d3删除detach /字体冻结 /临时资源路径，恢复已Show窗口的实际client渲染，并用可容纳外窗尺寸、alpha>=250排除真正透明 /裁剪而容忍量化。此后不再改变产品样式。

最终[运行41101011409](https://github.com/forrestzhu/youmu-link/actions/runs/41101011409)的逐项执行日志与产物已核验：

- Windows Rust103 passed /2 ignored；生产WPF模型 /getter-only行绑定 /双语与两种外窗24状态 /native Popup /选择等检查通过。
- 真实Rust DLL并发读取 /命令 /销毁通过，Release自包含构建、窗口图标与后端启动关闭smoke通过。
- 安装器完整载荷、安装后启动 /退出 /卸载通过，EXE /ZIP /SHA-256资产已上传；源码revision为f9df8d3c6f89066ce561e98b400cd03d19457aa3。
- macOS Rust100 passed /1 ignored、Swift9、构建及DMG /ZIP通过；Linux Python /仓库 /rustfmt /Rust检查通过。
- 24 PNG下载回读为1008×681与844×541各12张；主代理抽检中三页 /英空态 /中配对错误6张，独立审阅抽检英镜像两尺寸 /英共享传输 /中窄窗共享传输6张，控件完整、长路径换行、无横向裁切或重叠。小窗镜像设置及配对底部动作需纵向滚动，不能把未展示在首屏的内容当缺失。
- GitHub读取状态曾持续返回queued /旧状态或502，但同一revision的已完成步骤日志、24图与双平台包均已取回；未仅凭状态标签宣称成功。未修改账户计费或发布新的GitHub Release。

实际用户Windows11窗口、完整键盘 /高对比 /辅助功能人工交互not-run；Windows10 /ARM64不推断通过。真实Quest /LAN /镜像 /传输性能沿用YML-003，不作为此视觉任务完成的证据。


## 交付入口

[Windows x64 CI制品（含setup.exe /ZIP及各自SHA-256）](https://github.com/forrestzhu/youmu-link/actions/runs/41101011409/artifacts/12998448730)，源码f9df8d3c6f89066ce561e98b400cd03d19457aa3。API回读制品124201429 bytes，expired=false，artifact digest `sha256:4aea4ced4bf0541c578d1b812c6f2c5b198ff42bb152e28903953678495cbea2`，保留至2026-11-07 01:51（Asia/Shanghai）。这是整组artifact的服务端digest，不是其中EXE的独立hash；各文件hash随制品提供。安装 /完整载荷 /启动关闭 /卸载由同源CI脚本实测。本机124MB下载速度受限，已停止不必要的大文件下载，交付使用上面的已生成制品入口；未声称完成本机EXE下载回读。

UI图已完整下载并抽检，最后一条记录提交仅补文档与测试注释，不改变经验证的产品 /依赖 /打包内容。任务保留blocked至Windows11用户机上线补验，未将自动渲染代签完整产品交互或真实Quest。
