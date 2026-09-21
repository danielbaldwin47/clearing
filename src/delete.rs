//! Descriptor-relative permanent deletion.
//!
//! Every traversed directory is opened without following symlinks, and every removed entry must retain its scanned identity.
use crate::scan::Node;
use std::{
    ffi::{CString, OsStr},
    io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::ffi::OsStrExt,
    },
    path::Path,
    sync::atomic::{AtomicBool, AtomicU64, Ordering},
};

fn cstr(s: &OsStr) -> io::Result<CString> {
    CString::new(s.as_bytes())
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "filename contains a NUL byte"))
}
fn opened(fd: i32) -> io::Result<OwnedFd> {
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { OwnedFd::from_raw_fd(fd) })
    }
}
fn open_root(path: &Path) -> io::Result<OwnedFd> {
    let p = cstr(path.as_os_str())?;
    opened(unsafe {
        libc::open(
            p.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    })
}
fn open_dir(parent: RawFd, name: &OsStr) -> io::Result<OwnedFd> {
    let n = cstr(name)?;
    opened(unsafe {
        libc::openat(
            parent,
            n.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    })
}
fn identity_fd(fd: RawFd) -> io::Result<(u64, u64)> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, stat.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let s = unsafe { stat.assume_init() };
    Ok(crate::platform::identity(&s))
}
fn identity_at(parent: RawFd, name: &OsStr) -> io::Result<(u64, u64)> {
    let name = cstr(name)?;
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            parent,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        return Err(io::Error::last_os_error());
    }
    let s = unsafe { stat.assume_init() };
    Ok(crate::platform::identity(&s))
}
fn changed() -> io::Error {
    io::Error::other("entry changed since the scan; rescan before deleting")
}
fn verify(fd: RawFd, node: &Node) -> io::Result<()> {
    if identity_fd(fd)? != node.identity {
        Err(changed())
    } else {
        Ok(())
    }
}
fn basename(node: &Node) -> io::Result<&OsStr> {
    node.path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to delete a filesystem root",
        )
    })
}

/// How a deletion ended. `removed` on [`Outcome`] is exact in every case.
#[derive(Debug)]
pub enum Status {
    /// The selected entry and everything scanned beneath it was removed.
    Completed,
    /// Cancellation was observed before a removal; remaining entries are intact.
    Cancelled,
    /// A safety check or system call failed; nothing further was removed.
    Failed(io::Error),
}
#[derive(Debug)]
pub struct Outcome {
    /// Directory entries actually unlinked, including removed directories.
    pub removed: u64,
    pub status: Status,
}
enum Stop {
    Cancelled,
    Failed(io::Error),
}
impl From<io::Error> for Stop {
    fn from(e: io::Error) -> Self {
        Stop::Failed(e)
    }
}
struct Control<'a> {
    cancel: &'a AtomicBool,
    removed: &'a AtomicU64,
    /// Runs after every successful removal with the running total.
    after: &'a mut dyn FnMut(u64),
}
impl Control<'_> {
    fn check(&self) -> Result<(), Stop> {
        if self.cancel.load(Ordering::SeqCst) {
            Err(Stop::Cancelled)
        } else {
            Ok(())
        }
    }
}

/// Remove only a selected child, never the scan root or the current view.
#[cfg(test)]
pub fn remove(root: &Node, route: &[usize], selected: usize) -> io::Result<()> {
    let (cancel, removed) = (AtomicBool::new(false), AtomicU64::new(0));
    match remove_cancellable(root, route, selected, &cancel, &removed).status {
        Status::Completed => Ok(()),
        Status::Cancelled => Err(io::Error::other("cancelled")),
        Status::Failed(e) => Err(e),
    }
}
/// Like the plain removal, but `cancel` is honoured before every removal and
/// `removed` counts each entry as soon as it has been unlinked.
pub fn remove_cancellable(
    root: &Node,
    route: &[usize],
    selected: usize,
    cancel: &AtomicBool,
    removed: &AtomicU64,
) -> Outcome {
    remove_hooked(root, route, selected, cancel, removed, &mut |_| {})
}
fn remove_hooked(
    root: &Node,
    route: &[usize],
    selected: usize,
    cancel: &AtomicBool,
    removed: &AtomicU64,
    after: &mut dyn FnMut(u64),
) -> Outcome {
    let mut control = Control {
        cancel,
        removed,
        after,
    };
    let status = match remove_selected(root, route, selected, &mut control) {
        Ok(()) => Status::Completed,
        Err(Stop::Cancelled) => Status::Cancelled,
        Err(Stop::Failed(e)) => Status::Failed(e),
    };
    Outcome {
        removed: removed.load(Ordering::SeqCst),
        status,
    }
}
fn remove_selected(
    root: &Node,
    route: &[usize],
    selected: usize,
    control: &mut Control,
) -> Result<(), Stop> {
    control.check()?;
    if root.path == Path::new("/") && route.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing deletion directly from the filesystem root",
        )
        .into());
    }
    let mut parent = open_root(&root.path)?;
    verify(parent.as_raw_fd(), root)?;
    let mut current = root;
    for &index in route {
        control.check()?;
        let child = current.children.get(index).ok_or_else(changed)?;
        if !child.is_dir {
            return Err(changed().into());
        }
        let next = open_dir(parent.as_raw_fd(), basename(child)?)?;
        verify(next.as_raw_fd(), child)?;
        parent = next;
        current = child;
    }
    let node = current.children.get(selected).ok_or_else(changed)?;
    remove_node(parent.as_raw_fd(), node, current.identity.0, control)
}
fn remove_node(parent: RawFd, node: &Node, device: u64, control: &mut Control) -> Result<(), Stop> {
    control.check()?;
    let name = basename(node)?;
    if identity_at(parent, name)? != node.identity {
        return Err(changed().into());
    }
    if node.identity.0 != device {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "refusing to cross a filesystem boundary during deletion",
        )
        .into());
    }
    if node.is_dir {
        if node.errors > 0 {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "directory was not completely scanned; rescan with access before deleting",
            )
            .into());
        }
        let fd = open_dir(parent, name)?;
        verify(fd.as_raw_fd(), node)?;
        for child in &node.children {
            control.check()?;
            remove_node(fd.as_raw_fd(), child, device, control)?;
        }
        // Keep the opened inode pinned and verify its name still refers to it.
        if identity_at(parent, name)? != node.identity {
            return Err(changed().into());
        }
    }
    let name = cstr(name)?;
    let flags = if node.is_dir { libc::AT_REMOVEDIR } else { 0 };
    // Last look at the flag: once cancellation is visible nothing else is unlinked.
    control.check()?;
    if unsafe { libc::unlinkat(parent, name.as_ptr(), flags) } < 0 {
        return Err(io::Error::last_os_error().into());
    }
    let total = control.removed.fetch_add(1, Ordering::SeqCst) + 1;
    (control.after)(total);
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan;
    use std::{
        fs,
        os::unix::fs::symlink,
        sync::Arc,
        time::{SystemTime, UNIX_EPOCH},
    };
    fn dir() -> std::path::PathBuf {
        let p = std::env::temp_dir().join(format!(
            "clearing-delete-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&p).unwrap();
        p
    }
    fn scan(p: &Path) -> Node {
        scan::scan(p, Arc::new(AtomicU64::new(0))).unwrap()
    }
    #[test]
    fn removes_scanned_tree_but_not_symlink_targets() {
        let p = dir();
        let outside = dir();
        fs::write(outside.join("keep"), "untouched").unwrap();
        fs::create_dir(p.join("remove")).unwrap();
        fs::write(p.join("remove/file"), "delete").unwrap();
        symlink(&outside, p.join("remove/link")).unwrap();
        let root = scan(&p);
        remove(&root, &[], 0).unwrap();
        assert!(!p.join("remove").exists());
        assert!(outside.join("keep").exists());
        fs::remove_dir_all(p).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
    #[test]
    fn refuses_replaced_selected_inode() {
        let p = dir();
        fs::write(p.join("victim"), "old").unwrap();
        let root = scan(&p);
        fs::rename(p.join("victim"), p.join("old")).unwrap();
        fs::write(p.join("victim"), "new").unwrap();
        assert!(remove(&root, &[], 0).is_err());
        assert_eq!(fs::read_to_string(p.join("victim")).unwrap(), "new");
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn refuses_parent_swapped_to_symlink() {
        let p = dir();
        let outside = dir();
        fs::create_dir(p.join("parent")).unwrap();
        fs::write(p.join("parent/victim"), "inside").unwrap();
        fs::write(outside.join("victim"), "outside").unwrap();
        let root = scan(&p);
        fs::rename(p.join("parent"), p.join("original")).unwrap();
        symlink(&outside, p.join("parent")).unwrap();
        assert!(remove(&root, &[0], 0).is_err());
        assert_eq!(
            fs::read_to_string(outside.join("victim")).unwrap(),
            "outside"
        );
        fs::remove_dir_all(p).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
    #[test]
    fn new_files_are_not_deleted() {
        let p = dir();
        fs::create_dir(p.join("victim")).unwrap();
        let root = scan(&p);
        fs::write(p.join("victim/new"), "keep").unwrap();
        assert!(remove(&root, &[], 0).is_err());
        assert!(p.join("victim/new").exists());
        fs::remove_dir_all(p).unwrap();
    }
    /// victim/{a,b,c,sub/{x,y}} scanned, then deleted with cancellation
    /// requested by the hook once `stop_after` entries have been removed.
    fn cancel_after(stop_after: u64) -> (std::path::PathBuf, Outcome, Vec<u64>) {
        let p = dir();
        fs::create_dir_all(p.join("victim/sub")).unwrap();
        for f in ["a", "b", "c", "sub/x", "sub/y"] {
            fs::write(p.join("victim").join(f), "data").unwrap();
        }
        let root = scan(&p);
        let (cancel, removed) = (AtomicBool::new(stop_after == 0), AtomicU64::new(0));
        let mut seen = vec![];
        let outcome = remove_hooked(&root, &[], 0, &cancel, &removed, &mut |total| {
            seen.push(total);
            if total == stop_after {
                cancel.store(true, Ordering::SeqCst)
            }
        });
        (p, outcome, seen)
    }
    fn remaining(p: &Path) -> u64 {
        fs::read_dir(p)
            .unwrap()
            .map(|e| {
                let e = e.unwrap();
                1 + if e.file_type().unwrap().is_dir() {
                    remaining(&e.path())
                } else {
                    0
                }
            })
            .sum()
    }
    #[test]
    fn cancel_before_start_leaves_everything() {
        let (p, outcome, seen) = cancel_after(0);
        assert!(matches!(outcome.status, Status::Cancelled));
        assert_eq!(outcome.removed, 0);
        assert!(seen.is_empty());
        assert_eq!(remaining(&p), 7);
        for f in ["a", "b", "c", "sub/x", "sub/y"] {
            assert_eq!(
                fs::read_to_string(p.join("victim").join(f)).unwrap(),
                "data"
            );
        }
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn cancel_midway_is_partial_with_exact_count() {
        for stop_after in 1..=6 {
            let (p, outcome, seen) = cancel_after(stop_after);
            assert!(matches!(outcome.status, Status::Cancelled), "{stop_after}");
            assert_eq!(outcome.removed, stop_after);
            assert_eq!(seen, (1..=stop_after).collect::<Vec<_>>());
            // Exactly the counted entries are gone; the selected directory survives.
            assert_eq!(remaining(&p), 7 - stop_after);
            assert!(p.join("victim").is_dir());
            fs::remove_dir_all(p).unwrap();
        }
    }
    #[test]
    fn cancel_after_final_removal_is_completed() {
        let (p, outcome, seen) = cancel_after(7);
        assert!(matches!(outcome.status, Status::Completed));
        assert_eq!(outcome.removed, 7);
        assert_eq!(seen.len(), 7);
        assert_eq!(remaining(&p), 0);
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn uncancelled_run_counts_every_entry() {
        let (p, outcome, _) = cancel_after(u64::MAX);
        assert!(matches!(outcome.status, Status::Completed));
        assert_eq!(outcome.removed, 7);
        assert!(!p.join("victim").exists());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn cancel_in_nested_route_keeps_later_siblings() {
        let p = dir();
        fs::create_dir_all(p.join("outer/victim")).unwrap();
        for f in ["one", "two", "three"] {
            fs::write(p.join("outer/victim").join(f), "data").unwrap();
        }
        let root = scan(&p);
        let order: Vec<_> = root.children[0].children[0]
            .children
            .iter()
            .map(|n| n.path.clone())
            .collect();
        let (cancel, removed) = (AtomicBool::new(false), AtomicU64::new(0));
        let outcome = remove_hooked(&root, &[0], 0, &cancel, &removed, &mut |_| {
            cancel.store(true, Ordering::SeqCst)
        });
        assert!(matches!(outcome.status, Status::Cancelled));
        assert_eq!(outcome.removed, 1);
        assert!(!order[0].exists());
        assert!(order[1].exists() && order[2].exists());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn failure_midway_stops_and_reports_count() {
        let p = dir();
        fs::create_dir(p.join("victim")).unwrap();
        for f in ["a", "b", "c"] {
            fs::write(p.join("victim").join(f), "old").unwrap();
        }
        let root = scan(&p);
        let order: Vec<_> = root.children[0]
            .children
            .iter()
            .map(|n| n.path.clone())
            .collect();
        // Replace the second entry's inode after the first removal.
        let (cancel, removed) = (AtomicBool::new(false), AtomicU64::new(0));
        let outcome = remove_hooked(&root, &[], 0, &cancel, &removed, &mut |total| {
            if total == 1 {
                // Renaming keeps the scanned inode alive so its number cannot be reused.
                fs::rename(&order[1], p.join("moved")).unwrap();
                fs::write(&order[1], "new").unwrap();
            }
        });
        if let Status::Failed(_) = outcome.status {
            assert_eq!(outcome.removed, 1);
            assert!(order[1].exists() && order[2].exists());
        } else {
            panic!("replaced entry must stop deletion: {outcome:?}");
        }
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn symlink_swap_during_deletion_is_not_followed() {
        let p = dir();
        let outside = dir();
        fs::write(outside.join("x"), "outside").unwrap();
        for d in ["victim/one", "victim/two"] {
            fs::create_dir_all(p.join(d)).unwrap();
            fs::write(p.join(d).join("x"), "inside").unwrap();
        }
        let root = scan(&p);
        // Whichever directory is visited second becomes a symlink mid-deletion.
        let second = root.children[0].children[1].path.clone();
        let (cancel, removed) = (AtomicBool::new(false), AtomicU64::new(0));
        let outcome = remove_hooked(&root, &[], 0, &cancel, &removed, &mut |total| {
            if total == 1 {
                fs::rename(&second, p.join("moved")).unwrap();
                symlink(&outside, &second).unwrap();
            }
        });
        assert!(matches!(outcome.status, Status::Failed(_)));
        // The first directory's file and the directory itself, nothing more.
        assert_eq!(outcome.removed, 2);
        assert_eq!(fs::read_to_string(p.join("moved/x")).unwrap(), "inside");
        assert_eq!(fs::read_to_string(outside.join("x")).unwrap(), "outside");
        assert!(second.is_symlink());
        fs::remove_dir_all(p).unwrap();
        fs::remove_dir_all(outside).unwrap();
    }
    #[test]
    fn refuses_entry_with_mismatched_device() {
        let p = dir();
        fs::create_dir(p.join("victim")).unwrap();
        fs::write(p.join("victim/file"), "keep").unwrap();
        let mut root = scan(&p);
        // A device number that no longer matches must stop before any removal.
        root.children[0].children[0].identity.0 ^= 1;
        let (cancel, removed) = (AtomicBool::new(false), AtomicU64::new(0));
        let outcome = remove_cancellable(&root, &[], 0, &cancel, &removed);
        assert!(matches!(outcome.status, Status::Failed(_)));
        assert_eq!(outcome.removed, 0);
        assert!(p.join("victim/file").exists());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn refuses_incompletely_scanned_directory() {
        let p = dir();
        fs::create_dir(p.join("victim")).unwrap();
        fs::write(p.join("victim/file"), "keep").unwrap();
        let mut root = scan(&p);
        root.children[0].errors = 1;
        let (cancel, removed) = (AtomicBool::new(false), AtomicU64::new(0));
        let outcome = remove_cancellable(&root, &[], 0, &cancel, &removed);
        assert!(matches!(outcome.status, Status::Failed(_)));
        assert_eq!(outcome.removed, 0);
        assert!(p.join("victim/file").exists());
        fs::remove_dir_all(p).unwrap();
    }
}
