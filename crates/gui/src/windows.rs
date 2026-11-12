//! The open windows of the one session: a data window per dataset, a syntax window per syntax
//! file and an output window per output document.
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
//!
//! Data and output windows follow the session: after every run, `sync_data` and `sync_output`
//! open a window for each new dataset and output document and close the windows of those that
//! are gone, so syntax typed anywhere (DATASET COPY, OUTPUT NEW, DATASET CLOSE) shows the same
//! as a menu item.

use crate::app::DataView;
use crate::output_view::OutputView;
use crate::syntax_window::SyntaxWindow;
use eframe::egui::{Pos2, Vec2, ViewportId};
use statscraft_engine::live::{SyntaxInfo, SyntaxWindows};
use std::path::{Path, PathBuf};

pub(crate) type WindowId = u64;

pub(crate) enum Kind {
    Data(Box<DataWindow>),
    Syntax(Box<SyntaxWindow>),
    Output(Box<OutputWindow>),
}

/// A data window: the dataset it shows and its Data and Variables tabs.
#[derive(Default)]
pub(crate) struct DataWindow {
    /// The dataset's name; None for the active dataset without a name, or for no data at all.
    pub dataset: Option<String>,
    pub view: DataView,
}

/// An output window: the engine's output document it shows, and its tables.
pub(crate) struct OutputWindow {
    pub document: String,
    pub view: OutputView,
}

pub(crate) struct Window {
    pub id: WindowId,
    pub kind: Kind,
    /// Pasted syntax goes to the designated syntax window, new tables to the designated output
    /// window. One window of each of these kinds is designated.
    pub designated: bool,
    /// Where the window was last drawn, for the root to take its place when it closes.
    pub position: Option<Pos2>,
    pub size: Option<Vec2>,
    /// The syntax that closes its dataset or output document waits or runs: a second close
    /// does nothing.
    pub closing: bool,
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

/// Whether two dataset names are the same; names are not case sensitive.
pub(crate) fn same(a: Option<&str>, b: Option<&str>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a.eq_ignore_ascii_case(b),
        (a, b) => a.is_none() && b.is_none(),
    }
}

pub(crate) struct Registry {
    windows: Vec<Window>,
    next: WindowId,
    /// The number of the next new syntax: Syntax1, Syntax2 and so on.
    untitled: usize,
    /// The window that had the keyboard last.
    pub focused: Option<WindowId>,
    /// The syntax window Agent, which keeps the syntax of the runs sent to the window.
    agent: Option<WindowId>,
}

impl Registry {
    /// One empty data window, a place to open files from.
    pub fn new() -> Self {
        let mut registry = Registry {
            windows: Vec::new(),
            next: 1,
            untitled: 1,
            focused: None,
            agent: None,
        };
        registry.open(Kind::Data(Box::default()));
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
            closing: false,
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

    /// Removes a window; the designation of a syntax window passes to the newest one left, other
    /// than the Agent window. Output windows follow the session's designated document instead
    /// (`sync_output`).
    pub fn close(&mut self, id: WindowId) {
        let Some(index) = self.windows.iter().position(|w| w.id == id) else {
            return;
        };
        let window = self.windows.remove(index);
        let agent = self.agent;
        if window.designated
            && matches!(window.kind, Kind::Syntax(_))
            && let Some(next) = self
                .windows
                .iter_mut()
                .rev()
                .find(|w| matches!(w.kind, Kind::Syntax(_)) && Some(w.id) != agent)
        {
            next.designated = true;
        }
        if self.focused == Some(id) {
            self.focused = None;
        }
    }

    /// What closing this window takes. `data` is what its dataset holds unsaved, for a data
    /// window; output is never asked about, as it cannot be saved yet.
    pub fn closing(&self, id: WindowId, data: Option<String>) -> Closing {
        if self.windows.len() == 1 {
            return Closing::Quit;
        }
        let item = match &self.get(id).map(|w| &w.kind) {
            Some(Kind::Syntax(s)) => s.unsaved(),
            Some(Kind::Data(_)) => data,
            _ => None,
        };
        match item {
            Some(item) => Closing::Ask(vec![item]),
            None => Closing::Close,
        }
    }

    /// The unsaved syntax of every window, for the question before the program ends.
    pub fn unsaved(&self) -> Vec<String> {
        self.iter()
            .filter_map(|w| match &w.kind {
                Kind::Syntax(s) => s.unsaved(),
                _ => None,
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
            _ => None,
        }
    }

    pub fn syntax_mut(&mut self, id: WindowId) -> Option<&mut SyntaxWindow> {
        match &mut self.get_mut(id)?.kind {
            Kind::Syntax(s) => Some(s),
            _ => None,
        }
    }

    pub fn data(&self, id: WindowId) -> Option<&DataWindow> {
        match &self.get(id)?.kind {
            Kind::Data(d) => Some(d),
            _ => None,
        }
    }

    pub fn data_mut(&mut self, id: WindowId) -> Option<&mut DataWindow> {
        match &mut self.get_mut(id)?.kind {
            Kind::Data(d) => Some(d),
            _ => None,
        }
    }

    pub fn output(&self, id: WindowId) -> Option<&OutputWindow> {
        match &self.get(id)?.kind {
            Kind::Output(o) => Some(o),
            _ => None,
        }
    }

    pub fn output_mut(&mut self, id: WindowId) -> Option<&mut OutputWindow> {
        match &mut self.get_mut(id)?.kind {
            Kind::Output(o) => Some(o),
            _ => None,
        }
    }

    /// The data windows' views, to reset what a run may have made stale.
    pub fn data_views(&mut self) -> impl Iterator<Item = &mut DataView> {
        self.windows.iter_mut().filter_map(|w| match &mut w.kind {
            Kind::Data(d) => Some(&mut d.view),
            _ => None,
        })
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

    pub fn designated_syntax(&self) -> Option<WindowId> {
        self.iter()
            .find(|w| w.designated && matches!(w.kind, Kind::Syntax(_)))
            .map(|w| w.id)
    }

    pub fn designated_output(&self) -> Option<WindowId> {
        self.iter()
            .find(|w| w.designated && matches!(w.kind, Kind::Output(_)))
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

    /// The data window of a dataset; None names the active dataset without a name, or the
    /// empty data window.
    pub fn with_dataset(&self, name: Option<&str>) -> Option<WindowId> {
        self.iter()
            .find(|w| matches!(&w.kind, Kind::Data(d) if same(d.dataset.as_deref(), name)))
            .map(|w| w.id)
    }

    /// The output window of a document.
    pub fn with_document(&self, name: &str) -> Option<WindowId> {
        self.iter()
            .find(|w| matches!(&w.kind, Kind::Output(o) if o.document.eq_ignore_ascii_case(name)))
            .map(|w| w.id)
    }

    fn data_count(&self) -> usize {
        self.iter()
            .filter(|w| matches!(w.kind, Kind::Data(_)))
            .count()
    }

    /// Makes the data windows match the datasets that hold data after a run: `shown` names them
    /// (None for the active dataset without a name), `was` and `now` are the active dataset
    /// before and after the run. The window of the data that was active shows the new active
    /// data when its own is gone: a name given (DATASET NAME), a file read over unnamed data,
    /// the empty window filled. Other windows whose data is gone close, except the last data
    /// window, which stays empty. Returns the windows opened and closed.
    pub fn sync_data(
        &mut self,
        shown: &[Option<String>],
        was: Option<&str>,
        now: Option<&str>,
    ) -> (Vec<WindowId>, Vec<WindowId>) {
        let present = |name: Option<&str>| shown.iter().any(|s| same(s.as_deref(), name));
        if let Some(id) = self.with_dataset(was)
            && !present(was)
            && present(now)
            && self.with_dataset(now).is_none()
            && let Some(d) = self.data_mut(id)
        {
            *d = DataWindow {
                dataset: now.map(str::to_string),
                ..Default::default()
            };
        }
        let mut closed = Vec::new();
        let stale: Vec<WindowId> = self
            .iter()
            .filter(|w| matches!(&w.kind, Kind::Data(d) if !present(d.dataset.as_deref())))
            .map(|w| w.id)
            .collect();
        for id in stale {
            if self.data_count() > 1 {
                self.close(id);
                closed.push(id);
            } else if let Some(d) = self.data_mut(id)
                && d.dataset.is_some()
            {
                *d = DataWindow::default();
            }
        }
        let mut opened = Vec::new();
        for name in shown {
            if self.with_dataset(name.as_deref()).is_none() {
                let window = DataWindow {
                    dataset: name.clone(),
                    ..Default::default()
                };
                opened.push(self.open(Kind::Data(Box::new(window))));
            }
        }
        (opened, closed)
    }

    /// Makes the output windows match the open output documents after a run, and marks the
    /// window of the designated one. `renamed` is the document that OUTPUT NAME renamed: its
    /// window takes the new name and keeps its tables. Returns the windows opened and closed.
    pub fn sync_output(
        &mut self,
        documents: &[String],
        designated: Option<&str>,
        renamed: Option<&str>,
    ) -> (Vec<WindowId>, Vec<WindowId>) {
        let present = |name: &str| documents.iter().any(|d| d.eq_ignore_ascii_case(name));
        if let (Some(old), Some(new)) = (renamed, designated)
            && !present(old)
            && self.with_document(new).is_none()
            && let Some(id) = self.with_document(old)
            && let Some(o) = self.output_mut(id)
        {
            o.document = new.to_string();
        }
        let stale: Vec<WindowId> = self
            .iter()
            .filter(|w| matches!(&w.kind, Kind::Output(o) if !present(&o.document)))
            .map(|w| w.id)
            .collect();
        for &id in &stale {
            self.close(id);
        }
        let mut opened = Vec::new();
        for name in documents {
            if self.with_document(name).is_none() {
                let window = OutputWindow {
                    document: name.clone(),
                    view: OutputView::default(),
                };
                opened.push(self.open(Kind::Output(Box::new(window))));
            }
        }
        for w in &mut self.windows {
            if let Kind::Output(o) = &w.kind {
                w.designated = designated.is_some_and(|d| d.eq_ignore_ascii_case(&o.document));
            }
        }
        (opened, stale)
    }
}

/// The syntax windows as the requests from the command line and agents see them.
impl SyntaxWindows for Registry {
    fn list(&self) -> Vec<SyntaxInfo> {
        self.iter()
            .filter_map(|w| match &w.kind {
                Kind::Syntax(s) => Some(SyntaxInfo {
                    id: w.id,
                    name: s.name(),
                    file: s.file.clone(),
                    unsaved: s.unsaved().is_some(),
                    lines: statscraft_engine::live::line_count(&s.text),
                    agent: self.agent == Some(w.id),
                    designated: w.designated,
                    focused: self.focused == Some(w.id),
                }),
                _ => None,
            })
            .collect()
    }

    fn text_mut(&mut self, id: WindowId) -> Option<&mut String> {
        self.syntax_mut(id).map(|s| &mut s.text)
    }

    fn create(
        &mut self,
        name: Option<String>,
        text: String,
        file: Option<PathBuf>,
        designate: bool,
    ) -> WindowId {
        let before = self.designated_syntax();
        let id = match name {
            Some(name) => self.open(Kind::Syntax(Box::new(SyntaxWindow::new(name, text, file)))),
            None => self.open_syntax(text, file.as_deref()),
        };
        if let Some(s) = self.syntax_mut(id)
            && s.file.is_none()
        {
            // text an agent put in a new window is lost on close unless saved: ask then
            s.saved.clear();
        }
        if !designate {
            match before {
                Some(before) => self.designate(before),
                None => self.get_mut(id).expect("just opened").designated = false,
            }
        }
        id
    }

    fn saved(&mut self, id: WindowId, file: PathBuf) {
        if let Some(s) = self.syntax_mut(id) {
            s.saved = s.text.clone();
            s.file = Some(file);
        }
    }

    fn agent(&mut self) -> &mut Option<WindowId> {
        &mut self.agent
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn edit(registry: &mut Registry, id: WindowId, text: &str) {
        registry.syntax_mut(id).unwrap().text = text.to_string();
    }

    fn names(list: &[&str]) -> Vec<Option<String>> {
        list.iter().map(|n| Some(n.to_string())).collect()
    }

    fn shown(r: &Registry) -> Vec<Option<String>> {
        r.iter()
            .filter_map(|w| Some(r.data(w.id)?.dataset.clone()))
            .collect()
    }

    #[test]
    fn the_root_passes_to_the_next_window_and_the_last_close_quits() {
        let mut r = Registry::new();
        let data = r.with_dataset(None).unwrap();
        assert_eq!(r.root(), data);
        assert_eq!(r.viewport(data), ViewportId::ROOT);
        let a = r.open_syntax(String::new(), None);
        let b = r.open_syntax(String::new(), None);
        assert_ne!(r.viewport(a), ViewportId::ROOT);
        assert_ne!(r.viewport(a), r.viewport(b));
        assert_eq!(r.syntax(a).unwrap().name(), "Syntax1");
        assert_eq!(r.syntax(b).unwrap().name(), "Syntax2");
        // an empty data window holds nothing to save
        assert_eq!(r.closing(data, None), Closing::Close);
        r.close(data);
        assert_eq!(r.root(), a);
        assert_eq!(r.viewport(a), ViewportId::ROOT);
        assert_eq!(r.ids(), [a, b]);
        r.close(a);
        assert_eq!(r.closing(b, None), Closing::Quit);
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
        // a data window cannot be designated for syntax
        r.designate(r.with_dataset(None).unwrap());
        assert_eq!(r.designated_syntax(), Some(a));
        r.close(a);
        assert_eq!(r.designated_syntax(), Some(b));
        r.close(b);
        assert_eq!(r.designated_syntax(), None);
    }

    #[test]
    fn the_agent_window_keeps_runs_without_taking_pasted_syntax() {
        use statscraft_engine::live::{self, SyntaxWindows};
        let mut r = Registry::new();
        let mine = r.open_syntax("FREQ x.".into(), None);
        assert_eq!(live::keep_trail(&mut r, "DESC y."), "syntax window Agent");
        let agent = r.agent.unwrap();
        assert_eq!(
            r.designated_syntax(),
            Some(mine),
            "pasted syntax still goes to mine"
        );
        live::keep_trail(&mut r, "MEANS y BY x.");
        let text = &r.syntax(agent).unwrap().text;
        assert!(
            text.starts_with("* Agent ") && text.ends_with(".\nMEANS y BY x.\n"),
            "{text}"
        );
        let listed = r.list();
        assert_eq!(listed.len(), 2);
        assert!(listed[1].agent && !listed[1].designated && listed[1].unsaved);
        assert_eq!(listed[1].lines, 5);
        // the person closed it: the next run opens a new one, and it is never designated
        r.close(mine);
        assert_eq!(r.designated_syntax(), None);
        r.close(agent);
        live::keep_trail(&mut r, "SHOW.");
        let again = r.agent.unwrap();
        assert_ne!(again, agent);
        assert_eq!(r.designated_syntax(), None);
        assert!(r.syntax(again).unwrap().text.ends_with(".\nSHOW.\n"));
        // a new window from an agent is designated, and asked about when it holds text
        let new = r.create(None, "SHOW N.".into(), None, true);
        assert_eq!(r.designated_syntax(), Some(new));
        assert!(r.syntax(new).unwrap().unsaved().is_some());
    }

    #[test]
    fn unsaved_syntax_is_asked_about_per_window_and_all_together_at_the_end() {
        let mut r = Registry::new();
        let a = r.open_syntax(String::new(), None);
        let file = Path::new("/d/job.sps");
        let b = r.open_syntax("FREQ x.".into(), Some(file));
        let c = r.open_syntax(String::new(), None);
        assert!(r.unsaved().is_empty());
        assert_eq!(r.closing(a, None), Closing::Close);
        assert_eq!(r.closing(b, None), Closing::Close);
        edit(&mut r, a, "DESC y.");
        edit(&mut r, b, "FREQ y.");
        assert_eq!(
            r.closing(a, None),
            Closing::Ask(vec!["Syntax: Syntax1".into()])
        );
        assert_eq!(
            r.closing(b, None),
            Closing::Ask(vec!["Syntax: job.sps".into()])
        );
        assert_eq!(r.closing(c, None), Closing::Close);
        assert_eq!(r.unsaved(), ["Syntax: Syntax1", "Syntax: job.sps"]);
        assert_eq!(r.with_file(file), Some(b));
        // back to what the file holds: nothing to save
        edit(&mut r, b, "FREQ x.");
        assert_eq!(r.unsaved(), ["Syntax: Syntax1"]);
        r.close(r.with_dataset(None).unwrap());
        r.close(b);
        r.close(c);
        // the last window: the question covers everything, so closing it quits
        assert_eq!(r.closing(a, None), Closing::Quit);
    }

    #[test]
    fn a_data_window_asks_about_its_data_and_an_output_window_about_nothing() {
        let mut r = Registry::new();
        let data = r.with_dataset(None).unwrap();
        r.sync_output(&["Document1".into()], Some("Document1"), None);
        let output = r.with_document("Document1").unwrap();
        let item = || Some("Data: a.sav".to_string());
        assert_eq!(
            r.closing(data, item()),
            Closing::Ask(vec!["Data: a.sav".into()])
        );
        assert_eq!(r.closing(output, item()), Closing::Close);
        r.close(data);
        // the last window quits, and the question at the end covers the data
        assert_eq!(r.closing(output, None), Closing::Quit);
    }

    #[test]
    fn data_windows_follow_the_datasets_of_the_session() {
        let mut r = Registry::new();
        let first = r.with_dataset(None).unwrap();
        // a file opened from the window fills the empty window
        let (opened, closed) = r.sync_data(&names(&["DataSet1"]), None, Some("DataSet1"));
        assert!(opened.is_empty() && closed.is_empty());
        assert_eq!(r.with_dataset(Some("dataset1")), Some(first));
        // a second file gets a window of its own, and the first stays
        let (opened, _) = r.sync_data(
            &names(&["DataSet2", "DataSet1"]),
            Some("DataSet1"),
            Some("DataSet2"),
        );
        let second = opened[0];
        assert_eq!(r.with_dataset(Some("DataSet2")), Some(second));
        // GET typed in syntax: the named data stays, the unnamed data gets a window
        let mut now = vec![None];
        now.extend(names(&["DataSet1", "DataSet2"]));
        let (opened, _) = r.sync_data(&now, Some("DataSet2"), None);
        let unnamed = opened[0];
        assert_eq!(r.with_dataset(None), Some(unnamed));
        // DATASET NAME: the same window, now under the name
        let (opened, closed) = r.sync_data(
            &names(&["copy", "DataSet1", "DataSet2"]),
            None,
            Some("copy"),
        );
        assert!(opened.is_empty() && closed.is_empty());
        assert_eq!(r.with_dataset(Some("copy")), Some(unnamed));
        // DATASET CLOSE of another dataset closes its window
        let (_, closed) = r.sync_data(&names(&["copy", "DataSet1"]), Some("copy"), Some("copy"));
        assert_eq!(closed, [second]);
        // DATASET CLOSE ALL: the active data loses its name and keeps its window
        let (_, closed) = r.sync_data(&[None], Some("copy"), None);
        assert_eq!(closed, [first]);
        assert_eq!(r.with_dataset(None), Some(unnamed));
        // NEW FILE: the last data window stays, empty
        let (opened, closed) = r.sync_data(&[], None, None);
        assert!(opened.is_empty() && closed.is_empty());
        assert_eq!(shown(&r), [None]);
    }

    #[test]
    fn output_windows_follow_the_documents_and_mark_the_designated_one() {
        let mut r = Registry::new();
        let docs = |list: &[&str]| list.iter().map(|d| d.to_string()).collect::<Vec<_>>();
        // the first tables start Document1
        let (opened, _) = r.sync_output(&docs(&["Document1"]), Some("Document1"), None);
        let one = opened[0];
        assert_eq!(r.designated_output(), Some(one));
        // OUTPUT NEW: a second window, designated
        let (opened, _) =
            r.sync_output(&docs(&["Document1", "Document2"]), Some("Document2"), None);
        let two = opened[0];
        assert_eq!(r.designated_output(), Some(two));
        // OUTPUT ACTIVATE
        r.sync_output(&docs(&["Document1", "Document2"]), Some("Document1"), None);
        assert_eq!(r.designated_output(), Some(one));
        // OUTPUT NAME: the window keeps its tables under the new name
        r.sync_output(&docs(&["men", "Document2"]), Some("men"), Some("Document1"));
        assert_eq!(r.with_document("MEN"), Some(one));
        // OUTPUT CLOSE: its window closes; no window is designated until the next tables
        let (_, closed) = r.sync_output(&docs(&["Document2"]), None, None);
        assert_eq!(closed, [one]);
        assert_eq!(r.designated_output(), None);
        // designation is per kind: a syntax window keeps its own
        let s = r.open_syntax(String::new(), None);
        r.sync_output(&docs(&["Document2"]), Some("Document2"), None);
        assert_eq!(r.designated_syntax(), Some(s));
        assert_eq!(r.designated_output(), Some(two));
    }
}
