use std::fs;
use std::path::PathBuf;
use ymlink_core::files::{FileCatalog, FileError, FileRequest, FileResponse, MAX_READ_BYTES};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path =
            std::env::temp_dir().join(format!("ymlink-files-{:016x}", rand::random::<u64>()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn catalog(&self) -> FileCatalog {
        let mut catalog = FileCatalog::new();
        catalog
            .add("films".into(), "Films".into(), &self.0)
            .unwrap();
        catalog
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stat(catalog: &FileCatalog, path: &str) -> ymlink_core::files::FileEntry {
    match catalog.handle(FileRequest::Stat {
        share_id: "films".into(),
        path: path.into(),
    }) {
        FileResponse::Stat { entry } => entry,
        _ => panic!("expected file metadata"),
    }
}

fn read(
    catalog: &FileCatalog,
    path: &str,
    version: &str,
    offset: u64,
    length: u32,
) -> FileResponse {
    catalog.handle(FileRequest::Read {
        share_id: "films".into(),
        path: path.into(),
        version: version.into(),
        offset,
        length,
    })
}

#[test]
fn shares_list_in_name_order_with_bounded_pages_and_no_local_paths() {
    let fixture = Fixture::new();
    for name in ["z.mkv", "a.mkv", "b.mkv"] {
        fs::write(fixture.0.join(name), b"movie").unwrap();
    }
    let catalog = fixture.catalog();
    assert_eq!(catalog.shares()[0].name, "Films");
    let first = catalog.handle(FileRequest::List {
        share_id: "films".into(),
        path: "".into(),
        cursor: None,
        limit: 2,
    });
    let cursor = match first {
        FileResponse::List {
            entries,
            next_cursor,
        } => {
            assert_eq!(
                entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
                ["a.mkv", "b.mkv"]
            );
            assert!(entries.iter().all(|e| !e.path.starts_with('/')));
            next_cursor
        }
        _ => panic!("expected directory page"),
    };
    match catalog.handle(FileRequest::List {
        share_id: "films".into(),
        path: "".into(),
        cursor,
        limit: 2,
    }) {
        FileResponse::List {
            entries,
            next_cursor,
        } => {
            assert_eq!(entries[0].name, "z.mkv");
            assert!(next_cursor.is_none());
        }
        _ => panic!("expected final directory page"),
    }
}

#[test]
fn read_supports_seek_resume_eof_and_rejects_changed_files() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join("film.mkv"), b"0123456789").unwrap();
    let catalog = fixture.catalog();
    let entry = stat(&catalog, "film.mkv");
    let version = entry.version.unwrap();
    assert_eq!(entry.size, 10);
    assert!(
        matches!(read(&catalog, "film.mkv", &version, 3, 4), FileResponse::Read { data, eof: false, .. } if data == b"3456")
    );
    assert!(
        matches!(read(&catalog, "film.mkv", &version, 7, 9), FileResponse::Read { data, eof: true, total: 10, .. } if data == b"789")
    );
    assert!(
        matches!(read(&catalog, "film.mkv", &version, 10, 1), FileResponse::Read { data, eof: true, .. } if data.is_empty())
    );
    fs::write(fixture.0.join("replacement"), b"abcdefghij").unwrap();
    fs::rename(fixture.0.join("replacement"), fixture.0.join("film.mkv")).unwrap();
    assert_eq!(
        read(&catalog, "film.mkv", &version, 3, 4),
        FileResponse::Error {
            code: FileError::Changed
        }
    );
}

#[test]
fn invalid_paths_unknown_or_removed_shares_have_the_same_denial() {
    let fixture = Fixture::new();
    let mut catalog = fixture.catalog();
    for path in [
        "../secret",
        "/etc/passwd",
        "./movie",
        "a/../movie",
        "a//b",
        "a\\b",
        "C:secret",
        "bad\0name",
    ] {
        assert_eq!(
            catalog.handle(FileRequest::Stat {
                share_id: "films".into(),
                path: path.into()
            }),
            FileResponse::Error {
                code: FileError::Denied
            }
        );
    }
    assert!(catalog.remove("films"));
    assert_eq!(
        catalog.handle(FileRequest::Stat {
            share_id: "films".into(),
            path: "movie".into()
        }),
        FileResponse::Error {
            code: FileError::Denied
        }
    );
    assert!(catalog.shares().is_empty());
}

#[cfg(windows)]
#[test]
fn windows_first_read_after_aged_access_time_keeps_the_issued_version() {
    for from_list in [false, true] {
        let fixture = Fixture::new();
        let path = fixture.0.join("film.mp4");
        fs::write(&path, b"0123456789abcdef").unwrap();
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_times(std::fs::FileTimes::new().set_accessed(
            std::time::SystemTime::now() - std::time::Duration::from_secs(2 * 24 * 60 * 60),
        ))
        .unwrap();
        drop(file);
        let catalog = fixture.catalog();
        let version = if from_list {
            match catalog.handle(FileRequest::List {
                share_id: "films".into(),
                path: "".into(),
                cursor: None,
                limit: 1,
            }) {
                FileResponse::List { entries, .. } => entries[0].version.clone().unwrap(),
                _ => panic!("expected file page"),
            }
        } else {
            stat(&catalog, "film.mp4").version.unwrap()
        };
        assert!(
            matches!(read(&catalog,"film.mp4",&version,0,4),FileResponse::Read{data,..} if data==b"0123"),
            "first read must preserve a metadata-only access update"
        );
        assert!(
            matches!(read(&catalog,"film.mp4",&version,12,4),FileResponse::Read{data,eof:true,..} if data==b"cdef")
        );
        assert_eq!(stat(&catalog, "film.mp4").version.as_ref(), Some(&version));
    }
}

#[cfg(windows)]
#[test]
fn windows_readonly_sparse_five_gib_file_supports_list_seek_and_eof() {
    use std::io::{Seek, SeekFrom, Write};
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::{Ioctl::FSCTL_SET_SPARSE, IO::DeviceIoControl};
    let fixture = Fixture::new();
    let path = fixture.0.join("film.mp4");
    let mut writer = fs::File::create(&path).unwrap();
    let mut returned = 0;
    assert_ne!(
        unsafe {
            DeviceIoControl(
                writer.as_raw_handle(),
                FSCTL_SET_SPARSE,
                std::ptr::null(),
                0,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        },
        0
    );
    let size = 5u64 * 1024 * 1024 * 1024;
    writer.set_len(size).unwrap();
    writer.write_all(b"head").unwrap();
    writer.seek(SeekFrom::Start(size - 4)).unwrap();
    writer.write_all(b"tail").unwrap();
    drop(writer);
    struct RestorePermissions(PathBuf, fs::Permissions);
    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            fs::set_permissions(&self.0, self.1.clone()).unwrap();
        }
    }
    let permissions = fs::metadata(&path).unwrap().permissions();
    let _restore = RestorePermissions(path.clone(), permissions.clone());
    let mut readonly = permissions;
    readonly.set_readonly(true);
    fs::set_permissions(&path, readonly).unwrap();
    let catalog = fixture.catalog();
    let FileResponse::List { entries, .. } = catalog.handle(FileRequest::List {
        share_id: "films".into(),
        path: "".into(),
        cursor: None,
        limit: 1,
    }) else {
        panic!("expected file page")
    };
    let version = entries[0].version.as_ref().unwrap();
    assert_eq!(entries[0].size, size);
    for (offset, expected) in [
        (0, b"head".as_slice()),
        (2 * 1024 * 1024 * 1024 + 7, &[0u8; 4]),
        (4 * 1024 * 1024 * 1024 + 7, &[0u8; 4]),
        (size - 4, b"tail".as_slice()),
        (size, b"".as_slice()),
    ] {
        assert!(matches!(
            read(&catalog, "film.mp4", version, offset, 4),
            FileResponse::Read { data, total, eof, .. }
                if data == expected && total == size && eof == (offset >= size - 4)
        ));
    }
    assert_eq!(stat(&catalog, "film.mp4").version.as_ref(), Some(version));
    assert!(fs::metadata(&path).unwrap().permissions().readonly());
}

#[cfg(windows)]
#[test]
fn windows_same_file_overwrite_with_restored_mtime_invalidates_resume() {
    use std::io::Write;
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION,
    };
    fn id(file: &fs::File) -> (u32, u32, u32) {
        let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
        assert_ne!(
            unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) },
            0
        );
        (
            info.dwVolumeSerialNumber,
            info.nFileIndexHigh,
            info.nFileIndexLow,
        )
    }
    let fixture = Fixture::new();
    let path = fixture.0.join("film.mp4");
    fs::write(&path, b"old film").unwrap();
    let catalog = fixture.catalog();
    let old_version = stat(&catalog, "film.mp4").version.unwrap();
    let before = fs::metadata(&path).unwrap();
    let old_id = id(&fs::File::open(&path).unwrap());
    std::thread::sleep(std::time::Duration::from_millis(20));
    let mut writer = fs::OpenOptions::new().write(true).open(&path).unwrap();
    writer.write_all(b"new film").unwrap();
    writer
        .set_times(std::fs::FileTimes::new().set_modified(before.modified().unwrap()))
        .unwrap();
    drop(writer);
    let after = fs::metadata(&path).unwrap();
    assert_eq!(before.len(), after.len());
    assert_eq!(before.created().unwrap(), after.created().unwrap());
    assert_eq!(before.modified().unwrap(), after.modified().unwrap());
    assert_eq!(old_id, id(&fs::File::open(&path).unwrap()));
    assert_eq!(
        read(&catalog, "film.mp4", &old_version, 0, 8),
        FileResponse::Error {
            code: FileError::Changed
        }
    );
    assert_ne!(stat(&catalog, "film.mp4").version.unwrap(), old_version);
}

#[cfg(windows)]
#[test]
fn windows_replacement_with_same_size_and_timestamps_invalidates_resume() {
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::Storage::FileSystem::{GetFileTime, SetFileTime};
    let fixture = Fixture::new();
    let path = fixture.0.join("film.mkv");
    fs::write(&path, b"old film").unwrap();
    let catalog = fixture.catalog();
    let old_version = stat(&catalog, "film.mkv").version.unwrap();
    let source = fs::File::open(&path).unwrap();
    unsafe {
        let mut created = std::mem::zeroed();
        let mut modified = std::mem::zeroed();
        assert_ne!(
            GetFileTime(
                source.as_raw_handle(),
                &mut created,
                std::ptr::null_mut(),
                &mut modified
            ),
            0
        );
        fs::write(fixture.0.join("replacement"), b"new film").unwrap();
        let replacement = fs::OpenOptions::new()
            .write(true)
            .open(fixture.0.join("replacement"))
            .unwrap();
        assert_ne!(
            SetFileTime(
                replacement.as_raw_handle(),
                &created,
                std::ptr::null(),
                &modified
            ),
            0
        );
    }
    drop(source);
    fs::rename(fixture.0.join("replacement"), &path).unwrap();
    assert_eq!(
        read(&catalog, "film.mkv", &old_version, 0, 8),
        FileResponse::Error {
            code: FileError::Changed
        }
    );
    assert_ne!(stat(&catalog, "film.mkv").version.unwrap(), old_version);
}

#[cfg(windows)]
#[test]
fn windows_ambiguous_names_and_device_paths_are_denied_before_open() {
    let fixture = Fixture::new();
    let catalog = fixture.catalog();
    for path in [
        "NUL",
        "nul.mkv",
        "CON",
        "sub/COM1.txt",
        "AUX",
        "LPT9",
        "film.",
        "film ",
        "sub /file",
    ] {
        assert_eq!(
            catalog.handle(FileRequest::Stat {
                share_id: "films".into(),
                path: path.into()
            }),
            FileResponse::Error {
                code: FileError::Denied
            },
            "{path}"
        );
    }
}

#[test]
fn request_limits_bound_allocation_and_noise_frame_size() {
    let fixture = Fixture::new();
    fs::write(
        fixture.0.join("bytes"),
        vec![255u8; MAX_READ_BYTES as usize],
    )
    .unwrap();
    let catalog = fixture.catalog();
    let version = stat(&catalog, "bytes").version.unwrap();
    for (offset, length) in [(0, 0), (0, MAX_READ_BYTES + 1), (u64::MAX, 1)] {
        assert_eq!(
            read(&catalog, "bytes", &version, offset, length),
            FileResponse::Error {
                code: FileError::InvalidRequest
            }
        );
    }
    let mut cbor = Vec::new();
    ciborium::into_writer(
        &read(&catalog, "bytes", &version, 0, MAX_READ_BYTES),
        &mut cbor,
    )
    .unwrap();
    assert!(cbor.len() < 65_519);
    for limit in [0, 257] {
        assert_eq!(
            catalog.handle(FileRequest::List {
                share_id: "films".into(),
                path: "".into(),
                cursor: None,
                limit
            }),
            FileResponse::Error {
                code: FileError::InvalidRequest
            }
        );
    }
}

#[test]
fn long_directory_pages_stay_in_one_noise_frame_and_cover_every_entry() {
    let fixture = Fixture::new();
    for index in 0..300 {
        fs::write(
            fixture.0.join(format!("{index:03}-{}", "x".repeat(240))),
            [],
        )
        .unwrap();
    }
    let catalog = fixture.catalog();
    let mut cursor = None;
    let mut names = std::collections::BTreeSet::new();
    loop {
        let response = catalog.handle(FileRequest::List {
            share_id: "films".into(),
            path: "".into(),
            cursor,
            limit: 256,
        });
        let mut encoded = Vec::new();
        ciborium::into_writer(&response, &mut encoded).unwrap();
        assert!(encoded.len() < 65_519);
        match response {
            FileResponse::List {
                entries,
                next_cursor,
            } => {
                assert!(!entries.is_empty());
                assert!(entries.len() <= 256);
                for entry in entries {
                    assert!(names.insert(entry.name));
                }
                match next_cursor {
                    Some(next) => cursor = Some(next),
                    None => break,
                }
            }
            _ => panic!("expected directory page"),
        }
    }
    assert_eq!(names.len(), 300);
}

#[cfg(unix)]
#[test]
fn capability_confines_symlinks_and_rejects_fifo_without_blocking() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    let outside = Fixture::new();
    fs::write(outside.0.join("secret"), b"private").unwrap();
    fs::write(fixture.0.join("film"), b"public").unwrap();
    symlink(outside.0.join("secret"), fixture.0.join("escape")).unwrap();
    symlink("film", fixture.0.join("inside")).unwrap();
    let fifo =
        std::ffi::CString::new(fixture.0.join("pipe").as_os_str().as_encoded_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
    let catalog = fixture.catalog();
    assert!(matches!(
        catalog.handle(FileRequest::Stat {
            share_id: "films".into(),
            path: "escape".into()
        }),
        FileResponse::Error { .. }
    ));
    assert!(stat(&catalog, "inside").version.is_some());
    assert_eq!(
        catalog.handle(FileRequest::Stat {
            share_id: "films".into(),
            path: "pipe".into()
        }),
        FileResponse::Error {
            code: FileError::NotFile
        }
    );
    assert_eq!(
        read(&catalog, "pipe", &"0".repeat(64), 0, 1),
        FileResponse::Error {
            code: FileError::NotFile
        }
    );
    match catalog.handle(FileRequest::List {
        share_id: "films".into(),
        path: "".into(),
        cursor: None,
        limit: 256,
    }) {
        FileResponse::List { entries, .. } => assert_eq!(
            entries.iter().map(|e| e.name.as_str()).collect::<Vec<_>>(),
            ["film", "inside"]
        ),
        _ => panic!("expected directory page"),
    }
}
