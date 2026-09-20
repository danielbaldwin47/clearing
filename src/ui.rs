use crate::{
    collector::{Collector, Toggle},
    scan::{Node, display_path},
    trash::Guard,
};
use crossterm::event::KeyCode;
mod confirm;
mod foundation;
mod review;
mod view;
pub use foundation::size;
pub use view::{draw, draw_wireframe};

pub struct App {
    pub root: Node,
    pub route: Vec<usize>,
    pub selected: usize,
    pub previous: Vec<usize>,
    pub confirm: bool,
    pub typed: String,
    pub message: String,
    pub help: bool,
    /// Items picked for the Trash. It outlives the scan tree: see `rebuild`.
    pub collector: Collector,
    pub review: bool,
    pub review_selected: usize,
    pub trash_confirm: bool,
}
impl App {
    pub fn new(root: Node, seconds: f64) -> Self {
        Self::with_collector(root, seconds, Collector::new(Guard::from_system()))
    }
    pub fn with_collector(root: Node, _seconds: f64, collector: Collector) -> Self {
        Self {
            root,
            route: vec![],
            selected: 0,
            previous: vec![],
            confirm: false,
            typed: String::new(),
            message: String::new(),
            help: false,
            collector,
            review: false,
            review_selected: 0,
            trash_confirm: false,
        }
    }
    /// Replace the scan tree after a rescan, keeping the place in the tree and
    /// the collector. Records are compared with the fresh tree and flagged when
    /// they no longer match; their collected identities stay as they were.
    pub fn rebuild(self, root: Node, seconds: f64) -> Self {
        let current_path = self.current().path.clone();
        let mut collector = self.collector;
        collector.reconcile(&root);
        let mut app = Self::with_collector(root, seconds, collector);
        app.restore_path(&current_path);
        app.review = self.review;
        app.review_selected = self.review_selected;
        app.clamp_review();
        app
    }
    pub fn current(&self) -> &Node {
        self.root.at(&self.route)
    }
    pub fn selection(&self) -> Option<&Node> {
        self.current().children.get(self.selected)
    }
    pub fn drill(&mut self) {
        if self.selection().is_some_and(|n| n.is_dir) {
            self.previous.push(self.selected);
            self.route.push(self.selected);
            self.selected = 0;
            self.message.clear()
        }
    }
    pub fn back(&mut self) {
        if self.route.pop().is_some() {
            self.selected = self.previous.pop().unwrap_or(0);
            self.message.clear()
        }
    }
    pub fn restore_path(&mut self, path: &std::path::Path) {
        while self.current().path != path {
            let Some(index) = self
                .current()
                .children
                .iter()
                .position(|n| n.is_dir && path.starts_with(&n.path))
            else {
                break;
            };
            self.selected = index;
            self.drill();
        }
    }
    pub fn next(&mut self, delta: isize) {
        let len = self.current().children.len();
        if len > 0 {
            self.selected = (self.selected as isize + delta).rem_euclid(len as isize) as usize;
        }
    }
    /// Space while browsing: collect the highlighted entry, or drop it again.
    pub fn toggle_collect(&mut self) {
        let Some(name) = self.selection().map(|n| n.name.clone()) else {
            return;
        };
        self.message = match self
            .collector
            .toggle(&self.root, &self.route, self.selected)
        {
            Toggle::Added { replaced: 0 } => format!("Collected {name} · c review"),
            Toggle::Added { replaced } => format!(
                "Collected {name}, replacing {replaced} collected item(s) inside it · c review"
            ),
            Toggle::Removed => format!("Removed {name} from the collector"),
            Toggle::Covered(parent) => format!(
                "{name} is already included by collected folder {} · remove that folder in review (c) to pick items inside it",
                display_path(parent.as_os_str())
            ),
            Toggle::Refused(why) => format!("Cannot collect {name}: {why}"),
        };
    }
    pub fn open_review(&mut self) {
        self.review = true;
        self.trash_confirm = false;
        self.clamp_review();
        self.message.clear()
    }
    fn clamp_review(&mut self) {
        self.review_selected = self
            .review_selected
            .min(self.collector.len().saturating_sub(1));
    }
    /// Keys while the review or its Trash confirmation is open. Returns true
    /// once the user has typed the confirmation and the batch should run.
    pub fn review_key(&mut self, code: KeyCode) -> bool {
        if self.trash_confirm {
            match code {
                KeyCode::Esc => {
                    self.trash_confirm = false;
                    self.typed.clear()
                }
                KeyCode::Backspace => {
                    self.typed.pop();
                }
                KeyCode::Char(c) => {
                    if self.typed.len() < 32 {
                        self.typed.push(c)
                    }
                }
                KeyCode::Enter if self.typed == "trash" && !self.collector.is_empty() => {
                    self.trash_confirm = false;
                    self.typed.clear();
                    return true;
                }
                _ => {}
            }
            return false;
        }
        let len = self.collector.len();
        match code {
            KeyCode::Esc => {
                self.review = false;
                self.message.clear()
            }
            KeyCode::Down | KeyCode::Char('j') if len > 0 => {
                self.review_selected = (self.review_selected + 1) % len
            }
            KeyCode::Up | KeyCode::Char('k') if len > 0 => {
                self.review_selected = (self.review_selected + len - 1) % len
            }
            KeyCode::Char(' ') | KeyCode::Backspace => {
                self.message = match self.collector.remove(self.review_selected) {
                    Some(record) => format!(
                        "Removed {} from the collector",
                        display_path(record.path.as_os_str())
                    ),
                    None => "Collector is empty · nothing to remove".into(),
                };
                self.clamp_review()
            }
            KeyCode::Char('t') if len == 0 => {
                self.message = "Collector is empty · nothing to move to Trash".into()
            }
            KeyCode::Char('t') => {
                self.trash_confirm = true;
                self.typed.clear();
                self.message.clear()
            }
            _ => {}
        }
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{Terminal, backend::TestBackend};
    use std::{
        fs,
        path::PathBuf,
        sync::{
            Arc,
            atomic::{AtomicU64, Ordering},
        },
    };

    fn fixture(tag: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let base = std::env::temp_dir().join(format!(
            "tuidisk-ui-{tag}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(base.join("dir/sub")).unwrap();
        fs::write(base.join("dir/sub/deep"), [1u8; 9000]).unwrap();
        fs::write(base.join("dir/inner"), [1u8; 5000]).unwrap();
        fs::write(base.join("file"), [1u8; 20000]).unwrap();
        fs::canonicalize(base).unwrap()
    }
    fn app(base: &std::path::Path) -> App {
        let root = crate::scan::scan(base, Arc::new(AtomicU64::new(0))).unwrap();
        App::with_collector(root, 0., Collector::new(Guard::default()))
    }
    fn select(app: &mut App, name: &str) {
        app.selected = app
            .current()
            .children
            .iter()
            .position(|n| n.name == name)
            .unwrap();
    }
    fn screen(app: &App, width: u16, height: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal.draw(|f| draw(f, app)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>()
                    + "\n"
            })
            .collect()
    }

    #[test]
    fn the_collector_survives_navigation_and_app_replacement() {
        let base = fixture("rebuild");
        let mut app = app(&base);
        select(&mut app, "file");
        app.toggle_collect();
        select(&mut app, "dir");
        app.drill();
        select(&mut app, "inner");
        app.toggle_collect();
        let identities = app
            .collector
            .records()
            .iter()
            .map(|r| r.identity)
            .collect::<Vec<_>>();
        assert_eq!(identities.len(), 2);
        // "file" is replaced on disk before the rescan that rebuilds the App.
        fs::write(base.join("file.new"), b"impostor").unwrap();
        fs::rename(base.join("file.new"), base.join("file")).unwrap();
        let fresh = crate::scan::scan(&base, Arc::new(AtomicU64::new(0))).unwrap();
        let app = app.rebuild(fresh, 0.);
        assert_eq!(app.current().path, base.join("dir"));
        let after = app
            .collector
            .records()
            .iter()
            .map(|r| r.identity)
            .collect::<Vec<_>>();
        assert_eq!(after, identities);
        assert!(app.collector.records()[0].stale.is_some());
        assert!(app.collector.records()[1].stale.is_none());
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn a_covered_child_is_explained_and_not_added() {
        let base = fixture("covered");
        let mut app = app(&base);
        select(&mut app, "dir");
        app.toggle_collect();
        app.drill();
        select(&mut app, "inner");
        app.toggle_collect();
        assert_eq!(app.collector.len(), 1);
        assert!(app.message.contains("inner") && app.message.contains("dir"));
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn review_keys_remove_rows_and_require_the_typed_word() {
        let base = fixture("keys");
        let mut app = app(&base);
        app.open_review();
        assert!(!app.review_key(KeyCode::Char('t')));
        assert!(!app.trash_confirm && !app.message.is_empty());
        app.review_key(KeyCode::Esc);
        assert!(!app.review);
        for name in ["file", "dir"] {
            select(&mut app, name);
            app.toggle_collect();
        }
        app.open_review();
        app.review_key(KeyCode::Down);
        assert_eq!(app.review_selected, 1);
        app.review_key(KeyCode::Char(' '));
        assert_eq!((app.collector.len(), app.review_selected), (1, 0));
        app.review_key(KeyCode::Char('t'));
        assert!(app.trash_confirm);
        // Enter alone, or the permanent-delete word, never starts the batch.
        assert!(!app.review_key(KeyCode::Enter));
        for c in "delete".chars() {
            app.review_key(KeyCode::Char(c));
        }
        assert!(!app.review_key(KeyCode::Enter));
        app.review_key(KeyCode::Esc);
        assert!(app.review && !app.trash_confirm && app.typed.is_empty());
        app.review_key(KeyCode::Char('t'));
        for c in "trash".chars() {
            app.review_key(KeyCode::Char(c));
        }
        assert!(app.review_key(KeyCode::Enter));
        assert_eq!(app.collector.len(), 1);
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn narrow_review_keeps_size_and_actions_visible_when_items_need_attention() {
        let base = fixture("narrow-actions");
        let mut app = app(&base);
        let empty = screen(&app, 60, 20);
        assert!(empty.contains("0 items") && empty.contains("0 B"));
        select(&mut app, "file");
        app.toggle_collect();
        select(&mut app, "dir");
        app.toggle_collect();
        let report = crate::trash::Report {
            outcomes: app
                .collector
                .records()
                .iter()
                .map(|r| {
                    (
                        r.path.clone(),
                        crate::trash::Outcome::Failed("unavailable".into()),
                    )
                })
                .collect(),
            attempted: 2,
            cancelled: false,
        };
        app.collector.apply(&report);
        app.open_review();
        for (width, height) in [(60, 20), (80, 24)] {
            let rendered = screen(&app, width, height);
            let heading = rendered
                .lines()
                .find(|line| line.contains("TRASH COLLECTOR"))
                .unwrap();
            assert!(
                heading.contains("2 items") && heading.contains(&size(app.collector.total_bytes()))
            );
            let controls = rendered
                .lines()
                .rev()
                .find(|line| line.contains("Space remove"))
                .unwrap();
            assert!(controls.contains("t Trash") && controls.contains("Esc close"));
        }
        fs::remove_dir_all(base).unwrap();
    }
    #[test]
    fn every_collector_state_renders_at_supported_sizes() {
        let base = fixture("render");
        let mut app = app(&base);
        let empty = screen(&app, 140, 44);
        select(&mut app, "file");
        app.toggle_collect();
        let browsing = screen(&app, 140, 44);
        assert_ne!(empty, browsing);
        assert!(browsing.contains(&size(app.collector.total_bytes())));
        app.open_review();
        for (width, height) in [(60, 20), (80, 24), (140, 44), (300, 90)] {
            app.trash_confirm = false;
            let review = screen(&app, width, height);
            assert!(review.contains("Trash is emptied"), "{width}x{height}");
            app.trash_confirm = true;
            let confirm = screen(&app, width, height);
            assert!(confirm.contains("Trash is emptied"), "{width}x{height}");
        }
        app.trash_confirm = false;
        let full_path = display_path(base.join("file").as_os_str());
        let review = screen(&app, 300, 44);
        assert!(review.contains(&full_path));
        app.review_key(KeyCode::Char(' '));
        assert_ne!(screen(&app, 140, 44), review);
        screen(&app, 60, 20);
        fs::remove_dir_all(base).unwrap();
    }
}
