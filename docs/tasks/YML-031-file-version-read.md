# YML-031：Windows 文件读取版本误判

Status: verifying
Workflow: behavior
Requires-Device: yes
Tests: cargo test --locked -p ymlink-core --test files; cargo test --workspace --all-targets --locked (CI)

## Scope

用户在实际 Windows / Quest 连接后报告首个约 5GB MP4 不可读，其余视频可播。桌面负责修复版本签发 / 首次读取误判，Quest 的目录名称与端侧回归见 [YM-334](https://github.com/forrestzhu/youmu/blob/main/docs/tasks/YM-334-link-large-file-and-breadcrumb.md)。不改用户媒体、不扩权限、不降级内容变化守卫；按用户要求不用 Computer Use，也不再要求重复人工复现。

## Spec

给定授权目录内未改变内容的普通 Windows 文件，List / Stat 签发版本后首次 Read 不因首次访问的元数据更新误报 Changed。只在签发版本前以只读 capability 句柄尝试 1 字节读取，关闭后重开并从最终句柄计算完整版本；offline / recall 标记跳过新增的数据读取。List 只为实际返回页准备文件，不为未返回候选做数据 I/O。保留文件 ID / ChangeTime 以及 Read 前后校验；文件替换、同文件等长覆写且恢复 mtime 仍应拒绝旧版本。协议 v4 报文和版本格式不变，旧缓存 token 不自动迁移。

## Acceptance

- [x] 授权共享目录的同源自动探测证明 List / Stat token 与首读、seek / EOF 相容；原 Changed 失败证据保留。
- [x] 只读 5GiB 稀疏文件、64 位偏移、原文件替换与等长覆写 / 恢复 mtime 回归通过。
- [ ] 第一轮独立 CR 属实问题修复后先推 main，随后独立第二轮及适用门禁完成。
- [ ] 更新安装版并验证真实双端制品；未执行的设备播放不记通过。

## Verification

先用同源 FileCatalog 在已授权共享内仅读少量 header / 元数据，输出枚举错误码与版本分量是否变化，不输出名称、路径或数据。相关层运行 core files 测试；仓库层运行 Python 回归、check / rustfmt。完整 harness verify 与三平台 cargo / Windows 构建由 CI 执行；本地运行桌面与真实 Quest 当前占用发现端口，不以会碰触 LAN 的并行测试干扰当前会话。安装候选先绑定源码 / DLL SHA，再验证 ABI / startup。Quest 新包与设备验收在 YM-334 跟踪。

## Evidence

2026-10-31 初始 a16d6f7；fetch / ff-only 至 c383805，带入另一会话的 Windows 设备状态 UI。其归档任务已占 YML-030，故本新任务在提交前改为 YML-031；未覆盖另一任务。

用户连续人工重现后，JDWP 仅取得异常链 Read 字段校验 → DataSource → 16 字节容器检测；早期探针没有可靠取得错误码，此后不再要求用户重现。同源只读探测在真实授权 NTFS 共享的多份冷文件上取得 Changed，只有 ChangeTime / LastAccessTime 改变，二者初值 / 终值相等且增量相等；size / mtime / 创建 / ID / 属性未变，连续 Stat 稳定。非通用 5GB 整数溢出；不能从 NTFS 名称推断具体过滤驱动。原始日志留 ignored `artifacts/ym334`（Quest 工作区），不入库。

实现前 Windows 4 项基线通过：临时 C 盘的 aged-access 用例不能复现该环境首读失败，不冒称 RED；行为 RED 来自实际共享冷首读。外部 1 字节预读 / close 后 4 个冷样本成功，证明修改方向。生产首版 files 套件 9/9 通过；独立 R1 发现 P2：List 在裁剪候选前为整目录预读。已改为候选只取元数据、仅返回页准备，等待复审 / 新测试结果。

Windows 时间语义参考 [FILE_BASIC_INFO](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_basic_info)、[File Times](https://learn.microsoft.com/en-us/windows/win32/sysinfo/file-times)：ChangeTime 是元数据变化，时间保证提交涉及句柄关闭。本实现未忽略 ChangeTime，也未请求 FILE_WRITE_ATTRIBUTES。

R1 由独立 yml030_cr1 审查 c383805 工作树：原核心 blob `bb5bdeafaa4d607701818372b462f48de0691202`。整目录预读 P2 修复后，复审又发现最后准备失败若 continue，会在并发删除时提前终止分页；改为显式返回错误供重试，不静默遗漏。两项复审关闭，最终受审 blobs：core `ef5d8f71a37c6bb50274cd172dab8b7dcf594e94`、测试 `bc6efcde6d72ec2c0b3ca6539b86c678927e61ab`、Cargo `f0a18dca3430aded3cd7443bcc8a387c4f1a5e5f`。用户要求的 R1 修复后先推、再 R2 顺序继续执行。

最终相关测试 green-windows-first-read-04：`cargo test --locked -p ymlink-core --test files -- --test-threads=1` 10/10、0 filtered、退出 0，含 300 长名称跨页及只读稀疏 5GiB。中间 02 因测试误写 FSCTL_SET_SPARSE 模块编译失败，修正到 System::Ioctl 后 03 /04 通过，保留原日志。Python 47 项 /3 skipped 通过，check /rustfmt /diff --check 通过。完整本地 harness verify not-run（安装版 /Quest 活跃，避免发现测试干扰 LAN）；推送后以 CI 无过滤执行，待结果。

生产候选同源 probe-07（无外部 primer、无人工 header 预读）用 List token 读取 4 个实际 MP4 的头 /中 /尾 /EOF 共 16 项，含 5,038,659,618 与 5,111,234,825 字节文件；所有结果长度 /偏移 /EOF 与 Stat token 相符，退出 0。部分样本已被早期探测访问，故不把本轮全称冷文件证明；冷首读 RED 与预读 POC 分别保留。此层是本机 API，不等同安装版 LAN /Quest 解码播放。

## Next

R1 修复已复审且相关门禁通过，先 commit / push；继续 R2 / CI、安装更新和绑定制品验收。保持 verifying，不以同源 API 探测代替 Quest 实际解码验收。
