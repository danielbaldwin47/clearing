//! Small Unix differences used by the scanner and filesystem guards.
#[cfg(not(any(target_os = "linux", target_os = "macos")))]
compile_error!("spacemap currently supports Linux and macOS");
use std::path::PathBuf;

#[allow(clippy::unnecessary_cast)] // st_dev is i32 on macOS; u64 only on Linux
pub fn identity(stat: &libc::stat) -> (u64, u64) {
    (stat.st_dev as u64, stat.st_ino as u64)
}

pub fn clear_errno() {
    #[cfg(target_os = "linux")]
    unsafe {
        *libc::__errno_location() = 0
    };
    #[cfg(target_os = "macos")]
    unsafe {
        *libc::__error() = 0
    };
}

#[cfg(target_os = "linux")]
pub const PATH_OPEN: i32 = libc::O_PATH;
#[cfg(target_os = "macos")]
pub const PATH_OPEN: i32 = libc::O_RDONLY;

#[cfg(target_os = "macos")]
pub fn mounts() -> std::io::Result<Vec<PathBuf>> {
    use std::{ffi::CStr, os::unix::ffi::OsStrExt};
    // getfsstat writes into caller-owned storage, unlike getmntinfo's shared buffer.
    let count = unsafe { libc::getfsstat(std::ptr::null_mut(), 0, libc::MNT_NOWAIT) };
    if count < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let mut entries: Vec<libc::statfs> = (0..count as usize + 16)
        .map(|_| unsafe { std::mem::zeroed() })
        .collect();
    let length = (entries.len() * std::mem::size_of::<libc::statfs>()) as i32;
    let found = unsafe { libc::getfsstat(entries.as_mut_ptr(), length, libc::MNT_NOWAIT) };
    if found < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(entries
        .iter()
        .take(found as usize)
        .map(|entry| {
            let bytes = unsafe { CStr::from_ptr(entry.f_mntonname.as_ptr()) }.to_bytes();
            PathBuf::from(std::ffi::OsStr::from_bytes(bytes))
        })
        .collect())
}

#[cfg(target_os = "linux")]
pub fn mounts() -> std::io::Result<Vec<PathBuf>> {
    std::fs::read("/proc/self/mountinfo").map(|bytes| crate::trash::parse_mountinfo(&bytes))
}
