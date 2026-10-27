# YM_Link 互联协议规范

状态：**v0.1 草案**（2026-10-27）。§2 发现 beacon 已随 `ymlink-core` M0 骨架部分实现；§3-§5 为设计稿，字段在实现里程碑（M2 / M3 / M4）冻结前可修订，修订必须升小版本并同步两端实现。

互联模型参考内部项目 klip 的已验证设计；本文为 YM_Link 独立规范，报文与常量不复用 klip 取值。

## 1. 总则

- 传输：业务流量 TCP + Noise 加密；发现 UDP 广播 / 单播 + mDNS 辅助。无 HTTP。
- 序列化：CBOR，单报文 ≤512B（发现层）；业务层消息上限与分片在 §3 冻结。
- 版本：所有报文携带 `v`（uint）。未知字段一律拒绝（`deny_unknown_fields` 语义），向前兼容靠显式新版本 + legacy 双发，不靠宽容解析。
- 身份原则：**发现报文零身份信息**（只有 tag 指纹 / nonce / 端口 / 能力位）；设备名、平台、device_id 只在加密 Hello 内交换。

## 2. 发现层（M0 实现）

### 2.1 常量

| 常量 | 值 | 说明 |
| --- | --- | --- |
| `BEACON_PORTS` | 47610, 47611, 47612 | IANA 未注册段；绑定首个可用端口，其余做回退（与 klip 47470-47472 错开，避免串扰） |
| `MAX_PACKET` | 512 | 发现报文硬上限 |
| `PROTOCOL_VERSION` | 1 | 发现层版本 |
| `HEARTBEAT_INTERVAL_MS` | 60000 | 周期广播间隔 |
| `QUERY_REPLY_COOLDOWN_MS` | 1000 | 同一源 IP 两次单播回复的最小间隔 |
| `PEER_EXPIRY_MS` | 180000 | 3× 心跳无新 beacon 判过期 |

### 2.2 报文

```cddl
Beacon = { v: 1, tag_hash: bytes .size 16, session_nonce: bytes .size 16,
           listen_port: uint, caps: [ text ], discoverable: bool }
Query  = { v: 1, query: true, tag_hash: bytes .size 16, session_nonce: bytes .size 16 }
```

- `tag_hash = SHA256(tag_utf8)[0..16]`；YM_Link 默认 tag 为 `"youmu-link"`。tag 是分组指纹，**不是密码、不承载鉴权**。
- `session_nonce`：进程启动随机 16B，用于自回环抑制与对端重启检测（nonce 变化 = 重启，重置退避）。
- `caps`：能力位，M0 桌面端 `{ "discovery" }`；后续 `"files"` / `"mirror"` / `"control"`，单项短标识、总数 ≤8，接收方忽略未知能力。
- `listen_port`：M0 阶段为本机发现 UDP 端口（即 beacon 绑定端口）；M2 起改为本机业务 TCP listener 端口。

### 2.3 行为

1. **心跳广播**：每 `HEARTBEAT_INTERVAL_MS` 向各接口定向广播地址 + `255.255.255.255` 发一份 Beacon。
2. **显式 Query**：搜索状态下立即发出；接收方校验通过后**单播**回复一份 Beacon，同一源 IP 冷却 1s，防放大 / 回环。
3. **接收过滤**（不满足即静默丢弃）：
   - 源为私有 IPv4 且与本机某接口同网段（按 netmask 逐位比较；排除 loopback / 组播 / 网络号 / 定向广播地址本身）。loopback 仅显式配置（测试 / CI）允许。
   - `session_nonce` ≠ 本机 nonce（自回环抑制）。
   - `tag_hash` 匹配本机 tag（未配对不同 tag 的设备互相不可见）。
   - CBOR 解码通过、无未知字段、定长字段长度正确、总长 ≤512B。
4. **候选表**：按源地址记录 `{addr, listen_port, caps, nonce, last_seen}`；新地址或 nonce 变化 → `PeerDiscovered` 事件；`PEER_EXPIRY_MS` 无更新 → `PeerExpired`。上限 128 条 LRU。

## 3. 会话与配对层（M2，设计稿）

- Noise `Noise_XX_25519_ChaChaPoly_SHA256`，prologue `b"youmu-link-v1"`；每次连接完整握手，无会话 token。
- 身份：X25519 静态密钥 + Ed25519 签名密钥，存 OS keyring（service `app.youmu.link`，account `device-identity-v1`）；`device_id = hex(SHA256(noise_pub)[0..16])`。
- Hello（加密内）：tag_hash、device_id、双公钥、设备名、平台、`requires_pairing`、listen_port。
- 首连配对：SPAKE2 六位数字码，有效期 300s，绑定本条会话 handshake hash（防中继 / 跨会话重用），60s 内最多 5 次尝试；双端屏幕同时显示短码。
- 重连：静态密钥互信直接进入 paired；对端公钥与持久化不符 → 拒绝（`Paired identity key changed`），需删除信任重新配对。
- 心跳 / 空闲：会话内 60s Ping，130s 无收包判 peerUnresponsive；空闲 1s 发 Bye 优雅断开，presence 由 beacon 维持（分钟级 beacon + 秒级空闲断开的省电模型）。
- 信任持久化：桌面 SQLite（devices / peer_endpoints / trust_log），头显侧沿用游目既有存储，字段对齐。

## 4. 文件 lane（M3，设计稿）

- TCP listener 复用，magic 前缀 `YL-FILES1\0` 分流。
- 能力协商后：`List{path, depth}` / `Stat{path}` / `OpenRead{path}`（流式，Range 语义）/ `Transfer{files[]}`（≤128 文件 / 批，分块 ≤256KB 停等 Ack、断点续传、300s 无活动超时）。
- 桌面授权目录白名单为硬边界：白名单外路径一律 `denied`，不回显其存在性。

## 5. 镜像 lane（M4，设计稿）

- magic 前缀 `YL-MIRROR1\0`；会话内协商编码（H.264 / HEVC）、分辨率档位、帧率上限。
- 采集：macOS ScreenCaptureKit，Windows Graphics Capture；编码走平台硬件编码器，参数与实测延迟基准 M4 冻结后补 §5.3。
- 音频：可选系统音频轨，与视频同 lane 复用，时间戳同源。

## 6. 一致性测试要求

- §2 必须有双端（Rust / Kotlin）各自的一致性测试夹具：同构 beacon 样本（golden bytes）编解码比对。
- 协议改动流程：先改本文 + 版本号 → 双端实现 → 互操作测试（测试桩对拍）→ 合入。
