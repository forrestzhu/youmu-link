# 当前进度

更新：2026-10-27。只维护最近观察到的事实、未关闭问题和下一步；详细证据以链接为准。

## 当前制品与服务

| 项目 | 最近实际记录及边界 |
| --- | --- |
| 仓库 | GitHub 私有仓库 `forrestzhu/youmu-link` 建立（2026-10-27），暂闭源、无 LICENSE；后续转 GitLab 分发与开源由用户决定。 |
| harness | `scripts/harness.py` 任务闭环（YML 编号合同 / 状态 / Tests 字段 / 设备报告绑定）与仓库检查可用；CI 矩阵 ubuntu / windows / macos 跑仓库检查 + rustfmt + cargo test。 |
| 设计 | 产品定义、架构 ADR、[协议 v0](engineering/PROTOCOL.md)、[demo 设计](plans/2026-10-27-demo-design.md)与[可点击原型](../design/prototypes/desktop/2026-10-27-ym-link-demo/README.md)就位；原型仅证明信息架构与交互流，不是视觉合同。 |
| `ymlink-core` | 发现 beacon 骨架：CBOR 编解码（≤512B、deny-unknown）、端口回退、同网段 / tag / 自回环过滤、Query 单播回复与冷却、候选表事件；cargo 单测 + loopback 集成测试通过。配对加密、文件与镜像 lane 未实现。 |

## 当前工作与待验收

| 工作 | 当前结论 / 下一动作 |
| --- | --- |
| 框架启动 [YML-001](tasks/YML-001-bootstrap.md) | **doing**：本页与首任务；完成即归档。 |
| 双机 LAN 真实验收 | not-run：发现骨架仅 loopback 验证；需要第二台电脑 / 头显真机互联后补 docs/evidence 报告。 |

## 下一步

按 [ROADMAP](product/ROADMAP.md)：M1 头显侧 demo 评审 → M2 配对加密（Noise XX + SPAKE2 短码）与双机发现真实验收 → M3 文件浏览 / 流式播放 / 传输 lane → M4 屏幕镜像管线 → M5 游目端 YM_Link 面板集成（游目仓库 YM 任务）。Tauri 桌面壳随 M2 落地。
