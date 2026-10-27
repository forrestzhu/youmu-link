# YM_Link demo 设计（M1 原型）

日期：2026-10-27。任务：[YML-001](../tasks/YML-001-bootstrap.md)。原型：[design/prototypes/desktop/2026-10-27-ym-link-demo](../../design/prototypes/desktop/2026-10-27-ym-link-demo/README.md)。

## 目的与边界

在写桌面 UI 与游目面板代码之前，用可点击原型对齐**信息架构、状态集与交互流**。原型是纯前端模拟（无真实网络），用于 M1 用户评审；它不是视觉合同，也不约束最终视觉细节（间距 / 配色可调），但约束页面结构、状态名称与转移。

## 两视角与状态机

### 桌面端（youmu-link 主窗口）

页面结构：左栏「设备」+ 主区「屏幕镜像 / 文件共享」两标签 + 底部状态条。

```
未发现(搜索中) ──收到beacon──► discovered ──点击连接──► pairing(显示六位码)
pairing ──对端确认──► connected ──停止/断开──► disconnected ──beacon──► discovered
connected + 镜像开启 ──► mirroring（分辨率/帧率/延迟档位、停止按钮）
```

关键交互：
1. 设备卡片显示：设备名（配对后）、平台、状态徽章；「连接」→ 弹出本机六位配对码 + 等待。
2. 屏幕镜像标签：显示器卡片（主屏 / 副屏、分辨率）、音频开关、画质档位（流畅 / 平衡 / 画质）、「开始镜像」→ 状态条显示模拟码率与延迟。
3. 文件共享标签：桌面端只做授权目录管理（添加 / 移除，原型为占位）；目录浏览本身在头显侧进行（见下）。

### 头显端（游目内 YM_Link 面板）

页面结构：标题 + 设备列表 + 主区（配对输入 / 电脑屏幕 / 电脑文件 / 传输任务）。

```
未发现 ──► discovered(电脑卡片) ──连接──► entering-code(输入桌面端显示的六位码)
entering-code ──正确──► connected ──► browsing(电脑文件) / mirroring(电脑屏幕)
browsing ──选片播放──► streaming(进度/暂停) ──选传输──► transferring(进度/取消)
```

关键交互：
1. 电脑卡片：名称、系统、在线状态；连接后显示电脑屏幕入口与电脑文件入口。
2. 电脑文件：面包屑 + 目录表 + 视频行内「播放」「传输到本机」；播放进入流式播放态（模拟缓冲 / 进度）。
3. 传输任务：进度、暂停 / 取消；完成后引导到本地库。

## 状态命名（与实现一致）

原型实现为单一连接状态机加场景态：`link: searching / discovered / pairing / connected / disconnected`，场景态 `mirroring`（布尔）、`streaming` / `transferring`（由头显页 `streaming` / `transfers` 呈现）。与本文最初草案及 [ARCHITECTURE](../engineering/ARCHITECTURE.md) 术语的映射：桌面显示码与头显输入码（草案 `entering-code`）是同一 `pairing` 状态的两个视角呈现；`mirroring / streaming / transferring` 是 `connected` 下的场景态而非独立连接态；架构文档的 `connecting / paired` 对应实现中的 `pairing / connected`。产品实现期 UI 文案可本地化，状态机名称以本节映射为准。断线转移在原型中简化为 2.4 秒自动回到 `connected`（beacon 重发现路径属产品实现，原型不模拟）。

## 错误与边界展示（原型必须可见）

- 发现列表空态：搜索动画 + 「确认电脑已打开 YM_Link 且同一 Wi-Fi」提示（不显示 IP 术语）。
- 配对码过期（模拟 5 分钟倒计时）与重试。
- 连接中断：镜像 / 传输页显示「连接已断开，正在重连…」，恢复后继续。

## 评审关注点（M1）

1. 六位码出现在桌面端（显示）还是头显端（输入）是否直觉 —— 当前：桌面显示、头显输入。
2. 屏幕镜像入口是否需要独立于文件标签的更醒目位置。
3. 传输与流式播放的文案区分是否清楚（“传输到本机” vs “直接播放”）。
