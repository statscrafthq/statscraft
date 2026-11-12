//! The open windows of the one session.
//!
//! Every window is an egui viewport (a native window). eframe has one root viewport, and the
//! program ends when it closes. Its role passes along instead of staying with one window: the
//! root draws the first window of the registry, every other window is an immediate viewport.
//! When the first window closes while others stay open, the root draws the next one and takes
//! that window's place and size; its own viewport is dropped. A hidden root was the other way,
//! and it does not work: immediate viewports are drawn in the root's pass, and a hidden window
//! gets no frames on Wayland (eframe then runs only `App::logic`), so the other windows would
//! freeze. Deferred viewports would draw on their own, but their callbacks must be `Send +
//! Sync + 'static`, which the session and its windows are not. So the program ends exactly when
//! the root closes, and the root closes only with the last window.

use crate::syntax_window::SyntaxWindow;
use eframe::egui::{Pos2, Vec2, ViewportId};
use std::path::Path;

pub(crate) type WindowId = u64;

pub(crate) enum Kind {
    /// The Data, Variables and Output tabs, until data and output windows take them over.
    Main,
    Syntax(Box<SyntaxWindow>),
}

pub(crate) struct Window {
    pub id: WindowId,
    pub kind: Kind,
    /// Pasted syntax goes to the designated syntax window.
    pub designated: bool,
    /// Where the window was last drawn, for the root to take its place when it closes.
    pub position: Option<Pos2>,
    pub size: Option<Vec2>,
}

/// What closing a window takes.
#[derive(Debug, PartialEq)]
pub(crate) enum Closing {
    /// The last window: the program ends, after the question about everything unsaved.
    Quit,
    /// The window holds unsaved work: the question first.
    Ask(Vec<String>),
    /// Nothing is lost.
    Close,
}

pub(crate) struct Registry {
    windows: Vec<Window>,
    next: WindowId,
    /// The number of the next new syntax: Syntax1, Syntax2 and so on.
    untitled: usize,
    /// The window that had the keyboard last.
    pub focused: Option<WindowId>,
}

impl Registry {
    /// The main window alone.
    pub fn new() -> Self {
        let mut registry = Registry {
            windows: Vec::new(),
            next: 1,
            untitled: 1,
            focused: None,
        };
        registry.open(Kind::Main);
        registry
    }

    /// Adds a window at the end. A new syntax window becomes the designated one.
    pub fn open(&mut self, kind: Kind) -> WindowId {
        let id = self.next;
        self.next += 1;
        let syntax = matches!(kind, Kind::Syntax(_));
        if syntax {
            for w in &mut self.windows {
                w.designated &= !matches!(w.kind, Kind::Syntax(_));
            }
        }
        self.windows.push(Window {
            id,
            kind,
            designated: syntax,
            position: None,
            size: None,
        });
        id
    }

    /// A new syntax window with the next untitled name, or with this file's syntax.
    pub fn open_syntax(&mut self, text: String, file: Option<&Path>) -> WindowId {
        // a file names its window; only new syntax takes a number
        let name = match file {
            Some(_) => String::new(),
            None => {
                self.untitled += 1;
                format!("Syntax{}", self.untitled - 1)
            }
        };
        let window = SyntaxWindow::new(name, text, file.map(Path::to_path_buf));
        self.open(Kind::Syntax(Box::new(window)))
    }

    /// Removes a window; the designation passes to the newest syntax window left.
    pub fn close(&mut self, id: WindowId) {
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        let window = self.windows.remove(index);
        if window.designated
            && let Some(next) = self
                .windows
                .iter_mut()
                .rev()
                .find(|w| matches!(w.kind, Kind::Syntax(_)))
        {
            next.designated = true;
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    /// What closing this window takes. Only syntax windows hold work of their own: the data
    /// belongs to the session and stays when the main window closes.
    pub fn closing(&self, id: WindowId) -> Closing {
        if self.windows.len() == 1 {
            return Closing::Quit;
        }
        match self.syntax(id).and_then(SyntaxWindow::unsaved) {
            Some(item) => Closing::Ask(vec![item]),
            None => Closing::Close,
        }
    }

    /// The unsaved syntax of every window, for the question before the program ends.
    pub fn unsaved(&self) -> Vec<String> {
        self.iter()
            .filter_map(|w| match &w.kind {
                Kind::Syntax(s) => s.unsaved(),
                Kind::Main => None,
            })
            .collect()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Window> {
        self.windows.iter()
    }

    pub fn ids(&self) -> Vec<WindowId> {
        self.windows.iter().map(|w| w.id).collect()
    }

    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }

    pub fn syntax(&self, id: WindowId) -> Option<&SyntaxWindow> {
        match &self.get(id)?.kind {
            Kind::Syntax(s) => Some(s),
            Kind::Main => None,
        }
    }

    pub fn syntax_mut(&mut self, id: WindowId) -> Option<&mut SyntaxWindow> {
        match &mut self.get_mut(id)?.kind {
            Kind::Syntax(s) => Some(s),
            Kind::Main => None,
        }
    }

    /// The window the root viewport draws.
    pub fn root(&self) -> WindowId {
        self.windows[0].id
    }

    /// The native window of a window.
    pub fn viewport(&self, id: WindowId) -> ViewportId {
        if id == self.root() {
            ViewportId::ROOT
        } else {
            ViewportId::from_hash_of(("statscraft window", id))
        }
    }

    pub fn main(&self) -> Option<WindowId> {
        self.iter()
            .find(|w| matches!(w.kind, Kind::Main))
            .map(|w| w.id)
    }

    /// The main window, opened again when it was closed.
    pub fn open_main(&mut self) -> WindowId {
        self.main().unwrap_or_else(|| self.open(Kind::Main))
    }

    pub fn designated_syntax(&self) -> Option<WindowId> {
        self.iter()
            .find(|w| w.designated && matches!(w.kind, Kind::Syntax(_)))
            .map(|w| w.id)
    }

    /// Makes a syntax window the designated one.
    pub fn designate(&mut self, id: WindowId) {
        if self.syntax(id).is_none() {
            return;
        }
        for w in &mut self.windows {
            if matches!(w.kind, Kind::Syntax(_)) {
                w.designated = w.id == id;
            }
        }
    }

    /// The syntax window that holds this file.
    pub fn with_file(&self, path: &Path) -> Option<WindowId> {
        self.iter()
            .find(|w| matches!(&w.kind, Kind::Syntax(s) if s.file.as_deref() == Some(path)))
            .map(|w| w.id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(registry: &mut Registry, id: WindowId, text: &str) {
        registry.syntax_mut(id).unwrap().text = text.to_string();
    }

    #[test]
    fn the_root_passes_to_the_next_window_and_the_last_close_quits() {
        let mut r = Registry::new();
        let main = r.main().unwrap();
        assert_eq!(r.root(), main);
        assert_eq!(r.viewport(main), ViewportId::ROOT);
        let a = r.open_syntax(String::new(), None);
        let b = r.open_syntax(String::new(), None);
        assert_ne!(r.viewport(a), ViewportId::ROOT);
        assert_ne!(r.viewport(a), r.viewport(b));
        assert_eq!(r.syntax(a).unwrap().name(), "Syntax1");
        assert_eq!(r.syntax(b).unwrap().name(), "Syntax2");
        // the main window holds no work of its own
        assert_eq!(r.closing(main), Closing::Close);
        r.close(main);
        assert_eq!(r.root(), a);
        assert_eq!(r.viewport(a), ViewportId::ROOT);
        assert_eq!(r.ids(), [a, b]);
        // data opened again brings the main window back, at the end
        let again = r.open_main();
        assert_eq!(r.ids(), [a, b, again]);
        assert_eq!(r.open_main(), again);
        r.close(a);
        r.close(again);
        assert_eq!(r.closing(b), Closing::Quit);
    }

    #[test]
    fn the_newest_syntax_window_is_designated_until_another_is_chosen() {
        let mut r = Registry::new();
        assert_eq!(r.designated_syntax(), None);
        let a = r.open_syntax(String::new(), None);
        assert_eq!(r.designated_syntax(), Some(a));
        let b = r.open_syntax(String::new(), None);
        assert_eq!(r.designated_syntax(), Some(b));
        r.designate(a);
        assert_eq!(r.designated_syntax(), Some(a));
        // the main window cannot be designated for syntax
        r.designate(r.main().unwrap());
        assert_eq!(r.designated_syntax(), Some(a));
        r.close(a);
        assert_eq!(r.designated_syntax(), Some(b));
        r.close(b);
        assert_eq!(r.designated_syntax(), None);
    }

    #[test]
    fn unsaved_syntax_is_asked_about_per_window_and_all_together_at_the_end() {
        let mut r = Registry::new();
        let a = r.open_syntax(String::new(), None);
        let file = Path::new("/d/job.sps");
        let b = r.open_syntax("FREQ x.".into(), Some(file));
        let c = r.open_syntax(String::new(), None);
        assert!(r.unsaved().is_empty());
        assert_eq!(r.closing(a), Closing::Close);
        assert_eq!(r.closing(b), Closing::Close);
        edit(&mut r, a, "DESC y.");
        edit(&mut r, b, "FREQ y.");
        assert_eq!(r.closing(a), Closing::Ask(vec!["Syntax: Syntax1".into()]));
        assert_eq!(r.closing(b), Closing::Ask(vec!["Syntax: job.sps".into()]));
        assert_eq!(r.closing(c), Closing::Close);
        assert_eq!(r.unsaved(), ["Syntax: Syntax1", "Syntax: job.sps"]);
        assert_eq!(r.with_file(file), Some(b));
        // back to what the file holds: nothing to save
        edit(&mut r, b, "FREQ x.");
        assert_eq!(r.unsaved(), ["Syntax: Syntax1"]);
        r.close(r.main().unwrap());
        r.close(b);
        r.close(c);
        // the last window: the question covers everything, so closing it quits
        assert_eq!(r.closing(a), Closing::Quit);
    }
}
