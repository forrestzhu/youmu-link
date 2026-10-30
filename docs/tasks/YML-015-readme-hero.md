# YML-015：GitHub 首页示意图视觉升级

Status: verifying
Workflow: maintenance
Requires-Device: no
Tests: not-applicable (仅 README 图像与文档引用更新，无应用或协议行为变化；执行图像视觉检查、独立审阅与 harness check)

## Scope

按用户要求用内置 imagegen 重新设计 GitHub 首页的电脑与 Quest 互联示意图，提升设备表现、留白与排版质感；保留电脑授权片库、局域网加密、四位配对及 Quest 端入口等事实。新图以独立文件保存并接入中英 README，原 SVG 保留。应用图标、应用实现、官网与发布制品不在本次范围。

## Acceptance

- [x] 生成并检查新的横向图像，电脑与头显关系清楚，正文缩放可读。
- [x] 图中文字与产品事实一致，保留实验镜像限制，未虚构性能或验收结果。
- [x] 中英 README 引用项目内新资源并保留有效替代文本；记录生成提示词与工具。
- [ ] 独立审阅、harness check、自查 diff、main commit / push 与 require-pushed 门禁完成。

## Verification

检查生成原图及 GitHub README 常见显示宽度下的图像，核对文字、设备造型、内容层级与明暗背景适用性；独立 subagent 核对产品事实与可读性。用 harness check 检查链接与任务合同，git diff --check 检查补丁格式。Rust / Swift / Windows / 真机测试 not-applicable：本次没有应用代码或协议变化。

## Evidence

开工 main 干净；fetch 后 ff-only 从 5e85961 同步到 1671533，保留上轮图标任务收尾。已检查用户截图、原 SVG、中英 README、产品定义与最新状态；原图信息完整，但设备符号与排版偏基础说明图。用户明确希望使用生图技能提升 GitHub 首页图品质。

使用内置 imagegen（未调用 CLI / API key），首轮按用户截图的信息关系重新生成暖白背景、海军蓝设备与文字、香槟金连接线的 3D 产品示意图；第二轮仅要求增加辅助文字尺寸，并将实验说明从浅金改为深灰蓝。最终原始 PNG 为 1942 × 809，直接复制入 docs/assets，不做外部重绘或转码；SHA-256：12b284ba9dd5c1517823b55d68de481bd26c699595cece92e5b4800323f94084。两轮完整提示词见 docs/assets/ym-link-overview-v2.prompt.txt；保留旧 ym-link-overview.svg。

实际本地浏览器检查：850px 宽度在白色与 GitHub 深色背景上文字可读、设备未裁切、关系清楚。390px 手机缩略图仍可辨识标题与电脑 → Quest 关系，辅助细节需要点开原图；中英 README 正文与完整 alt text 保留相同信息及实验边界，不声称手机缩略图中的全部文字可读。本地预览页只用于验收，不入库。

独立 subagent hero_audit 检查原图、产品定义、首轮生成图与最终 PNG / README diff / 提示词：首轮指出实验说明偏小且对比低，第二轮放大并改深色后复审通过，无剩余可操作问题；未新增性能或真实播放验收承诺。自查 README 引用与 git diff --check 通过。

2026-10-31：python3 scripts/harness.py check 通过（82 text files），仅证明仓库记录与本地链接合同；应用代码未修改，Rust / Swift / Windows 构建及真机测试 not-applicable。

首次 main 推送因远端并行增加 Windows 性能测试包记录而被拒；fetch 后将本地尚未推送的图像提交重放至 1654d97，保留远端记录。远端已分配 YML-014，本任务同步顺延为 YML-015，不改写已推送历史、不强推。

## Next

执行 harness check，提交并推送 main 后执行 require-pushed；通过后归档并更新 STATUS。
