#![cfg(windows)]

use std::fs;
use std::path::PathBuf;
use ymlink_core::storage::private_directory;

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        Self(std::env::temp_dir().join(format!("ymlink-win-{:016x}", rand::random::<u64>())))
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn private_state_directory_protects_inherited_permissions_and_can_be_reopened() {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Foundation::LocalFree;
    use windows_sys::Win32::Security::Authorization::{GetNamedSecurityInfoW, SE_FILE_OBJECT};
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorControl, DACL_SECURITY_INFORMATION, SE_DACL_PROTECTED,
    };
    let root = Scratch::new();
    private_directory(&root.0).unwrap();
    fs::write(root.0.join("state"), b"private").unwrap();
    private_directory(&root.0).unwrap();
    let mut path: Vec<u16> = root.0.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        let mut descriptor = std::ptr::null_mut();
        assert_eq!(
            GetNamedSecurityInfoW(
                path.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                &mut descriptor
            ),
            0
        );
        let mut control = 0;
        let mut revision = 0;
        assert_ne!(
            GetSecurityDescriptorControl(descriptor, &mut control, &mut revision),
            0
        );
        LocalFree(descriptor);
        assert_ne!(control & SE_DACL_PROTECTED, 0);
    }
    assert_eq!(fs::read(root.0.join("state")).unwrap(), b"private");
}

#[test]
fn regular_file_is_not_accepted_as_private_state_directory() {
    let root = Scratch::new();
    fs::write(&root.0, b"file").unwrap();
    assert!(private_directory(&root.0).is_err());
    fs::remove_file(&root.0).unwrap();
}

// Mount-point junctions need no Developer Mode or symlink privilege, so this
// regression runs on an ordinary Windows account and CI runner as well.
fn junction(link: &std::path::Path, target: &std::path::Path) {
    use std::os::windows::{ffi::OsStrExt, fs::OpenOptionsExt, io::AsRawHandle};
    use windows_sys::Win32::System::IO::DeviceIoControl;
    fs::create_dir(link).unwrap();
    let substitute: Vec<u16> = format!("\\??\\{}", target.display())
        .encode_utf16()
        .collect();
    let print: Vec<u16> = target.as_os_str().encode_wide().collect();
    let mut buffer = Vec::new();
    buffer.extend_from_slice(&0xA0000003u32.to_le_bytes()); // IO_REPARSE_TAG_MOUNT_POINT
    buffer
        .extend_from_slice(&((8 + 2 * (substitute.len() + print.len() + 2)) as u16).to_le_bytes());
    buffer.extend_from_slice(&0u16.to_le_bytes());
    for value in [
        0,
        (substitute.len() * 2) as u16,
        ((substitute.len() + 1) * 2) as u16,
        (print.len() * 2) as u16,
    ] {
        buffer.extend_from_slice(&value.to_le_bytes());
    }
    for unit in substitute
        .into_iter()
        .chain(Some(0))
        .chain(print)
        .chain(Some(0))
    {
        buffer.extend_from_slice(&unit.to_le_bytes());
    }
    let file = fs::OpenOptions::new()
        .write(true)
        .custom_flags(0x02000000 | 0x00200000)
        .open(link)
        .unwrap();
    let mut returned = 0;
    assert_ne!(
        unsafe {
            DeviceIoControl(
                file.as_raw_handle(),
                0x000900A4,
                buffer.as_ptr().cast(),
                buffer.len() as u32,
                std::ptr::null_mut(),
                0,
                &mut returned,
                std::ptr::null_mut(),
            )
        },
        0,
        "{}",
        std::io::Error::last_os_error()
    );
}

#[test]
fn junction_cannot_redirect_private_state_or_escape_authorized_media_root() {
    use ymlink_core::files::{FileCatalog, FileRequest, FileResponse};
    let root = Scratch::new();
    let outside = Scratch::new();
    fs::create_dir(&root.0).unwrap();
    fs::create_dir(&outside.0).unwrap();
    fs::write(outside.0.join("private.mkv"), b"secret").unwrap();
    let link = root.0.join("escape");
    junction(&link, &outside.0);
    assert!(private_directory(&link).is_err());
    let mut catalog = FileCatalog::new();
    catalog
        .add("media".into(), "Media".into(), &root.0)
        .unwrap();
    assert!(matches!(
        catalog.handle(FileRequest::Stat {
            share_id: "media".into(),
            path: "escape/private.mkv".into()
        }),
        FileResponse::Error { .. }
    ));
    fs::remove_dir(&link).unwrap();
    assert_eq!(fs::read(outside.0.join("private.mkv")).unwrap(), b"secret");
}
