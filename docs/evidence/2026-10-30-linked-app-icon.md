# YM_Link 蓝金链环图标验收（2026-10-30）

任务：[YML-012](../tasks/archive/YML-012-link-logo.md)。用户明确选定右侧加宽米白留白的蓝金链环版本；此前镂刻版的交付见[YML-011记录](2026-10-30-app-icon.md)。

## 正式资源

原实心M、留白Y和金色影院底弧直接复用游目原始矢量路径；按选定图重绘蓝金链环及外围约30像素的米白隔离带。导出时使用标准透明边距与米白应用底板，不复制生成稿的材质或边缘残留。独立视觉对照无必须修复项：64–256px层级清楚，32px保留链环分色，16px以主轮廓为主。

| 文件 | SHA-256 |
| --- | --- |
| `assets/app-icon/ym-link.svg` | `77e917f001f9bf21a3fd109a2a702c8d5df718158942dc60ca8bebfd526f1cb3` |
| `ym-link-1024.png` | `bff64637a1842a56cb9a9ca78c3be7703c6ec4dfb6ca955cdf35a4ac7250faf3` |
| `YM_Link.icns` | `af0551d8acf88d0d89770d7efdbd9722661ecb3416097fc7d351c8ace69bb2eb` |
| `YM_Link.ico` | `bd14c448a303d49fe32f178cac7db6b86d28a20876269a5e500c09ff8184b131` |

Pillow独立解码1024 RGBA PNG、ICNS普通/Retina表示与ICO16/24/32/48/64/128/256七帧通过。沿用既有原生/安装器接入及资源字节校验，没有新增构建工具依赖、修改协议/业务或用户文案。GitHub中英README通过同一路径引用新标志。

## 本机验证及外部影响

环境：macOS26.3.1 arm64、Xcode26.4.1（17E202）、SDK26.4。

- `harness verify`：Python47/47、仓库78文本与rustfmt通过；Rust固定UDP端口测试受到另一checkout运行中的YM_Link占用47610影响，不能记完整通过。
- 首个失败为`discovery::tests::query_targets_survive_unreachable_broadcast_and_probe_every_port`（AddrInUse48）。仅过滤这一项后的运行又在集成测试`beacon_ports_fall_back_in_order`的“block first port”绑定处遇到同一占用；保留单项过滤并`--no-fail-fast`后的实际结果为98 passed/1 failed/1 ignored/1 filtered。未关闭外部实例或改动测试来伪造通过，完整回归由新提交的独立CI环境核验。
- `build_macos.py`首次exit0，图标原包/移位包字节、静态链接、ad-hoc签名与移位中英资源均通过；`swift test --package-path macos -c release`6/6通过。
- 构建完成后观察到默认dist应用图标回到此前ICNS，而冻结源未变化；不把此时默认产物当成新图标。已仅补一次构建，成功后立即核对默认bundle图标、复制至owned独立副本、再次核对图标字节并通过严格签名检查。默认dist与独立副本最终均匹配冻结ICNS。

原始命令输出只保留于ignored `dist/icon-preview/linked/yml-012-*.log`，不入库。独立副本通过CUA绝对路径实际启动，进程路径核对确认属于本次副本；标准系统“关于YM_Link”面板实际显示新蓝金链环。主窗口为已断开状态，验收后正常退出owned副本，外部checkout实例保持运行。Windows与官网最终结果继续记录，不沿用旧提交的绿色结果。

## 官网同步

游目后继[YM-320](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/archive/YM-320-ym-link-linked-logo.md)负责复制同字节源到独立`art/ym-link-icon.svg`；父品牌、首页、favicon、产品正文、下载Release元数据及部署逻辑保持。当前冻结15文件候选归档SHA`db9244cdd14f34ef6e80d2faefea6399ec9cdb951810a7925fff97549684e94e`，manifest SHA`2d37d5020e58a7551c40ee3c4f12a5a0fdbd5227daab77922ed468044595f4af`。34网站回归与9页build/check通过，两轮独立只读CR均无必须问题，三语桌面/390×844实际Chrome检查通过；同一冻结候选经TCCLI/TAT部署，25/25正常TLS回读及线上Chrome页眉实际确认通过。实现786bdc8a、收尾1768888f均已push且严格门禁通过。当前静态目录`20261030-ym-link-linked-logo-1`，上一`20261030-ym-link-app-logo-1`保留。


## 新资源CI

实现提交`a438c0b8ebbcbe85d3ccbc1a926cdf13f958382d`的[repository-checks run41082260025](https://github.com/forrestzhu/youmu-link/actions/runs/41082260025)三平台全部success，北京时间23:42:35完成；[gitleaks run41082260166](https://github.com/forrestzhu/youmu-link/actions/runs/41082260166)也success，无失败或重跑。

| 平台 | 实际结果 |
| --- | --- |
| Linux Ubuntu24.04.5 | Rust94 passed；Python47 ran/1 skipped，仓库79文本与rustfmt通过。 |
| Mac15.8.2（24H232）arm64 | Rust100 passed/1 ignored，两个本机被端口影响的测试均真实ok且无过滤；Swift6/6、重链接2/2、bundle图标/静态链接/ad-hoc签名/移位中英资源、DMG挂载与完整移位payload通过。Xcode26.3/SDK26.2/Swift6.2.4。 |
| Windows Server2025 Datacenter 10.0.26100 x64 | Rust103 passed/2 ignored，原生模型/本地化/WPF模板/实际DLL生命周期、EXE七帧图标/WPF窗口图标smoke、安装器图标/完整安装后payload/启动/退出/卸载全部通过。 |

Actions runner2.338.2；Linux image20261025.353.1、Mac image20261028.0489.1、Windows windows-2025-vs2026 image20261021.278.1。真实任务栏人工视觉、Windows10/ARM64、Intel Mac/macOS14未在本轮验收，不从CI推断。

制品保留到2026-11-06。以下SHA为GitHub artifact ZIP容器摘要，未在本机下载独立复验，不冒充内含安装器摘要或公开Release资产。

| Artifact | ID / 字节数 | 容器SHA-256 |
| --- | --- | --- |
| `YM_Link-macos-arm64-a438c0b8ebbcbe85d3ccbc1a926cdf13f958382d` | 12990950048 / 6698653 | `0c555c1fe9ae5ea86918fcb94254a6f565e9505841cc272fa995d3e8383016ed` |
| `YM_Link-windows-x64-a438c0b8ebbcbe85d3ccbc1a926cdf13f958382d` | 12989274934 / 124117439 | `8610cf0da420a2ad2deb144b2e4f108d5eecf5543c1b39f5fbbfe2d7800d57a4` |


GitHub README在Chrome实际刷新后确认新蓝金链环图标显示正常；未修改其他页面或用户标签，检查后恢复原先浏览器标签。公开GitHub Release及官网下载包仍为先前手动发布版本，本轮只更新源与CI制品，不声称新包已公开发行。
