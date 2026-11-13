//! Windows: replace a file's DACL with a single entry granting the current
//! user full access, with inheritance from the parent folder turned off.

use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::ptr;

use windows_sys::Win32::Foundation::{CloseHandle, LocalFree, ERROR_SUCCESS, GENERIC_ALL, HANDLE};
use windows_sys::Win32::Security::Authorization::{
    SetEntriesInAclW, SetNamedSecurityInfoW, EXPLICIT_ACCESS_W, NO_MULTIPLE_TRUSTEE, SET_ACCESS,
    SE_FILE_OBJECT, TRUSTEE_IS_SID, TRUSTEE_IS_USER, TRUSTEE_W,
};
use windows_sys::Win32::Security::{
    GetTokenInformation, TokenUser, ACL, DACL_SECURITY_INFORMATION, NO_INHERITANCE,
    PROTECTED_DACL_SECURITY_INFORMATION, TOKEN_QUERY, TOKEN_USER,
};
use windows_sys::Win32::System::Threading::{GetCurrentProcess, OpenProcessToken};

pub fn restrict_to_current_user(path: &Path) -> io::Result<()> {
    let user = current_user_token_info()?;
    // SAFETY: `user` holds a TOKEN_USER written by GetTokenInformation; its SID
    // pointer stays valid while `user` is alive.
    let sid = unsafe { (*(user.as_ptr() as *const TOKEN_USER)).User.Sid };

    let access = EXPLICIT_ACCESS_W {
        grfAccessPermissions: GENERIC_ALL,
        grfAccessMode: SET_ACCESS,
        grfInheritance: NO_INHERITANCE,
        Trustee: TRUSTEE_W {
            pMultipleTrustee: ptr::null_mut(),
            MultipleTrusteeOperation: NO_MULTIPLE_TRUSTEE,
            TrusteeForm: TRUSTEE_IS_SID,
            TrusteeType: TRUSTEE_IS_USER,
            ptstrName: sid as *mut u16,
        },
    };
    let mut acl: *mut ACL = ptr::null_mut();
    // SAFETY: one valid EXPLICIT_ACCESS_W, no old ACL; `acl` receives a
    // LocalAlloc'd ACL that we free below.
    let status = unsafe { SetEntriesInAclW(1, &access, ptr::null(), &mut acl) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }

    let wide: Vec<u16> = path.as_os_str().encode_wide().chain([0]).collect();
    // SAFETY: `wide` is NUL-terminated and `acl` is a valid ACL.
    let status = unsafe {
        SetNamedSecurityInfoW(
            wide.as_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            acl,
            ptr::null(),
        )
    };
    // SAFETY: `acl` came from SetEntriesInAclW.
    unsafe { LocalFree(acl as _) };
    if status != ERROR_SUCCESS {
        return Err(io::Error::from_raw_os_error(status as i32));
    }
    Ok(())
}

/// TOKEN_USER of the current process, in a u64-aligned buffer.
fn current_user_token_info() -> io::Result<Vec<u64>> {
    let mut token: HANDLE = ptr::null_mut();
    // SAFETY: the pseudo-handle from GetCurrentProcess needs no closing.
    if unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) } == 0 {
        return Err(io::Error::last_os_error());
    }
    let result = (|| {
        let mut len = 0u32;
        // SAFETY: size query with a null buffer; fails with the needed size.
        unsafe { GetTokenInformation(token, TokenUser, ptr::null_mut(), 0, &mut len) };
        let mut buffer = vec![0u64; (len as usize).div_ceil(8)];
        // SAFETY: `buffer` holds at least `len` bytes.
        let ok = unsafe {
            GetTokenInformation(token, TokenUser, buffer.as_mut_ptr().cast(), len, &mut len)
        };
        if ok == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(buffer)
    })();
    // SAFETY: `token` was opened above.
    unsafe { CloseHandle(token) };
    result
}
