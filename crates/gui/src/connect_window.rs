//! Edit > Settings > AI agents: `statscraft connect` with clicks. It adds the MCP server, the
//! command-line program that ships next to the window, to the AI apps on this computer, through
//! the same library as the command line. Claude Desktop gets the extension (.mcpb) of this
//! release instead, which does not depend on its settings file.

use eframe::egui::{self, RichText, Ui};
use statscraft_connect::{ClientStatus, Connector, DOCS, Outcome, entry_in, note, plain_path};
use statscraft_update::{Client, DOWNLOADS, Release, bundle, current};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;

/// What a click asks for; it runs on a worker thread, as Claude Code's command and the download
/// can take a while.
enum Job {
    Connect(Vec<&'static str>),
    Remove(&'static str),
    /// Download Claude Desktop's extension and open it.
    Install,
}

/// One line per app about what an action did; an error is Err.
type Messages = Vec<Result<String, String>>;

/// The id of the app that gets the extension rather than a config entry.
const DESKTOP: &str = "claude-desktop";

#[derive(Default)]
pub struct Connect {
    /// The command-line program the entries start, or why there is none; None until the section
    /// is first shown.
    server: Option<Result<PathBuf, String>>,
    clients: Vec<ClientStatus>,
    messages: Messages,
    /// The app whose files Check read last, with a line per file.
    checked: Option<(&'static str, Vec<String>)>,
    /// The action running now, with its result to come.
    running: Option<mpsc::Receiver<(Messages, Vec<ClientStatus>)>>,
}

impl Connect {
    /// A fresh look at the apps, each time the section opens.
    pub fn refresh(&mut self) {
        self.messages.clear();
        self.checked = None;
        let server = this_server();
        self.clients = Connector::from_env().clients(server.as_deref().unwrap_or(Path::new("")));
        self.server = Some(server);
    }

    /// The section in the Settings dialog.
    pub fn ui(&mut self, ui: &mut Ui) {
        if self.server.is_none() {
            self.refresh();
        }
        if let Some(Ok((messages, clients))) = self.running.as_ref().map(|r| r.try_recv()) {
            self.messages = messages;
            self.clients = clients;
            self.running = None;
        }
        if let Some(job) = self.body(ui) {
            self.start(job, ui.ctx().clone());
        }
    }

    fn body(&mut self, ui: &mut Ui) -> Option<Job> {
        let mut job = None;
        ui.label(
            "Statscraft can work as a set of tools inside your AI app. Your data stay on this \
             computer, but whatever the app reads from them goes to its AI provider.",
        );
        ui.add_space(8.0);
        let server = match &self.server {
            Some(Ok(server)) => Some(server.clone()),
            Some(Err(e)) => {
                ui.colored_label(ui.visuals().error_fg_color, e);
                ui.add_space(8.0);
                None
            }
            None => None,
        };
        let server_ok = server.is_some();
        let idle = self.running.is_none();
        let mut check = None;
        egui::ScrollArea::vertical()
            .max_height(440.0)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for c in &self.clients {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(c.name).strong());
                        ui.weak(state(c, server_ok));
                    });
                    if let Some(note) = note(c.id) {
                        ui.weak(note);
                    }
                    ui.horizontal_wrapped(|ui| {
                        let here = c.connected && c.points_here;
                        let can_connect = idle && server_ok && c.detected;
                        if c.id == DESKTOP {
                            if ui
                                .add_enabled(idle, egui::Button::new("Install extension"))
                                .on_hover_text(
                                    "Downloads the Statscraft extension of this version, checks \
                                     it and opens it. Claude Desktop then asks to install it.",
                                )
                                .clicked()
                            {
                                job = Some(Job::Install);
                            }
                            if !here
                                && ui
                                    .add_enabled(
                                        can_connect,
                                        egui::Button::new("Connect via config file"),
                                    )
                                    .on_hover_text(
                                        "Writes the entry into Claude Desktop's settings file \
                                         instead. Quit Claude Desktop completely first.",
                                    )
                                    .clicked()
                            {
                                job = Some(Job::Connect(vec![c.id]));
                            }
                        } else if !here
                            && ui
                                .add_enabled(can_connect, egui::Button::new("Connect"))
                                .clicked()
                        {
                            job = Some(Job::Connect(vec![c.id]));
                        }
                        if c.connected
                            && ui
                                .add_enabled(idle && server_ok, egui::Button::new("Remove"))
                                .clicked()
                        {
                            job = Some(Job::Remove(c.id));
                        }
                        if ui
                            .button("Check")
                            .on_hover_text(
                                "Reads the settings files again and shows whether the \
                                 Statscraft entry is in them now.",
                            )
                            .clicked()
                        {
                            check = Some(c.id);
                        }
                    });
                    if let Some(problem) = &c.problem {
                        ui.colored_label(ui.visuals().error_fg_color, problem);
                    }
                    if let Some((_, lines)) = self.checked.as_ref().filter(|(id, _)| *id == c.id) {
                        for line in lines {
                            ui.weak(line);
                        }
                    }
                    ui.add_space(10.0);
                }
            });
        if let Some(id) = check {
            let server = server.unwrap_or_default();
            self.clients = Connector::from_env().clients(&server);
            if let Some(c) = self.clients.iter().find(|c| c.id == id) {
                self.checked = Some((id, check_lines(c)));
            }
        }
        let waiting = connect_all(&self.clients);
        ui.horizontal(|ui| {
            if ui
                .add_enabled(
                    idle && server_ok && !waiting.is_empty(),
                    egui::Button::new("Connect all"),
                )
                .on_hover_text(
                    "Connects every installed app that is not connected yet. Claude Desktop is \
                     left to its own buttons.",
                )
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
            ui.label("To connect an app by hand, see");
            ui.hyperlink_to("Connect your AI agent", DOCS);
        });
        job
    }

    fn start(&mut self, job: Job, ctx: egui::Context) {
        let server = match &self.server {
            Some(Ok(server)) => server.clone(),
            _ => PathBuf::new(),
        };
        let (sender, receiver) = mpsc::channel();
        self.running = Some(receiver);
        self.messages.clear();
        self.checked = None;
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
        Job::Install => {
            let clients = connector.clients(server);
            let mut messages = vec![install()];
            let desktop = clients.iter().find(|c| c.id == DESKTOP);
            if messages[0].is_ok() && desktop.is_some_and(|c| c.connected) {
                messages.push(Ok(
                    "Claude Desktop also has the entry in its settings file. \
                     Remove takes it out, so that the tools do not show up twice."
                        .into(),
                ));
            }
            return (messages, clients);
        }
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

/// The installed apps that are not connected yet; one whose settings file cannot be read would
/// only fail again.
fn connect_all(clients: &[ClientStatus]) -> Vec<&'static str> {
    clients
        .iter()
        .filter(|c| c.detected && !c.connected && c.problem.is_none() && c.id != DESKTOP)
        .map(|c| c.id)
        .collect()
}

/// An app's state in the list; without our own server path an entry cannot be told apart.
fn state(c: &ClientStatus, server_known: bool) -> &'static str {
    if c.problem.is_some() {
        return "Settings file unreadable";
    }
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
    let files: Vec<String> = outcome
        .files
        .iter()
        .map(|f| f.display().to_string())
        .collect();
    let files = files.join(" and ");
    let mut text = match (outcome.changed, remove, files.is_empty()) {
        (false, false, _) => return Ok(format!("{name} was already connected.")),
        (false, true, _) => return Ok(format!("{name} was not connected.")),
        (true, false, true) => format!("{name}: connected."),
        (true, false, false) => format!("{name}: connected in {files}."),
        (true, true, true) => format!("{name}: Statscraft removed."),
        (true, true, false) => format!("{name}: Statscraft removed from {files}."),
    };
    if let Some(restart) = outcome.restart {
        text.push_str(&format!(" To finish, {restart}."));
    }
    if let Some(warning) = outcome.warning {
        text.push_str(&format!(" {warning}."));
    }
    Ok(text)
}

/// What Check shows for one app: each settings file and whether our entry is in it now.
fn check_lines(c: &ClientStatus) -> Vec<String> {
    if c.configs.is_empty() {
        return vec![format!(
            "{} keeps its own list of servers; `claude mcp list` in a terminal shows it.",
            c.name
        )];
    }
    c.configs
        .iter()
        .map(|path| match entry_in(path) {
            Ok(Some(command)) => format!(
                "{}: has the Statscraft entry, which starts {command}",
                path.display()
            ),
            Ok(None) if path.is_file() => format!("{}: no Statscraft entry", path.display()),
            Ok(None) => format!("{}: no such file", path.display()),
            Err(e) => e,
        })
        .collect()
}

/// Downloads Claude Desktop's extension of this release for this system, checked against the
/// release's SHA256SUMS, and opens it, which makes Claude Desktop offer to install it.
fn install() -> Result<String, String> {
    let version = current();
    let name = bundle(&version, std::env::consts::OS, std::env::consts::ARCH)
        .ok_or("There is no Claude Desktop extension for this system.")?;
    let release = Release::files(DOWNLOADS, version, &[&name]);
    let dir = downloads(std::env::home_dir());
    let file = Client::new()
        .download(&release, &name, &dir)
        .map_err(|e| format!("Could not get the Claude Desktop extension: {e}."))?;
    let drag = "drag it onto Claude Desktop's Settings, Extensions";
    let mut child = opener(&file).spawn().map_err(|e| {
        format!(
            "Saved {}, but could not open it ({e}); {drag}.",
            file.display()
        )
    })?;
    // reaps the opener, which ends once it has handed the file over
    std::thread::spawn(move || child.wait());
    Ok(format!(
        "Saved {} and opened it. Claude Desktop shows the extension: click Install. If nothing \
         opens, {drag}.",
        file.display()
    ))
}

/// The user's Downloads folder, where the extension can be installed from again; else a folder
/// of our own in the temporary folder.
fn downloads(home: Option<PathBuf>) -> PathBuf {
    home.map(|h| h.join("Downloads"))
        .filter(|d| d.is_dir())
        .unwrap_or_else(|| std::env::temp_dir().join("statscraft"))
}

/// Opens a file with the program the system has for its kind, as a double click does. On
/// Windows, Explorer does that without a shell that would read characters of the path.
fn opener(file: &Path) -> Command {
    let program = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    let mut command = Command::new(program);
    command.arg(file);
    command
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
        let mut broken = status(true, false, false);
        broken.problem = Some("not plain JSON".into());
        assert_eq!(state(&broken, true), "Settings file unreadable");
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
        clients[1].problem = Some("not plain JSON".into());
        assert!(connect_all(&clients).is_empty());
        // Claude Desktop gets the extension from its own button, not a config entry
        clients[1].problem = None;
        clients[1].id = DESKTOP;
        assert!(connect_all(&clients).is_empty());
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
            warning: None,
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
        // where it was written, both files when there are two, and a warning at the end
        let mut two = outcome(true);
        two.files = vec![PathBuf::from("/a/c.json"), PathBuf::from("/b/c.json")];
        two.warning = Some("Quit it first");
        assert_eq!(
            message("Cursor", Ok(two.clone()), false),
            Ok(
                "Cursor: connected in /a/c.json and /b/c.json. To finish, quit Cursor and open \
                it again. Quit it first."
                    .into()
            )
        );
        assert!(
            message("Cursor", Ok(two), true)
                .unwrap()
                .starts_with("Cursor: Statscraft removed from /a/c.json and /b/c.json.")
        );
    }

    #[test]
    fn check_reads_each_file_again() {
        let dir = temp();
        let mut c = status(true, true, true);
        c.configs = vec![dir.join("a.json"), dir.join("b.json"), dir.join("c.json")];
        fs::write(
            &c.configs[0],
            r#"{"mcpServers":{"statscraft":{"command":"/opt/statscraft"}}}"#,
        )
        .unwrap();
        fs::write(&c.configs[1], r#"{"mcpServers":{}}"#).unwrap();
        let lines = check_lines(&c);
        assert_eq!(
            lines,
            [
                format!(
                    "{}: has the Statscraft entry, which starts /opt/statscraft",
                    c.configs[0].display()
                ),
                format!("{}: no Statscraft entry", c.configs[1].display()),
                format!("{}: no such file", c.configs[2].display()),
            ]
        );
        // an app that keeps the entry in its own file, out of the list
        fs::write(&c.configs[0], r#"{"mcpServers":{}}"#).unwrap();
        assert!(check_lines(&c)[0].ends_with("no Statscraft entry"));
        c.configs.clear();
        assert!(check_lines(&c)[0].contains("claude mcp list"));
    }

    #[test]
    fn the_extension_goes_to_downloads_and_opens_with_the_system() {
        let home = temp();
        let fallback = std::env::temp_dir().join("statscraft");
        assert_eq!(downloads(Some(home.clone())), fallback);
        assert_eq!(downloads(None), fallback);
        fs::create_dir_all(home.join("Downloads")).unwrap();
        assert_eq!(downloads(Some(home.clone())), home.join("Downloads"));
        // only built here, never started: no real app opens in a test
        let file = home.join("Downloads/statscraft-1.0.0-linux-x86_64.mcpb");
        let command = opener(&file);
        let expected = if cfg!(windows) {
            "explorer"
        } else if cfg!(target_os = "macos") {
            "open"
        } else {
            "xdg-open"
        };
        assert_eq!(command.get_program(), expected);
        assert_eq!(command.get_args().collect::<Vec<_>>(), [file.as_os_str()]);
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
                .starts_with("Cursor: connected in ")
        );
        assert!(home.join(".cursor/mcp.json").is_file());
        assert!(connect_all(&clients).is_empty());
        let (messages, clients) = work(&connector, server, &Job::Remove("gemini"));
        assert_eq!(
            messages[0].as_ref().unwrap(),
            &format!(
                "Gemini CLI: Statscraft removed from {}. To finish, start a new Gemini CLI session.",
                fs::canonicalize(home.join(".gemini/settings.json"))
                    .unwrap()
                    .display()
            )
        );
        assert_eq!(connect_all(&clients), ["gemini"]);
    }
}
