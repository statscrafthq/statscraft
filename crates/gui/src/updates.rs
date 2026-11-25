//! Updates in the window: a check at most once a day in the background, or from Help, and a
//! small bar under the menu when there is something to say.

use crate::app::file_name;
use crate::i18n::{t, t_args};
use crate::settings::Settings;
use eframe::egui::{self, Ui};
use statscraft_update::{Client, Done, Install, Release, current};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

pub const DAY: u64 = 24 * 60 * 60;

/// A result from a worker thread.
enum Event {
    /// The latest release, and whether the user asked for the check.
    Checked(Result<Option<Release>, String>, bool),
    Updated(Result<(Done, String), String>),
}

enum Bar {
    Hidden,
    Checking,
    Available(Release),
    Downloading(String),
    /// The programs in this folder are the new version now.
    Restart(PathBuf, String),
    Saved {
        file: PathBuf,
        command: String,
    },
    Message(Result<String, String>),
}

/// What a click in the bar asks for.
enum Click {
    Hide,
    Update(Release),
    Restart(PathBuf),
}

pub struct Updates {
    bar: Bar,
    sender: mpsc::Sender<Event>,
    events: mpsc::Receiver<Event>,
    ctx: egui::Context,
    install: Install,
    /// What starts once this window has closed: the new window after Restart now, or the
    /// Windows installer, which cannot replace a window that is still open.
    restart: Option<std::process::Command>,
    /// The program should end, for the installer or Restart now (`take_quit`).
    quit: bool,
}

pub fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Updates {
    /// Checks in the background when the last check is a day old, unless checks are turned off
    /// (Edit > Settings, Updates). They are on by default and say nothing unless
    /// there is an update.
    pub fn new(ctx: &egui::Context, settings: &mut Settings) -> Self {
        // the Windows installer of the last update cannot delete itself, nor a running program
        // that a portable update moved aside
        statscraft_update::clean();
        let (sender, events) = mpsc::channel();
        let mut updates = Updates {
            bar: Bar::Hidden,
            sender,
            events,
            ctx: ctx.clone(),
            install: Install::detect(),
            restart: None,
            quit: false,
        };
        let due = settings
            .update_checked
            .is_none_or(|t| now().abs_diff(t) >= DAY);
        if settings.updates != Some(false) && due {
            settings.update_checked = Some(now());
            let _ = settings.save();
            updates.check(false);
        }
        updates
    }

    /// Asks GitHub on a worker thread; `manual` shows the outcome even when there is no update.
    fn check(&mut self, manual: bool) {
        if manual {
            self.bar = Bar::Checking;
        }
        let (sender, ctx) = (self.sender.clone(), self.ctx.clone());
        std::thread::spawn(move || {
            let _ = sender.send(Event::Checked(Client::new().latest(), manual));
            ctx.request_repaint();
        });
    }

    fn update(&mut self, release: Release) {
        let version = release.version.to_string();
        self.bar = Bar::Downloading(version.clone());
        let (sender, ctx, install) = (self.sender.clone(), self.ctx.clone(), self.install.clone());
        std::thread::spawn(move || {
            let done = Client::new()
                .update(&release, &install)
                .map(install_package);
            let _ = sender.send(Event::Updated(done.map(|d| (d, version))));
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self) {
        while let Ok(event) = self.events.try_recv() {
            self.bar = match event {
                Event::Checked(Ok(Some(release)), _) if release.is_newer() => {
                    Bar::Available(release)
                }
                Event::Checked(Ok(latest), true) => Bar::Message(Ok(match latest {
                    Some(_) => t_args(
                        "update-up-to-date",
                        &[("version", current().to_string().into())],
                    ),
                    None => t("update-no-release").into(),
                })),
                Event::Checked(Err(e), true) => Bar::Message(Err(t_args(
                    "update-check-failed",
                    &[("error", e.to_string().into())],
                ))),
                // an automatic check says nothing unless there is an update
                Event::Checked(_, false) => continue,
                Event::Updated(Ok((Done::Installer(file), _))) => {
                    // the installer starts on exit and opens the window again; the quit asks
                    // first when work is unsaved, and a cancelled quit drops the installer
                    self.restart = Some(statscraft_update::installer(&file, true));
                    self.quit = true;
                    Bar::Hidden
                }
                Event::Updated(Ok((Done::Replaced(bin), version))) => Bar::Restart(bin, version),
                Event::Updated(Ok((Done::Saved { file, command }, _))) => {
                    Bar::Saved { file, command }
                }
                Event::Updated(Err(e)) => Bar::Message(Err(t_args(
                    "update-failed",
                    &[("error", e.to_string().into())],
                ))),
            };
        }
    }

    pub fn visible(&self) -> bool {
        !matches!(self.bar, Bar::Hidden)
    }

    /// The bar under the menu.
    pub fn bar(&mut self, ui: &mut Ui) {
        let mut click = None;
        ui.horizontal_wrapped(|ui| match &self.bar {
            Bar::Hidden => {}
            Bar::Checking => {
                ui.spinner();
                ui.label(t("update-checking"));
            }
            Bar::Available(release) => {
                let args = [
                    ("version", release.version.to_string().into()),
                    ("current", current().to_string().into()),
                ];
                ui.label(t_args("update-available", &args));
                ui.hyperlink_to(t("update-release-notes"), &release.notes);
                match &self.install {
                    Install::Unmanaged(dir) => {
                        let folder = [("folder", dir.display().to_string().into())];
                        ui.weak(t_args("update-unmanaged", &folder));
                    }
                    // no replacing inside the bundle (see Client::update): a link to the package
                    Install::Mac(_) => {
                        let name = self.install.asset(&release.version).unwrap_or_default();
                        ui.hyperlink_to(
                            t("update-download"),
                            release.url(&name).unwrap_or(&release.notes),
                        );
                        ui.weak(t("update-mac-replace"));
                    }
                    install => {
                        if ui
                            .button(t("update-now"))
                            .on_hover_text(update_hint(install))
                            .clicked()
                        {
                            click = Some(Click::Update(release.clone()));
                        }
                        if *install == Install::Windows {
                            ui.weak(t("update-windows-restart"));
                        }
                    }
                }
                if ui.button(t("update-later")).clicked() {
                    click = Some(Click::Hide);
                }
            }
            Bar::Downloading(version) => {
                ui.spinner();
                ui.label(t_args(
                    "update-downloading",
                    &[("version", version.as_str().into())],
                ));
            }
            Bar::Restart(bin, version) => {
                ui.label(t_args(
                    "update-installed",
                    &[("version", version.as_str().into())],
                ));
                if ui.button(t("update-restart-now")).clicked() {
                    click = Some(Click::Restart(bin.clone()));
                }
                if ui.button(t("update-later")).clicked() {
                    click = Some(Click::Hide);
                }
            }
            Bar::Saved { file, command } => {
                let file = [("file", file.display().to_string().into())];
                ui.label(t_args("update-saved", &file));
                ui.code(command);
                if ui.button(t("update-copy-command")).clicked() {
                    ui.ctx().copy_text(command.clone());
                }
                if ui.button(t("button-ok")).clicked() {
                    click = Some(Click::Hide);
                }
            }
            Bar::Message(message) => {
                match message {
                    Ok(text) => ui.label(text),
                    Err(text) => ui.colored_label(ui.visuals().error_fg_color, text),
                };
                if ui.button(t("button-ok")).clicked() {
                    click = Some(Click::Hide);
                }
            }
        });
        match click {
            Some(Click::Hide) => self.bar = Bar::Hidden,
            Some(Click::Update(release)) => self.update(release),
            Some(Click::Restart(bin)) => {
                let gui = format!("statscraft-gui{}", std::env::consts::EXE_SUFFIX);
                let program = bin.join(gui);
                if program.is_file() {
                    // the quit asks about unsaved work first; the new window starts on exit
                    self.restart = Some(std::process::Command::new(program));
                    self.quit = true;
                    ui.ctx().request_repaint();
                } else {
                    let missing = t_args(
                        "core-cannot-restart",
                        &[("file", file_name(&program).into())],
                    );
                    self.bar = Bar::Message(Err(missing));
                }
            }
            None => {}
        }
    }

    /// Whether the program should end now, for the installer or Restart now; asked once.
    pub fn take_quit(&mut self) -> bool {
        std::mem::take(&mut self.quit)
    }

    /// The close for an update was cancelled: no new window and no installer.
    pub fn cancel_restart(&mut self) {
        self.restart = None;
    }

    /// Starts the new window or the installer when the update closed this one.
    pub fn restart(&mut self) {
        if let Some(mut command) = self.restart.take()
            && let Err(e) = command.spawn()
        {
            eprintln!("cannot start {}: {e}", command.get_program().display());
        }
    }

    /// The update item of the Help menu.
    pub fn menu_items(&mut self, ui: &mut Ui) {
        let busy = matches!(self.bar, Bar::Checking | Bar::Downloading(_));
        if ui
            .add_enabled(
                !busy,
                egui::Button::new(crate::i18n::t("menu-check-for-updates")),
            )
            .clicked()
        {
            self.check(true);
        }
    }
}

/// Installs a saved .deb through pkexec, which asks for the password in a window of its own.
/// Without pkexec, or when the install fails or is cancelled, the bar shows the command instead.
fn install_package(done: Done) -> Done {
    let Done::Saved { file, .. } = &done else {
        return done;
    };
    let installed = std::process::Command::new("pkexec")
        .args(["apt-get", "install", "-y"])
        .arg(file)
        .stdin(std::process::Stdio::null())
        .status()
        .is_ok_and(|s| s.success());
    match installed {
        true => Done::Replaced(PathBuf::from("/usr/bin")),
        false => done,
    }
}

fn update_hint(install: &Install) -> String {
    match install {
        Install::Windows => t("update-hint-windows").into(),
        Install::Portable(bin) | Install::Tarball(bin) => t_args(
            "update-hint-programs",
            &[("folder", bin.display().to_string().into())],
        ),
        Install::Deb => t("update-hint-deb").into(),
        Install::Mac(_) | Install::Unmanaged(_) => String::new(),
    }
}
