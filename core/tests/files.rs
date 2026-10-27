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
