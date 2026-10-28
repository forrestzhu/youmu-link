//! Device identity and explicit peer trust. Private keys never enter the trust file.

use crate::session::{Error, Hello, Result};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use zeroize::Zeroizing;

#[derive(Clone)]
pub struct Identity {
    secret: Zeroizing<[u8; 32]>,
    public: [u8; 32],
}

impl Identity {
    /// Generates a fresh random identity.
    ///
    /// # Errors
    ///
    /// This cannot currently fail; the `Result` keeps the identity API uniform
    /// with [`Identity::load_or_create`].
    pub fn generate() -> Result<Self> {
        let mut secret = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut secret);
        Ok(Self::from_secret(secret))
    }

    fn from_secret(secret: [u8; 32]) -> Self {
        Self {
            public: x25519_dalek::x25519(secret, x25519_dalek::X25519_BASEPOINT_BYTES),
            secret: Zeroizing::new(secret),
        }
    }

    /// A Keychain failure is fatal: never silently downgrade identity storage.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Storage`] when the Keychain entry cannot be opened,
    /// read, validated or written.
    #[cfg(target_os = "macos")]
    pub fn load_or_create(_state_dir: &Path) -> Result<Self> {
        let entry = keyring::Entry::new("app.youmu.link", "device-identity-v2")
            .map_err(|_| Error::Storage("cannot open identity Keychain entry".into()))?;
        match entry.get_secret() {
            Ok(secret) => {
                let secret = Zeroizing::new(secret);
                let bytes: [u8; 32] = secret
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::Storage("invalid identity Keychain entry".into()))?;
                Ok(Self::from_secret(bytes))
            }
            Err(keyring::Error::NoEntry) => {
                let identity = Self::generate()?;
                entry
                    .set_secret(identity.secret.as_ref())
                    .map_err(|_| Error::Storage("cannot save identity in Keychain".into()))?;
                Ok(identity)
            }
            Err(_) => Err(Error::Storage("cannot read identity from Keychain".into())),
        }
    }

    /// A Credential Manager failure is fatal: never silently downgrade storage.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Storage`] when the credential entry cannot be opened,
    /// read, validated or written.
    #[cfg(windows)]
    pub fn load_or_create(_state_dir: &Path) -> Result<Self> {
        let entry = keyring::Entry::new("app.youmu.link", "device-identity-v2")
            .map_err(|_| Error::Storage("cannot open identity Credential Manager entry".into()))?;
        load_windows_identity(&entry)
    }

    /// # Errors
    ///
    /// Always returns [`Error::Storage`]: persistent identity requires macOS or
    /// Windows.
    #[cfg(not(any(target_os = "macos", windows)))]
    pub fn load_or_create(_state_dir: &Path) -> Result<Self> {
        Err(Error::Storage(
            "persistent identity requires macOS or Windows".into(),
        ))
    }

    #[must_use]
    pub fn device_id(&self) -> String {
        device_id(&self.public)
    }

    #[must_use]
    pub fn public_key(&self) -> [u8; 32] {
        self.public
    }

    pub(crate) fn secret_key(&self) -> &[u8] {
        self.secret.as_ref()
    }
}

#[cfg(windows)]
fn load_windows_identity(entry: &keyring::Entry) -> Result<Identity> {
    match entry.get_secret() {
        Ok(secret) => {
            let secret = Zeroizing::new(secret);
            let bytes: [u8; 32] = secret
                .as_slice()
                .try_into()
                .map_err(|_| Error::Storage("invalid identity Credential Manager entry".into()))?;
            Ok(Identity::from_secret(bytes))
        }
        Err(keyring::Error::NoEntry) => {
            let identity = Identity::generate()?;
            entry
                .set_secret(identity.secret.as_ref())
                .map_err(|_| Error::Storage("cannot save identity in Credential Manager".into()))?;
            Ok(identity)
        }
        Err(_) => Err(Error::Storage(
            "cannot read identity from Credential Manager".into(),
        )),
    }
}

#[must_use]
pub fn device_id(key: &[u8]) -> String {
    use std::fmt::Write as _;
    let hash = Sha256::digest(key);
    let mut id = String::with_capacity(32);
    for byte in &hash[..16] {
        let _ = write!(id, "{byte:02x}");
    }
    id
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrustedPeer {
    pub device_id: String,
    pub public_key: Vec<u8>,
    pub name: String,
    pub platform: String,
    pub endpoint: Option<String>,
}

pub struct TrustStore {
    path: Option<PathBuf>,
    peers: BTreeMap<String, TrustedPeer>,
}

impl TrustStore {
    #[must_use]
    pub fn memory() -> Self {
        Self {
            path: None,
            peers: BTreeMap::new(),
        }
    }

    /// A non-persisting snapshot for network authorization. The runtime commits
    /// confirmed trust to the live store only after checking cancellation and
    /// its active-connection generation, without locking the UI for network IO.
    #[must_use]
    pub fn detached(&self) -> Self {
        Self {
            path: None,
            peers: self.peers.clone(),
        }
    }

    /// Loads the trust store, treating a missing file as empty.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Storage`] when the file cannot be read or contains a
    /// structurally or cryptographically invalid trust entry.
    pub fn load(path: &Path) -> Result<Self> {
        let peers: BTreeMap<String, TrustedPeer> = match fs::read(path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|_| Error::Storage("invalid trust store".into()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(_) => return Err(Error::Storage("cannot read trust store".into())),
        };
        for (id, peer) in &peers {
            if peer.public_key.len() != 32
                || peer.device_id != *id
                || device_id(&peer.public_key) != *id
            {
                return Err(Error::Storage("invalid trusted identity".into()));
            }
        }
        Ok(Self {
            path: Some(path.to_owned()),
            peers,
        })
    }

    #[must_use]
    pub fn peers(&self) -> Vec<TrustedPeer> {
        self.peers.values().cloned().collect()
    }

    /// Validates the cryptographic pin even when the peer is not yet trusted.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IdentityChanged`] when the hello key or device id is
    /// inconsistent, or a stored peer holds a different key.
    pub fn check(&self, hello: &Hello, key: &[u8; 32]) -> Result<bool> {
        if hello.public_key != key || hello.device_id != device_id(key) {
            return Err(Error::IdentityChanged);
        }
        match self.peers.get(&hello.device_id) {
            Some(peer) if peer.public_key != key => Err(Error::IdentityChanged),
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }

    /// Before offering a fresh pairing, reject a different identity at a pinned
    /// endpoint. A previously trusted key may legitimately move to a new IP.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IdentityChanged`] on an inconsistent identity or when a
    /// different device is already pinned to this endpoint.
    pub fn check_endpoint(&self, hello: &Hello, key: &[u8; 32], endpoint: &str) -> Result<bool> {
        let known = self.check(hello, key)?;
        if !known
            && self.peers.values().any(|peer| {
                peer.endpoint.as_deref() == Some(endpoint) && peer.device_id != hello.device_id
            })
        {
            return Err(Error::IdentityChanged);
        }
        Ok(known)
    }

    /// Remembers a peer without changing its endpoint.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IdentityChanged`] or [`Error::Storage`] propagation from
    /// [`Self::remember_endpoint`].
    pub fn remember(&mut self, hello: &Hello, key: &[u8; 32]) -> Result<()> {
        self.remember_endpoint(hello, key, None)
    }

    /// Remembers a peer and records the endpoint it was authorized on.
    ///
    /// # Errors
    ///
    /// Returns [`Error::IdentityChanged`] when the identity is inconsistent and
    /// [`Error::Storage`] when the persisted store cannot be updated.
    pub fn remember_endpoint(
        &mut self,
        hello: &Hello,
        key: &[u8; 32],
        endpoint: Option<String>,
    ) -> Result<()> {
        self.check(hello, key)?;
        let mut next = self.peers.clone();
        let endpoint = endpoint.or_else(|| {
            next.get(&hello.device_id)
                .and_then(|peer| peer.endpoint.clone())
        });
        next.insert(
            hello.device_id.clone(),
            TrustedPeer {
                device_id: hello.device_id.clone(),
                public_key: key.to_vec(),
                name: hello.name.clone(),
                platform: hello.platform.clone(),
                endpoint,
            },
        );
        self.persist(&next)?;
        self.peers = next;
        Ok(())
    }

    /// Forgets a peer; forgetting an unknown id is not an error.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Storage`] when the persisted store cannot be updated.
    pub fn forget(&mut self, id: &str) -> Result<()> {
        let mut next = self.peers.clone();
        next.remove(id);
        self.persist(&next)?;
        self.peers = next;
        Ok(())
    }

    fn persist(&self, next: &BTreeMap<String, TrustedPeer>) -> Result<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let parent = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .ok_or_else(|| Error::Storage("trust store needs an application directory".into()))?;
        crate::storage::private_directory(parent)
            .map_err(|_| Error::Storage("cannot secure trust directory".into()))?;
        let temp = parent.join(format!(".trust-{:016x}.tmp", rand::random::<u64>()));
        let result = (|| -> Result<()> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options
                .open(&temp)
                .map_err(|_| Error::Storage("cannot create private trust file".into()))?;
            serde_json::to_writer(&mut file, next)
                .map_err(|_| Error::Storage("cannot encode trust store".into()))?;
            file.write_all(b"\n")
                .and_then(|()| file.sync_all())
                .map_err(|_| Error::Storage("cannot sync trust store".into()))?;
            fs::rename(&temp, path)
                .map_err(|_| Error::Storage("cannot replace trust store".into()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(temp);
        }
        result
    }
}

#[cfg(all(test, windows))]
mod windows_tests {
    use super::*;
    #[test]
    fn credential_identity_survives_reload_and_rejects_corruption() {
        let account = format!("test-{:016x}", rand::random::<u64>());
        let entry = keyring::Entry::new("app.youmu.link.tests", &account).unwrap();
        struct Cleanup(keyring::Entry);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                let _ = self.0.delete_credential();
            }
        }
        let cleanup = Cleanup(entry);
        let first = load_windows_identity(&cleanup.0).unwrap();
        let second = load_windows_identity(&cleanup.0).unwrap();
        assert_eq!(first.public_key(), second.public_key());
        assert_eq!(cleanup.0.get_secret().unwrap().len(), 32);
        cleanup.0.set_secret(b"corrupt").unwrap();
        assert!(load_windows_identity(&cleanup.0).is_err());
        assert_eq!(cleanup.0.get_secret().unwrap(), b"corrupt");
    }
}
