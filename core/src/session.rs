//! Noise XX sessions with explicit SPAKE2 pairing (PROTOCOL §3).

use crate::beacon;
use crate::identity::{device_id, Identity, TrustStore};
use hmac::{Hmac, Mac};
use rand::Rng;
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use sha2::Sha256;
use snow::StatelessTransportState;
use spake2::{Ed25519Group, Identity as SpakeIdentity, Password, Spake2};
use std::fmt;
use std::io::Cursor;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use zeroize::Zeroizing;

pub const VERSION: u8 = 3;
pub const MAGIC: &[u8; 9] = b"YL-CTRL2\0";
pub const MAX_CIPHERTEXT: usize = 65_535;
pub const MAX_PLAINTEXT: usize = MAX_CIPHERTEXT - 16;
pub const PAIRING_TTL: Duration = Duration::from_secs(300);
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(5);
const RECEIVE_TIMEOUT: Duration = Duration::from_secs(130);
const NOISE: &str = "Noise_XX_25519_ChaChaPoly_SHA256";
const PROLOGUE: &[u8] = b"youmu-link-v2";

#[derive(Debug)]
pub enum Error {
    Io(std::io::Error),
    Protocol(&'static str),
    Crypto,
    Timeout,
    PairingExpired,
    PairingRejected,
    PairingRequired,
    IdentityChanged,
    Storage(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "connection error: {error}"),
            Self::Protocol(message) => write!(f, "protocol error: {message}"),
            Self::Crypto => f.write_str("session authentication failed"),
            Self::Timeout => f.write_str("connection timed out"),
            Self::PairingExpired => f.write_str("pairing code expired"),
            Self::PairingRejected => f.write_str("pairing code rejected"),
            Self::PairingRequired => f.write_str("explicit pairing required"),
            Self::IdentityChanged => f.write_str("paired identity key changed"),
            Self::Storage(message) => write!(f, "identity storage error: {message}"),
        }
    }
}

impl std::error::Error for Error {}
impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}
impl From<snow::Error> for Error {
    fn from(_: snow::Error) -> Self {
        Self::Crypto
    }
}
pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub v: u8,
    pub tag_hash: Vec<u8>,
    pub device_id: String,
    pub public_key: Vec<u8>,
    pub name: String,
    pub platform: String,
    pub listen_port: u16,
}

impl Hello {
    pub fn new(
        identity: &Identity,
        name: impl Into<String>,
        platform: impl Into<String>,
        listen_port: u16,
    ) -> Self {
        Self {
            v: VERSION,
            tag_hash: beacon::tag_hash("youmu-link").to_vec(),
            device_id: identity.device_id(),
            public_key: identity.public_key().to_vec(),
            name: name.into(),
            platform: platform.into(),
            listen_port,
        }
    }

    fn validate(&self, noise_key: &[u8; 32], local_tag: &[u8]) -> Result<()> {
        if self.v != VERSION {
            return Err(Error::Protocol(
                "incompatible application version; upgrade YM_Link on both devices",
            ));
        }
        if self.tag_hash.len() != 16
            || self.tag_hash != local_tag
            || self.name.is_empty()
            || self.name.len() > 128
            || self.platform.is_empty()
            || self.platform.len() > 32
            || self.name.chars().any(char::is_control)
            || self.platform.chars().any(char::is_control)
            || self.listen_port == 0
        {
            return Err(Error::Protocol("invalid or incompatible Hello"));
        }
        if self.public_key != noise_key || self.device_id != device_id(noise_key) {
            return Err(Error::IdentityChanged);
        }
        Ok(())
    }
}

pub enum Pairing {
    Display { code: String, expires_at: Instant },
    Enter { code: String },
    ReconnectOnly,
}

pub fn new_pairing_code() -> String {
    format!("{:06}", rand::rngs::OsRng.gen_range(0..1_000_000u32))
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Authentication {
    Known { v: u8, known: bool },
    Spake { v: u8, message: Vec<u8> },
    Confirm { v: u8, mac: Vec<u8> },
    Accepted { v: u8 },
}

pub struct SecureSession {
    reader: SecureReader,
    writer: SecureWriter,
    remote: Hello,
    remote_key: [u8; 32],
    handshake_hash: Vec<u8>,
    needs_pairing: Option<bool>,
    pairing_deadline: Option<Instant>,
    authorized: bool,
    attempted: bool,
}

impl SecureSession {
    pub async fn handshake(
        mut stream: TcpStream,
        initiator: bool,
        identity: &Identity,
        local: Hello,
    ) -> Result<Self> {
        tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            stream.set_nodelay(true)?;
            local.validate(&identity.public_key(), &local.tag_hash)?;
            if initiator {
                stream.write_all(MAGIC).await?;
            } else {
                let mut magic = [0u8; MAGIC.len()];
                stream.read_exact(&mut magic).await?;
                if magic != *MAGIC {
                    return Err(Error::Protocol("unknown connection prefix"));
                }
            }
            let params = NOISE.parse().map_err(|_| Error::Crypto)?;
            let builder = snow::Builder::new(params)
                .prologue(PROLOGUE)
                .local_private_key(identity.secret_key());
            let mut handshake = if initiator {
                builder.build_initiator()?
            } else {
                builder.build_responder()?
            };
            let mut output = [0u8; 4096];
            while !handshake.is_handshake_finished() {
                if handshake.is_my_turn() {
                    let size = handshake.write_message(&[], &mut output)?;
                    write_frame(&mut stream, &output[..size], 4096).await?;
                } else {
                    let input = read_frame(&mut stream, 4096).await?;
                    if handshake.read_message(&input, &mut output)? != 0 {
                        return Err(Error::Protocol("unexpected handshake payload"));
                    }
                }
            }
            let remote_key: [u8; 32] = handshake
                .get_remote_static()
                .ok_or(Error::Crypto)?
                .try_into()
                .map_err(|_| Error::Crypto)?;
            let handshake_hash = handshake.get_handshake_hash().to_vec();
            let transport = Arc::new(handshake.into_stateless_transport_mode()?);
            let (read, write) = stream.into_split();
            let mut reader = SecureReader {
                stream: read,
                transport: transport.clone(),
                nonce: 0,
                failed: false,
            };
            let mut writer = SecureWriter {
                stream: write,
                transport,
                nonce: 0,
                failed: false,
            };
            writer.send_inner(&local).await?;
            let remote: Hello = reader.recv_inner().await?;
            remote.validate(&remote_key, &local.tag_hash)?;
            if remote.device_id == local.device_id {
                return Err(Error::Protocol("cannot connect to own identity"));
            }
            Ok(Self {
                reader,
                writer,
                remote,
                remote_key,
                handshake_hash,
                needs_pairing: None,
                pairing_deadline: None,
                authorized: false,
                attempted: false,
            })
        })
        .await
        .map_err(|_| Error::Timeout)?
    }

    pub fn remote(&self) -> &Hello {
        &self.remote
    }

    pub fn remote_key(&self) -> [u8; 32] {
        self.remote_key
    }

    /// Both sides must call this before waiting for UI input. A unilateral trust
    /// assertion can never authorize a connection.
    pub async fn needs_pairing(&mut self, trust: &TrustStore) -> Result<bool> {
        if let Some(value) = self.needs_pairing {
            return Ok(value);
        }
        let known = trust.check(&self.remote, &self.remote_key)?;
        let result = tokio::time::timeout(HANDSHAKE_TIMEOUT, async {
            self.writer
                .send_inner(&Authentication::Known { v: VERSION, known })
                .await?;
            match self.reader.recv_inner::<Authentication>().await? {
                Authentication::Known {
                    v: VERSION,
                    known: remote_known,
                } => Ok(!(known && remote_known)),
                _ => Err(Error::Protocol("expected trust status")),
            }
        })
        .await
        .map_err(|_| Error::Timeout)??;
        self.needs_pairing = Some(result);
        if result {
            self.pairing_deadline = Some(Instant::now() + PAIRING_TTL);
        }
        Ok(result)
    }

    pub async fn authorize(&mut self, trust: &mut TrustStore, pairing: Pairing) -> Result<()> {
        if self.attempted {
            return Err(Error::PairingRejected);
        }
        self.attempted = true;
        if !self.needs_pairing(trust).await? {
            // Trust can be revoked while the runtime moves from the initial
            // Known exchange to authorization. Do not use a stale local grant.
            if !trust.check(&self.remote, &self.remote_key)? {
                return Err(Error::PairingRequired);
            }
            self.authorized = true;
            return Ok(());
        }
        let (code, display, ttl) = match pairing {
            Pairing::Display { code, expires_at } => (
                code,
                true,
                expires_at
                    .checked_duration_since(Instant::now())
                    .ok_or(Error::PairingExpired)?,
            ),
            Pairing::Enter { code } => (code, false, PAIRING_TTL),
            Pairing::ReconnectOnly => return Err(Error::PairingRequired),
        };
        let code = Zeroizing::new(code);
        if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Error::PairingRejected);
        }
        let remaining = self
            .pairing_deadline
            .and_then(|deadline| deadline.checked_duration_since(Instant::now()))
            .ok_or(Error::PairingExpired)?;
        tokio::time::timeout(ttl.min(remaining), self.pair(&code, display))
            .await
            .map_err(|_| Error::PairingExpired)??;
        // No private key or pairing secret is stored. Persist only after mutual
        // key confirmation has completed on this connection.
        trust.remember(&self.remote, &self.remote_key)?;
        self.authorized = true;
        Ok(())
    }

    async fn pair(&mut self, code: &str, display: bool) -> Result<()> {
        let mut display_id = b"youmu-link-display-v2".to_vec();
        display_id.extend_from_slice(&self.handshake_hash);
        let mut enter_id = b"youmu-link-enter-v2".to_vec();
        enter_id.extend_from_slice(&self.handshake_hash);
        let password = Password::new(code.as_bytes());
        let id_a = SpakeIdentity::new(&display_id);
        let id_b = SpakeIdentity::new(&enter_id);
        let (spake, message) = if display {
            Spake2::<Ed25519Group>::start_a(&password, &id_a, &id_b)
        } else {
            Spake2::<Ed25519Group>::start_b(&password, &id_a, &id_b)
        };
        self.writer
            .send_inner(&Authentication::Spake {
                v: VERSION,
                message,
            })
            .await?;
        let peer_message = match self.reader.recv_inner::<Authentication>().await? {
            Authentication::Spake {
                v: VERSION,
                message,
            } if message.len() == 33 => message,
            _ => return Err(Error::Protocol("invalid SPAKE2 message")),
        };
        let key = Zeroizing::new(
            spake
                .finish(&peer_message)
                .map_err(|_| Error::PairingRejected)?,
        );
        let our_label: &[u8] = if display {
            b"youmu-link-confirm-display-v2"
        } else {
            b"youmu-link-confirm-enter-v2"
        };
        let their_label: &[u8] = if display {
            b"youmu-link-confirm-enter-v2"
        } else {
            b"youmu-link-confirm-display-v2"
        };
        let mut ours = Hmac::<Sha256>::new_from_slice(&key).map_err(|_| Error::Crypto)?;
        ours.update(our_label);
        ours.update(&self.handshake_hash);
        self.writer
            .send_inner(&Authentication::Confirm {
                v: VERSION,
                mac: ours.finalize().into_bytes().to_vec(),
            })
            .await?;
        let peer_mac = match self.reader.recv_inner::<Authentication>().await? {
            Authentication::Confirm { v: VERSION, mac } if mac.len() == 32 => mac,
            _ => return Err(Error::Protocol("invalid key confirmation")),
        };
        let mut theirs = Hmac::<Sha256>::new_from_slice(&key).map_err(|_| Error::Crypto)?;
        theirs.update(their_label);
        theirs.update(&self.handshake_hash);
        theirs
            .verify_slice(&peer_mac)
            .map_err(|_| Error::PairingRejected)?;
        self.writer
            .send_inner(&Authentication::Accepted { v: VERSION })
            .await?;
        match self.reader.recv_inner::<Authentication>().await? {
            Authentication::Accepted { v: VERSION } => Ok(()),
            _ => Err(Error::Protocol("expected pairing acceptance")),
        }
    }

    pub fn into_split(self) -> Result<(SecureReader, SecureWriter)> {
        if !self.authorized {
            return Err(Error::PairingRequired);
        }
        Ok((self.reader, self.writer))
    }
}

/// One dedicated receive task must own the reader. If a receive future is
/// cancelled, close the connection: a partially read TCP frame cannot be resumed
/// by starting another receive call.
pub struct SecureReader {
    stream: OwnedReadHalf,
    transport: Arc<StatelessTransportState>,
    nonce: u64,
    failed: bool,
}

impl SecureReader {
    pub async fn recv<T: DeserializeOwned>(&mut self) -> Result<T> {
        if self.failed {
            return Err(Error::Protocol("receive direction is closed"));
        }
        // Set before awaiting so dropping a partially completed receive future
        // cannot make the next call treat the rest of a frame as a new header.
        self.failed = true;
        let result = tokio::time::timeout(RECEIVE_TIMEOUT, self.recv_inner())
            .await
            .map_err(|_| Error::Timeout)
            .and_then(|value| value);
        if result.is_ok() {
            self.failed = false;
        }
        result
    }

    async fn recv_inner<T: DeserializeOwned>(&mut self) -> Result<T> {
        let frame = read_frame(&mut self.stream, MAX_CIPHERTEXT).await?;
        if frame.len() < 16 {
            return Err(Error::Protocol("truncated encrypted frame"));
        }
        let mut plaintext = Zeroizing::new(vec![0u8; frame.len()]);
        let size = self
            .transport
            .read_message(self.nonce, &frame, &mut plaintext)?;
        self.nonce = self.nonce.checked_add(1).ok_or(Error::Crypto)?;
        let mut cursor = Cursor::new(&plaintext[..size]);
        let value = ciborium::from_reader(&mut cursor)
            .map_err(|_| Error::Protocol("invalid CBOR frame"))?;
        if cursor.position() != size as u64 {
            return Err(Error::Protocol("trailing data in CBOR frame"));
        }
        Ok(value)
    }
}

pub struct SecureWriter {
    stream: OwnedWriteHalf,
    transport: Arc<StatelessTransportState>,
    nonce: u64,
    failed: bool,
}

impl SecureWriter {
    pub async fn send<T: Serialize>(&mut self, value: &T) -> Result<()> {
        if self.failed {
            return Err(Error::Protocol("send direction is closed"));
        }
        self.failed = true;
        let result = tokio::time::timeout(RECEIVE_TIMEOUT, self.send_inner(value))
            .await
            .map_err(|_| Error::Timeout)
            .and_then(|value| value);
        if result.is_ok()
            || matches!(
                result,
                Err(Error::Protocol(
                    "encrypted frame exceeds limit" | "cannot encode CBOR frame"
                ))
            )
        {
            // These local validation failures occur before advancing the nonce
            // or writing any bytes and therefore leave the direction reusable.
            self.failed = false;
        }
        result
    }

    async fn send_inner<T: Serialize>(&mut self, value: &T) -> Result<()> {
        let mut plaintext = Zeroizing::new(Vec::new());
        ciborium::into_writer(value, &mut *plaintext)
            .map_err(|_| Error::Protocol("cannot encode CBOR frame"))?;
        if plaintext.len() > MAX_PLAINTEXT {
            return Err(Error::Protocol("encrypted frame exceeds limit"));
        }
        let mut ciphertext = vec![0u8; plaintext.len() + 16];
        let size = self
            .transport
            .write_message(self.nonce, &plaintext, &mut ciphertext)?;
        self.nonce = self.nonce.checked_add(1).ok_or(Error::Crypto)?;
        write_frame(&mut self.stream, &ciphertext[..size], MAX_CIPHERTEXT).await
    }
}

async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R, max: usize) -> Result<Vec<u8>> {
    let size = reader.read_u32().await? as usize;
    if size == 0 || size > max {
        return Err(Error::Protocol("invalid frame length"));
    }
    let mut frame = vec![0u8; size];
    reader.read_exact(&mut frame).await?;
    Ok(frame)
}

async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    frame: &[u8],
    max: usize,
) -> Result<()> {
    if frame.is_empty() || frame.len() > max {
        return Err(Error::Protocol("invalid frame length"));
    }
    writer.write_u32(frame.len() as u32).await?;
    writer.write_all(frame).await?;
    Ok(())
}
