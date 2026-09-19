use crate::scan::Node;
mod confirm;
mod foundation;
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
}
impl App {
    pub fn new(root: Node, _seconds: f64) -> Self {
        Self {
            root,
            route: vec![],
            selected: 0,
            previous: vec![],
            confirm: false,
            typed: String::new(),
            message: String::new(),
            help: false,
        }
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
}
