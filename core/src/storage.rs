//! Private per-user state directory. Never applied to user-authorized media roots.
use std::{fs, io, path::Path};

/// Creates a per-user directory, inspects it and restricts its permissions.
///
/// # Errors
///
/// Returns the underlying [`io::Error`] when the directory cannot be created,
/// inspected or locked down; a non-directory or symlink found during inspection
/// yields [`io::ErrorKind::PermissionDenied`].
pub fn private_directory(path: &Path) -> io::Result<()> {
    fs::create_dir_all(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "invalid state directory",
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(windows)]
    windows::secure(path)?;
    Ok(())
}

#[cfg(windows)]
mod windows {
    use super::*;
    use std::os::windows::{
        fs::{MetadataExt, OpenOptionsExt},
        io::AsRawHandle,
    };
    use std::ptr::null_mut;
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, HANDLE};
    use windows_sys::Win32::Security::Authorization::{
        ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
        SetSecurityInfo, SE_FILE_OBJECT,
    };
    use windows_sys::Win32::Security::{
        GetSecurityDescriptorDacl, GetTokenInformation, TokenUser, DACL_SECURITY_INFORMATION,
        PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
    };
    use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

    struct Local(*mut std::ffi::c_void);
    impl Drop for Local {
        fn drop(&mut self) {
            unsafe {
                LocalFree(self.0);
            }
        }
    }
    struct Token(HANDLE);
    impl Drop for Token {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }

    pub(super) fn secure(path: &Path) -> io::Result<()> {
        // Open the directory itself, never a junction target, and keep this handle
        // through the ACL change so a path replacement cannot redirect it.
        let directory = fs::OpenOptions::new()
            .read(true)
            .access_mode(0x0002_0000 | 0x0004_0000 | 0x80) // READ_CONTROL | WRITE_DAC | FILE_READ_ATTRIBUTES
            .custom_flags(0x0200_0000 | 0x0020_0000) // BACKUP_SEMANTICS | OPEN_REPARSE_POINT
            .open(path)?;
        let metadata = directory.metadata()?;
        if !metadata.is_dir() || metadata.file_attributes() & 0x400 != 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "state directory is a reparse point",
            ));
        }
        // SAFETY: all Windows outputs have aligned owned backing storage. Each
        // LocalAlloc/token resource is released once by its owner on every path.
        unsafe {
            let mut token = null_mut();
            if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
                return Err(io::Error::last_os_error());
            }
            let token = Token(token);
            let mut needed = 0;
            GetTokenInformation(token.0, TokenUser, null_mut(), 0, &mut needed);
            if needed == 0 || needed > 1024 * 1024 {
                return Err(io::Error::last_os_error());
            }
            let mut buffer = vec![0usize; (needed as usize).div_ceil(std::mem::size_of::<usize>())];
            if GetTokenInformation(
                token.0,
                TokenUser,
                buffer.as_mut_ptr().cast(),
                needed,
                &mut needed,
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let user = &*buffer.as_ptr().cast::<TOKEN_USER>();
            let mut sid = null_mut();
            if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
                return Err(io::Error::last_os_error());
            }
            let _sid = Local(sid.cast());
            let mut length = 0;
            while *sid.add(length) != 0 {
                length += 1;
            }
            let sid = String::from_utf16(std::slice::from_raw_parts(sid, length))
                .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid account SID"))?;
            let sddl: Vec<u16> = format!("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;{sid})")
                .encode_utf16()
                .chain(Some(0))
                .collect();
            let mut descriptor = null_mut();
            if ConvertStringSecurityDescriptorToSecurityDescriptorW(
                sddl.as_ptr(),
                1,
                &mut descriptor,
                null_mut(),
            ) == 0
            {
                return Err(io::Error::last_os_error());
            }
            let _descriptor = Local(descriptor);
            let mut present = 0;
            let mut defaulted = 0;
            let mut acl = null_mut();
            if GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted) == 0
                || present == 0
                || acl.is_null()
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "invalid private ACL",
                ));
            }
            let result = SetSecurityInfo(
                directory.as_raw_handle(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
                null_mut(),
                null_mut(),
                acl,
                null_mut(),
            );
            if result != 0 {
                return Err(io::Error::from_raw_os_error(result as i32));
            }
        }
        Ok(())
    }
}
