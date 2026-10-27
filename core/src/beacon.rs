//! 发现层 beacon 报文：常量与 CBOR 编解码（PROTOCOL §2）。
//!
//! 报文零身份信息：只携带 tag 指纹 / 会话 nonce / 端口 / 能力位；
//! 未知字段一律拒绝，向前兼容靠显式新版本，不做宽容解析。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// 绑定候选端口，按序回退；独立分配端口，避免与其他局域网服务串扰。
pub const BEACON_PORTS: [u16; 3] = [47610, 47611, 47612];
/// 发现报文硬上限。
pub const MAX_PACKET: usize = 512;
/// 发现层协议版本。
pub const PROTOCOL_VERSION: u8 = 1;
/// 心跳广播间隔。
pub const HEARTBEAT_INTERVAL_MS: u64 = 60_000;
/// 同一源 IP 两次 Query 单播回复的最小间隔。
pub const QUERY_REPLY_COOLDOWN_MS: u64 = 1_000;
/// 3× 心跳无新 beacon 判候选过期。
pub const PEER_EXPIRY_MS: u64 = 180_000;

const HASH_LEN: usize = 16;
const MAX_CAPS: usize = 8;

fn cbor_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, BeaconError> {
    let mut bytes = Vec::new();
    ciborium::into_writer(value, &mut bytes).map_err(|_| BeaconError::Decode)?;
    Ok(bytes)
}

/// `tag_hash = SHA256(tag_utf8)[0..16]`；tag 是分组指纹，不承载鉴权。
pub fn tag_hash(tag: &str) -> Vec<u8> {
    Sha256::digest(tag.as_bytes())[..HASH_LEN].to_vec()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Beacon {
    pub v: u8,
    pub tag_hash: Vec<u8>,
    pub session_nonce: Vec<u8>,
    pub listen_port: u16,
    pub caps: Vec<String>,
    pub discoverable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Query {
    pub v: u8,
    pub query: bool,
    pub tag_hash: Vec<u8>,
    pub session_nonce: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Packet {
    Beacon(Beacon),
    Query(Query),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BeaconError {
    Oversized(usize),
    Decode,
    InvalidVersion(u8),
    InvalidLength { field: &'static str, len: usize },
    InvalidQueryFlag,
}

impl fmt::Display for BeaconError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BeaconError::Oversized(size) => write!(f, "packet exceeds {MAX_PACKET} bytes: {size}"),
            BeaconError::Decode => write!(f, "packet is not a valid discovery frame"),
            BeaconError::InvalidVersion(v) => write!(f, "unsupported version {v}"),
            BeaconError::InvalidLength { field, len } => {
                write!(f, "field {field} has invalid length {len}")
            }
            BeaconError::InvalidQueryFlag => write!(f, "query frame must set query to true"),
        }
    }
}

impl std::error::Error for BeaconError {}

impl Beacon {
    pub fn new(
        tag: &str,
        session_nonce: [u8; HASH_LEN],
        listen_port: u16,
        caps: &[&str],
        discoverable: bool,
    ) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            tag_hash: tag_hash(tag),
            session_nonce: session_nonce.to_vec(),
            listen_port,
            caps: caps.iter().map(|cap| cap.to_string()).collect(),
            discoverable,
        }
    }

    fn validate(&self) -> Result<(), BeaconError> {
        if self.v != PROTOCOL_VERSION {
            return Err(BeaconError::InvalidVersion(self.v));
        }
        if self.tag_hash.len() != HASH_LEN {
            return Err(BeaconError::InvalidLength {
                field: "tag_hash",
                len: self.tag_hash.len(),
            });
        }
        if self.session_nonce.len() != HASH_LEN {
            return Err(BeaconError::InvalidLength {
                field: "session_nonce",
                len: self.session_nonce.len(),
            });
        }
        if self.caps.len() > MAX_CAPS {
            return Err(BeaconError::InvalidLength {
                field: "caps",
                len: self.caps.len(),
            });
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, BeaconError> {
        self.validate()?;
        let bytes = cbor_bytes(self)?;
        if bytes.len() > MAX_PACKET {
            return Err(BeaconError::Oversized(bytes.len()));
        }
        Ok(bytes)
    }
}

impl Query {
    pub fn new(tag: &str, session_nonce: [u8; HASH_LEN]) -> Self {
        Self {
            v: PROTOCOL_VERSION,
            query: true,
            tag_hash: tag_hash(tag),
            session_nonce: session_nonce.to_vec(),
        }
    }

    fn validate(&self) -> Result<(), BeaconError> {
        if self.v != PROTOCOL_VERSION {
            return Err(BeaconError::InvalidVersion(self.v));
        }
        if self.tag_hash.len() != HASH_LEN {
            return Err(BeaconError::InvalidLength {
                field: "tag_hash",
                len: self.tag_hash.len(),
            });
        }
        if self.session_nonce.len() != HASH_LEN {
            return Err(BeaconError::InvalidLength {
                field: "session_nonce",
                len: self.session_nonce.len(),
            });
        }
        Ok(())
    }

    pub fn encode(&self) -> Result<Vec<u8>, BeaconError> {
        self.validate()?;
        let bytes = cbor_bytes(self)?;
        if bytes.len() > MAX_PACKET {
            return Err(BeaconError::Oversized(bytes.len()));
        }
        Ok(bytes)
    }
}

/// 解码发现报文；`query` 字段存在与否区分两类帧，两者都拒绝未知字段。
pub fn decode_packet(bytes: &[u8]) -> Result<Packet, BeaconError> {
    if bytes.len() > MAX_PACKET {
        return Err(BeaconError::Oversized(bytes.len()));
    }
    if let Some(query) = decode_value::<Query>(bytes) {
        if !query.query {
            return Err(BeaconError::InvalidQueryFlag);
        }
        query.validate()?;
        return Ok(Packet::Query(query));
    }
    if let Some(beacon) = decode_value::<Beacon>(bytes) {
        beacon.validate()?;
        return Ok(Packet::Beacon(beacon));
    }
    Err(BeaconError::Decode)
}

fn decode_value<T: serde::de::DeserializeOwned>(bytes: &[u8]) -> Option<T> {
    let mut cursor = std::io::Cursor::new(bytes);
    let value = ciborium::from_reader(&mut cursor).ok()?;
    (cursor.position() == bytes.len() as u64).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ciborium::value::Value;

    fn nonce(seed: u8) -> [u8; HASH_LEN] {
        [seed; HASH_LEN]
    }

    #[test]
    fn beacon_roundtrip() {
        let beacon = Beacon::new("youmu-link", nonce(7), 47610, &["discovery", "files"], true);
        let bytes = beacon.encode().unwrap();
        assert!(bytes.len() <= MAX_PACKET);
        assert_eq!(decode_packet(&bytes).unwrap(), Packet::Beacon(beacon));
    }

    #[test]
    fn query_roundtrip() {
        let query = Query::new("youmu-link", nonce(9));
        let bytes = query.encode().unwrap();
        assert_eq!(decode_packet(&bytes).unwrap(), Packet::Query(query));
    }

    #[test]
    fn trailing_cbor_data_is_rejected() {
        let mut bytes = Beacon::new("youmu-link", nonce(7), 32001, &["control"], true)
            .encode()
            .unwrap();
        bytes.push(0);
        assert_eq!(decode_packet(&bytes), Err(BeaconError::Decode));
    }

    #[test]
    fn tag_hash_matches_sha256_prefix() {
        assert_eq!(
            tag_hash("abc"),
            vec![
                0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
                0x22, 0x23
            ]
        );
        assert_eq!(tag_hash("youmu-link"), tag_hash("youmu-link"));
        assert_ne!(tag_hash("youmu-link"), tag_hash("other"));
    }

    #[test]
    fn unknown_fields_rejected() {
        let mut bytes = Vec::new();
        let map = Value::Map(vec![
            (Value::Text("v".into()), Value::Integer(1.into())),
            (
                Value::Text("tag_hash".into()),
                Value::Bytes(tag_hash("youmu-link")),
            ),
            (
                Value::Text("session_nonce".into()),
                Value::Bytes(nonce(1).to_vec()),
            ),
            (
                Value::Text("listen_port".into()),
                Value::Integer(47610.into()),
            ),
            (Value::Text("caps".into()), Value::Array(vec![])),
            (Value::Text("discoverable".into()), Value::Bool(true)),
            (Value::Text("future_field".into()), Value::Bool(true)),
        ]);
        ciborium::into_writer(&map, &mut bytes).unwrap();
        assert_eq!(decode_packet(&bytes).unwrap_err(), BeaconError::Decode);
    }

    #[test]
    fn wrong_version_rejected() {
        let mut beacon = Beacon::new("youmu-link", nonce(1), 47610, &["discovery"], true);
        beacon.v = 2;
        assert_eq!(beacon.encode().unwrap_err(), BeaconError::InvalidVersion(2));
    }

    #[test]
    fn wrong_field_lengths_rejected() {
        let mut beacon = Beacon::new("youmu-link", nonce(1), 47610, &["discovery"], true);
        beacon.tag_hash.truncate(8);
        assert_eq!(
            decode_packet(&cbor_bytes(&beacon).unwrap()).unwrap_err(),
            BeaconError::InvalidLength {
                field: "tag_hash",
                len: 8
            }
        );
        let mut caps = vec![String::from("cap"); MAX_CAPS + 1];
        std::mem::swap(&mut beacon.caps, &mut caps);
        beacon.tag_hash = tag_hash("youmu-link");
        assert_eq!(
            beacon.encode().unwrap_err(),
            BeaconError::InvalidLength {
                field: "caps",
                len: MAX_CAPS + 1
            }
        );
    }

    #[test]
    fn query_must_be_true() {
        let mut query = Query::new("youmu-link", nonce(2));
        query.query = false;
        let bytes = cbor_bytes(&query).unwrap();
        assert_eq!(
            decode_packet(&bytes).unwrap_err(),
            BeaconError::InvalidQueryFlag
        );
    }

    #[test]
    fn oversized_caps_rejected_by_packet_limit() {
        let big = "x".repeat(MAX_PACKET);
        let beacon = Beacon {
            v: PROTOCOL_VERSION,
            tag_hash: tag_hash("youmu-link"),
            session_nonce: nonce(1).to_vec(),
            listen_port: 47610,
            caps: vec![big],
            discoverable: true,
        };
        assert!(matches!(
            beacon.encode().unwrap_err(),
            BeaconError::Oversized(_)
        ));
    }
}
