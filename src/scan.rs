//! The scanner: a descriptor-relative parallel walk into a `Node` tree of allocated sizes, hard links counted once, and in-place subtree removal with ancestor totals adjusted.
use rayon::prelude::*;
use serde::Serialize;
use std::{
    collections::HashSet,
    ffi::{CStr, CString, OsString},
    fs, io,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
        unix::{
            ffi::{OsStrExt, OsStringExt},
            fs::MetadataExt,
        },
    },
    path::{Path, PathBuf},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};

pub fn display_path(name: &std::ffi::OsStr) -> String {
    if let Some(text) = name.to_str()
        && !text.chars().any(|c| c.is_control() || c == '\\')
    {
        return text.to_owned();
    }
    let mut out = String::with_capacity(name.len());
    let mut remaining = name.as_bytes();
    while !remaining.is_empty() {
        match std::str::from_utf8(remaining) {
            Ok(text) => {
                append_display(&mut out, text);
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                append_display(&mut out, std::str::from_utf8(&remaining[..valid]).unwrap());
                let length = error.error_len().unwrap_or(remaining.len() - valid);
                for byte in &remaining[valid..valid + length] {
                    out.push_str(&format!("\\x{byte:02X}"));
                }
                remaining = &remaining[valid + length..];
            }
        }
    }
    out
}
fn append_display(out: &mut String, text: &str) {
    for c in text.chars() {
        if c == '\\' {
            out.push_str("\\\\")
        } else if c.is_control() {
            out.extend(c.escape_default())
        } else {
            out.push(c)
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Node {
    pub name: String,
    #[serde(skip)]
    pub path: PathBuf,
    pub bytes: u64,
    pub apparent: u64,
    pub files: u64,
    pub directories: u64,
    pub errors: u64,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub shared: bool,
    #[serde(skip)]
    pub identity: (u64, u64),
    pub children: Vec<Node>,
}
impl Node {
    pub fn at<'a>(&'a self, route: &[usize]) -> &'a Node {
        route.iter().fold(self, |n, &i| &n.children[i])
    }

    /// Forget a successfully removed subtree. Shared allocations require a
    /// rescan, including when the removed link was the one counted by the scan.
    /// A refusal leaves the tree intact.
    pub fn remove(&mut self, route: &[usize], index: usize) -> bool {
        let Some(target) = route
            .iter()
            .try_fold(&*self, |n, &i| n.children.get(i))
            .and_then(|n| n.children.get(index))
        else {
            return false;
        };
        fn identities(node: &Node, into: &mut HashSet<(u64, u64)>) {
            into.insert(node.identity);
            for child in &node.children {
                identities(child, into);
            }
        }
        fn shares(node: &Node, removed: &HashSet<(u64, u64)>) -> bool {
            (node.shared && removed.contains(&node.identity))
                || node.children.iter().any(|child| shares(child, removed))
        }
        let mut removed = HashSet::new();
        identities(target, &mut removed);
        if shares(self, &removed) {
            return false;
        }
        self.remove_unshared(route, index).is_some()
    }

    fn remove_unshared(&mut self, route: &[usize], index: usize) -> Option<Node> {
        // Unlinking can change a directory's own size (notably on tmpfs).
        // Read only the ancestor metadata, before changing any of the tree.
        let meta = fs::symlink_metadata(&self.path).ok()?;
        if !meta.is_dir() || (meta.dev(), meta.ino()) != self.identity {
            return None;
        }
        let removed = if let Some((&next, rest)) = route.split_first() {
            self.children.get_mut(next)?.remove_unshared(rest, index)?
        } else {
            self.children.remove(index)
        };
        self.bytes = meta.blocks() * 512 + self.children.iter().map(|n| n.bytes).sum::<u64>();
        self.apparent = meta.len() + self.children.iter().map(|n| n.apparent).sum::<u64>();
        self.files = self.files.saturating_sub(removed.files);
        self.directories = self.directories.saturating_sub(removed.directories);
        self.errors = self.errors.saturating_sub(removed.errors);
        Some(removed)
    }
}
#[derive(Clone)]
struct Context {
    links: Arc<Mutex<HashSet<(u64, u64)>>>,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
}
pub fn scan(path: &Path, progress: Arc<AtomicU64>) -> io::Result<Node> {
    scan_cancellable(path, progress, Arc::new(AtomicBool::new(false)))
}
pub fn scan_cancellable(
    path: &Path,
    progress: Arc<AtomicU64>,
    cancel: Arc<AtomicBool>,
) -> io::Result<Node> {
    let started = std::time::Instant::now();
    let (absolute, fd, stat) = (|| {
        let absolute = fs::canonicalize(path)?;
        let name = CString::new(absolute.as_os_str().as_bytes())
            .map_err(|_| io::Error::other("invalid path"))?;
        let raw = unsafe {
            libc::open(
                name.as_ptr(),
                libc::O_RDONLY
                    | libc::O_DIRECTORY
                    | libc::O_NOFOLLOW
                    | libc::O_CLOEXEC
                    | libc::O_NONBLOCK,
            )
        };
        if raw < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        let stat = stat_fd(fd.as_raw_fd())?;
        Ok((absolute, fd, stat))
    })()
    .map_err(|error| {
        let path = display_path(path.as_os_str());
        let message = if error.kind() == io::ErrorKind::NotADirectory {
            format!("not a directory: {path}")
        } else {
            format!("{path}: {error}")
        };
        io::Error::new(error.kind(), message)
    })?;
    let ctx = Context {
        links: Arc::new(Mutex::new(HashSet::new())),
        progress,
        cancel,
    };
    ctx.progress.store(1, Ordering::Relaxed);
    let mut root = node(absolute, stat, &ctx);
    if root.is_dir {
        match names(fd.as_raw_fd()) {
            Ok(entries) => {
                let prelude = started.elapsed();
                let workers = pool(workers(fd.as_raw_fd(), prelude, entries.len()))?;
                workers.install(|| populate_entries(&mut root, fd, entries, &ctx))
            }
            Err(_) => root.errors += 1,
        }
    }
    if ctx.cancel.load(Ordering::Relaxed) {
        return Err(io::Error::new(io::ErrorKind::Interrupted, "scan cancelled"));
    }
    Ok(root)
}
fn stat_fd(fd: RawFd) -> io::Result<libc::stat> {
    let mut s = std::mem::MaybeUninit::uninit();
    if unsafe { libc::fstat(fd, s.as_mut_ptr()) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { s.assume_init() })
}
fn stat_at(fd: RawFd, name: &CStr) -> io::Result<libc::stat> {
    let mut s = std::mem::MaybeUninit::uninit();
    if unsafe { libc::fstatat(fd, name.as_ptr(), s.as_mut_ptr(), libc::AT_SYMLINK_NOFOLLOW) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { s.assume_init() })
}
fn node(path: PathBuf, meta: libc::stat, ctx: &Context) -> Node {
    let is_dir = meta.st_mode & libc::S_IFMT == libc::S_IFDIR;
    let is_symlink = meta.st_mode & libc::S_IFMT == libc::S_IFLNK;
    let identity = crate::platform::identity(&meta);
    let shared = !is_dir && meta.st_nlink > 1 && !ctx.links.lock().unwrap().insert(identity);
    Node {
        name: display_path(
            path.file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("/")),
        ),
        path,
        bytes: if shared {
            0
        } else {
            meta.st_blocks.max(0) as u64 * 512
        },
        apparent: if shared {
            0
        } else {
            meta.st_size.max(0) as u64
        },
        files: (!is_dir) as u64,
        directories: is_dir as u64,
        errors: 0,
        is_dir,
        is_symlink,
        shared,
        identity,
        children: Vec::new(),
    }
}
struct Entry {
    name: CString,
    is_dir: bool,
    inode: u64,
}
fn names(fd: RawFd) -> io::Result<Vec<Entry>> {
    let dup = unsafe { libc::fcntl(fd, libc::F_DUPFD_CLOEXEC, 3) };
    if dup < 0 {
        return Err(io::Error::last_os_error());
    }
    let dir = unsafe { libc::fdopendir(dup) };
    if dir.is_null() {
        unsafe { libc::close(dup) };
        return Err(io::Error::last_os_error());
    }
    let mut names = Vec::new();
    let mut error = None;
    loop {
        crate::platform::clear_errno();
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            let e = io::Error::last_os_error();
            if e.raw_os_error() != Some(0) {
                error = Some(e)
            }
            break;
        }
        let n = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        if n.to_bytes() != b"." && n.to_bytes() != b".." {
            names.push(Entry {
                name: n.to_owned(),
                is_dir: unsafe { (*entry).d_type } == libc::DT_DIR,
                inode: unsafe { (*entry).d_ino } as u64,
            })
        }
    }
    unsafe { libc::closedir(dir) };
    if let Some(e) = error {
        Err(e)
    } else {
        Ok(names)
    }
}
// Metadata scans block on storage when caches are cold, so a worker waiting on a
// disk read leaves a core idle and the device queue shallow. When resolving and
// listing the root was slow enough to have waited for storage, the scanner uses
// more workers than cores. With warm caches the work is CPU-bound and extra
// workers only add lock contention, so it stays at one worker per core. A small
// filesystem cannot hold enough entries to repay starting many threads.
fn workers(fd: RawFd, prelude: std::time::Duration, listed: usize) -> usize {
    if let Some(n) = std::env::var("CLEARING_SCAN_THREADS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
    {
        return n.clamp(1, 256);
    }
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    let cached = std::time::Duration::from_micros(200)
        + std::time::Duration::from_nanos(250) * listed as u32;
    let target = if prelude > cached {
        (cores * 2).clamp(8, 64)
    } else {
        cores
    };
    let mut fs = std::mem::MaybeUninit::<libc::statfs>::uninit();
    if unsafe { libc::fstatfs(fd, fs.as_mut_ptr()) } == 0 {
        let fs = unsafe { fs.assume_init() };
        // Filesystems without a fixed inode table report zero and stay unbounded.
        if fs.f_files > 0 {
            let used = fs.f_files.saturating_sub(fs.f_ffree) as usize;
            return target.min((used / 64).max(8));
        }
    }
    target
}
fn pool(threads: usize) -> io::Result<rayon::ThreadPool> {
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .thread_name(|i| format!("scan-{i}"))
        .build()
        .map_err(io::Error::other)
}
// Non-directories in one directory usually share a few inode-table blocks, so one
// worker reads them in inode order. Only very large directories are split.
const FILE_CHUNK: usize = 512;
fn open_dir(fd: RawFd, name: &CStr) -> Option<OwnedFd> {
    let raw = unsafe {
        libc::openat(
            fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    (raw >= 0).then(|| unsafe { OwnedFd::from_raw_fd(raw) })
}
fn child_path(parent: &Path, name: &CStr) -> PathBuf {
    let mut bytes = Vec::with_capacity(parent.as_os_str().len() + 1 + name.to_bytes().len());
    bytes.extend_from_slice(parent.as_os_str().as_bytes());
    let mut path = PathBuf::from(OsString::from_vec(bytes));
    path.push(std::ffi::OsStr::from_bytes(name.to_bytes()));
    path
}
fn visit(parent: &Path, fd: RawFd, entry: &Entry, ctx: &Context) -> Option<Node> {
    if ctx.cancel.load(Ordering::Relaxed) {
        return None;
    }
    let name = &entry.name;
    let path = child_path(parent, name);
    // A known directory can be opened safely first. fstat then describes
    // exactly the inode whose children we will read, avoiding two stats.
    if entry.is_dir
        && let Some(owned) = open_dir(fd, name)
    {
        let stat = stat_fd(owned.as_raw_fd()).ok()?;
        let mut child = node(path, stat, ctx);
        populate(&mut child, owned, ctx);
        return Some(child);
    }
    let stat = stat_at(fd, name).ok()?;
    let mut child = node(path, stat, ctx);
    if child.is_dir {
        match open_dir(fd, name) {
            None => child.errors += 1,
            Some(owned) => match stat_fd(owned.as_raw_fd()) {
                Ok(s) if crate::platform::identity(&s) == child.identity => {
                    populate(&mut child, owned, ctx)
                }
                _ => child.errors += 1,
            },
        }
    }
    Some(child)
}
fn populate(parent: &mut Node, fd: OwnedFd, ctx: &Context) {
    if ctx.cancel.load(Ordering::Relaxed) {
        return;
    }
    match names(fd.as_raw_fd()) {
        Ok(entries) => populate_entries(parent, fd, entries, ctx),
        Err(_) => parent.errors += 1,
    }
}
fn populate_entries(parent: &mut Node, fd: OwnedFd, entries: Vec<Entry>, ctx: &Context) {
    // Progress is approximate while scanning; the final tree contains exact counts.
    // One update per directory avoids a contended atomic for every file.
    ctx.progress
        .fetch_add(entries.len() as u64, Ordering::Relaxed);
    let (dirs, mut files): (Vec<Entry>, Vec<Entry>) = entries.into_iter().partition(|e| e.is_dir);
    files.sort_unstable_by_key(|e| e.inode);
    let total = dirs.len() + files.len();
    let raw = fd.as_raw_fd();
    let path = parent.path.as_path();
    let mut dir_slots: Vec<Option<Node>> = Vec::new();
    dir_slots.resize_with(dirs.len(), || None);
    let mut children: Vec<Node> = Vec::with_capacity(total);
    // Subdirectories become stealable tasks before this worker touches any file,
    // so their disk reads start while the files here are being read in order.
    rayon::scope(|s| {
        for (entry, slot) in dirs.iter().zip(dir_slots.iter_mut()) {
            s.spawn(move |_| *slot = visit(path, raw, entry, ctx));
        }
        if files.len() <= FILE_CHUNK {
            // A d_type of DT_UNKNOWN may still turn out to be a directory; visit handles it.
            children.extend(files.iter().filter_map(|e| visit(path, raw, e, ctx)));
        } else {
            let chunks: Vec<Vec<Node>> = files
                .par_chunks(FILE_CHUNK)
                .map(|chunk| {
                    chunk
                        .iter()
                        .filter_map(|e| visit(path, raw, e, ctx))
                        .collect()
                })
                .collect();
            children.extend(chunks.into_iter().flatten());
        }
    });
    children.extend(dir_slots.into_iter().flatten());
    parent.errors += (total - children.len()) as u64;
    for c in &children {
        parent.bytes += c.bytes;
        parent.apparent += c.apparent;
        parent.files += c.files;
        parent.directories += c.directories;
        parent.errors += c.errors;
    }
    children.sort_unstable_by(|a, b| b.bytes.cmp(&a.bytes).then(a.name.cmp(&b.name)));
    parent.children = children;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Seek, SeekFrom, Write},
        os::unix::fs::symlink,
        time::{SystemTime, UNIX_EPOCH},
    };
    // Tests run in parallel and a macOS clock ticks in microseconds, so the counter keeps two names apart.
    static FIXTURES: AtomicU64 = AtomicU64::new(0);
    fn fixture() -> PathBuf {
        let p = std::env::temp_dir().join(format!(
            "clearing-test-{}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            FIXTURES.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&p).unwrap();
        fs::canonicalize(p).unwrap()
    }
    #[test]
    fn removal_matches_fresh_scan_at_every_ancestor() {
        let p = fixture();
        fs::create_dir_all(p.join("a/b/drop/deep")).unwrap();
        fs::write(p.join("a/b/drop/deep/data"), [1; 8192]).unwrap();
        fs::write(p.join("a/b/keep"), [2; 4096]).unwrap();
        let mut tree = scan(&p, Arc::new(AtomicU64::new(0))).unwrap();
        let a = tree.children.iter().position(|n| n.name == "a").unwrap();
        let b = tree
            .at(&[a])
            .children
            .iter()
            .position(|n| n.name == "b")
            .unwrap();
        let index = tree
            .at(&[a, b])
            .children
            .iter()
            .position(|n| n.name == "drop")
            .unwrap();
        // Model an error inside the removed subtree, also counted by ancestors.
        tree.errors = 1;
        tree.children[a].errors = 1;
        tree.children[a].children[b].errors = 1;
        tree.children[a].children[b].children[index].errors = 1;
        fs::remove_dir_all(p.join("a/b/drop")).unwrap();
        assert!(tree.remove(&[a, b], index));
        let fresh = scan(&p, Arc::new(AtomicU64::new(0))).unwrap();
        fn compare(actual: &Node, fresh: &Node) {
            assert_eq!(
                (
                    actual.bytes,
                    actual.apparent,
                    actual.files,
                    actual.directories,
                    actual.errors
                ),
                (
                    fresh.bytes,
                    fresh.apparent,
                    fresh.files,
                    fresh.directories,
                    fresh.errors
                ),
                "{}",
                actual.path.display()
            );
            assert_eq!(actual.children.len(), fresh.children.len());
            for child in &actual.children {
                compare(
                    child,
                    fresh
                        .children
                        .iter()
                        .find(|n| n.path == child.path)
                        .unwrap(),
                );
            }
        }
        compare(&tree, &fresh);
        // The last file leaves an empty directory, whose own size still counts.
        fs::remove_file(p.join("a/b/keep")).unwrap();
        assert!(tree.remove(&[a, b], 0));
        compare(&tree, &scan(&p, Arc::new(AtomicU64::new(0))).unwrap());
        fs::remove_dir_all(p).unwrap();
    }

    #[test]
    fn removal_refuses_shared_subtrees_and_the_counted_link_without_mutation() {
        let p = fixture();
        fs::create_dir(p.join("links")).unwrap();
        fs::write(p.join("links/data"), [1; 8192]).unwrap();
        fs::hard_link(p.join("links/data"), p.join("links/other")).unwrap();
        let tree = scan(&p, Arc::new(AtomicU64::new(0))).unwrap();
        for (route, index) in [(vec![], 0), (vec![0], 0), (vec![0], 1)] {
            let mut updated = tree.clone();
            assert!(!updated.remove(&route, index));
            assert_eq!(
                serde_json::to_value(&updated).unwrap(),
                serde_json::to_value(&tree).unwrap()
            );
        }
        fs::remove_dir_all(p).unwrap();
    }

    #[test]
    fn display_path_escapes_invalid_bytes_controls_and_backslashes() {
        let plain = std::ffi::OsStr::from_bytes("plain \u{2603}".as_bytes());
        assert_eq!(display_path(plain), "plain \u{2603}");
        let raw = std::ffi::OsStr::from_bytes(b"bad\xFFname\n");
        assert_eq!(display_path(raw), "bad\\xFFname\\n");
        let mixed = std::ffi::OsStr::from_bytes(b"a\\b\tc\xC3(\xE2\x98\x83\x80");
        assert_eq!(display_path(mixed), "a\\\\b\\tc\\xC3(\u{2603}\\x80");
    }
    #[test]
    fn counts_sparse_hardlinks_and_does_not_follow_symlinks() {
        let p = fixture();
        let mut f = fs::File::create(p.join("data")).unwrap();
        f.write_all(&[42u8; 8192]).unwrap();
        fs::hard_link(p.join("data"), p.join("linked")).unwrap();
        let mut sparse = fs::File::create(p.join("sparse")).unwrap();
        sparse.seek(SeekFrom::Start(100 * 1024 * 1024)).unwrap();
        sparse.write_all(&[1]).unwrap();
        symlink("/", p.join("outside")).unwrap();
        let n = scan(&p, Arc::new(AtomicU64::new(0))).unwrap();
        assert_eq!(n.files, 4);
        assert_eq!(n.directories, 1);
        assert!(n.bytes < 1024 * 1024);
        assert!(n.apparent > 100 * 1024 * 1024);
        assert_eq!(n.children.iter().filter(|n| n.shared).count(), 1);
        assert!(
            n.children
                .iter()
                .find(|n| n.name == "outside")
                .unwrap()
                .children
                .is_empty()
        );
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn large_directories_nested_trees_and_raw_names_are_complete() {
        let p = fixture();
        let wide = p.join("wide");
        fs::create_dir(&wide).unwrap();
        for i in 0..(FILE_CHUNK * 2 + 37) {
            fs::write(wide.join(format!("f{i}")), b"x").unwrap();
        }
        let deep = p.join("a/b/c/d");
        fs::create_dir_all(&deep).unwrap();
        fs::write(deep.join("leaf"), b"leaf").unwrap();
        symlink(&wide, deep.join("loop")).unwrap();
        // APFS rejects names that are not valid UTF-8, so macOS keeps only the
        // control character and a non-ASCII scalar with no decomposition.
        #[cfg(not(target_os = "macos"))]
        let (raw, escaped) = (
            std::ffi::OsStr::from_bytes(b"bad\xFFname\n"),
            "bad\\xFFname\\n",
        );
        #[cfg(target_os = "macos")]
        let (raw, escaped) = (
            std::ffi::OsStr::from_bytes("bad\u{2603}name\n".as_bytes()),
            "bad\u{2603}name\\n",
        );
        fs::write(p.join(raw), b"raw").unwrap();
        let n = scan(&p, Arc::new(AtomicU64::new(0))).unwrap();
        assert_eq!(n.files, FILE_CHUNK as u64 * 2 + 37 + 3);
        assert_eq!(n.directories, 6);
        assert_eq!(n.errors, 0);
        fn apparent(n: &Node) -> u64 {
            fs::symlink_metadata(&n.path).unwrap().len()
                + n.children.iter().map(apparent).sum::<u64>()
        }
        assert_eq!(n.apparent, apparent(&n));
        let named = n.children.iter().find(|c| c.name == escaped).unwrap();
        assert_eq!(
            named.path.as_os_str().as_bytes(),
            p.join(raw).as_os_str().as_bytes()
        );
        let wide_node = n.children.iter().find(|c| c.name == "wide").unwrap();
        assert_eq!(wide_node.children.len(), FILE_CHUNK * 2 + 37);
        let link = n.at(&[]).children.iter().find(|c| c.name == "a").unwrap();
        let link = &link.children[0].children[0].children[0];
        let link = link.children.iter().find(|c| c.name == "loop").unwrap();
        assert!(link.is_symlink && !link.is_dir && link.children.is_empty());
        fs::remove_dir_all(p).unwrap();
    }
    #[test]
    fn unreadable_directories_are_errors_and_cancel_interrupts() {
        let p = fixture();
        let locked = p.join("locked");
        fs::create_dir(&locked).unwrap();
        fs::write(locked.join("hidden"), b"x").unwrap();
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let n = scan(&p, Arc::new(AtomicU64::new(0))).unwrap();
        if unsafe { libc::geteuid() } != 0 {
            assert_eq!(n.errors, 1);
            assert_eq!(n.directories, 2);
        }
        let cancelled = scan_cancellable(
            &p,
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicBool::new(true)),
        );
        assert_eq!(cancelled.unwrap_err().kind(), io::ErrorKind::Interrupted);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_dir_all(p).unwrap();
    }
}
