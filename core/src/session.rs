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

pub const VERSION: u8 = 4;
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
    RateLimited(u32),
    Cancelled,
    Busy,
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
            Self::RateLimited(_) => f.write_str("pairing rate limit; wait one minute"),
            Self::Cancelled => f.write_str("pairing cancelled"),
            Self::Busy => f.write_str("pairing busy; try again"),
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
            tag_hash: beacon::tag_hash("youmu-link"),
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

#[must_use]
pub fn new_pairing_code() -> String {
    format!("{:04}", rand::rngs::OsRng.gen_range(0..10_000u32))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthErrorCode {
    Expired,
    RateLimited,
    Cancelled,
    Busy,
}

#[derive(Debug, Clone)]
pub struct PairingReady {
    pub window_id: [u8; 16],
    pub remaining_ms: u32,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Authentication {
    Known {
        v: u8,
        known: bool,
    },
    PairRequest {
        v: u8,
        #[serde(deserialize_with = "present_resume_id")]
        resume_id: Option<Vec<u8>>,
    },
    PairReady {
        v: u8,
        window_id: Vec<u8>,
        remaining_ms: u32,
    },
    AuthError {
        v: u8,
        code: AuthErrorCode,
        retry_after_ms: u32,
    },
    Spake {
        v: u8,
        message: Vec<u8>,
    },
    Confirm {
        v: u8,
        mac: Vec<u8>,
    },
    Accepted {
        v: u8,
    },
}

fn present_resume_id<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<Vec<u8>>, D::Error> {
    Option::<Vec<u8>>::deserialize(deserializer)
}

pub struct SecureSession {
    reader: SecureReader,
    writer: SecureWriter,
    remote: Hello,
    remote_key: [u8; 32],
    handshake_hash: Vec<u8>,
    needs_pairing: Option<bool>,
    pairing_deadline: Option<Instant>,
    request_received: bool,
    pairing_ready: Option<PairingReady>,
    authorized: bool,
    attempted: bool,
}

impl SecureSession {
    /// Performs the Noise XX handshake and exchanges authenticated hellos.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Timeout`] when the handshake stalls, [`Error::Io`] on
    /// transport failure, [`Error::Crypto`] on Noise failure,
    /// [`Error::Protocol`] on invalid frames or local / remote hello fields,
    /// and [`Error::IdentityChanged`] on inconsistent hello identity fields.
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
                transport: Arc::clone(&transport),
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
                request_received: false,
                pairing_ready: None,
                authorized: false,
                attempted: false,
            })
        })
        .await
        .map_err(|_| Error::Timeout)?
    }

    #[must_use]
    pub fn remote(&self) -> &Hello {
        &self.remote
    }

    #[must_use]
    pub fn remote_key(&self) -> [u8; 32] {
        self.remote_key
    }

    /// Both sides must call this before waiting for UI input. A unilateral trust
    /// assertion can never authorize a connection.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IdentityChanged`] on inconsistent trust pins,
    /// [`Error::Protocol`] on an unexpected frame, [`Error::Crypto`] on encryption
    /// or decryption failure, [`Error::Timeout`] when the exchange stalls and
    /// [`Error::Io`] on transport failure.
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

    /// Requests a fresh or resumed display-side pairing window after Known.
    ///
    /// # Errors
    /// Returns a typed admission failure or a transport / protocol error.
    pub async fn request_pairing(&mut self, resume_id: Option<[u8; 16]>) -> Result<PairingReady> {
        if self.needs_pairing != Some(true) || self.authorized || self.pairing_ready.is_some() {
            return Err(Error::Protocol("unexpected pairing request"));
        }
        let deadline = self.pairing_deadline.ok_or(Error::PairingExpired)?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::PairingExpired)?;
        tokio::time::timeout(HANDSHAKE_TIMEOUT.min(remaining), async {
            self.writer
                .send_inner(&Authentication::PairRequest {
                    v: VERSION,
                    resume_id: resume_id.map(|id| id.to_vec()),
                })
                .await?;
            match self.reader.recv_inner::<Authentication>().await? {
                Authentication::PairReady {
                    v: VERSION,
                    window_id,
                    remaining_ms,
                } if window_id.len() == 16 && (1..=300_000).contains(&remaining_ms) => {
                    let window_id = window_id
                        .try_into()
                        .map_err(|_| Error::Protocol("invalid pairing window"))?;
                    if resume_id.is_some_and(|id| id != window_id) {
                        return Err(Error::Protocol("pairing window changed"));
                    }
                    let deadline = Instant::now() + Duration::from_millis(u64::from(remaining_ms));
                    self.pairing_deadline = Some(
                        self.pairing_deadline
                            .map_or(deadline, |old| old.min(deadline)),
                    );
                    let ready = PairingReady {
                        window_id,
                        remaining_ms,
                    };
                    self.pairing_ready = Some(ready.clone());
                    Ok(ready)
                }
                message => Err(authentication_error(message)),
            }
        })
        .await
        .map_err(|_| {
            if Instant::now() >= deadline {
                Error::PairingExpired
            } else {
                Error::Timeout
            }
        })?
    }

    /// Caps a resumed connection at the original logical window deadline.
    /// This never extends the Known or remote-offered deadline.
    ///
    /// # Errors
    /// Requires an unauthorised pairing session and a deadline still in the future.
    pub fn restrict_pairing_deadline(&mut self, deadline: Instant) -> Result<()> {
        if self.needs_pairing != Some(true) || self.authorized || self.attempted {
            return Err(Error::Protocol("unexpected pairing deadline"));
        }
        let deadline = self
            .pairing_deadline
            .ok_or(Error::PairingExpired)?
            .min(deadline);
        self.pairing_deadline = Some(deadline);
        if Instant::now() >= deadline {
            return Err(Error::PairingExpired);
        }
        Ok(())
    }

    /// Receives the input peer's window request. The caller owns admission/cache.
    ///
    /// # Errors
    /// Returns timeout, transport or protocol errors; no business is authorized.
    pub async fn receive_pairing_request(&mut self) -> Result<Option<[u8; 16]>> {
        if self.needs_pairing != Some(true) || self.authorized || self.request_received {
            return Err(Error::Protocol("unexpected pairing request"));
        }
        let remaining = self
            .pairing_deadline
            .and_then(|d| d.checked_duration_since(Instant::now()))
            .ok_or(Error::PairingExpired)?;
        let message = tokio::time::timeout(
            HANDSHAKE_TIMEOUT.min(remaining),
            self.reader.recv_inner::<Authentication>(),
        )
        .await
        .map_err(|_| {
            if remaining <= HANDSHAKE_TIMEOUT {
                Error::PairingExpired
            } else {
                Error::Timeout
            }
        })??;
        match message {
            Authentication::PairRequest {
                v: VERSION,
                resume_id,
            } => {
                let id = resume_id
                    .map(|id| {
                        id.try_into()
                            .map_err(|_| Error::Protocol("invalid pairing window"))
                    })
                    .transpose()?;
                self.request_received = true;
                Ok(id)
            }
            message => Err(authentication_error(message)),
        }
    }

    /// Publishes the admitted logical window without extending the Known deadline.
    ///
    /// # Errors
    /// Returns expired, transport or invalid-state errors.
    pub async fn offer_pairing(&mut self, window_id: [u8; 16], expires_at: Instant) -> Result<()> {
        if !self.request_received || self.authorized || self.pairing_ready.is_some() {
            return Err(Error::Protocol("unexpected pairing offer"));
        }
        let deadline = self
            .pairing_deadline
            .ok_or(Error::PairingRequired)?
            .min(expires_at);
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::PairingExpired)?;
        let remaining_ms = remaining.as_millis().clamp(1, PAIRING_TTL.as_millis()) as u32;
        self.writer
            .send_inner(&Authentication::PairReady {
                v: VERSION,
                window_id: window_id.to_vec(),
                remaining_ms,
            })
            .await?;
        self.pairing_deadline = Some(deadline);
        self.pairing_ready = Some(PairingReady {
            window_id,
            remaining_ms,
        });
        Ok(())
    }

    /// Sends a terminal admission error. The connection must then be dropped.
    ///
    /// # Errors
    /// Returns protocol or transport errors; never grants authorization.
    pub async fn reject_pairing(&mut self, code: AuthErrorCode, retry_after_ms: u32) -> Result<()> {
        if self.authorized
            || self.needs_pairing != Some(true)
            || !valid_auth_error(code, retry_after_ms)
        {
            return Err(Error::Protocol("invalid authentication rejection"));
        }
        self.writer
            .send_inner(&Authentication::AuthError {
                v: VERSION,
                code,
                retry_after_ms,
            })
            .await
    }

    async fn begin_authorization(&mut self, trust: &TrustStore) -> Result<bool> {
        if self.attempted {
            return Err(Error::PairingRejected);
        }
        self.attempted = true;
        if !self.needs_pairing(trust).await? {
            if !trust.check(&self.remote, &self.remote_key)? {
                return Err(Error::PairingRequired);
            }
            self.authorized = true;
            return Ok(false);
        }
        Ok(true)
    }

    /// Authorizes one PAKE attempt on this connection, or the mutual Known path.
    ///
    /// # Errors
    /// Returns typed pairing/admission, identity, transport, protocol or storage errors.
    pub async fn authorize(&mut self, trust: &mut TrustStore, pairing: Pairing) -> Result<()> {
        if let Pairing::Enter { code } = pairing {
            return self
                .authorize_enter(trust, std::future::ready(Ok(code)))
                .await;
        }
        if !self.begin_authorization(trust).await? {
            return Ok(());
        }
        let Pairing::Display { code, expires_at } = pairing else {
            return Err(Error::PairingRequired);
        };
        let code = Zeroizing::new(code);
        validate_code(&code)?;
        self.pairing_deadline = Some(
            self.pairing_deadline
                .ok_or(Error::PairingRequired)?
                .min(expires_at),
        );
        if self.pairing_ready.is_none() {
            if self.receive_pairing_request().await?.is_some() {
                self.reject_pairing(AuthErrorCode::Expired, 0).await?;
                return Err(Error::PairingExpired);
            }
            self.offer_pairing(rand::rngs::OsRng.gen(), expires_at)
                .await?;
        }
        let deadline = self
            .pairing_deadline
            .ok_or(Error::PairingExpired)?
            .min(expires_at);
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::PairingExpired)?;
        tokio::time::timeout(remaining, self.pair_display(&code))
            .await
            .map_err(|_| Error::PairingExpired)??;
        if Instant::now() >= deadline {
            return Err(Error::PairingExpired);
        }
        trust.remember(&self.remote, &self.remote_key)?;
        self.authorized = true;
        Ok(())
    }

    /// Waits for UI input while retaining one in-flight authentication read.
    /// Cancelling this method requires dropping the whole connection.
    ///
    /// # Errors
    /// Returns typed admission, expiry, cancellation, transport/protocol or trust errors.
    pub async fn authorize_enter<F>(&mut self, trust: &mut TrustStore, input: F) -> Result<()>
    where
        F: std::future::Future<Output = Result<String>>,
    {
        if !self.begin_authorization(trust).await? {
            return Ok(());
        }
        if self.pairing_ready.is_none() {
            self.request_pairing(None).await?;
        }
        let deadline = self.pairing_deadline.ok_or(Error::PairingExpired)?;
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or(Error::PairingExpired)?;
        tokio::time::timeout(remaining, self.pair_enter(input))
            .await
            .map_err(|_| Error::PairingExpired)??;
        if Instant::now() >= deadline {
            return Err(Error::PairingExpired);
        }
        trust.remember(&self.remote, &self.remote_key)?;
        self.authorized = true;
        Ok(())
    }

    async fn pair_display(&mut self, code: &str) -> Result<()> {
        let (spake, message) = start_spake(&self.handshake_hash, code, true);
        let peer_message = peer_spake(self.reader.recv_inner::<Authentication>().await?)?;
        self.writer
            .send_inner(&Authentication::Spake {
                v: VERSION,
                message,
            })
            .await?;
        self.finish_pair(spake, peer_message, true).await
    }

    async fn pair_enter<F>(&mut self, input: F) -> Result<()>
    where
        F: std::future::Future<Output = Result<String>>,
    {
        let hash = self.handshake_hash.clone();
        let (spake, peer_message) = {
            let first = self.reader.recv_inner::<Authentication>();
            tokio::pin!(first);
            tokio::pin!(input);
            let code = tokio::select! {
                code = &mut input => Zeroizing::new(code?),
                message = &mut first => return Err(authentication_error(message?)),
            };
            validate_code(&code)?;
            let (spake, message) = start_spake(&hash, &code, false);
            self.writer
                .send_inner(&Authentication::Spake {
                    v: VERSION,
                    message,
                })
                .await?;
            (spake, peer_spake(first.await?)?)
        };
        self.finish_pair(spake, peer_message, false).await
    }

    async fn finish_pair(
        &mut self,
        spake: Spake2<Ed25519Group>,
        peer_message: Vec<u8>,
        display: bool,
    ) -> Result<()> {
        let key = Zeroizing::new(spake.finish(&peer_message).map_err(|_| Error::Crypto)?);
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
            message => return Err(authentication_error(message)),
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
            message => Err(authentication_error(message)),
        }
    }

    /// Splits the authorized session into its reader and writer halves.
    ///
    /// # Errors
    ///
    /// Returns [`Error::PairingRequired`] when the session was never authorized.
    pub fn into_split(self) -> Result<(SecureReader, SecureWriter)> {
        if !self.authorized {
            return Err(Error::PairingRequired);
        }
        Ok((self.reader, self.writer))
    }
}

fn validate_code(code: &str) -> Result<()> {
    if code.len() != 4 || !code.bytes().all(|b| b.is_ascii_digit()) {
        return Err(Error::PairingRejected);
    }
    Ok(())
}

fn valid_auth_error(code: AuthErrorCode, retry_after_ms: u32) -> bool {
    if code == AuthErrorCode::RateLimited {
        (1..=60_000).contains(&retry_after_ms)
    } else {
        retry_after_ms == 0
    }
}

fn authentication_error(message: Authentication) -> Error {
    match message {
        Authentication::AuthError {
            v: VERSION,
            code,
            retry_after_ms,
        } if valid_auth_error(code, retry_after_ms) => match code {
            AuthErrorCode::Expired => Error::PairingExpired,
            AuthErrorCode::RateLimited => Error::RateLimited(retry_after_ms),
            AuthErrorCode::Cancelled => Error::Cancelled,
            AuthErrorCode::Busy => Error::Busy,
        },
        _ => Error::Protocol("unexpected authentication message"),
    }
}

fn peer_spake(message: Authentication) -> Result<Vec<u8>> {
    match message {
        Authentication::Spake {
            v: VERSION,
            message,
        } if message.len() == 33 => Ok(message),
        message => Err(authentication_error(message)),
    }
}

fn start_spake(hash: &[u8], code: &str, display: bool) -> (Spake2<Ed25519Group>, Vec<u8>) {
    let mut display_id = b"youmu-link-display-v2".to_vec();
    display_id.extend_from_slice(hash);
    let mut enter_id = b"youmu-link-enter-v2".to_vec();
    enter_id.extend_from_slice(hash);
    let password = Password::new(code.as_bytes());
    let id_a = SpakeIdentity::new(&display_id);
    let id_b = SpakeIdentity::new(&enter_id);
    if display {
        Spake2::<Ed25519Group>::start_a(&password, &id_a, &id_b)
    } else {
        Spake2::<Ed25519Group>::start_b(&password, &id_a, &id_b)
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
    /// Receives and decrypts one CBOR frame.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Protocol`] when the direction is closed or the frame is
    /// invalid, [`Error::Timeout`] when the receive stalls, [`Error::Crypto`] on
    /// decryption failure and [`Error::Io`] on transport failure.
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
    /// Encrypts and sends one CBOR frame.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Protocol`] when the direction is closed or the payload
    /// cannot be encoded, [`Error::Timeout`] when the send stalls,
    /// [`Error::Crypto`] on encryption failure and [`Error::Io`] on transport
    /// failure.
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

#[cfg(test)]
mod auth_regressions {
    use super::*;
    use tokio::net::TcpListener;

    async fn sessions() -> (SecureSession, SecureSession) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let a = Identity::generate().unwrap();
        let b = Identity::generate().unwrap();
        let stream = TcpStream::connect(address).await.unwrap();
        let (a, b) = tokio::join!(
            SecureSession::handshake(
                stream,
                true,
                &a,
                Hello::new(&a, "Quest", "quest", address.port())
            ),
            async {
                let (stream, _) = listener.accept().await.unwrap();
                SecureSession::handshake(
                    stream,
                    false,
                    &b,
                    Hello::new(&b, "Mac", "macos", address.port()),
                )
                .await
            }
        );
        (a.unwrap(), b.unwrap())
    }

    #[tokio::test]
    async fn missing_resume_field_is_not_a_fresh_window_request() {
        let (mut enter, mut display) = sessions().await;
        let trust = TrustStore::memory();
        let (a, b) = tokio::join!(enter.needs_pairing(&trust), display.needs_pairing(&trust));
        assert!(a.unwrap() && b.unwrap());
        enter
            .writer
            .send_inner(&serde_json::json!({"kind":"pair_request","v":VERSION}))
            .await
            .unwrap();
        assert!(matches!(
            display.receive_pairing_request().await,
            Err(Error::Protocol(_))
        ));
        assert!(display.pairing_ready.is_none());
        assert!(trust.peers().is_empty());
    }

    #[tokio::test]
    async fn malformed_admission_frames_never_create_a_ready_window() {
        use serde_json::json;
        for (resume, frame) in [
            (
                None,
                json!({"kind":"pair_ready","v":VERSION,"window_id":vec![7;15],"remaining_ms":20}),
            ),
            (
                None,
                json!({"kind":"pair_ready","v":VERSION,"window_id":vec![7;16],"remaining_ms":0}),
            ),
            (
                None,
                json!({"kind":"pair_ready","v":VERSION,"window_id":vec![7;16],"remaining_ms":300001}),
            ),
            (
                Some([8; 16]),
                json!({"kind":"pair_ready","v":VERSION,"window_id":vec![7;16],"remaining_ms":20}),
            ),
            (
                None,
                json!({"kind":"pair_ready","v":3,"window_id":vec![7;16],"remaining_ms":20}),
            ),
            (
                None,
                json!({"kind":"auth_error","v":VERSION,"code":"rate_limited","retry_after_ms":0}),
            ),
            (
                None,
                json!({"kind":"auth_error","v":VERSION,"code":"expired","retry_after_ms":1}),
            ),
            (
                None,
                json!({"kind":"auth_error","v":VERSION,"code":"incorrect","retry_after_ms":0}),
            ),
            (None, json!({"kind":"auth_error","v":VERSION,"code":"busy"})),
        ] {
            let (mut enter, mut display) = sessions().await;
            let trust = TrustStore::memory();
            let (a, b) = tokio::join!(enter.needs_pairing(&trust), display.needs_pairing(&trust));
            assert!(a.unwrap() && b.unwrap());
            let (result, ()) = tokio::join!(enter.request_pairing(resume), async {
                display.receive_pairing_request().await.unwrap();
                display.writer.send_inner(&frame).await.unwrap();
            });
            assert!(
                matches!(result, Err(Error::Protocol(_))),
                "accepted malformed frame: {frame:?}"
            );
            assert!(enter.pairing_ready.is_none());
            assert!(trust.peers().is_empty());
            assert!(matches!(enter.into_split(), Err(Error::PairingRequired)));
        }
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "current_thread")]
    async fn buffered_accepted_polled_after_deadline_must_not_authorize() {
        use std::os::fd::AsRawFd;
        use tokio::sync::oneshot;
        let (mut enter, mut display) = sessions().await;
        let mut trust = TrustStore::memory();
        let display_trust = TrustStore::memory();
        let (a, b) = tokio::join!(
            enter.needs_pairing(&trust),
            display.needs_pairing(&display_trust)
        );
        assert!(a.unwrap() && b.unwrap());
        let deadline = Instant::now() + Duration::from_secs(2);
        let (ready, ()) = tokio::join!(enter.request_pairing(None), async {
            display.receive_pairing_request().await.unwrap();
            display.offer_pairing([7; 16], deadline).await.unwrap();
        });
        ready.unwrap();
        let fd = enter.reader.stream.as_ref().as_raw_fd();
        let mut plain = Vec::new();
        ciborium::into_writer(&Authentication::Accepted { v: VERSION }, &mut plain).unwrap();
        let size = 4 + plain.len() + 16;
        let (reached_tx, reached_rx) = oneshot::channel();
        let (send_tx, send_rx) = oneshot::channel();
        let (sent_tx, sent_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let peer = tokio::spawn(async move {
            let hash = display.handshake_hash.clone();
            let (spake, message) = start_spake(&hash, "0427", true);
            let remote =
                peer_spake(display.reader.recv_inner::<Authentication>().await.unwrap()).unwrap();
            display
                .writer
                .send_inner(&Authentication::Spake {
                    v: VERSION,
                    message,
                })
                .await
                .unwrap();
            let key = Zeroizing::new(spake.finish(&remote).unwrap());
            let mac = match display.reader.recv_inner::<Authentication>().await.unwrap() {
                Authentication::Confirm { v: VERSION, mac } => mac,
                _ => panic!("expected Confirm"),
            };
            let mut verify = Hmac::<Sha256>::new_from_slice(&key).unwrap();
            verify.update(b"youmu-link-confirm-enter-v2");
            verify.update(&hash);
            verify.verify_slice(&mac).unwrap();
            let mut mac = Hmac::<Sha256>::new_from_slice(&key).unwrap();
            mac.update(b"youmu-link-confirm-display-v2");
            mac.update(&hash);
            display
                .writer
                .send_inner(&Authentication::Confirm {
                    v: VERSION,
                    mac: mac.finalize().into_bytes().to_vec(),
                })
                .await
                .unwrap();
            assert!(matches!(
                display.reader.recv_inner::<Authentication>().await.unwrap(),
                Authentication::Accepted { v: VERSION }
            ));
            reached_tx.send(()).unwrap();
            send_rx.await.unwrap();
            display
                .writer
                .send_inner(&Authentication::Accepted { v: VERSION })
                .await
                .unwrap();
            sent_tx.send(()).unwrap();
            let _ = release_rx.await;
        });
        let result = {
            let authorization =
                enter.authorize_enter(&mut trust, std::future::ready(Ok("0427".into())));
            tokio::pin!(authorization);
            tokio::select! { reached=reached_rx=>reached.unwrap(),result=&mut authorization=>panic!("completed before final frame: {result:?}") }
            send_tx.send(()).unwrap();
            sent_rx.await.unwrap();
            tokio::time::timeout(Duration::from_secs(1), async {
                loop {
                    let mut available: libc::c_int = 0;
                    // SAFETY: the live session owns fd; ioctl only fills this valid c_int and consumes no bytes.
                    assert_eq!(
                        unsafe { libc::ioctl(fd, libc::FIONREAD, &mut available) },
                        0
                    );
                    if available >= 0 && available as usize >= size {
                        break;
                    }
                    tokio::time::sleep(Duration::from_millis(1)).await;
                }
            })
            .await
            .unwrap();
            assert!(Instant::now() < deadline, "setup missed deadline");
            tokio::time::sleep(
                deadline.saturating_duration_since(Instant::now()) + Duration::from_millis(25),
            )
            .await;
            authorization.await
        };
        let _ = release_tx.send(());
        peer.await.unwrap();
        assert!(
            matches!(result, Err(Error::PairingExpired)),
            "late authorization: {result:?}"
        );
        assert!(trust.peers().is_empty());
        assert!(matches!(enter.into_split(), Err(Error::PairingRequired)));
    }
}
