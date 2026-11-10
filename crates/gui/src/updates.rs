//! Updates in the window: a check at most once a day in the background, or from Help, and a
//! small bar under the menu when there is something to say.

use crate::settings::Settings;
use eframe::egui::{self, Ui};
use statscraft_update::{Client, Done, Install, Release, current};
use std::path::PathBuf;
use std::sync::mpsc;
use std::time::{SystemTime, UNIX_EPOCH};

const DAY: u64 = 24 * 60 * 60;

/// A result from a worker thread.
enum Event {
    /// The latest release, and whether the user asked for the check.
    Checked(Result<Option<Release>, String>, bool),
    Updated(Result<(Done, String), String>),
}

enum Bar {
    Hidden,
    /// The first start: checks exist, and how to turn them off.
    Notice,
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
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

impl Updates {
    /// On the first start only says that checks exist; later checks when the last check is a
    /// day old and checks are on.
    pub fn new(ctx: &egui::Context, settings: &mut Settings) -> Self {
        // the Windows installer of the last update cannot delete itself
        statscraft_update::clean();
        let (sender, events) = mpsc::channel();
        let mut updates = Updates {
            bar: Bar::Hidden,
            sender,
            events,
            ctx: ctx.clone(),
            install: Install::detect(),
        };
        match settings.updates {
            None => {
                updates.bar = Bar::Notice;
                settings.updates = Some(true);
                let _ = settings.save();
            }
            Some(true)
                if settings
                    .update_checked
                    .is_none_or(|t| now().abs_diff(t) >= DAY) =>
            {
                settings.update_checked = Some(now());
                let _ = settings.save();
                updates.check(false);
            }
            _ => {}
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
            let done = Client::new().update(&release, &install, true);
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
                    Some(_) => format!("StatsCraft {} is up to date.", current()),
                    None => "No StatsCraft release has been published yet.".into(),
                })),
                Event::Checked(Err(e), true) => {
                    Bar::Message(Err(format!("The update check failed: {e}")))
                }
                // an automatic check says nothing unless there is an update
                Event::Checked(_, false) => continue,
                Event::Updated(Ok((Done::InstallerStarted, _))) => {
                    // the installer replaces the programs once the window is gone, then
                    // starts it again
                    self.ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    Bar::Hidden
                }
                Event::Updated(Ok((Done::Replaced(bin), version))) => Bar::Restart(bin, version),
                Event::Updated(Ok((Done::Saved { file, command }, _))) => {
                    Bar::Saved { file, command }
                }
                Event::Updated(Err(e)) => Bar::Message(Err(format!("The update failed: {e}"))),
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
            Bar::Notice => {
                ui.label(
                    "StatsCraft asks GitHub once a day whether a new version is out. Nothing \
                     about you or your data is sent. To turn this off, untick Help, Check for \
                     updates automatically.",
                );
                if ui.button("OK").clicked() {
                    click = Some(Click::Hide);
                }
            }
            Bar::Checking => {
                ui.spinner();
                ui.label("Checking for updates");
            }
            Bar::Available(release) => {
                ui.label(format!(
                    "StatsCraft {} is available (you have {}).",
                    release.version,
                    current()
                ));
                ui.hyperlink_to("Release notes", &release.notes);
                match &self.install {
                    Install::Unmanaged(_) => {
                        ui.weak("Install it the way you installed this copy.");
                    }
                    install => {
                        if ui
                            .button("Update now")
                            .on_hover_text(update_hint(install))
                            .clicked()
                        {
                            click = Some(Click::Update(release.clone()));
                        }
                        if *install == Install::Windows {
                            ui.weak("The window closes and opens again: save your work first.");
                        }
                    }
                }
                if ui.button("Later").clicked() {
                    click = Some(Click::Hide);
                }
            }
            Bar::Downloading(version) => {
                ui.spinner();
                ui.label(format!("Downloading and checking StatsCraft {version}"));
            }
            Bar::Restart(bin, version) => {
                ui.label(format!(
                    "StatsCraft {version} is installed. Restart the window to use it; save your \
                     work first."
                ));
                if ui.button("Restart now").clicked() {
                    click = Some(Click::Restart(bin.clone()));
                }
                if ui.button("Later").clicked() {
                    click = Some(Click::Hide);
                }
            }
            Bar::Saved { file, command } => {
                ui.label(format!(
                    "The new package is checked and saved as {}. Install it in a terminal with",
                    file.display()
                ));
                ui.code(command);
                if ui.button("Copy command").clicked() {
                    ui.ctx().copy_text(command.clone());
                }
                if ui.button("OK").clicked() {
                    click = Some(Click::Hide);
                }
            }
            Bar::Message(message) => {
                match message {
                    Ok(text) => ui.label(text),
                    Err(text) => ui.colored_label(ui.visuals().error_fg_color, text),
                };
                if ui.button("OK").clicked() {
                    click = Some(Click::Hide);
                }
            }
        });
        match click {
            Some(Click::Hide) => self.bar = Bar::Hidden,
            Some(Click::Update(release)) => self.update(release),
            Some(Click::Restart(bin)) => {
                match std::process::Command::new(bin.join("statscraft-gui")).spawn() {
                    Ok(_) => ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close),
                    Err(e) => self.bar = Bar::Message(Err(format!("cannot restart: {e}"))),
                }
            }
            None => {}
        }
    }

    /// The Help menu.
    pub fn menu(&mut self, ui: &mut Ui, settings: &mut Settings) {
        ui.menu_button("Help", |ui| {
            let busy = matches!(self.bar, Bar::Checking | Bar::Downloading(_));
            if ui
                .add_enabled(!busy, egui::Button::new("Check for updates"))
                .clicked()
            {
                self.check(true);
            }
            let mut on = settings.updates != Some(false);
            if ui
                .checkbox(&mut on, "Check for updates automatically")
                .changed()
            {
                settings.updates = Some(on);
                let _ = settings.save();
            }
            ui.separator();
            ui.weak(format!("StatsCraft {}", current()));
        });
    }
}

fn update_hint(install: &Install) -> String {
    match install {
        Install::Windows => "Downloads the installer and checks it against the release's \
                             checksums. The window then closes, the installer updates StatsCraft \
                             and opens the window again."
            .into(),
        Install::Tarball(bin) => format!(
            "Downloads the new programs, checks them against the release's checksums and \
             replaces the ones in {}.",
            bin.display()
        ),
        Install::Deb => "Downloads the package to your Downloads folder and checks it against \
                         the release's checksums. Installing it takes one command with sudo."
            .into(),
        Install::Unmanaged(_) => String::new(),
    }
}
