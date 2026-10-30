# YML-011：YM_Link 家族应用图标与原生平台接入

Status: verifying
Workflow: maintenance
Requires-Device: no
Tests: 资源与打包回归、harness verify、Swift测试、Mac实际构建/启动与Windows CI；不改Rust协议/业务行为

## Scope

用户截图显示macOS Dock仍为默认占位图标，要求为YM_Link设计与Youmu呼应的程序Logo。沿用已批准YM字形及深蓝/米白/金色，先验证与Youmu的家族关系及桌面伴侣的视觉区分，再交付矢量源、PNG及macOS ICNS/Windows ICO，接入双平台应用/安装包构建。仅应用品牌资源与配置，不改协议、用户文案或公开Release资产；既有手动发布约定保留。

## Acceptance

- [ ] 图标与游目家族一致、无小字，16/32/64/128与大图可辨识，附实际预览。
- [ ] macOS bundle和Windows EXE/窗口使用同一资产，未来构建不丢失。
- [ ] 资源/脚本检查与适用完整verify、Swift测试、Mac实际app构建及启动有结果；Windows云构建结果与设备限制分别记录。
- [ ] 自查/独立审查、main提交推送与严格门禁闭环。

## Verification

核对主Logo源与图标几何，检查RGBA/ICNS/ICO多尺寸；打包资源字节核对。运行harness verify、Swift测试、Mac app构建/本机启动与Dock/Finder实际检查；Windows按CI真实构建/安装smoke。必要限制如实记录，未测试不冒记通过。不自动替换已公开Release。

## Evidence

开工main干净，fetch/ff-only无新更新，基线330a9f0。Mac Info.plist缺CFBundleIconFile，Windows csproj缺ApplicationIcon，仓库无应用图标资产，与用户截图的系统占位图一致。参考Youmu批准源art/youmu-icon.svg及主应用商店图标，只沿用既有品牌。

## Next

2026-10-30用户明确选择round-3第4个“镂刻”。已按所选结构重绘正式矢量、导出原生资源并通过Mac本机验收；完成main提交与三平台CI，随后归档。官网三语下载页的独立YM_Link页眉标识在游目YM-314跟踪，父品牌保持独立；不替换既有公开Release资产。


用户追加：2026-10-30要求5个Logo变体先看，视觉选择尚未完成。初版资产仅候选，不能作为通过。平台接入已准备、Python47及打包18回归和独立只读资源API审查通过；这些只证明配置 / 校验逻辑，不代表图标已获用户认可或原生包已启动。当前尚未构建 / 覆盖用户app、未提交推送、未发布Release。


五个生成式概念已完成，预览保存在ignored dist/icon-concepts/round-1/，带编号对比图和各自独立PNG。第5方向因与第1过近已换成“同心连接”，旧图保留于生成工具默认目录而不作为最终编号5。当前仅用户选型，不据此替换native资产或完成任务；已准备接入仍未提交。


用户复评：第一轮仅1号较成立，其余缺乏美感，5号产生不合适的旋转图腾联想；此前推荐4号撤回。重新核对品牌语法：M外轮廓、Y留白、克制的金色影院弧线构成统一识别，不能用独立连接符号拼接。round-2以原SVG作受控矢量推演：反白版本有家族区分价值；横向压缩、金色填Y、单独Y均经自查与独立只读视觉审阅淘汰，不作为同等强度方案交付。展示原标基准与深底伴侣候选并排图及32/48px预览，位于ignored dist/icon-concepts/round-2/family-comparison.png；仍待用户视觉反馈，未替换原生资产/应用、未构建/提交/发布。


用户再次明确要求5个新变体。round-3使用原Youmu SVG的无字标渲染作为参考，按同一米白/深蓝/金色体系分别生成：连弧（底弧贯通）、双幕（中央分体）、曲幕（上缘圆弧）、镂刻（轮廓M与实形Y）、折面（中央折线与轻微面色差）。五个独立原图、编号对比图及32/48px缩略预览已保存于ignored dist/icon-concepts/round-3/；concepts.json保存完整提示和生成源路径。五款全部供用户选择，未用旧图替代或只交付反白一款。一次完整视觉检查与独立只读对照确认了实际差异；未改原生资源/应用，仍待视觉选择。文档harness check与diff检查结果见本轮工具记录，不将概念图当成最终可分发图标。


定稿与本机验收：用户选定round-3第4个后，正式源assets/app-icon/ym-link.svg保留轮廓M、实心Y、分段金弧，清理生成稿边缘和材质；独立只读视觉对照无必须修复项。ICNS/ICO各帧已独立解码，16–256尺寸预览可辨。Python47、完整Rust100 passed/1 ignored、Swift6/6、Mac26.3.1 arm64实际构建/签名/移位中英资源通过。旧应用无活动连接后正常结束进程，重建并启动；CUA实际About面板和Finder均显示新图标。Dock直接观察超时，以运行时系统About与Finder结果为证，不冒记Dock截图通过。原生资源接入经独立只读审查无必须问题；Windows实际PE/WPF/安装器检查待CI。完整结果见[图标验收](../evidence/2026-10-30-app-icon.md)。
