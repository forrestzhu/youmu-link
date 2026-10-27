//! Encrypted application messages. No application message precedes authorization.
use serde::{Deserialize, Serialize};
use ymlink_core::files::{FileRequest, FileResponse};

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Message {
    Ping {
        v: u8,
    },
    Pong {
        v: u8,
    },
    Bye {
        v: u8,
    },
    FileRequest {
        v: u8,
        id: u64,
        request: FileRequest,
    },
    FileResponse {
        v: u8,
        id: u64,
        response: FileResponse,
    },
    TransferOpen {
        v: u8,
        id: String,
        share_id: String,
        path: String,
        version: String,
        offset: u64,
    },
    TransferChunk {
        v: u8,
        id: String,
        offset: u64,
        total: u64,
        version: String,
        data: Vec<u8>,
        eof: bool,
    },
    TransferAck {
        v: u8,
        id: String,
        offset: u64,
    },
    TransferAction {
        v: u8,
        id: String,
        action: String,
    },
    TransferError {
        v: u8,
        id: String,
        code: String,
    },
    MirrorState {
        v: u8,
        status: String,
        stream_id: u64,
    },
    Media {
        v: u8,
        stream_id: u64,
        packet_id: u64,
        part: u32,
        parts: u32,
        media_kind: String,
        codec: String,
        pts_us: u64,
        keyframe: bool,
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
}

pub const VERSION: u8 = 2;
