//! The in-session trash collector: items picked across directories for one
//! reviewed batch move to the Trash.
//!
//! Records own what they need (raw path, identities, size) and never refer to
//! the scan tree, so they survive rescans. Identities are captured once, when an
//! item is collected. A rescan can mark a record stale but never refreshes it:
//! whatever now sits at a collected path is not authorized by the old selection.
use crate::{
    scan::{Node, display_path},
    trash::{Guard, Outcome, Report, is_volume_trash_name},
};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Dir,
    Symlink,
    File,
}
impl Kind {
    fn of_node(node: &Node) -> Self {
        if node.is_dir {
            Kind::Dir
        } else if node.is_symlink {
            Kind::Symlink
        } else {
            Kind::File
        }
    }
    pub fn of_mode(mode: libc::mode_t) -> Self {
        match mode & libc::S_IFMT {
            libc::S_IFDIR => Kind::Dir,
            libc::S_IFLNK => Kind::Symlink,
            _ => Kind::File,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Kind::Dir => "folder",
            Kind::Symlink => "symlink",
            Kind::File => "file",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Record {
    /// Raw absolute path, used unchanged for syscalls and for gio.
    pub path: PathBuf,
    /// Device and inode seen by the scan that was on screen when collecting.
    pub identity: (u64, u64),
    pub kind: Kind,
    /// Allocated bytes and file count as scanned; informational only.
    pub bytes: u64,
    pub files: u64,
    pub root: PathBuf,
    /// Identity of the scanned root, then of every directory down to the parent.
    pub chain: Vec<(u64, u64)>,
    /// Set when the latest scan no longer agrees with this record.
    pub stale: Option<String>,
    /// Why the last batch left this item in place.
    pub error: Option<String>,
}
impl Record {
    pub fn problem(&self) -> Option<&str> {
        self.error.as_deref().or(self.stale.as_deref())
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Toggle {
    Added {
        replaced: usize,
    },
    Removed,
    /// A collected folder already includes this item.
    Covered(PathBuf),
    Refused(String),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mark {
    None,
    Collected,
    /// Collected, but stale or failed: needs the user's attention.
    Attention,
    /// Inside a collected folder.
    Covered,
}

pub struct Collector {
    records: Vec<Record>,
    guard: Guard,
}
impl Collector {
    pub fn new(guard: Guard) -> Self {
        Self {
            records: Vec::new(),
            guard,
        }
    }
    pub fn records(&self) -> &[Record] {
        &self.records
    }
    #[cfg(test)]
    pub fn guard(&self) -> &Guard {
        &self.guard
    }
    pub fn len(&self) -> usize {
        self.records.len()
    }
    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
    /// Scanned allocation of all records. Parents replace their descendants on
    /// insertion, so nothing is counted twice.
    pub fn total_bytes(&self) -> u64 {
        self.records.iter().map(|r| r.bytes).sum()
    }
    pub fn attention(&self) -> usize {
        self.records
            .iter()
            .filter(|r| r.problem().is_some())
            .count()
    }
    pub fn remove(&mut self, index: usize) -> Option<Record> {
        (index < self.records.len()).then(|| self.records.remove(index))
    }
    pub fn mark(&self, path: &Path) -> Mark {
        for record in &self.records {
            if record.path == path {
                return if record.problem().is_some() {
                    Mark::Attention
                } else {
                    Mark::Collected
                };
            }
            if path.starts_with(&record.path) {
                return Mark::Covered;
            }
        }
        Mark::None
    }
    /// Add the child `index` of the directory at `route`, or remove it if its
    /// path is already collected (stale and failed records included).
    pub fn toggle(&mut self, root: &Node, route: &[usize], index: usize) -> Toggle {
        let mut chain = vec![root.identity];
        let mut dir = root;
        for &i in route {
            let Some(next) = dir.children.get(i) else {
                return Toggle::Refused("selection is not in the current scan".into());
            };
            chain.push(next.identity);
            dir = next;
        }
        let Some(node) = dir.children.get(index) else {
            return Toggle::Refused("nothing is selected".into());
        };
        if let Some(at) = self.records.iter().position(|r| r.path == node.path) {
            self.records.remove(at);
            return Toggle::Removed;
        }
        if let Some(parent) = self.records.iter().find(|r| node.path.starts_with(&r.path)) {
            return Toggle::Covered(parent.path.clone());
        }
        if !node.path.starts_with(&root.path) || node.path == root.path {
            return Toggle::Refused("only items inside the scanned root can be collected".into());
        }
        if let Some(reason) = self.guard.refusal(&node.path) {
            return Toggle::Refused(reason);
        }
        if let Some(inner) = volume_trash_inside(node) {
            return Toggle::Refused(format!(
                "contains a Trash directory ({})",
                display_path(inner.as_os_str())
            ));
        }
        let before = self.records.len();
        self.records.retain(|r| !r.path.starts_with(&node.path));
        let replaced = before - self.records.len();
        self.records.push(Record {
            path: node.path.clone(),
            identity: node.identity,
            kind: Kind::of_node(node),
            bytes: node.bytes,
            files: node.files,
            root: root.path.clone(),
            chain,
            stale: None,
            error: None,
        });
        Toggle::Added { replaced }
    }
    /// Compare every record with a fresh scan. Only the stale note changes;
    /// recorded identities are never updated.
    pub fn reconcile(&mut self, root: &Node) {
        for record in &mut self.records {
            record.stale = stale_reason(record, root);
        }
    }
    /// Drop what reached the Trash; keep everything else with its status.
    pub fn apply(&mut self, report: &Report) {
        for (path, outcome) in &report.outcomes {
            let Some(at) = self.records.iter().position(|r| r.path == *path) else {
                continue;
            };
            match outcome {
                Outcome::Trashed => {
                    self.records.remove(at);
                }
                Outcome::Refused(why) => self.records[at].error = Some(format!("refused: {why}")),
                Outcome::Failed(why) => self.records[at].error = Some(why.clone()),
                Outcome::Unprocessed => {
                    self.records[at].error = Some("not processed: batch was stopped".into())
                }
            }
        }
    }
}
fn stale_reason(record: &Record, root: &Node) -> Option<String> {
    if root.path != record.root || root.identity != record.chain[0] {
        return Some("stale: the scanned root changed".into());
    }
    let Ok(relative) = record.path.strip_prefix(&root.path) else {
        return Some("stale: outside the scanned root".into());
    };
    let mut node = root;
    let mut seen = vec![root.identity];
    for name in relative.components() {
        let name = name.as_os_str();
        let Some(next) = node
            .children
            .iter()
            .find(|c| c.path.file_name() == Some(name))
        else {
            return Some("stale: not present in the latest scan".into());
        };
        node = next;
        seen.push(node.identity);
    }
    let target = seen.pop();
    if seen != record.chain {
        return Some("stale: a parent folder was replaced since collecting".into());
    }
    if target != Some(record.identity) || Kind::of_node(node) != record.kind {
        return Some("stale: the path now holds a different item".into());
    }
    None
}
/// Iterative so that very deep trees cannot exhaust the stack.
fn volume_trash_inside(node: &Node) -> Option<&Path> {
    let mut pending = vec![node];
    while let Some(dir) = pending.pop() {
        for child in dir.children.iter().filter(|c| c.is_dir) {
            if child.path.file_name().is_some_and(is_volume_trash_name) {
                return Some(&child.path);
            }
            pending.push(child);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    fn fixture(tag: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "tuidisk-collector-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(base.join("root/dir/sub")).unwrap();
        fs::write(base.join("root/dir/sub/deep"), [7u8; 5000]).unwrap();
        fs::write(base.join("root/dir/inner"), [7u8; 5000]).unwrap();
        fs::write(base.join("root/a"), [7u8; 5000]).unwrap();
        fs::canonicalize(base).unwrap()
    }
    fn scan(base: &Path) -> Node {
        crate::scan::scan(&base.join("root"), Arc::new(AtomicU64::new(0))).unwrap()
    }
    fn toggle(collector: &mut Collector, tree: &Node, path: &Path) -> Toggle {
        let mut route = Vec::new();
        let mut node = tree;
        loop {
            let index = node
                .children
                .iter()
                .position(|c| path.starts_with(&c.path))
                .unwrap();
            if node.children[index].path == path {
                return collector.toggle(tree, &route, index);
            }
            route.push(index);
            node = &node.children[index];
        }
    }
    fn collector(base: &Path) -> Collector {
        Collector::new(Guard {
            trash_dirs: vec![base.join("root/home/Trash")],
            mounts: vec![],
        })
    }

    #[test]
    fn a_parent_replaces_descendants_and_covers_later_children() {
        let base = fixture("normalize");
        let root = base.join("root");
        let tree = scan(&base);
        let mut c = collector(&base);
        for path in ["dir/sub/deep", "dir/inner", "a"] {
            assert_eq!(
                toggle(&mut c, &tree, &root.join(path)),
                Toggle::Added { replaced: 0 }
            );
        }
        assert_eq!(
            toggle(&mut c, &tree, &root.join("dir")),
            Toggle::Added { replaced: 2 }
        );
        let paths = c.records().iter().map(|r| &r.path).collect::<Vec<_>>();
        assert_eq!(paths, [&root.join("a"), &root.join("dir")]);
        let expected = tree
            .children
            .iter()
            .filter(|n| n.path == root.join("a") || n.path == root.join("dir"))
            .map(|n| n.bytes)
            .sum::<u64>();
        assert_eq!(c.total_bytes(), expected);
        assert_eq!(
            toggle(&mut c, &tree, &root.join("dir/sub")),
            Toggle::Covered(root.join("dir"))
        );
        assert_eq!(c.len(), 2);
        assert_eq!(c.mark(&root.join("dir/sub/deep")), Mark::Covered);
        assert_eq!(c.mark(&root.join("dir")), Mark::Collected);
        assert_eq!(c.mark(&root.join("directory")), Mark::None);
        // Toggling the parent off frees its children for individual collection.
        assert_eq!(toggle(&mut c, &tree, &root.join("dir")), Toggle::Removed);
        assert!(matches!(
            toggle(&mut c, &tree, &root.join("dir/sub")),
            Toggle::Added { .. }
        ));
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn records_capture_the_ancestor_chain_and_raw_path() {
        let base = fixture("chain");
        let root = base.join("root");
        let tree = scan(&base);
        let mut c = collector(&base);
        toggle(&mut c, &tree, &root.join("dir/sub/deep"));
        let record = &c.records()[0];
        let dir = tree.children.iter().find(|n| n.name == "dir").unwrap();
        let sub = dir.children.iter().find(|n| n.name == "sub").unwrap();
        assert_eq!(record.chain, [tree.identity, dir.identity, sub.identity]);
        assert_eq!(record.identity, sub.children[0].identity);
        assert_eq!(record.kind, Kind::File);
        assert_eq!(record.root, root);
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn a_rescan_never_refreshes_the_collected_identity() {
        let base = fixture("rescan");
        let root = base.join("root");
        let mut c = collector(&base);
        let tree = scan(&base);
        toggle(&mut c, &tree, &root.join("a"));
        toggle(&mut c, &tree, &root.join("dir/inner"));
        let original = c.records()[0].identity;
        // An unchanged rescan leaves records clean.
        c.reconcile(&scan(&base));
        assert_eq!(c.attention(), 0);
        // Replace "a" with a different inode, then rescan.
        fs::write(root.join("a.new"), b"impostor").unwrap();
        fs::rename(root.join("a.new"), root.join("a")).unwrap();
        let fresh = scan(&base);
        c.reconcile(&fresh);
        let record = &c.records()[0];
        assert_eq!(record.identity, original);
        assert!(record.stale.is_some());
        assert_eq!(c.mark(&root.join("a")), Mark::Attention);
        assert!(c.records()[1].stale.is_none());
        // The live check refuses the impostor too.
        assert!(crate::trash::preflight(record, c.guard()).is_err());
        // Only an explicit remove and recollect authorizes the new item.
        assert_eq!(toggle(&mut c, &fresh, &root.join("a")), Toggle::Removed);
        assert!(matches!(
            toggle(&mut c, &fresh, &root.join("a")),
            Toggle::Added { .. }
        ));
        let recollected = c.records().iter().find(|r| r.path == root.join("a"));
        assert_ne!(recollected.unwrap().identity, original);
        assert_eq!(
            crate::trash::preflight(recollected.unwrap(), c.guard()),
            Ok(())
        );
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn vanished_items_and_replaced_parents_become_stale() {
        let base = fixture("vanish");
        let root = base.join("root");
        let mut c = collector(&base);
        let tree = scan(&base);
        toggle(&mut c, &tree, &root.join("a"));
        toggle(&mut c, &tree, &root.join("dir/sub/deep"));
        fs::remove_file(root.join("a")).unwrap();
        fs::rename(root.join("dir/sub"), root.join("dir/old")).unwrap();
        fs::create_dir(root.join("dir/sub")).unwrap();
        fs::hard_link(root.join("dir/old/deep"), root.join("dir/sub/deep")).unwrap();
        c.reconcile(&scan(&base));
        // Same inode at the same path, but through a different parent folder.
        assert!(c.records().iter().all(|r| r.stale.is_some()));
        assert_eq!(c.len(), 2);
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn trash_locations_cannot_be_collected() {
        let base = fixture("protect");
        let root = base.join("root");
        fs::create_dir_all(root.join("home/Trash/files")).unwrap();
        fs::write(root.join("home/Trash/files/old"), b"x").unwrap();
        fs::create_dir_all(root.join("volume/nested/.Trash-1000")).unwrap();
        let tree = scan(&base);
        let mut c = collector(&base);
        for refused in [
            "home/Trash",
            "home/Trash/files/old",
            "home",
            "volume",
            "volume/nested/.Trash-1000",
        ] {
            let result = toggle(&mut c, &tree, &root.join(refused));
            assert!(
                matches!(result, Toggle::Refused(_)),
                "{refused}: {result:?}"
            );
        }
        assert!(c.is_empty());
        assert!(matches!(
            c.toggle(&tree, &[], usize::MAX),
            Toggle::Refused(_)
        ));
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn a_report_removes_successes_and_annotates_the_rest() {
        let base = fixture("apply");
        let root = base.join("root");
        let tree = scan(&base);
        let mut c = collector(&base);
        for path in ["a", "dir/inner", "dir/sub"] {
            toggle(&mut c, &tree, &root.join(path));
        }
        let report = Report {
            outcomes: vec![
                (root.join("a"), Outcome::Trashed),
                (
                    root.join("dir/inner"),
                    Outcome::Failed("no trash here".into()),
                ),
                (root.join("dir/sub"), Outcome::Unprocessed),
            ],
            attempted: 2,
            cancelled: true,
        };
        c.apply(&report);
        assert_eq!(c.len(), 2);
        assert_eq!(c.attention(), 2);
        assert!(c.records()[0].problem().unwrap().contains("no trash here"));
        assert!(c.records()[1].error.is_some());
        let summary = report.summary();
        assert!(!summary.to_lowercase().contains("freed up"));
        assert!(!summary.to_lowercase().contains("reclaimed"));
        fs::remove_dir_all(base).unwrap();
    }
}
