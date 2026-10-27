# YM_Link 互联协议规范

状态：**v0.5 实现规范**（2026-10-27）。§2 发现线协议保持 v1；§3–§5 的应用 Hello、认证消息与业务报文为 v3；Noise 承载的 framing / magic / prologue 仍使用第 2 版定义。v0.5 补充发送拥塞降分辨率策略，不新增报文字段，应用线协议保持 v3。冻结不表示已完成 Quest 互操作或真实双设备验收。Quest / Kotlin 的 L4 一致性与互操作测试仍为 **not-run**。

互联模型参考内部项目 klip 的已验证设计；本文为 YM_Link 独立规范，报文与常量不复用 klip 取值。

历史文档 v0.3 修正 LAN 地址边界：以实际接口子网判定，不以 RFC1918 地址分类代替局域网；报文结构仍为发现 v1、会话与业务 v2。

## 1. 总则

- 当前传输：发现使用 IPv4 UDP 广播 / 单播，业务使用同一 TCP 连接上的 Noise 加密帧；无 HTTP、独立明文文件 / 媒体服务或常驻云端。mDNS / Bonjour 与 IPv6 尚未实现。
- 序列化：CBOR map，字段名为本规范所列的 UTF-8 文本；发现 UDP 数据报最多 512B，业务加密帧限制见 §3。CBOR map 不要求 canonical 排序。
- **字节序列的实际编码**：当前 Rust `Vec<u8>` 经 Serde / ciborium 编码为 CBOR unsigned-integer **array**，每项 0–255；不是 CBOR byte string。这同时适用于发现 v1 的 tag_hash / nonce，以及 v3 的公钥、SPAKE2 消息、MAC、文件和媒体 data。本文 `octets(n)` 表示恰好 n 个 uint8 的 CBOR array；`octets(*)` 表示变长数组。发现 v1 保留该既有编码，不在本轮静默改线格式。
- 版本：发现顶层 `v=1`；Hello、配对认证和应用外层 `v=3`。内层文件请求 / 响应以及 audio_config JSON 不另加 v。各类型拒绝未知字段，不兼容版本明确拒绝；当前不实现旧会话版本兼容或 legacy 双发。
- 身份原则：**发现报文零身份信息**，只含 tag 指纹 / 进程 nonce / 端口 / 能力位；设备名、平台、device_id 只在 Noise 加密 Hello 内交换。tag 是公开分组提示，不是认证凭据。

## 2. 发现层（v1 已冻结）

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

```text
Beacon = { v: 1, tag_hash: octets(16), session_nonce: octets(16),
           listen_port: uint, caps: [ text ], discoverable: bool }
Query  = { v: 1, query: true, tag_hash: octets(16), session_nonce: octets(16) }
```

- `tag_hash = SHA256(tag_utf8)[0..16]`；YM_Link 默认 tag 为 `"youmu-link"`。tag 是分组指纹，**不是密码、不承载鉴权**。
- `session_nonce`：进程启动随机 16B，用于自回环抑制与对端重启检测；相同源地址的 nonce 变化会再次发出 PeerDiscovered。
- `caps`：文本能力位，总数 ≤8；当前桌面广播 `["control","files","mirror"]`。发现模块单独运行的默认值为 `["discovery"]`。未知能力不赋予权限，接收方可忽略。
- `listen_port`：桌面运行时先绑定系统分配的非零 TCP listener 端口，再以 discovery 的 `advertised_port` 广播该 TCP 端口；UDP 绑定端口独立。仅独立发现测试未设置 advertised_port 时沿用本地 UDP 端口，不将其视为可配对服务。

### 2.3 行为

1. **绑定与心跳**：按 47610→47611→47612 顺序绑定首个可用 UDP 端口。启动即广播，之后每 60000ms 刷新接口列表，向去重后的各接口定向广播地址 + `255.255.255.255` 的**全部三个候选端口**发送 Beacon；不能只向本机绑定端口广播。
2. **显式 rescan / Query**：桌面启动和用户重扫时立即刷新接口，并向上述全部广播地址 × 三个候选端口发送 Query，再广播本机 Beacon。测试显式允许 loopback 时另探测 127.0.0.1。接收方校验通过后向 Query 的实际源 IP:UDP 端口**单播**回复 Beacon，同一源 IP 冷却 1s，防放大 / 回环。手动 IP:TCP 端口直连使用与自动发现相同的会话及配对语义。
3. **接收过滤**（不满足即静默丢弃）：
   - 源为可单播 IPv4 且与本机某个有效 IPv4 接口同网段（按 netmask 逐位比较，接受实际配置的企业 / 公网地址段及链路本地网段；拒绝未指定地址与零掩码接口，排除 loopback / 组播 / 网络号 / 定向广播地址本身）。loopback 仅显式配置（测试 / CI）允许。
   - `session_nonce` ≠ 本机 nonce（自回环抑制）。
   - `tag_hash` 匹配本机 tag（未配对不同 tag 的设备互相不可见）。
   - CBOR 解码通过、无未知字段、定长字段长度正确、总长 ≤512B。
4. **候选与限额**：按源 IP 记录 `{addr, listen_port, caps, nonce, last_seen}`；新地址、nonce 或公告 TCP 端口变化 → PeerDiscovered，其余心跳只刷新记录；超过 180000ms 无更新 → PeerExpired。候选表最多 128 条，满时淘汰最久未更新者并发 PeerExpired；Query 冷却表硬上限 256 个来源。发现仅提示可达地址，不能更新信任或证明已认证在线；同 IP 多实例共享一个发现候选槽。

## 3. 会话与配对层（M2，应用 v3 / transport-v2 已冻结）

### 3.1 连接与身份

- TCP 控制连接前缀为 `YL-CTRL2\0`（9 字节）；其后每帧为大端 `u32` 长度 + 消息。Noise 握手帧上限 4096B，传输密文上限 65535B（明文最多 65519B）；零长度、超限或截断即关闭连接。完整握手限时 5s。
- Noise `Noise_XX_25519_ChaChaPoly_SHA256`，prologue `b"youmu-link-v2"`；每次 TCP 连接完整握手。业务不会复用旧会话 token。双方独立、顺序递增的 Noise nonce 从 0 开始；不发送明文业务内容。
- 身份仅使用 Noise X25519 静态密钥；`device_id = hex(SHA256(noise_pub)[0..16])`。本阶段不做组内信任事件传播，因此删除未使用的 Ed25519 身份；macOS 私钥保存于 OS Keychain（service `app.youmu.link`，account `device-identity-v2`），Keychain 失败不得回退明文文件。
- 首个传输帧为 CBOR Hello：`{v:3,tag_hash,device_id,public_key,name,platform,listen_port}`。`tag_hash` 为 octets(16)、`public_key` 为 octets(32)，device_id 为 32 个小写十六进制字符，名称 1–128 UTF-8 字节、平台 1–32 UTF-8 字节且均无控制字符、端口非零；未知字段拒绝。Hello 内的公钥与 Noise 握手认证的 remote static 必须完全相同，device_id 必须由该公钥派生；tag_hash 必须等于本机组指纹。
- TCP 监听 IPv4；产品模式只接受 / 拨号实际接口同子网的可单播 IPv4（包括企业地址段和链路本地地址），loopback 仅显式测试配置允许。最多 4 个入站连接任务，单个主动拨号流程。双向竞争时同一身份对以 `initiator == (local.device_id < remote.device_id)` 确定首选方向，仅保留一个 active；不同设备不能顶替当前 active。
- 已配对公钥不匹配、已绑定端点出现新身份或自动重连目标 device_id 不符均拒绝，不以新 id 静默绕过旧信任。原生界面的显式断开禁止该身份入站 / 自动重连，直到用户主动连接；断线后的已信任设备每 5s 重试一个可用候选端点，未信任设备始终需要用户连接及短码。

### 3.2 首次配对与重连

- Hello 后双方发送加密 `{kind:"known",v:3,known:bool}`；只有双方都已持久信任对方的本次 Noise 静态公钥，才可跳过首次配对。任何一端忘记信任都必须重新配对；单方宣称 `known=true` 不足以放行业务。
- 桌面端显示随机均匀六位数字码，头显端输入。显示 / 输入角色独立于 TCP 发起方；当前两台 macOS 桌面互测时，TCP responder 显示、initiator 输入。平台标识 `headset-test` 的 Rust 测试对端始终输入桌面显示的码。码有效期 300s（单调时钟）；取消、过期、错误码或连接断开立即废弃本次配对。
- 配对使用 SPAKE2 Ed25519Group：显示方 `start_a`，输入方 `start_b`。身份串分别为 `b"youmu-link-display-v2" || handshake_hash` 与 `b"youmu-link-enter-v2" || handshake_hash`，将 PAKE 结果绑定本次 Noise 握手，抵抗中继和跨连接重用。先各发 `{kind:"spake",v:3,message:octets(33)}`，再计算共享 key。
- 双向 key confirmation 使用 HMAC-SHA256：key 为 SPAKE2 输出，消息分别为 `b"youmu-link-confirm-display-v2" || handshake_hash` 与 `b"youmu-link-confirm-enter-v2" || handshake_hash`；传输 `{kind:"confirm",v:3,mac:octets(32)}`，常量时间验 MAC，随后双方发送 `{kind:"accepted",v:3}`。任何失败即关闭连接、不保存信任、不开放文件或镜像业务。
- 运行时按来源 IP 跨连接限制 60s 内最多 5 次首次配对；每条连接仅允许一次 SPAKE2 尝试。限速在生成 / 展示新配对码前检查，避免攻击者无限重连猜码。
- 信任存储为应用私有目录中的小型 JSON（目录 0700、文件 0600、原子替换）；仅保存设备 id、公钥、名称、平台和最后端点，不保存私钥或短码。删除信任立即关闭对应活动连接。公钥 pin 不符返回 `IdentityChanged`，需要用户删除旧信任后重新配对。

### 3.3 已授权传输

- 只有通过双向 Known 或 SPAKE2+双向 key confirmation 的连接才允许 `into_split()`。每个方向的 nonce 从握手后的首个加密 Hello 帧起计数，配对认证与应用帧连续递增，拆分或切换消息类型不重置。单方向帧保持顺序，解密 / CBOR 错误、CBOR 后残留字节或未知应用类型立即关闭。
- 应用消息外层以 `kind` 的 snake_case 文本区分类型并携带 `v:3`；控制、文件、传输及媒体复用同一已授权 TCP。`ping` / `pong` / `bye` 均为 `{kind,v}`；收到 ping 回复 pong，bye 关闭会话。
- Rust sender 的控制队列 32 项、媒体队列 8 项、接收队列 32 项，采集包队列 2 项，均有界；独立控制 / 媒体队列复用一个顺序 writer，控制和入站处理优先于待发送媒体。已开始写出的单帧仍按顺序完成，不在帧中插入控制数据。慢接收端不应让本地主控制 actor 等待满媒体队列；停止镜像 / 断开可取消媒体生产。
- 会话内每 60s 发送 ping；单次完整帧读取 / 写入限时 130s，超时关闭连接。持续文件 / 镜像任务保持连接，显式断开或网络失败关闭并停止采集；不在活跃播放 / 传输中做 1s 空闲断连。

## 4. 文件服务（M3，应用 v3 已冻结）

### 4.1 授权与请求

- 文件请求复用 §3 已授权的 Noise 控制连接，不再建立未认证的独立明文文件 lane。外层确切格式为 `{kind:"file_request",v:3,id:uint64,request:FileRequest}` 与 `{kind:"file_response",v:3,id:uint64,response:FileResponse}`；响应沿用请求 id，实际字段名为 `id`。内层 CBOR map 也以 `kind` 的 snake_case 文本区分变体，未知字段拒绝。
- 桌面用户通过原生目录选择器显式授权；远端仅见 `ShareInfo{ id, name }`，请求只传 `share_id` 与以 `/` 分隔的 UTF-8 相对路径，绝不传输本机授权根绝对路径。最多 64 个共享目录，id 为 1–64 字节 ASCII 字母 / 数字 / `_` / `-`，展示名称 1–128 字节。
- 授权根在用户授权时打开为 `cap_std::fs::Dir` capability；后续所有 metadata / open / enumerate 操作相对于该句柄进行，不以字符串前缀检查或先 canonicalize 再环境 open 代替授权。路径最多 4096 字节，每段最多 255 字节；拒绝绝对路径、`..`、`.`、空中间段、反斜线、冒号和控制字符。空路径仅表示共享根。
- capability 解析拒绝越界 symlink，防止 symlink / rename 竞态逃逸；根内相对 symlink 可访问。只提供普通文件与目录，FIFO / socket / 字符及块设备不得读取或列出。Unix open 使用 `O_NONBLOCK` 后检查同一文件句柄的 metadata，避免攻击者在类型检查和 open 之间换入 FIFO 导致阻塞。撤销共享后每个新请求立即被拒绝。

```text
ShareInfo = {id:text,name:text}
FileRequest = {kind:"shares"}
            | {kind:"list",share_id:text,path:text,cursor:text|null,limit:uint16}
            | {kind:"stat",share_id:text,path:text}
            | {kind:"read",share_id:text,path:text,version:text,offset:uint64,length:uint32}
FileEntry = {name:text,path:text,is_directory:bool,size:uint64,version:text|null}
FileResponse = {kind:"shares",shares:[ShareInfo]}
             | {kind:"list",entries:[FileEntry],next_cursor:text|null}
             | {kind:"stat",entry:FileEntry}
             | {kind:"read",offset:uint64,total:uint64,version:text,data:octets(*),eof:bool}
             | {kind:"error",code:text}
```

### 4.2 分页、流式读取与断点

- List 按 UTF-8 文件名的字节序升序，cursor 为上一页最后一个已返回文件名（exclusive），limit 必须为 1–256。单页另设保守 48KiB 内容预算，防止长路径列表超出 Noise 明文上限；`next_cursor=null` 表示本次枚举已结束。并发目录增删采用尽力而为分页，需要一致视图时客户端重扫。
- Stat 的普通文件 `version` 是不透明的 64 字符 SHA-256 token；macOS / Unix 以同一文件句柄的设备号、inode、大小、mtime 与 ctime 纳秒值派生，目录为 null。它是变化检测 token，不是文件内容哈希；不承诺本地恶意写入者保留所有 metadata 时的内容完整性。
- Read 要求 version 与当前同 fd metadata 相同，offset 不大于大小，length 为 1–16384 字节。读取后再次复核同 fd metadata，变化 / 替换 / 截短均返回 `changed` 并丢弃本块。数据按剩余大小截短；offset 等于大小返回空数据与 eof=true。单块最坏 CBOR uint 数组编码仍小于 §3 的 65519B 明文限制。
- 流式播放使用 Read range / seek；当前桌面实现提供加密范围读取服务，Quest Media3 的实际解码 / 播放仍为 not-run。接收端处理落盘、文件名冲突与原子完成，本服务只读，不允许远端写入电脑。下载使用下节停等传输。
- 错误码为 `denied` / `invalid_request` / `unavailable` / `not_file` / `not_directory` / `changed`。未授权 share、非法路径与越界访问返回 denied，错误不含本机路径或授权外文件存在性。客户端不得把错误正文作为路径日志记录。

### 4.3 停等传输与断点恢复

外层应用消息均为下列 CBOR map，`v` 固定为 3：

```text
{kind:"transfer_open",v:3,id:text,share_id:text,path:text,version:text,offset:uint64}
{kind:"transfer_chunk",v:3,id:text,offset:uint64,total:uint64,version:text,data:octets(*),eof:bool}
{kind:"transfer_ack",v:3,id:text,offset:uint64}
{kind:"transfer_action",v:3,id:text,action:text}
{kind:"transfer_error",v:3,id:text,code:text}
```

- id 为 1–64 字节 ASCII 字母 / 数字 / `_` / `-`。每条连接最多 4 个服务端传输 job；同一连接最多接受 1024 个不同 id，任何已使用 id 不可重开，避免旧 Ack / 控制误作用于新任务。重连后可用原 id 续传；超过上限需重连。桌面任务视图最多保留 128 条最近记录。Open 的 share_id / path / version / offset 受 §4.1–§4.2 同样的授权、版本和偏移校验。
- 接收端先 Stat 取得 version / size，再 Open；服务端最多发送 16384B 的一块数据，然后等待 Ack。Ack.offset 必须恰好等于刚发块 offset + data 长度，不是块起始位置；错误 Ack 关闭连接。仅接收端成功落盘后才能 Ack，服务端接到 Ack 才推进下一块。offset 等于 total 时返回空 data、eof=true，仍需要 Ack；空文件同理。
- 接收端逐块核对 id、连续 offset、固定 total / version、数据长度和 eof 与末尾位置一致；不符即停止，不能把不同版本拼接。服务端在每块读取前后核对文件版本；错误发 transfer_error，不携带本地绝对路径。`code` 当前是路径无关的错误文本（FileError 的显示文本或 `transfer timed out`），不是 §4.2 的 FileResponse.error 枚举编码；接收方应作为错误展示，恢复前重新 Stat。
- action 仅为 `pause` / `resume` / `cancel`，可由接收端或桌面原生控制发起。pause 停止新块，已经在途的一块仍可到达并 Ack；resume 继续当前 job；cancel 移除 job，本连接已使用且不再活动的 id 的迟到 Ack 被忽略。未知任务或非法 action 是协议错误；本地已经终态的过期控制只报告 UI 错误，不关闭正常连接。
- 传输活动时间在 Open / 合法 Ack 更新；包括暂停在内，超过 300s 无活动的 job 在 60s 会话维护 tick 检查时失败、移除并发 transfer_error。阈值为 300s，检查不是精确计时器；过期后需接收端重新 Open。
- 断开销毁该连接的服务 job；尚未完成的桌面记录进入 `waiting`，等待接收端重新打开。此时桌面的 resume 不伪造服务 job，仅提示由接收端恢复；cancel 可本地关闭 waiting 记录。
- 接收端保留部分文件及 version / total / 已落盘 offset；重连后先 Stat 核对原 version / size，再用同一来源及已落盘 offset 发新的 Open。变化则拒绝混合续传。Rust `headset_stub` 使用 `.part` + 本地 JSON checkpoint，最终以不覆盖已有目标的方式完成；取消 / 失败时保留部分文件供用户处理。服务端 completed 仅代表全部字节获 Ack，不独立证明接收端最终文件命名或解码成功。

参考：[cap-std capability 目录 API](https://docs.rs/cap-std/latest/cap_std/fs/struct.Dir.html)。

## 5. macOS 屏幕镜像（M4，应用 v3 已冻结）

### 5.1 控制与媒体封装

- 复用 §3 已授权的 Noise 连接；不另开未认证媒体端口。桌面原生界面选择显示器并显式开始 / 停止，断开会话停止采集。开始前由 macOS Screen Recording 权限保护，拒绝权限不得回退其它采集方法；首次开始仅允许本机发起，已授权对端的暂停 / 恢复 / 停止 / 切屏见 §5.4。
- 外层确切字段如下，`media_kind` 不与外层 `kind` 混淆：

```text
{kind:"mirror_state",v:3,status:text,stream_id:uint64}
{kind:"media",v:3,stream_id:uint64,packet_id:uint64,part:uint32,parts:uint32,
 media_kind:text,codec:text,pts_us:uint64,keyframe:bool,width:uint32,height:uint32,
 data:octets(*)}
```

- mirror_state.status 为 `running` / `paused` / `stopped` / `error`；每次开始随机生成 stream_id，packet_id 从 0 递增；一个完整包各片的所有头字段相同，part 从 0 到 parts−1 连续递增。data 每片不大于 8192B，一个完整包最多 16MiB / 2048 片；接收方有界重组完整包后才能解释配置或解码。media_kind 为 `video_config` / `video` / `audio_config` / `audio`，具体格式见下文。
- mirror_state 走优先的 32 项控制队列，media 走独立 8 项有界媒体队列，最后由独立单一加密 TCP writer 发送。慢写不会阻止读取对端停止指令；控制队列满时明确关闭拥塞连接。running 在新流数据前公告；结束通知尽力发送，可能先于旧队列残留片段到达。接收新流 running 或当前流 paused / stopped / error 时，接收方清理对应旧流重组 / 解码状态，丢弃旧 stream 的迟到片段 / 结束通知，不能让半个旧包污染新流。不能依赖结束通知一定送达；断开同样销毁旧流。
- `pts_us` 为 ScreenCaptureKit 同源采样时间戳换算的微秒值；视频编码不启用 B 帧，PTS 顺序即解码顺序。接收方以音频 / 视频共同时间轴调度，不把发送端 host 时间戳当作跨机器绝对墙钟。音频包 width / height 为 0，视频及视频配置携带实际编码尺寸。

### 5.2 视频

- macOS 使用 Rust 调用 ScreenCaptureKit 获取所选显示器，以 VideoToolbox **要求硬件编码**的 H.264 Main 编码器处理 CVPixelBuffer。硬件编码不可用或运行中失败明确报错；不静默切换软件编码。
- `video_config` 的 codec 为 `h264-avcc`，data 是 AVCDecoderConfigurationRecord（avcC）：版本 1、4 字节 NAL 长度、一个 SPS 和一个 PPS。每个关键帧前重复发送配置；`video` 包为 AVCC 格式（每个 NAL 前 4 字节大端长度），keyframe 由编码样本 sync attachment 判断。
- 分辨率保持源比例并取偶数尺寸。初始可调档位为 smooth（长边不超过 1280，30fps，4Mbps）、balanced（长边不超过 1920，60fps，12Mbps）、quality（长边不超过 2560，60fps，24Mbps）；这些为编码目标上限，不是画质 / 延迟保证，实际指标按真实硬件与接收端验收。
- 媒体队列有界；慢接收端时停止积压视频，丢帧后请求下一帧为关键帧，并在关键帧前重新发送配置。不得在解码依赖已丢失时持续发送后续 P 帧。停止 / 断开后镜像需用户重新开始；v3 的暂停 / 恢复与双端控制见 §5.4。
- 发送端每 2s 观察一次背压。连续两个窗口均满足以下任一条件：媒体队列满，或视频至少丢 10 帧且占尝试帧数至少 20%，将实际编码长边降至当前的 3/4（偶数尺寸，最低长边 640，不放大源尺寸），目标码率按实际像素面积比例降低（最低 256kbps，且不超过所选档）。短暂单窗口拥塞不切档；阈值是初始控制参数，真实弱网效果仍需硬件验收。
- 降分辨率沿用原显示器 / 音频选择和用户画质上限，停止并释放旧采集后创建新 stream_id、重新发配置与关键帧；不重置 Noise nonce，不再次申请录屏权限。一个开始周期只降不升，手动重新开始 / 恢复 / 切屏从用户所选档重新评估；最低分辨率持续拥塞则明确报慢连接错误并停止，不无界积压媒体。
- 系统停止 / 编码失败等致命错误独立于有界媒体队列保存，并优先于自动降档。暂停、恢复与切屏保留同一本机开始周期的故障信号；旧采集 teardown 才报告的错误也禁止恢复新采集。只有本机再次明确开始才能建立新的授权周期，不能用弱网自动重启绕过错误解除授权。

### 5.3 系统音频与错误

- 用户可选择系统音频。ScreenCaptureKit 请求 48000Hz、双声道系统音频（排除本进程声音），实际采样格式逐包核验为线性 PCM 32 位浮点、小端、48000Hz、2ch；平面音频转为交错排列，未知格式报错，不把未知数据误标为 PCM。
- `audio_config` 的 codec 为 `pcm-f32le`，data 为 UTF-8 JSON `{"sample_rate":48000,"channels":2,"sample_format":"f32le","interleaved":true}`；`audio` 为左右声道交错 f32le 原始样本（每帧 8B）。关闭音频时不注册音频 output、不发送音频包。
- 异步采集 / 编码失败以内部 `kind=error` 包向运行时上报（data 为不含文件路径的 UTF-8 错误），由运行时停止镜像并更新 UI；该内部包不作为视频帧发送给对端。拥塞可丢弃完整音频包，接收端按 PTS 跳过空隙。
- 真实 Quest 解码、音画同步、延迟 / 帧率与弱网体验仍需双端实测；单机硬件编码测试只证明本机采集 / 编码 API 路径。

参考：[Apple ScreenCaptureKit](https://developer.apple.com/documentation/screencapturekit)、[VideoToolbox compression session](https://developer.apple.com/documentation/videotoolbox/vtcompressionsession)、[Rust screencapturekit API](https://docs.rs/screencapturekit/latest/screencapturekit/)。

### 5.4 双端镜像控制（应用协议 v3，先冻结后实现）

- 发现仍为 v1。Noise XX 的 framing、`YL-CTRL2\0` magic、`youmu-link-v2` prologue、PAKE domain 与身份 keyring 格式保持不变；加密 Hello 的 `v` 与随后认证 / 业务消息 `v` 为 3。Hello v2 / v3 不兼容时在加密层明确拒绝并提示两端升级，不静默降级；既有 Keychain 身份与静态公钥信任不因版本升级重建。
- 已授权对端可发送 `MirrorQuery{v:3,id}`，或 `MirrorControl{v:3,id,command}`；command 用 `action` 标记，取 `pause` / `resume` / `stop` / `select_display{display_id}`，未知字段和动作拒绝。
- 实际 CBOR map 字段如下；id 为请求方分配的 uint64，reply 沿用同一 id，成功时顶层 error 为 null。嵌套 command、state 和 displays 项同样拒绝未知字段。

```text
MirrorAction = {action:"pause"} | {action:"resume"} | {action:"stop"}
             | {action:"select_display",display_id:uint32}
MirrorInfo = {status:text,display_id:uint32|null,audio:bool,quality:text,
              error:text|null,displays:[{id:uint32,name:text,width:uint32,height:uint32}]}
{kind:"mirror_query",v:3,id:uint64}
{kind:"mirror_control",v:3,id:uint64,command:MirrorAction}
{kind:"mirror_reply",v:3,id:uint64,ok:bool,error:text|null,state:MirrorInfo}
```

- 响应 `MirrorReply{v:3,id,ok,error,state}`。state 为 `{status,display_id,audio,quality,error,displays:[{id,name,width,height}]}`，是当前已接受命令后的实际状态；开始 / 恢复 / 切屏可先为 starting，真实采集成功后再发送 running。失败为 ok=false 与不含路径的 error，业务失败不应断开已认证连接。
- 首次开始必须由电脑本机明确发起。对端只能在本机已启动的 running / starting / paused 会话中恢复或切屏；实际恢复采集还要求现有系统录屏权限有效，对端请求绝不申请系统授权。显式 stop、断线或采集失败后解除本次授权，对端不能重新开启录屏。
- pause 停止真实采集与编码，状态为 paused，保留显示器 / 音频 / 画质选择；接收端保留最后画面并停止接受该流后续分片，丢弃半包。resume 创建新 stream_id 并重新发配置与关键帧，不依赖旧编码状态；同一 Noise 会话的各方向 nonce 继续递增，绝不重置。平台采集启动 / 停止串行化，旧采集实际释放后才启动新采集。
- select_display 在运行中停止旧流、切到当前实际存在的显示器并创建新流；paused 时只改选择，保持暂停，不采集。不存在的显示器拒绝。重复 pause / stop 可成功返回当前状态，running 的重复 resume 不重启流。
- MirrorState 的 status 扩展为 running / paused / stopped / error（stream_id 保持对应流）；MirrorReply state 另可为 starting。旧流通知不能覆盖新代际，旧流 paused / stopped 不能清空新流；命令有界排队，超时或忙碌明确拒绝。

### 5.5 版本兼容矩阵

| 发现版本 | 加密 Hello / 认证 / 业务版本 | Noise 承载 | 行为 |
| --- | --- | --- | --- |
| v1 | v3 ↔ v3 | transport-v2 | 当前支持：原文件 / 媒体消息及 §5.4 镜像控制 |
| v1 | v2 ↔ v3 | transport-v2 | 发现仍可互见；Noise 后的首个加密 Hello 即拒绝，明确提示两端升级；不进入配对 / 文件 / 录屏 |
| v1 | v2 ↔ v2 | transport-v2 | 旧文档 v0.2 / v0.3 的历史实现，当前应用不提供兼容或降级 |
| 非 v1 | 任意 | 任意 | 发现解码拒绝；不能据此建立应用兼容结论 |

transport-v2 专指 `YL-CTRL2\0`、`youmu-link-v2` prologue、`youmu-link-display-v2` / `youmu-link-enter-v2` 与双向确认 domain、原 Noise framing，不代表应用消息仍为 v2。升级应用到 v3 不轮换 `device-identity-v2` Keychain 条目，不清空静态公钥信任；已信任对端须双方都升级后才可按原互信重连。文档版本 v0.5、发现 v1、应用 v3 与 transport-v2 分别编号，不能混用。

## 6. 一致性与验收边界

- 已提供独立手工编码的 [发现 v1 固定向量](../../core/tests/fixtures/discovery-v1/README.md)，Rust `protocol_vectors` 测试核对语义、uint 数组编码与 map 键顺序无关性。Kotlin 尚未读取对拍，L4 仍 not-run。

- 当前实现与可运行检查入口分别为 `core/src/{beacon,discovery,identity,session,files}.rs`、`desktop/src/{wire,runtime,capture}.rs`；Rust 测试对端为 `desktop/src/bin/headset_stub.rs`。测试对端通过真实 TCP / Noise / SPAKE2 通信，不能视作已完成 Quest 产品端。
- Rust 单元 / 集成、同机 loopback、原生 UI、硬件编码分别证明各自范围；结果以任务与 evidence 的实际记录为准。本文冻结字段不自动证明任一验收通过。
- **Quest / Kotlin L4 golden 报文一致性、跨端配对、真实 LAN 双设备发现、Quest 解码 / 播放、音画同步及佩戴体验均为 not-run，尚无跨设备兼容结论。** 后续双端夹具必须按 §1 真实 uint-array 编码实现，不能误按 CBOR byte string 对拍。
- 协议改动先修订本文与版本 → 修改相关端实现 → Rust / Kotlin golden 报文与真实互操作 → 记录结果后交付。不兼容变更不得只改一端并继续宣称兼容。
