//! Platform permissions and durable replacement for private account data.
use crate::profiles::Result;
use std::path::Path;

#[cfg(unix)]
pub fn protect(path: &Path, directory: bool) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(
        path,
        std::fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
    )
    .map_err(|e| format!("Could not protect {}: {e}", path.display()))
}

#[cfg(windows)]
pub fn protect(path: &Path, directory: bool) -> Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr::null_mut;
    use windows_sys::Win32::{
        Foundation::{CloseHandle, LocalFree},
        Security::{
            Authorization::{
                ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW,
                SetNamedSecurityInfoW, SE_FILE_OBJECT,
            },
            GetSecurityDescriptorDacl, GetTokenInformation, TokenUser, TOKEN_QUERY, TOKEN_USER,
        },
        System::Threading::{GetCurrentProcess, OpenProcessToken},
    };
    // Apply a protected DACL granting only this user and SYSTEM access. New child
    // files inherit the directory ACL, so credentials are private at creation.
    unsafe {
        let mut token = null_mut();
        if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
            return Err("Could not inspect Windows account identity".into());
        }
        let mut size = 0;
        GetTokenInformation(token, TokenUser, null_mut(), 0, &mut size);
        let mut bytes = vec![0u64; (size as usize).div_ceil(8)];
        let success =
            GetTokenInformation(token, TokenUser, bytes.as_mut_ptr().cast(), size, &mut size);
        CloseHandle(token);
        if success == 0 {
            return Err("Could not read Windows account identity".into());
        }
        let user = &*(bytes.as_ptr().cast::<TOKEN_USER>());
        let mut sid = null_mut();
        if ConvertSidToStringSidW(user.User.Sid, &mut sid) == 0 {
            return Err("Could not encode Windows account identity".into());
        }
        let mut length = 0;
        while *sid.add(length) != 0 {
            length += 1;
        }
        let identity = String::from_utf16_lossy(std::slice::from_raw_parts(sid, length));
        LocalFree(sid.cast());
        let inherit = if directory { "OICI" } else { "" };
        let sddl: Vec<u16> = format!("D:P(A;{inherit};FA;;;{identity})(A;{inherit};FA;;;SY)")
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
            return Err("Could not prepare private Windows permissions".into());
        }
        let mut present = 0;
        let mut defaulted = 0;
        let mut acl = null_mut();
        if GetSecurityDescriptorDacl(descriptor, &mut present, &mut acl, &mut defaulted) == 0 {
            LocalFree(descriptor);
            return Err("Could not read private Windows permissions".into());
        }
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let result = SetNamedSecurityInfoW(
            path.as_ptr(),
            SE_FILE_OBJECT,
            0x00000004 | 0x80000000,
            null_mut(),
            null_mut(),
            acl,
            null_mut(),
        );
        LocalFree(descriptor);
        if result != 0 {
            return Err(format!(
                "Could not protect Windows account data (error {result})"
            ));
        }
    }
    Ok(())
}

pub fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    std::fs::File::open(path)
        .and_then(|f| f.sync_all())
        .map_err(|e| format!("Could not sync data directory: {e}"))?;
    #[cfg(windows)]
    let _ = path; // Persisted file handles are flushed; Windows does not support Unix directory fsync.
    Ok(())
}

pub fn lock_file(path: &Path) -> Result<std::fs::File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(path)
        .map_err(|_| "Could not open private account lock")?;
    protect(path, false)?;
    Ok(file)
}
