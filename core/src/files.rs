//! Read-only capability-confined files, served only over an authorized session.

use cap_std::fs::{Dir, Metadata, OpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

pub const MAX_READ_BYTES: u32 = 16_384;
pub const MAX_LIST_ENTRIES: u16 = 256;
const MAX_SHARES: usize = 64;
const LIST_BYTE_BUDGET: usize = 48 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ShareInfo {
    pub id: String,
    pub name: String,
}

/// An explicitly granted directory handle; no remote operation uses ambient paths.
pub struct SharedRoot {
    info: ShareInfo,
    dir: Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
    pub size: u64,
    pub version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileRequest {
    Shares,
    List {
        share_id: String,
        path: String,
        cursor: Option<String>,
        limit: u16,
    },
    Stat {
        share_id: String,
        path: String,
    },
    Read {
        share_id: String,
        path: String,
        version: String,
        offset: u64,
        length: u32,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum FileResponse {
    Shares {
        shares: Vec<ShareInfo>,
    },
    List {
        entries: Vec<FileEntry>,
        next_cursor: Option<String>,
    },
    Stat {
        entry: FileEntry,
    },
    Read {
        offset: u64,
        total: u64,
        version: String,
        data: Vec<u8>,
        eof: bool,
    },
    Error {
        code: FileError,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FileError {
    Denied,
    InvalidRequest,
    Unavailable,
    NotFile,
    NotDirectory,
    Changed,
}

impl fmt::Display for FileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Denied => "file access denied",
            Self::InvalidRequest => "invalid file request",
            Self::Unavailable => "file unavailable",
            Self::NotFile => "not a regular file",
            Self::NotDirectory => "not a directory",
            Self::Changed => "file changed; restart this read",
        })
    }
}

impl std::error::Error for FileError {}

#[derive(Default)]
pub struct FileCatalog {
    roots: BTreeMap<String, SharedRoot>,
}

impl FileCatalog {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Only call for a directory explicitly selected by the local user.
    ///
    /// # Errors
    ///
    /// Returns [`FileError::InvalidRequest`] when the id or name is invalid or
    /// the share limit is reached, and [`FileError::Unavailable`] when the
    /// directory cannot be opened.
    pub fn add(
        &mut self,
        id: String,
        name: String,
        local_path: &Path,
    ) -> Result<ShareInfo, FileError> {
        if id.is_empty()
            || id.len() > 64
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
            || name.trim().is_empty()
            || name.len() > 128
            || name.chars().any(char::is_control)
            || self.roots.len() >= MAX_SHARES
            || self.roots.contains_key(&id)
        {
            return Err(FileError::InvalidRequest);
        }
        let dir =
            Dir::open_ambient_dir(local_path, cap_std::ambient_authority()).map_err(io_error)?;
        let info = ShareInfo {
            id: id.clone(),
            name,
        };
        self.roots.insert(
            id,
            SharedRoot {
                info: info.clone(),
                dir,
            },
        );
        Ok(info)
    }

    pub fn remove(&mut self, id: &str) -> bool {
        self.roots.remove(id).is_some()
    }

    #[must_use]
    pub fn shares(&self) -> Vec<ShareInfo> {
        self.roots.values().map(|root| root.info.clone()).collect()
    }

    #[must_use]
    pub fn handle(&self, request: FileRequest) -> FileResponse {
        self.execute(request)
            .unwrap_or_else(|code| FileResponse::Error { code })
    }

    fn root(&self, share_id: &str, path: &str) -> Result<&SharedRoot, FileError> {
        if !valid_path(path) {
            return Err(FileError::Denied);
        }
        self.roots.get(share_id).ok_or(FileError::Denied)
    }

    fn execute(&self, request: FileRequest) -> Result<FileResponse, FileError> {
        match request {
            FileRequest::Shares => Ok(FileResponse::Shares {
                shares: self.shares(),
            }),
            FileRequest::Stat { share_id, path } => {
                let root = self.root(&share_id, &path)?;
                Ok(FileResponse::Stat {
                    entry: root.entry(&path)?,
                })
            }
            FileRequest::List {
                share_id,
                path,
                cursor,
                limit,
            } => {
                let root = self.root(&share_id, &path)?;
                if !(1..=MAX_LIST_ENTRIES).contains(&limit)
                    || cursor
                        .as_ref()
                        .is_some_and(|c| c.is_empty() || c.contains('/') || !valid_path(c))
                {
                    return Err(FileError::InvalidRequest);
                }
                if !root.entry(&path)?.is_directory {
                    return Err(FileError::NotDirectory);
                }
                let dir = root.dir.open_dir(local_path(&path)).map_err(io_error)?;
                // ponytail: each page scans the directory, with O(limit) memory;
                // add a bounded directory snapshot cache only if real library sizes need it.
                let mut candidates = BTreeMap::new();
                for item in dir.entries().map_err(io_error)? {
                    let Ok(item) = item else { continue };
                    let Ok(name) = item.file_name().into_string() else {
                        continue;
                    };
                    if !valid_path(&name)
                        || name.is_empty()
                        || cursor.as_ref().is_some_and(|after| name <= *after)
                    {
                        continue;
                    }
                    let child = if path.is_empty() {
                        name.clone()
                    } else {
                        format!("{path}/{name}")
                    };
                    if !valid_path(&child) {
                        continue;
                    }
                    if let Ok(entry) = root.entry(&child) {
                        candidates.insert(name, entry);
                        if candidates.len() > usize::from(limit) + 1 {
                            candidates.pop_last();
                        }
                    }
                }
                let mut entries = Vec::new();
                let mut bytes = 0;
                let mut more = false;
                for entry in candidates.into_values() {
                    // 160 covers CBOR keys, integer widths and the 64-byte version token.
                    let cost = entry.name.len() + entry.path.len() + 160;
                    if entries.len() == usize::from(limit) || bytes + cost > LIST_BYTE_BUDGET {
                        more = true;
                        break;
                    }
                    bytes += cost;
                    entries.push(entry);
                }
                let next_cursor = if more {
                    entries.last().map(|entry| entry.name.clone())
                } else {
                    None
                };
                Ok(FileResponse::List {
                    entries,
                    next_cursor,
                })
            }
            FileRequest::Read {
                share_id,
                path,
                version,
                offset,
                length,
            } => {
                let root = self.root(&share_id, &path)?;
                if length == 0
                    || length > MAX_READ_BYTES
                    || version.len() != 64
                    || !version.bytes().all(|b| b.is_ascii_hexdigit())
                {
                    return Err(FileError::InvalidRequest);
                }
                let mut file = root.open_file(&path)?;
                let before = file.metadata().map_err(io_error)?;
                if file_version(&file, &before)? != version {
                    return Err(FileError::Changed);
                }
                if offset > before.len() {
                    return Err(FileError::InvalidRequest);
                }
                let count = u64::from(length).min(before.len() - offset) as usize;
                file.seek(SeekFrom::Start(offset)).map_err(io_error)?;
                let mut data = vec![0; count];
                file.read_exact(&mut data).map_err(|_| FileError::Changed)?;
                if file_version(&file, &file.metadata().map_err(io_error)?)? != version {
                    return Err(FileError::Changed);
                }
                Ok(FileResponse::Read {
                    offset,
                    total: before.len(),
                    version,
                    eof: offset + count as u64 == before.len(),
                    data,
                })
            }
        }
    }
}

impl SharedRoot {
    fn entry(&self, path: &str) -> Result<FileEntry, FileError> {
        let metadata = if path.is_empty() {
            self.dir.dir_metadata()
        } else {
            self.dir.metadata(path)
        }
        .map_err(io_error)?;
        let (size, version) = if metadata.is_dir() {
            (0, None)
        } else if metadata.is_file() {
            let file = self.open_file(path)?;
            let actual = file.metadata().map_err(io_error)?;
            (actual.len(), Some(file_version(&file, &actual)?))
        } else {
            return Err(FileError::NotFile);
        };
        Ok(FileEntry {
            name: path
                .rsplit('/')
                .next()
                .filter(|s| !s.is_empty())
                .unwrap_or(&self.info.name)
                .into(),
            path: path.into(),
            is_directory: metadata.is_dir(),
            size,
            version,
        })
    }

    fn open_file(&self, path: &str) -> Result<cap_std::fs::File, FileError> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NONBLOCK);
        }
        let file = self
            .dir
            .open_with(local_path(path), &options)
            .map_err(io_error)?;
        if !file.metadata().map_err(io_error)?.is_file() {
            return Err(FileError::NotFile);
        }
        Ok(file)
    }
}

fn local_path(path: &str) -> &str {
    if path.is_empty() {
        "."
    } else {
        path
    }
}

fn valid_path(path: &str) -> bool {
    path.is_empty()
        || (path.len() <= 4096
            && !path.contains(['\\', ':'])
            && !path.chars().any(char::is_control)
            && path.split('/').all(|part| {
                !part.is_empty()
                    && part.len() <= 255
                    && part != "."
                    && part != ".."
                    && platform_component(part)
            }))
}

#[cfg(not(windows))]
fn platform_component(_: &str) -> bool {
    true
}

#[cfg(windows)]
fn platform_component(part: &str) -> bool {
    if part.ends_with([' ', '.']) {
        return false;
    }
    let stem = part
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    !matches!(
        stem.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !((stem.starts_with("COM") || stem.starts_with("LPT"))
        && (matches!(stem.as_bytes().get(3), Some(b'1'..=b'9')) && stem.len() == 4
            || matches!(&stem[3..], "¹" | "²" | "³")))
}

fn file_version(_file: &cap_std::fs::File, metadata: &Metadata) -> Result<String, FileError> {
    let mut digest = Sha256::new();
    digest.update(metadata.len().to_le_bytes());
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        digest.update(metadata.dev().to_le_bytes());
        digest.update(metadata.ino().to_le_bytes());
        digest.update(metadata.mtime().to_le_bytes());
        digest.update(metadata.mtime_nsec().to_le_bytes());
        digest.update(metadata.ctime().to_le_bytes());
        digest.update(metadata.ctime_nsec().to_le_bytes());
    }
    #[cfg(not(unix))]
    {
        for time in [metadata.modified(), metadata.created()] {
            let time = time
                .map_err(io_error)?
                .into_std()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|_| FileError::Unavailable)?;
            digest.update(time.as_nanos().to_le_bytes());
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::io::AsRawHandle;
        use windows_sys::Win32::Storage::FileSystem::{
            FileBasicInfo, GetFileInformationByHandle, GetFileInformationByHandleEx,
            BY_HANDLE_FILE_INFORMATION, FILE_BASIC_INFO,
        };
        // Query the capability-opened file itself: equal size / creation / mtime
        // on a replacement must not authorize continuing the previous stream.
        unsafe {
            let mut info: BY_HANDLE_FILE_INFORMATION = std::mem::zeroed();
            let mut basic: FILE_BASIC_INFO = std::mem::zeroed();
            if GetFileInformationByHandle(_file.as_raw_handle(), &mut info) == 0
                || GetFileInformationByHandleEx(
                    _file.as_raw_handle(),
                    FileBasicInfo,
                    (&mut basic as *mut FILE_BASIC_INFO).cast(),
                    std::mem::size_of::<FILE_BASIC_INFO>() as u32,
                ) == 0
            {
                return Err(FileError::Unavailable);
            }
            digest.update(info.dwVolumeSerialNumber.to_le_bytes());
            digest.update(info.nFileIndexHigh.to_le_bytes());
            digest.update(info.nFileIndexLow.to_le_bytes());
            digest.update(basic.ChangeTime.to_le_bytes());
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn io_error(error: std::io::Error) -> FileError {
    if error.kind() == std::io::ErrorKind::PermissionDenied {
        FileError::Denied
    } else {
        FileError::Unavailable
    }
}
