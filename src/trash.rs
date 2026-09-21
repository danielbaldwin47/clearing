//! Moving collected items to the desktop Trash through the platform service.
//!
//! Every item is re-verified against the identities recorded when it was
//! collected immediately before the Trash service runs. That check and the external move are
//! separate steps, so a concurrent writer can still swap a path in between: this
//! is strong best-effort validation, not an atomic guarantee. No failure path in
//! this module deletes anything.
use crate::collector::{Kind, Record};
#[cfg(any(target_os = "linux", test))]
use std::process::{Command, Stdio};
#[cfg(any(target_os = "linux", test))]
use std::{ffi::OsString, os::unix::ffi::OsStringExt};
use std::{
    ffi::{CString, OsStr},
    fs, io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::ffi::OsStrExt,
    },
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
};

/// Locations that must never be submitted to the Trash.
#[derive(Debug, Clone, Default)]
pub struct Guard {
    /// The home Trash directory, as configured and as resolved.
    pub trash_dirs: Vec<PathBuf>,
    /// Mount points; each may carry a `.Trash` or `.Trash-uid` directory.
    pub mounts: Vec<PathBuf>,
}
impl Guard {
    pub fn from_system() -> Self {
        let absolute = |key: &str| {
            std::env::var_os(key)
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
        };
        #[cfg(target_os = "linux")]
        let home_trash = absolute("XDG_DATA_HOME")
            .map(|p| p.join("Trash"))
            .or_else(|| absolute("HOME").map(|p| p.join(".local/share/Trash")));
        #[cfg(target_os = "macos")]
        let home_trash = absolute("HOME").map(|p| p.join(".Trash"));
        let mut trash_dirs = Vec::new();
        if let Some(configured) = home_trash {
            if let Ok(resolved) = fs::canonicalize(&configured)
                && resolved != configured
            {
                trash_dirs.push(resolved)
            }
            trash_dirs.push(configured)
        }
        let mounts = crate::platform::mounts().unwrap_or_default();
        Self { trash_dirs, mounts }
    }
    /// Why `path` may not be trashed, judged from its location alone.
    pub fn refusal(&self, path: &Path) -> Option<String> {
        if path
            .components()
            .any(|c| matches!(c, Component::Normal(name) if is_volume_trash_name(name)))
        {
            return Some("is, or is inside, a volume Trash directory".into());
        }
        for trash in &self.trash_dirs {
            if path.starts_with(trash) {
                return Some("is, or is inside, the Trash itself".into());
            }
            if trash.starts_with(path) {
                return Some("contains the Trash directory".into());
            }
        }
        for mount in &self.mounts {
            if mount.starts_with(path) {
                return Some(format!(
                    "is or contains the mount point {} (its volume Trash would be included)",
                    crate::scan::display_path(mount.as_os_str())
                ));
            }
        }
        None
    }
}
/// `.Trash` and `.Trash-<uid>` are the per-volume Trash names of the
/// freedesktop.org Trash specification.
pub fn is_volume_trash_name(name: &OsStr) -> bool {
    let bytes = name.as_bytes();
    if bytes == b".Trash" || bytes == b".Trashes" {
        return true;
    }
    bytes
        .strip_prefix(b".Trash-")
        .is_some_and(|uid| !uid.is_empty() && uid.iter().all(u8::is_ascii_digit))
}
#[cfg(any(target_os = "linux", test))]
pub(crate) fn parse_mountinfo(bytes: &[u8]) -> Vec<PathBuf> {
    bytes
        .split(|b| *b == b'\n')
        .filter_map(|line| line.split(|b| *b == b' ').nth(4))
        .map(|field| PathBuf::from(OsString::from_vec(unescape_octal(field))))
        .filter(|p| p.is_absolute())
        .collect()
}
#[cfg(any(target_os = "linux", test))]
fn unescape_octal(field: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(field.len());
    let mut i = 0;
    while i < field.len() {
        let digits = field.get(i + 1..i + 4);
        match digits {
            Some(d) if field[i] == b'\\' && d.iter().all(|c| (b'0'..=b'7').contains(c)) => {
                let value = d.iter().fold(0u32, |n, c| n * 8 + (c - b'0') as u32);
                out.push(value as u8);
                i += 4
            }
            _ => {
                out.push(field[i]);
                i += 1
            }
        }
    }
    out
}

fn cstring(name: &OsStr) -> Result<CString, String> {
    CString::new(name.as_bytes()).map_err(|_| "path contains a NUL byte".to_string())
}
fn open_path(dir: Option<RawFd>, name: &OsStr) -> Result<OwnedFd, io::Error> {
    let name = CString::new(name.as_bytes()).map_err(|_| io::Error::other("NUL in path"))?;
    let flags = crate::platform::PATH_OPEN | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC;
    let raw = unsafe {
        match dir {
            Some(fd) => libc::openat(fd, name.as_ptr(), flags),
            None => libc::open(name.as_ptr(), flags),
        }
    };
    if raw < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { OwnedFd::from_raw_fd(raw) })
}
/// Open the canonical scan-root path without following a symlink in any
/// component, including ancestors above the root that were not in the scan.
fn open_root(path: &Path) -> io::Result<OwnedFd> {
    if !path.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "root is not absolute",
        ));
    }
    let mut dir = open_path(None, OsStr::new("/"))?;
    for component in path.components() {
        match component {
            Component::RootDir => {}
            Component::Normal(name) => dir = open_path(Some(dir.as_raw_fd()), name)?,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "root contains non-normal components",
                ));
            }
        }
    }
    Ok(dir)
}
fn identity_of(fd: RawFd) -> io::Result<(u64, u64)> {
    let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe { libc::fstat(fd, s.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    let s = unsafe { s.assume_init() };
    Ok(crate::platform::identity(&s))
}
fn not_a_directory(e: &io::Error) -> bool {
    matches!(e.raw_os_error(), Some(libc::ENOTDIR) | Some(libc::ELOOP))
}

/// Check that `record` still names exactly what was scanned: the scan root,
/// every ancestor directory and the target keep their recorded device+inode,
/// no ancestor is a symlink, and the target is not a protected location.
///
/// This narrows, but cannot close, the window before the Trash service acts on the path.
pub fn preflight(record: &Record, guard: &Guard) -> Result<(), String> {
    let path = &record.path;
    if !path.is_absolute() {
        return Err("path is not absolute".into());
    }
    if *path == record.root {
        return Err("the scanned root itself cannot be trashed".into());
    }
    let relative = path
        .strip_prefix(&record.root)
        .map_err(|_| "path is outside the scanned root".to_string())?;
    let mut names = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(name) => names.push(name),
            _ => return Err("path is not a plain descendant of the scanned root".into()),
        }
    }
    if names.is_empty() || record.chain.len() != names.len() {
        return Err("recorded ancestors do not match the path".into());
    }
    if let Some(reason) = guard.refusal(path) {
        return Err(reason);
    }
    let mut dir =
        open_root(&record.root).map_err(|e| format!("scanned root cannot be opened: {e}"))?;
    if identity_of(dir.as_raw_fd()).ok() != Some(record.chain[0]) {
        return Err("the scanned root was replaced since the scan".into());
    }
    let (target, ancestors) = names.split_last().unwrap();
    for (name, expected) in ancestors.iter().zip(&record.chain[1..]) {
        let shown = crate::scan::display_path(name);
        let next = open_path(Some(dir.as_raw_fd()), name).map_err(|e| {
            if not_a_directory(&e) {
                format!("ancestor {shown} is now a symlink or not a directory")
            } else {
                format!("ancestor {shown} cannot be opened: {e}")
            }
        })?;
        if identity_of(next.as_raw_fd()).ok() != Some(*expected) {
            return Err(format!(
                "ancestor {shown} was replaced since it was collected"
            ));
        }
        dir = next;
    }
    let name = cstring(target)?;
    let mut s = std::mem::MaybeUninit::<libc::stat>::uninit();
    if unsafe {
        libc::fstatat(
            dir.as_raw_fd(),
            name.as_ptr(),
            s.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } < 0
    {
        return Err(format!("item is gone: {}", io::Error::last_os_error()));
    }
    let s = unsafe { s.assume_init() };
    if crate::platform::identity(&s) != record.identity {
        return Err("path now names a different item than the one collected".into());
    }
    if Kind::of_mode(s.st_mode) != record.kind {
        return Err("item changed type since it was collected".into());
    }
    if record.kind == Kind::Dir {
        let uid = unsafe { libc::geteuid() };
        for trash in [
            ".Trash".to_string(),
            ".Trashes".to_string(),
            format!(".Trash-{uid}"),
        ] {
            if fs::symlink_metadata(path.join(&trash)).is_ok() {
                return Err(format!("contains a volume Trash directory ({trash})"));
            }
        }
    }
    Ok(())
}

/// The seam between batch logic and the real desktop Trash.
pub trait Executor: Sync {
    fn trash(&self, path: &Path) -> Result<(), String>;
}
/// Exactly `gio trash -- ABSOLUTE_PATH`, built from raw arguments with no shell.
#[cfg(any(target_os = "linux", test))]
pub fn gio_command(path: &Path) -> Result<Command, String> {
    if !path.is_absolute() {
        return Err("refusing to trash a relative path".into());
    }
    let mut command = Command::new("gio");
    command.arg("trash").arg("--").arg(path.as_os_str());
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    Ok(command)
}
pub struct DesktopTrash;
#[cfg(target_os = "linux")]
impl Executor for DesktopTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        let output = gio_command(path)?.output().map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                "gio is not installed; item left in place".to_string()
            } else {
                format!("gio could not be started: {e}; item left in place")
            }
        })?;
        if output.status.success() {
            return Ok(());
        }
        let stderr = String::from_utf8_lossy(&output.stderr);
        let detail = stderr.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
        Err(match output.status.code() {
            Some(code) => format!("gio trash failed ({code}): {}", detail.trim()),
            None => format!("gio trash was killed by a signal: {}", detail.trim()),
        })
    }
}

#[cfg(target_os = "macos")]
impl Executor for DesktopTrash {
    fn trash(&self, path: &Path) -> Result<(), String> {
        use native_trash::macos::{DeleteMethod, TrashContextExtMacos};
        if !path.is_absolute() {
            return Err("refusing to trash a relative path".into());
        }
        // The native backend percent-encodes invalid UTF-8. Refuse those names
        // so it can never address a different, literal percent-encoded file.
        if path.to_str().is_none() {
            return Err("macOS Trash requires a UTF-8 path; item left in place".into());
        }
        let mut context = native_trash::TrashContext::new();
        context.set_delete_method(DeleteMethod::NsFileManager);
        context
            .delete(path)
            .map_err(|error| format!("macOS Trash failed: {error}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// `gio` succeeded and the collected inode no longer sits at the path.
    Trashed,
    /// Preflight refused; `gio` was never run for this item.
    Refused(String),
    /// `gio` ran and failed, or reported success without moving the item.
    Failed(String),
    /// The batch was cancelled before this item was reached.
    Unprocessed,
}
#[derive(Debug)]
pub struct Report {
    /// One outcome per submitted record, in order.
    pub outcomes: Vec<(PathBuf, Outcome)>,
    /// How many times the executor ran; any run may have changed the disk.
    pub attempted: usize,
    pub cancelled: bool,
}
impl Report {
    pub fn count(&self, matches: impl Fn(&Outcome) -> bool) -> usize {
        self.outcomes.iter().filter(|(_, o)| matches(o)).count()
    }
    /// Footer text. It reports moves, never reclaimed space.
    pub fn summary(&self) -> String {
        let moved = self.count(|o| *o == Outcome::Trashed);
        let failed = self.count(|o| matches!(o, Outcome::Failed(_) | Outcome::Refused(_)));
        let skipped = self.count(|o| *o == Outcome::Unprocessed);
        let mut parts = vec![format!("Moved to Trash: {moved}")];
        if failed > 0 {
            parts.push(format!("failed: {failed} (review collector)"))
        }
        if skipped > 0 {
            parts.push(format!("not processed after stop: {skipped}"))
        }
        if moved > 0 {
            parts.push("space is freed when Trash is emptied".into())
        }
        parts.join(" · ")
    }
}
/// Trash `records` one at a time. Cancellation is honoured only between items:
/// a running executor is never interrupted, because it may be mid-move.
/// `done` counts finished items for the progress display.
pub fn run_batch(
    records: &[Record],
    guard: &Guard,
    executor: &dyn Executor,
    cancel: &AtomicBool,
    done: &AtomicUsize,
) -> Report {
    let mut report = Report {
        outcomes: Vec::with_capacity(records.len()),
        attempted: 0,
        cancelled: false,
    };
    for record in records {
        if cancel.load(Ordering::SeqCst) {
            report.cancelled = true;
            report
                .outcomes
                .push((record.path.clone(), Outcome::Unprocessed));
            continue;
        }
        let outcome = match preflight(record, guard) {
            Err(reason) => Outcome::Refused(reason),
            Ok(()) => {
                report.attempted += 1;
                match executor.trash(&record.path) {
                    Err(e) => Outcome::Failed(e),
                    Ok(()) => match fs::symlink_metadata(&record.path) {
                        Ok(meta) if identity(&meta) == record.identity => Outcome::Failed(
                            "Trash service reported success but the item is still in place".into(),
                        ),
                        _ => Outcome::Trashed,
                    },
                }
            }
        };
        report.outcomes.push((record.path.clone(), outcome));
        done.fetch_add(1, Ordering::SeqCst);
    }
    report
}
fn identity(meta: &fs::Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (meta.dev(), meta.ino())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::collector::{Collector, Toggle};
    use std::{
        os::unix::fs::symlink,
        sync::{Arc, Mutex, atomic::AtomicU64},
    };

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
    }
    impl Fixture {
        fn new(tag: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let base = std::env::temp_dir().join(format!(
                "clearing-trash-{tag}-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::SeqCst)
            ));
            let root = base.join("root");
            fs::create_dir_all(root.join("dir/sub")).unwrap();
            fs::write(root.join("dir/sub/deep"), b"deep").unwrap();
            fs::write(root.join("dir/inner"), b"inner").unwrap();
            fs::write(root.join("a"), b"a").unwrap();
            fs::write(root.join("b"), b"b").unwrap();
            fs::write(root.join("c"), b"c").unwrap();
            let base = fs::canonicalize(base).unwrap();
            let root = base.join("root");
            Self { base, root }
        }
        fn guard(&self) -> Guard {
            Guard {
                trash_dirs: vec![self.base.join("home/Trash")],
                mounts: vec![],
            }
        }
        fn scan(&self) -> crate::scan::Node {
            crate::scan::scan(&self.root, Arc::new(AtomicU64::new(0))).unwrap()
        }
        /// Collect root-relative paths and return the resulting records.
        fn collect(&self, paths: &[&str]) -> Vec<Record> {
            let tree = self.scan();
            let mut collector = Collector::new(self.guard());
            for path in paths {
                let (route, index) = locate(&tree, &self.root.join(path));
                assert!(matches!(
                    collector.toggle(&tree, &route, index),
                    Toggle::Added { .. }
                ));
            }
            collector.records().to_vec()
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
    pub(crate) fn locate(tree: &crate::scan::Node, path: &Path) -> (Vec<usize>, usize) {
        let mut route = Vec::new();
        let mut node = tree;
        loop {
            let index = node
                .children
                .iter()
                .position(|c| path.starts_with(&c.path))
                .expect("path is in the scanned tree");
            if node.children[index].path == path {
                return (route, index);
            }
            route.push(index);
            node = &node.children[index];
        }
    }
    /// Stands in for gio: moves items into a private directory, never the Trash.
    struct FakeTrash {
        into: PathBuf,
        fail: Vec<PathBuf>,
        cancel_after_first: Option<Arc<AtomicBool>>,
        calls: Mutex<Vec<PathBuf>>,
    }
    impl FakeTrash {
        fn new(f: &Fixture) -> Self {
            let into = f.base.join("fake-trash");
            fs::create_dir_all(&into).unwrap();
            Self {
                into,
                fail: vec![],
                cancel_after_first: None,
                calls: Mutex::new(vec![]),
            }
        }
    }
    impl Executor for FakeTrash {
        fn trash(&self, path: &Path) -> Result<(), String> {
            let mut calls = self.calls.lock().unwrap();
            calls.push(path.to_path_buf());
            if let Some(cancel) = &self.cancel_after_first {
                cancel.store(true, Ordering::SeqCst)
            }
            if self.fail.iter().any(|p| p == path) {
                return Err("simulated: trashing is not supported here".into());
            }
            fs::rename(path, self.into.join(format!("item-{}", calls.len())))
                .map_err(|e| e.to_string())
        }
    }

    #[test]
    fn preflight_accepts_unchanged_items_and_refuses_a_replaced_target() {
        let f = Fixture::new("stale");
        let records = f.collect(&["a", "dir/sub/deep"]);
        for record in &records {
            assert_eq!(preflight(record, &f.guard()), Ok(()));
        }
        // Same path, new inode: both files exist at once, so the inode differs.
        fs::write(f.root.join("a.new"), b"impostor").unwrap();
        fs::rename(f.root.join("a.new"), f.root.join("a")).unwrap();
        assert!(preflight(&records[0], &f.guard()).is_err());
        assert_eq!(preflight(&records[1], &f.guard()), Ok(()));
        fs::remove_file(f.root.join("dir/sub/deep")).unwrap();
        assert!(preflight(&records[1], &f.guard()).is_err());
    }
    #[test]
    fn preflight_refuses_symlink_above_root_even_when_all_recorded_inodes_match() {
        let f = Fixture::new("root-prefix-link");
        let record = f.collect(&["dir/sub/deep"]).remove(0);
        let moved = f.base.with_extension("moved");
        fs::rename(&f.base, &moved).unwrap();
        symlink(&moved, &f.base).unwrap();
        let target_still_matches =
            identity(&fs::symlink_metadata(&record.path).unwrap()) == record.identity;
        let result = preflight(&record, &f.guard());
        fs::remove_file(&f.base).unwrap();
        fs::rename(&moved, &f.base).unwrap();
        assert!(target_still_matches);
        assert!(result.is_err());
    }
    #[test]
    fn preflight_refuses_a_target_that_changed_type() {
        let f = Fixture::new("type");
        let mut record = f.collect(&["a"]).remove(0);
        assert_eq!(preflight(&record, &f.guard()), Ok(()));
        record.kind = Kind::Dir;
        assert!(preflight(&record, &f.guard()).is_err());
    }
    #[test]
    fn preflight_refuses_symlinked_and_replaced_ancestors() {
        let f = Fixture::new("ancestor");
        let records = f.collect(&["dir/sub/deep"]);
        // The ancestor becomes a symlink to the very same directory: the target
        // inode is unchanged, only the route to it is no longer trustworthy.
        fs::rename(f.root.join("dir"), f.base.join("moved")).unwrap();
        symlink(f.base.join("moved"), f.root.join("dir")).unwrap();
        assert_eq!(
            fs::read(f.root.join("dir/sub/deep")).unwrap(),
            b"deep".to_vec()
        );
        let refusal = preflight(&records[0], &f.guard()).unwrap_err();
        assert!(refusal.contains("dir"), "{refusal}");
        // A real directory with the same layout but a different inode.
        fs::remove_file(f.root.join("dir")).unwrap();
        fs::create_dir_all(f.root.join("dir/sub")).unwrap();
        fs::write(f.root.join("dir/sub/deep"), b"other").unwrap();
        assert!(preflight(&records[0], &f.guard()).is_err());
    }
    #[test]
    fn preflight_refuses_a_replaced_scan_root() {
        let f = Fixture::new("root");
        let records = f.collect(&["a"]);
        fs::rename(&f.root, f.base.join("old-root")).unwrap();
        fs::create_dir(&f.root).unwrap();
        fs::write(f.root.join("a"), b"a").unwrap();
        let refusal = preflight(&records[0], &f.guard()).unwrap_err();
        assert!(refusal.contains("root"), "{refusal}");
        // A symlink back to the original root is refused as well.
        fs::remove_dir_all(&f.root).unwrap();
        symlink(f.base.join("old-root"), &f.root).unwrap();
        assert!(preflight(&records[0], &f.guard()).is_err());
    }
    #[test]
    fn preflight_rejects_root_outside_and_dotdot_paths() {
        let f = Fixture::new("outside");
        let template = f.collect(&["a"]).remove(0);
        fs::write(f.base.join("outside"), b"x").unwrap();
        for path in [
            f.root.clone(),
            f.base.join("outside"),
            f.root.join("dir/../a"),
            f.root.join("../outside"),
            PathBuf::from("a"),
        ] {
            let record = Record {
                path: path.clone(),
                ..template.clone()
            };
            assert!(preflight(&record, &f.guard()).is_err(), "{path:?}");
        }
    }
    #[test]
    fn trash_locations_are_protected() {
        let guard = Guard {
            trash_dirs: vec![PathBuf::from("/home/u/.local/share/Trash")],
            mounts: vec![PathBuf::from("/"), PathBuf::from("/mnt/usb")],
        };
        for refused in [
            "/home/u/.local/share/Trash",
            "/home/u/.local/share/Trash/files/old",
            "/home/u/.local/share",
            "/home/u",
            "/mnt/usb/.Trash-1000",
            "/mnt/usb/.Trash/1000/files/x",
            "/Volumes/External/.Trashes/501/files/x",
            "/Users/test/.Trash/item",
            "/mnt/usb",
            "/mnt",
        ] {
            assert!(guard.refusal(Path::new(refused)).is_some(), "{refused}");
        }
        for allowed in [
            "/home/u/.local/share/Trash-notes",
            "/home/u/.Trashy",
            "/home/u/.Trash-me",
            "/mnt/usb/photos",
            "/home/other",
        ] {
            assert_eq!(guard.refusal(Path::new(allowed)), None, "{allowed}");
        }
    }
    #[test]
    fn a_volume_trash_inside_a_live_directory_is_refused() {
        let f = Fixture::new("volume");
        let records = f.collect(&["dir"]);
        assert_eq!(preflight(&records[0], &f.guard()), Ok(()));
        let uid = unsafe { libc::geteuid() };
        fs::create_dir(f.root.join(format!("dir/.Trash-{uid}"))).unwrap();
        assert!(preflight(&records[0], &f.guard()).is_err());
    }
    #[test]
    fn mountinfo_mount_points_are_unescaped() {
        let info =
            b"36 35 98:0 / /mnt/with\\040space rw - ext3 /dev/x rw\n22 1 0:5 / / rw - tmpfs t rw\n";
        assert_eq!(
            parse_mountinfo(info),
            vec![PathBuf::from("/mnt/with space"), PathBuf::from("/")]
        );
    }
    #[test]
    fn gio_arguments_keep_unusual_names_as_one_raw_argument() {
        for raw in [
            &b"/tmp/-rf"[..],
            b"/tmp/two words; rm -rf $HOME `x`",
            b"/tmp/line\nbreak",
            b"/tmp/bad\xFFutf8",
            b"/tmp/--",
        ] {
            let path = Path::new(OsStr::from_bytes(raw));
            let command = gio_command(path).unwrap();
            assert_eq!(command.get_program(), "gio");
            let args = command.get_args().collect::<Vec<_>>();
            assert_eq!(args.len(), 3);
            assert_eq!(args[0], "trash");
            assert_eq!(args[1], "--");
            assert_eq!(args[2].as_bytes(), raw);
        }
        assert!(gio_command(Path::new("relative/-x")).is_err());
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn native_trash_refuses_paths_it_cannot_address_exactly() {
        assert!(DesktopTrash.trash(Path::new("relative")).is_err());
        let invalid = Path::new(OsStr::from_bytes(b"/tmp/non-utf8-\xff"));
        assert!(DesktopTrash.trash(invalid).unwrap_err().contains("UTF-8"));
    }
    #[test]
    fn batch_keeps_failures_and_never_runs_the_executor_for_refused_items() {
        let f = Fixture::new("partial");
        let records = f.collect(&["a", "b", "c", "dir"]);
        // "c" is swapped after collection, so preflight must stop it.
        fs::write(f.root.join("c.new"), b"impostor").unwrap();
        fs::rename(f.root.join("c.new"), f.root.join("c")).unwrap();
        let mut fake = FakeTrash::new(&f);
        fake.fail = vec![f.root.join("b")];
        let done = AtomicUsize::new(0);
        let report = run_batch(&records, &f.guard(), &fake, &AtomicBool::new(false), &done);
        let outcome = |name: &str| {
            let path = f.root.join(name);
            report
                .outcomes
                .iter()
                .find(|(p, _)| *p == path)
                .unwrap()
                .1
                .clone()
        };
        assert_eq!(outcome("a"), Outcome::Trashed);
        assert_eq!(outcome("dir"), Outcome::Trashed);
        assert!(matches!(outcome("b"), Outcome::Failed(_)));
        assert!(matches!(outcome("c"), Outcome::Refused(_)));
        assert_eq!(report.attempted, 3);
        assert_eq!(done.load(Ordering::SeqCst), 4);
        assert!(!report.cancelled);
        assert!(!fake.calls.lock().unwrap().contains(&f.root.join("c")));
        // Failure never turns into deletion.
        assert_eq!(fs::read(f.root.join("b")).unwrap(), b"b".to_vec());
        assert_eq!(fs::read(f.root.join("c")).unwrap(), b"impostor".to_vec());
        assert!(!f.root.join("a").exists() && !f.root.join("dir").exists());
    }
    #[test]
    fn cancellation_lets_the_running_item_finish_and_keeps_the_rest() {
        let f = Fixture::new("cancel");
        let records = f.collect(&["a", "b", "c"]);
        let cancel = Arc::new(AtomicBool::new(false));
        let mut fake = FakeTrash::new(&f);
        fake.cancel_after_first = Some(cancel.clone());
        let done = AtomicUsize::new(0);
        let report = run_batch(&records, &f.guard(), &fake, &cancel, &done);
        assert!(report.cancelled);
        assert_eq!(report.attempted, 1);
        assert_eq!(fake.calls.lock().unwrap().len(), 1);
        assert_eq!(report.outcomes[0].1, Outcome::Trashed);
        assert_eq!(report.count(|o| *o == Outcome::Unprocessed), 2);
        let survivors = records[1..].iter().filter(|r| r.path.exists()).count();
        assert_eq!(survivors, 2);
    }
    #[test]
    fn success_without_a_move_is_reported_as_failure() {
        struct Liar;
        impl Executor for Liar {
            fn trash(&self, _: &Path) -> Result<(), String> {
                Ok(())
            }
        }
        let f = Fixture::new("liar");
        let records = f.collect(&["a"]);
        let report = run_batch(
            &records,
            &f.guard(),
            &Liar,
            &AtomicBool::new(false),
            &AtomicUsize::new(0),
        );
        assert!(matches!(report.outcomes[0].1, Outcome::Failed(_)));
    }
}
