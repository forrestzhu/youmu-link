//! Rust-owned desktop application state and C ABI for the native macOS frontend.

pub mod capture;
mod model;
mod runtime;
pub mod wire;

pub use runtime::Backend;
use serde_json::json;
use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;

fn owned_string(value: String) -> *mut c_char {
    const FALLBACK: &str = "{\"ok\":false,\"error\":\"invalid output\"}";
    CString::new(value)
        .unwrap_or_else(|_| CString::new(FALLBACK).expect("fallback text has no NUL byte"))
        .into_raw()
}

/// Caller owns the returned backend and must destroy it exactly once, after all
/// concurrent calls have completed. Input strings must be valid NUL-terminated C strings.
///
/// # Safety
///
/// `state_dir` and `device_name` must be null or valid NUL-terminated C string
/// pointers; the returned pointer must only be released via [`ymlink_destroy`].
#[no_mangle]
pub unsafe extern "C" fn ymlink_create(
    state_dir: *const c_char,
    device_name: *const c_char,
) -> *mut c_void {
    catch_unwind(AssertUnwindSafe(|| {
        if state_dir.is_null() || device_name.is_null() {
            return std::ptr::null_mut();
        }
        let Ok(path) = CStr::from_ptr(state_dir).to_str() else {
            return std::ptr::null_mut();
        };
        let Ok(name) = CStr::from_ptr(device_name).to_str() else {
            return std::ptr::null_mut();
        };
        if path.len() > 4096 || !std::path::Path::new(path).is_absolute() {
            return std::ptr::null_mut();
        }
        match Backend::start(PathBuf::from(path), name.to_string()) {
            Ok(backend) => Box::into_raw(Box::new(backend)).cast(),
            Err(_) => std::ptr::null_mut(),
        }
    }))
    .unwrap_or(std::ptr::null_mut())
}

/// # Safety
///
/// `handle` must be null or a backend pointer from [`ymlink_create`]; the
/// caller owns the returned string and must free it with [`ymlink_string_free`].
#[no_mangle]
pub unsafe extern "C" fn ymlink_snapshot(handle: *mut c_void) -> *mut c_char {
    owned_string(
        catch_unwind(AssertUnwindSafe(|| {
            if handle.is_null() {
                return json!({"status":"error","error":"backend unavailable"}).to_string();
            }
            (*handle.cast::<Backend>()).snapshot()
        }))
        .unwrap_or_else(|_| json!({"status":"error","error":"backend failure"}).to_string()),
    )
}

/// # Safety
///
/// `handle` must be null or a backend pointer from [`ymlink_create`]; `input`
/// must be null or a valid NUL-terminated C string; the caller owns the
/// returned string and must free it with [`ymlink_string_free`].
#[no_mangle]
pub unsafe extern "C" fn ymlink_command(handle: *mut c_void, input: *const c_char) -> *mut c_char {
    owned_string(
        catch_unwind(AssertUnwindSafe(|| {
            let result = if handle.is_null() || input.is_null() {
                Err("backend unavailable".to_string())
            } else {
                CStr::from_ptr(input)
                    .to_str()
                    .map_err(|_| "command must be UTF-8".to_string())
                    .and_then(|input| (*handle.cast::<Backend>()).command(input))
            };
            match result {
                Ok(()) => json!({"ok":true,"error":null}).to_string(),
                Err(error) => json!({"ok":false,"error":error}).to_string(),
            }
        }))
        .unwrap_or_else(|_| json!({"ok":false,"error":"backend failure"}).to_string()),
    )
}

/// # Safety
///
/// `value` must be null or a pointer returned by this library that has not
/// been freed yet; free each pointer exactly once.
#[no_mangle]
pub unsafe extern "C" fn ymlink_string_free(value: *mut c_char) {
    if !value.is_null() {
        drop(CString::from_raw(value));
    }
}

/// # Safety
///
/// `handle` must be null or a backend pointer from [`ymlink_create`] with no
/// concurrent calls in flight; destroy each backend exactly once.
#[no_mangle]
pub unsafe extern "C" fn ymlink_destroy(handle: *mut c_void) {
    if !handle.is_null() {
        let _ = catch_unwind(AssertUnwindSafe(|| {
            drop(Box::from_raw(handle.cast::<Backend>()));
        }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ffi_rejects_null_handles_and_invalid_start_paths() {
        unsafe {
            assert!(ymlink_create(std::ptr::null(), std::ptr::null()).is_null());
            let result = ymlink_command(std::ptr::null_mut(), std::ptr::null());
            let value: serde_json::Value =
                serde_json::from_str(CStr::from_ptr(result).to_str().unwrap()).unwrap();
            assert_eq!(value["ok"], false);
            ymlink_string_free(result);
            ymlink_destroy(std::ptr::null_mut());
        }
    }
}
