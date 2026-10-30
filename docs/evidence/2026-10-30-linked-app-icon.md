# YM_Link 蓝金链环图标验收（2026-10-30）

任务：[YML-012](../tasks/YML-012-link-logo.md)。用户明确选定右侧加宽米白留白的蓝金链环版本；此前镂刻版的交付见[YML-011记录](2026-10-30-app-icon.md)。

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
- 构建完成后观察到默认dist应用图标回到此前ICNS，而冻结源未变化；不把此时默认产物当成新图标。正在补一次构建并立即保留独立验收副本，系统实显结果待记。

原始命令输出只保留于ignored `dist/icon-preview/linked/yml-012-*.log`，不入库。本轮完整Rust/Windows/Mac安装包CI、实际系统图标与官网结果尚待后续记录；不沿用旧提交的绿色结果。

## 官网同步

游目后继YM-318负责复制同字节源到独立`art/ym-link-icon.svg`；父品牌、首页、favicon、产品正文、下载Release元数据及部署逻辑保持。当前冻结15文件候选归档SHA`db9244cdd14f34ef6e80d2faefea6399ec9cdb951810a7925fff97549684e94e`，manifest SHA`2d37d5020e58a7551c40ee3c4f12a5a0fdbd5227daab77922ed468044595f4af`。34网站回归与9页build/check通过，第一轮独立CR无必须问题；第二轮CR、浏览器与上线回读待记。
