//! `ymlink-core`：YM_Link 桌面端协议核心。
//!
//! 当前实现发现层（PROTOCOL §2）；会话 / 配对与文件 / 镜像 lane 按
//! `docs/engineering/PROTOCOL.md` 的里程碑规划在后续版本加入。

pub mod beacon;
pub mod discovery;
pub mod files;
pub mod identity;
pub mod session;
