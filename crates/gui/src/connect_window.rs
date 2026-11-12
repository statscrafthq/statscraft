//! Help > Connect an AI agent: `statscraft connect` with clicks. It adds the MCP server, the
//! command-line program that ships next to the window, to the AI apps on this computer, through
//! the same library as the command line.

use eframe::egui::{self, Align2, Key, RichText, Ui, vec2};
use statscraft_connect::{ClientStatus, Connector, DOCS, Outcome, plain_path};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

/// What a click asks for; it runs on a worker thread, as Claude Code's command can take a while.
enum Job {
    Connect(Vec<&'static str>),
    Remove(&'static str),
}

/// One line per app about what an action did; an error is Err.
type Messages = Vec<Result<String, String>>;

#[derive(Default)]
pub struct ConnectWindow {
    open: bool,
    /// The command-line program the entries start, or why there is none.
    server: Option<Result<PathBuf, String>>,
    clients: Vec<ClientStatus>,
    messages: Messages,
    /// The action running now, with its result to come.
    running: Option<mpsc::Receiver<(Messages, Vec<ClientStatus>)>>,
}

impl ConnectWindow {
    /// Opens the dialog with a fresh look at the apps.
    pub fn open(&mut self) {
        self.open = true;
        self.messages.clear();
        let server = this_server();
        self.clients = Connector::from_env().clients(server.as_deref().unwrap_or(Path::new("")));
        self.server = Some(server);
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.open {
            return;
        }
        if let Some(Ok((messages, clients))) = self.running.as_ref().map(|r| r.try_recv()) {
            self.messages = messages;
            self.clients = clients;
            self.running = None;
        }
        let mut open = true;
        let mut close = false;
        let mut job = None;
        egui::Window::new("Connect an AI agent")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(Align2::CENTER_CENTER, vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_width(520.0);
                job = self.body(ui);
                ui.separator();
                close = ui.button("Close").clicked() || ui.input(|i| i.key_pressed(Key::Escape));
            });
        if let (Some(job), Some(Ok(server))) = (job, &self.server) {
            self.start(job, server.clone(), ctx.clone());
        }
        if close || !open {
            self.open = false;
        }
    }

    fn body(&self, ui: &mut Ui) -> Option<Job> {
        let mut job = None;
        ui.label(
            "Statscraft can work as a set of tools inside your AI app. Your data stay on this \
             computer, but whatever the app reads from them goes to its AI provider.",
        );
        ui.add_space(8.0);
        let server_ok = matches!(self.server, Some(Ok(_)));
        if let Some(Err(e)) = &self.server {
            ui.colored_label(ui.visuals().error_fg_color, e);
            ui.add_space(8.0);
        }
        let idle = self.running.is_none() && server_ok;
        egui::Grid::new("connect-apps")
            .num_columns(3)
            .spacing(vec2(16.0, 6.0))
            .show(ui, |ui| {
                for c in &self.clients {
                    ui.label(RichText::new(c.name).strong());
                    ui.label(state(c, server_ok));
                    ui.horizontal(|ui| {
                        let here = c.connected && c.points_here;
                        if !here
                            && ui
                                .add_enabled(idle && c.detected, egui::Button::new("Connect"))
                                .clicked()
                        {
                            job = Some(Job::Connect(vec![c.id]));
                        }
                        if c.connected
                            && ui.add_enabled(idle, egui::Button::new("Remove")).clicked()
                        {
                            job = Some(Job::Remove(c.id));
                        }
                    });
                    ui.end_row();
                    if let Some(problem) = &c.problem {
                        ui.label("");
                        ui.colored_label(ui.visuals().error_fg_color, problem);
                        ui.end_row();
                    }
                }
            });
        ui.add_space(8.0);
        let waiting = connect_all(&self.clients);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    idle && !waiting.is_empty(),
                    egui::Button::new("Connect all"),
                )
                .on_hover_text("Connects every installed app that is not connected yet.")
                .clicked()
            {
                job = Some(Job::Connect(waiting));
            }
            if self.running.is_some() {
                ui.spinner();
            }
        });
        for message in &self.messages {
            match message {
                Ok(text) => ui.label(text),
                Err(text) => ui.colored_label(ui.visuals().error_fg_color, text),
            };
        }
        ui.add_space(8.0);
        ui.horizontal_wrapped(|ui| {
            ui.label("To connect an app by hand, or to install the Claude Desktop bundle, see");
            ui.hyperlink_to("Connect your AI agent", DOCS);
        });
        job
    }

    fn start(&mut self, job: Job, server: PathBuf, ctx: egui::Context) {
        let (sender, receiver) = mpsc::channel();
        self.running = Some(receiver);
        self.messages.clear();
        std::thread::spawn(move || {
            // the connector holds a runner that cannot cross threads, so the worker makes its own
            let _ = sender.send(work(&Connector::from_env(), &server, &job));
            ctx.request_repaint();
        });
    }
}

/// Runs the job, then reads every app's state again.
fn work(connector: &Connector, server: &Path, job: &Job) -> (Messages, Vec<ClientStatus>) {
    let done: Vec<(&str, Result<Outcome, String>, bool)> = match job {
        Job::Connect(ids) => ids
            .iter()
            .map(|id| (*id, connector.connect(id, server), false))
            .collect(),
        Job::Remove(id) => vec![(*id, connector.disconnect(id), true)],
    };
    let clients = connector.clients(server);
    let messages = done
        .into_iter()
        .map(|(id, result, remove)| {
            let name = clients.iter().find(|c| c.id == id).map_or(id, |c| c.name);
            message(name, result, remove)
        })
        .collect();
    (messages, clients)
}

/// The installed apps that are not connected yet.
fn connect_all(clients: &[ClientStatus]) -> Vec<&'static str> {
    clients
        .iter()
        .filter(|c| c.detected && !c.connected)
        .map(|c| c.id)
        .collect()
}

/// An app's state in the list; without our own server path an entry cannot be told apart.
fn state(c: &ClientStatus, server_known: bool) -> &'static str {
    match (c.detected, c.connected, c.points_here || !server_known) {
        (false, ..) => "Not installed",
        (true, false, _) => "Not connected",
        (true, true, true) => "Connected",
        (true, true, false) => "Connected to another Statscraft copy",
    }
}

fn message(name: &str, result: Result<Outcome, String>, remove: bool) -> Result<String, String> {
    let outcome = match result {
        Ok(outcome) => outcome,
        Err(e) if remove => return Err(format!("Could not remove Statscraft from {name}: {e}.")),
        Err(e) => return Err(format!("Could not connect {name}: {e}.")),
    };
    let done = match (outcome.changed, remove) {
        (false, false) => return Ok(format!("{name} was already connected.")),
        (false, true) => return Ok(format!("{name} was not connected.")),
        (true, false) => "connected",
        (true, true) => "Statscraft removed",
    };
    Ok(match outcome.restart {
        Some(restart) => format!("{name}: {done}. To finish, {restart}."),
        None => format!("{name}: {done}."),
    })
}

/// The command-line program of this copy of Statscraft, named the way `statscraft connect` names
/// itself, so the two agree on what is connected.
fn this_server() -> Result<PathBuf, String> {
    let exe = std::env::current_exe()
        .and_then(|e| e.canonicalize())
        .map_err(|e| format!("Could not find this program's folder: {e}."))?;
    let server = server_next_to(&exe).ok_or_else(|| {
        format!(
            "The Statscraft command-line program, which the AI apps start, was not found next to \
             this window in {}. Install Statscraft from a release package, which has both.",
            exe.parent().unwrap_or(&exe).display()
        )
    })?;
    server
        .canonicalize()
        .map(plain_path)
        .map_err(|e| format!("Could not read {}: {e}.", server.display()))
}

/// Where the packages put the command line: next to the window (Windows, Linux, a build), or on
/// macOS next to Statscraft.app, whose window program is in Contents/MacOS.
fn server_next_to(window: &Path) -> Option<PathBuf> {
    let dir = window.parent()?;
    let name = format!("statscraft{}", std::env::consts::EXE_SUFFIX);
    let beside_app = dir
        .ends_with("Contents/MacOS")
        .then(|| dir.parent()?.parent())
        .flatten()
        .filter(|app| app.extension().is_some_and(|e| e == "app"))
        .and_then(Path::parent);
    [Some(dir), beside_app]
        .into_iter()
        .flatten()
        .map(|folder| folder.join(&name))
        .find(|p| p.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;
    use statscraft_connect::{Os, Places};
    use std::fs;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn temp() -> PathBuf {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "statscraft-gui-connect-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) -> PathBuf {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, "").unwrap();
        path.to_path_buf()
    }

    fn cli(dir: &Path) -> PathBuf {
        dir.join(format!("statscraft{}", std::env::consts::EXE_SUFFIX))
    }

    #[test]
    fn server_in_the_same_folder() {
        // the Linux tarball and .deb (bin/), the Windows installer and ZIP, a cargo build
        let bin = temp().join("bin");
        let window = touch(&bin.join("statscraft-gui"));
        assert_eq!(server_next_to(&window), None);
        let server = touch(&cli(&bin));
        assert_eq!(server_next_to(&window), Some(server));
    }

    #[test]
    fn server_next_to_the_mac_bundle() {
        let package = temp().join("statscraft-0.9.0-macos-universal");
        let window = touch(&package.join("Statscraft.app/Contents/MacOS/statscraft-gui"));
        assert_eq!(server_next_to(&window), None);
        let server = touch(&cli(&package));
        assert_eq!(server_next_to(&window), Some(server));
        // a Contents/MacOS that is not in a bundle looks only in its own folder
        let loose = temp();
        touch(&cli(&loose));
        let window = touch(&loose.join("Contents/MacOS/statscraft-gui"));
        assert_eq!(server_next_to(&window), None);
    }

    fn status(detected: bool, connected: bool, points_here: bool) -> ClientStatus {
        ClientStatus {
            id: "cursor",
            name: "Cursor",
            detected,
            configs: Vec::new(),
            connected,
            points_here,
            problem: None,
        }
    }

    #[test]
    fn states() {
        assert_eq!(state(&status(false, false, false), true), "Not installed");
        assert_eq!(state(&status(true, false, false), true), "Not connected");
        assert_eq!(state(&status(true, true, true), true), "Connected");
        assert_eq!(
            state(&status(true, true, false), true),
            "Connected to another Statscraft copy"
        );
        assert_eq!(state(&status(true, true, false), false), "Connected");
    }

    #[test]
    fn connect_all_takes_installed_apps_without_an_entry() {
        let mut clients = vec![
            status(false, false, false),
            status(true, false, false),
            status(true, true, true),
            status(true, true, false),
        ];
        clients[1].id = "gemini";
        assert_eq!(connect_all(&clients), ["gemini"]);
    }

    fn outcome(changed: bool) -> Outcome {
        Outcome {
            id: "cursor",
            name: "Cursor",
            changed,
            files: Vec::new(),
            backups: Vec::new(),
            command: None,
            restart: changed.then_some("quit Cursor and open it again"),
        }
    }

    #[test]
    fn messages() {
        assert_eq!(
            message("Cursor", Ok(outcome(true)), false),
            Ok("Cursor: connected. To finish, quit Cursor and open it again.".into())
        );
        assert_eq!(
            message("Cursor", Ok(outcome(true)), true),
            Ok("Cursor: Statscraft removed. To finish, quit Cursor and open it again.".into())
        );
        assert_eq!(
            message("Cursor", Ok(outcome(false)), false),
            Ok("Cursor was already connected.".into())
        );
        assert_eq!(
            message("Cursor", Err("mcp.json is not plain JSON".into()), false),
            Err("Could not connect Cursor: mcp.json is not plain JSON.".into())
        );
    }

    #[test]
    fn work_connects_and_removes_in_a_fake_home() {
        let home = temp();
        fs::create_dir_all(home.join(".cursor")).unwrap();
        fs::create_dir_all(home.join(".gemini")).unwrap();
        let places = Places {
            os: Os::Linux,
            home: home.clone(),
            appdata: None,
            local_appdata: None,
            xdg_config: None,
            codex_home: None,
            copilot_home: None,
            claude_config_dir: None,
            path: Vec::new(),
        };
        let connector = Connector::new(
            places,
            Box::new(|program, _| panic!("no command may run, asked for {}", program.display())),
        );
        let server = Path::new("/opt/statscraft/bin/statscraft");
        let waiting = connect_all(&connector.clients(server));
        assert_eq!(waiting, ["cursor", "gemini"]);
        let (messages, clients) = work(&connector, server, &Job::Connect(waiting));
        assert_eq!(messages.len(), 2);
        assert!(
            messages[0]
                .as_ref()
                .unwrap()
                .starts_with("Cursor: connected.")
        );
        assert!(home.join(".cursor/mcp.json").is_file());
        assert!(connect_all(&clients).is_empty());
        let (messages, clients) = work(&connector, server, &Job::Remove("gemini"));
        assert_eq!(
            messages[0].as_ref().unwrap(),
            "Gemini CLI: Statscraft removed. To finish, start a new Gemini CLI session."
        );
        assert_eq!(connect_all(&clients), ["gemini"]);
    }
}
