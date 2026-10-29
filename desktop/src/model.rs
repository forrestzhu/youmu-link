use serde::{Deserialize, Serialize};
use std::time::Instant;

#[derive(Clone, Serialize)]
pub struct Peer {
    pub id: String,
    pub address: String,
    pub name: String,
    pub platform: String,
    pub status: String,
    pub trusted: bool,
}

#[derive(Clone, Serialize)]
pub struct PairingView {
    pub peer_name: String,
    pub code: Option<String>,
    pub expires_in: u64,
    pub mode: String,
    pub window_id: String,
    pub attempt: u64,
    pub stage: String,
    pub error: Option<String>,
    pub error_revision: u64,
    pub retry_after_ms: u32,
    #[serde(skip)]
    pub cooldown_deadline: Option<Instant>,
    #[serde(skip)]
    pub deadline: Instant,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Share {
    pub id: String,
    pub name: String,
    pub path: String,
}

#[derive(Clone, Serialize)]
pub struct Transfer {
    pub id: String,
    pub name: String,
    pub transferred: u64,
    pub total: u64,
    pub status: String,
}

#[derive(Clone, Serialize)]
pub struct Mirror {
    pub status: String,
    pub permission: String,
    pub display_id: Option<u32>,
    pub audio: bool,
    pub quality: String,
    pub fps: Option<f64>,
    pub bitrate_mbps: Option<f64>,
    pub latency_ms: Option<f64>,
    pub error: Option<String>,
}

impl Default for Mirror {
    fn default() -> Self {
        Self {
            status: "stopped".into(),
            permission: "unknown".into(),
            display_id: None,
            audio: true,
            quality: "balanced".into(),
            fps: None,
            bitrate_mbps: None,
            latency_ms: None,
            error: None,
        }
    }
}

#[derive(Serialize)]
pub struct Snapshot {
    pub device_name: String,
    pub status: String,
    pub listen_port: u16,
    pub discovery_port: u16,
    pub peers: Vec<Peer>,
    pub pairing: Option<PairingView>,
    pub displays: Vec<crate::capture::Display>,
    pub mirror: Mirror,
    pub shares: Vec<Share>,
    pub transfers: Vec<Transfer>,
    pub error_id: u64,
    pub error: Option<String>,
}

impl Snapshot {
    pub fn new(name: String) -> Self {
        Self {
            device_name: name,
            status: "starting".into(),
            listen_port: 0,
            discovery_port: 0,
            peers: vec![],
            pairing: None,
            displays: vec![],
            mirror: Mirror::default(),
            shares: vec![],
            transfers: vec![],
            error_id: 0,
            error: None,
        }
    }

    pub fn set_error(&mut self, error: impl Into<String>) {
        self.error_id = self.error_id.wrapping_add(1);
        self.error = Some(error.into());
    }

    pub fn interrupt_transfers(&mut self) {
        for transfer in &mut self.transfers {
            if ["transferring", "paused"].contains(&transfer.status.as_str()) {
                transfer.status = "waiting".into();
            }
        }
    }
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Command {
    Rescan,
    Connect {
        address: Option<String>,
        peer_id: Option<String>,
    },
    Pair {
        code: String,
        window_id: String,
        attempt: u64,
    },
    CancelPairing,
    RetryPairing {},
    Disconnect {
        peer_id: Option<String>,
    },
    Forget {
        peer_id: String,
    },
    AddShare {
        path: String,
    },
    RemoveShare {
        id: String,
    },
    RefreshDisplays,
    StartMirror {
        display_id: u32,
        audio: bool,
        quality: String,
    },
    StopMirror,
    PauseMirror,
    ResumeMirror,
    SelectMirrorDisplay {
        display_id: u32,
    },
    PauseTransfer {
        id: String,
    },
    ResumeTransfer {
        id: String,
    },
    CancelTransfer {
        id: String,
    },
}
