//! Update check and update for Statscraft, shared by the command line and the window.
//!
//! The latest published release of the GitHub repository is the source of truth. The only
//! requests are the one GitHub API call for that release and, on update, its `SHA256SUMS` and
//! the one package for this installation. They carry the User-Agent `statscraft/VERSION` and
//! nothing else about the user. A package is used only when its SHA-256 matches `SHA256SUMS`.

use semver::Version;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const REPO: &str = "statscrafthq/statscraft";
const API: &str = "https://api.github.com";
/// Points the updater at another server instead of GitHub; for tests.
pub const URL_VARIABLE: &str = "STATSCRAFT_UPDATE_URL";
/// The programs an installation holds.
const PROGRAMS: [&str; 2] = ["statscraft", "statscraft-gui"];

pub type Result<T> = std::result::Result<T, String>;

/// The version of the running program (the workspace version).
pub fn current() -> Version {
    Version::parse(env!("CARGO_PKG_VERSION")).expect("the package version is semver")
}

#[derive(Debug, Clone, PartialEq)]
pub struct Release {
    pub version: Version,
    /// The release page with its notes.
    pub notes: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Asset {
    pub name: String,
    #[serde(rename = "browser_download_url")]
    pub url: String,
}

impl Release {
    /// Reads a release as the GitHub API gives it; `None` for a draft or a prerelease.
    pub fn parse(json: &str) -> Result<Option<Release>> {
        #[derive(Deserialize)]
        struct Raw {
            tag_name: String,
            html_url: String,
            #[serde(default)]
            draft: bool,
            #[serde(default)]
            prerelease: bool,
            #[serde(default)]
            assets: Vec<Asset>,
        }
        let raw: Raw = serde_json::from_str(json)
            .map_err(|e| format!("unexpected reply about the latest release: {e}"))?;
        if raw.draft || raw.prerelease {
            return Ok(None);
        }
        let tag = raw.tag_name.strip_prefix('v').unwrap_or(&raw.tag_name);
        let version = Version::parse(tag)
            .map_err(|e| format!("release tag {} is not a version: {e}", raw.tag_name))?;
        Ok(Some(Release {
            version,
            notes: raw.html_url,
            assets: raw.assets,
        }))
    }

    /// True when this release is newer than the running program.
    pub fn is_newer(&self) -> bool {
        self.version > current()
    }

    /// The download address of the release file `name`.
    pub fn url(&self, name: &str) -> Result<&str> {
        self.assets
            .iter()
            .find(|a| a.name == name)
            .map(|a| a.url.as_str())
            .ok_or_else(|| format!("release {} has no file {name}", self.version))
    }
}

/// The SHA-256 that a `SHA256SUMS` file (the output of `sha256sum`) gives for `name`.
pub fn sum_of<'a>(sums: &'a str, name: &str) -> Option<&'a str> {
    sums.lines().find_map(|line| {
        let (hash, file) = line.split_once(' ')?;
        // sha256sum writes two spaces, or a space and * in binary mode
        let file = file.strip_prefix([' ', '*']).unwrap_or(file).trim_end();
        let valid = hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit());
        (valid && file == name).then_some(hash)
    })
}

/// How this copy of Statscraft was installed, which decides how it is updated.
#[derive(Debug, Clone, PartialEq)]
pub enum Install {
    /// The per-user Windows installer; its uninstaller is next to the programs.
    Windows,
    /// The portable Windows ZIP unpacked anywhere: the folder with the programs and LICENSE.txt.
    Portable(PathBuf),
    /// The tarball unpacked under a prefix such as `~/.local`: the `bin` folder with the programs.
    Tarball(PathBuf),
    /// The .deb package, in `/usr/bin`.
    Deb,
    /// The macOS app bundle, `Statscraft.app`.
    Mac(PathBuf),
    /// A build from source or a copy elsewhere, with the folder it runs from.
    Unmanaged(PathBuf),
}

impl Install {
    /// The installation of the running program.
    pub fn detect() -> Install {
        match std::env::current_exe() {
            Ok(exe) => Self::at(&exe, cfg!(windows)),
            Err(_) => Install::Unmanaged(PathBuf::new()),
        }
    }

    /// The installation that the program `exe` belongs to.
    pub fn at(exe: &Path, windows: bool) -> Install {
        let dir = exe.parent().unwrap_or(Path::new("")).to_path_buf();
        if windows {
            // Inno Setup puts unins000.exe next to the programs it installs
            if dir.join("unins000.exe").is_file() {
                return Install::Windows;
            }
            // the ZIP has LICENSE.txt next to the programs, a build folder does not
            if dir.join("LICENSE.txt").is_file() {
                return Install::Portable(dir);
            }
        } else if let Some(app) = dir
            .ends_with("Contents/MacOS")
            .then(|| dir.parent().and_then(Path::parent))
            .flatten()
            .filter(|app| app.extension().is_some_and(|e| e == "app"))
        {
            return Install::Mac(app.to_path_buf());
        } else if dir == Path::new("/usr/bin") {
            return Install::Deb;
        } else if dir.file_name().is_some_and(|n| n == "bin")
            && dir
                .parent()
                .is_some_and(|p| p.join("share/applications/statscraft.desktop").is_file())
        {
            return Install::Tarball(dir);
        }
        Install::Unmanaged(dir)
    }

    /// The release file that updates this installation.
    pub fn asset(&self, version: &Version) -> Option<String> {
        match self {
            Install::Windows => Some(format!("statscraft-{version}-windows-x86_64-setup.exe")),
            Install::Portable(_) => Some(format!("statscraft-{version}-windows-x86_64.zip")),
            Install::Tarball(_) => Some(format!("statscraft-{version}-linux-x86_64.tar.gz")),
            Install::Deb => Some(format!("statscraft_{version}_amd64.deb")),
            Install::Mac(_) => Some(format!("statscraft-{version}-macos-universal.tar.gz")),
            Install::Unmanaged(_) => None,
        }
    }
}

/// What an update did.
#[derive(Debug, Clone, PartialEq)]
pub enum Done {
    /// The Windows installer runs now; the program must exit so that it can replace it.
    InstallerStarted,
    /// The programs in this folder were replaced; the next start runs the new version.
    Replaced(PathBuf),
    /// The package was saved here; this command installs it.
    Saved { file: PathBuf, command: String },
}

/// Our folder for downloads that are used once: the Windows installer and the tarball.
fn temp_folder() -> PathBuf {
    std::env::temp_dir().join("statscraft-update")
}

/// The file name of a program on this system.
fn program(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

/// Where `replace` moves the old program `name` (with its suffix) in `bin`.
fn aside(bin: &Path, name: &str) -> PathBuf {
    bin.join(format!(".{name}.old"))
}

/// Removes what an earlier update left behind, which Windows does not let a running program
/// delete: the installer in the temporary folder, and the old programs that a portable update
/// moved aside.
pub fn clean() {
    let _ = std::fs::remove_dir_all(temp_folder());
    if let Some(bin) = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent().map(Path::to_path_buf))
    {
        for p in PROGRAMS {
            let _ = std::fs::remove_file(aside(&bin, &program(p)));
        }
    }
}

pub struct Client {
    base: String,
    agent: ureq::Agent,
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// GitHub, or the server in `STATSCRAFT_UPDATE_URL`.
    pub fn new() -> Self {
        match std::env::var(URL_VARIABLE) {
            Ok(url) if !url.is_empty() => Self::at(&url),
            _ => Self::at(API),
        }
    }

    /// A client for the API at `base`. Only an `https` base is used with TLS, and then every
    /// download, redirects included, must be `https` too.
    pub fn at(base: &str) -> Self {
        let config = ureq::Agent::config_builder()
            .https_only(base.starts_with("https://"))
            .user_agent(format!("statscraft/{}", current()))
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            .timeout_recv_body(Some(Duration::from_secs(600)))
            .build();
        Client {
            base: base.trim_end_matches('/').to_string(),
            agent: config.into(),
        }
    }

    /// The latest published release; `None` when there is none yet.
    pub fn latest(&self) -> Result<Option<Release>> {
        let url = format!("{}/repos/{REPO}/releases/latest", self.base);
        let response = self
            .agent
            .get(&url)
            .header("Accept", "application/vnd.github+json")
            .call();
        match response {
            Ok(mut response) => {
                let text = response
                    .body_mut()
                    .read_to_string()
                    .map_err(|e| format!("cannot read the latest release: {e}"))?;
                Release::parse(&text)
            }
            Err(ureq::Error::StatusCode(404)) => Ok(None),
            Err(e) => Err(format!("cannot reach {}: {e}", self.base)),
        }
    }

    /// Downloads the file `name` of the release into `dir` and checks it against the release's
    /// `SHA256SUMS`. It is written as a hidden part file and gets its name only when it matches;
    /// otherwise it is deleted.
    pub fn download(&self, release: &Release, name: &str, dir: &Path) -> Result<PathBuf> {
        let mut sums = String::new();
        self.get(release.url("SHA256SUMS")?)?
            .take(1 << 20)
            .read_to_string(&mut sums)
            .map_err(|e| format!("cannot read SHA256SUMS: {e}"))?;
        let expected = sum_of(&sums, name)
            .ok_or_else(|| {
                format!(
                    "SHA256SUMS of release {} does not list {name}",
                    release.version
                )
            })?
            .to_ascii_lowercase();
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        let part = dir.join(format!(".{name}.part"));
        let result = self.save(release.url(name)?, &part).and_then(|hash| {
            if hash == expected {
                Ok(())
            } else {
                Err(format!(
                    "{name} does not match SHA256SUMS (expected {expected}, got {hash}); nothing was changed"
                ))
            }
        });
        if let Err(e) = result {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        let file = dir.join(name);
        std::fs::rename(&part, &file).map_err(|e| format!("{}: {e}", file.display()))?;
        Ok(file)
    }

    fn get(&self, url: &str) -> Result<impl Read + use<>> {
        let response = self
            .agent
            .get(url)
            .call()
            .map_err(|e| format!("cannot download {url}: {e}"))?;
        Ok(response.into_body().into_reader())
    }

    /// Writes the body at `url` to `file` and returns its SHA-256 in hex.
    fn save(&self, url: &str, file: &Path) -> Result<String> {
        let fail = |e: std::io::Error| format!("{}: {e}", file.display());
        let mut body = self.get(url)?;
        let mut out = std::fs::File::create(file).map_err(fail)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 1 << 16];
        loop {
            let n = body
                .read(&mut buffer)
                .map_err(|e| format!("cannot download {url}: {e}"))?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            out.write_all(&buffer[..n]).map_err(fail)?;
        }
        out.sync_all().map_err(fail)?;
        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Updates this installation to the release. With `relaunch` the Windows installer starts
    /// the window again when it is done.
    pub fn update(&self, release: &Release, install: &Install, relaunch: bool) -> Result<Done> {
        let Some(name) = install.asset(&release.version) else {
            let Install::Unmanaged(dir) = install else {
                unreachable!("every other installation has a package")
            };
            return Err(format!(
                "this copy runs from {}, which is not an installation from the Statscraft \
                 installer or packages; update it the way you installed it, or install a package \
                 from {}",
                dir.display(),
                release.notes
            ));
        };
        match install {
            Install::Windows => {
                clean();
                let file = self.download(release, &name, &temp_folder())?;
                // the same per-user install: Inno Setup keeps the folder of the earlier install
                let mut setup = Command::new(&file);
                setup.args([
                    "/VERYSILENT",
                    "/SUPPRESSMSGBOXES",
                    "/NORESTART",
                    "/CLOSEAPPLICATIONS",
                ]);
                // Restart Manager restarts only programs registered with it, so the installer
                // script starts the window itself when it gets this parameter
                if relaunch {
                    setup.arg("/RELAUNCH=1");
                }
                setup
                    .spawn()
                    .map_err(|e| format!("cannot start {}: {e}", file.display()))?;
                Ok(Done::InstallerStarted)
            }
            Install::Portable(bin) | Install::Tarball(bin) => {
                clean();
                let folder = temp_folder();
                let result = self
                    .download(release, &name, &folder)
                    .and_then(|file| unpack(&file, &name, &folder))
                    .and_then(|from| replace(&from, bin));
                clean();
                result.map(|()| Done::Replaced(bin.clone()))
            }
            Install::Deb => {
                // shortcut: ~/Downloads, not the localised folder of xdg-user-dirs; read
                // user-dirs.dirs if users ask for it
                let home = std::env::var_os("HOME").ok_or("HOME is not set")?;
                let file = self.download(release, &name, &Path::new(&home).join("Downloads"))?;
                let command = format!("sudo apt install {}", file.display());
                Ok(Done::Saved { file, command })
            }
            // Replacing files inside an app bundle breaks its signature once it is signed, and
            // a bundle started from Downloads runs from a read-only copy; the window links the
            // download instead
            Install::Mac(app) => Err(format!(
                "download {} and replace {} with the Statscraft.app in it",
                release.url(&name).unwrap_or(&release.notes),
                app.display()
            )),
            Install::Unmanaged(_) => unreachable!("handled above"),
        }
    }
}

/// Unpacks the two programs of the tarball or the ZIP `file` into `folder` and returns their
/// folder. The tarball has them in `bin`, the ZIP at its top.
fn unpack(file: &Path, name: &str, folder: &Path) -> Result<PathBuf> {
    let (top, sub) = match name.strip_suffix(".zip") {
        Some(top) => (top, ""),
        None => (name.strip_suffix(".tar.gz").unwrap_or(name), "bin/"),
    };
    let members = PROGRAMS.map(|p| format!("{top}/{sub}{}", program(p)));
    // Windows 10 and later have bsdtar, which also reads ZIP files, in System32; a GNU tar that
    // comes first on the PATH (Git's) does not
    let tar = match std::env::var_os("SystemRoot") {
        Some(root) if cfg!(windows) => Path::new(&root).join("System32").join("tar.exe"),
        _ => PathBuf::from("tar"),
    };
    // without -z: both tars find the compression themselves
    let status = Command::new(tar)
        .arg("-xf")
        .arg(file)
        .arg("-C")
        .arg(folder)
        .args(&members)
        .status()
        .map_err(|e| format!("cannot run tar: {e}"))?;
    if !status.success() {
        return Err(format!("tar could not unpack {name}"));
    }
    Ok(folder.join(top).join(sub.trim_end_matches('/')))
}

/// Replaces the programs in `bin` with those in `from`. Each new program is first copied next
/// to the old one, and only when both copies are there do they take the old ones' names, so
/// a failure leaves the old programs as they were. Windows cannot overwrite or delete a running
/// program but can rename it, so there the old one is first moved aside; `clean` deletes it on
/// a later start where it is still there.
pub fn replace(from: &Path, bin: &Path) -> Result<()> {
    let names = PROGRAMS.map(program);
    let staged = names.each_ref().map(|p| bin.join(format!(".{p}.new")));
    for (name, new) in names.iter().zip(&staged) {
        // fs::copy keeps the permissions, so the copy is executable
        if let Err(e) = std::fs::copy(from.join(name), new) {
            for file in &staged {
                let _ = std::fs::remove_file(file);
            }
            return Err(format!("cannot write {}: {e}", new.display()));
        }
    }
    for (name, new) in names.iter().zip(&staged) {
        let (old, aside) = (bin.join(name), aside(bin, name));
        let fail = |e: std::io::Error| format!("cannot replace {}: {e}", old.display());
        let _ = std::fs::remove_file(&aside);
        // elsewhere the rename below replaces a running program at once
        if cfg!(windows) {
            match std::fs::rename(&old, &aside) {
                Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(fail(e)),
                _ => {}
            }
        }
        if let Err(e) = std::fs::rename(new, &old) {
            let _ = std::fs::rename(&aside, &old);
            return Err(fail(e));
        }
        // fails on Windows while the old program runs
        let _ = std::fs::remove_file(&aside);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"{
        "url": "https://api.github.com/repos/statscrafthq/statscraft/releases/1",
        "html_url": "https://github.com/statscrafthq/statscraft/releases/tag/v0.2.0",
        "tag_name": "v0.2.0",
        "name": "Statscraft 0.2.0",
        "draft": false,
        "prerelease": false,
        "assets": [
            {"name": "SHA256SUMS", "size": 300,
             "browser_download_url": "https://github.com/statscrafthq/statscraft/releases/download/v0.2.0/SHA256SUMS"},
            {"name": "statscraft_0.2.0_amd64.deb", "size": 9000000,
             "browser_download_url": "https://github.com/statscrafthq/statscraft/releases/download/v0.2.0/statscraft_0.2.0_amd64.deb"}
        ]
    }"#;

    #[test]
    fn versions_compare_as_semver() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(v("0.10.0") > v("0.9.0"));
        assert!(v("1.0.0") > v("1.0.0-rc.1"));
        assert!(v("0.2.1") > v("0.2.0"));
        assert!(v("0.2.0") <= v("0.2.0"));
    }

    #[test]
    fn a_release_reads_from_the_api_reply() {
        let release = Release::parse(SAMPLE).unwrap().unwrap();
        assert_eq!(release.version, Version::new(0, 2, 0));
        assert_eq!(
            release.notes,
            "https://github.com/statscrafthq/statscraft/releases/tag/v0.2.0"
        );
        assert_eq!(release.assets.len(), 2);
        assert!(
            release
                .url("statscraft_0.2.0_amd64.deb")
                .unwrap()
                .ends_with(".deb")
        );
        assert!(release.url("missing.exe").is_err());
        // the workspace version is below 0.2.0 until a release bumps it
        assert_eq!(release.is_newer(), Version::new(0, 2, 0) > current());
        // drafts and prereleases are not updates; a tag that is no version is an error
        let draft = SAMPLE.replace(r#""draft": false"#, r#""draft": true"#);
        assert_eq!(Release::parse(&draft).unwrap(), None);
        let pre = SAMPLE.replace(r#""prerelease": false"#, r#""prerelease": true"#);
        assert_eq!(Release::parse(&pre).unwrap(), None);
        assert!(Release::parse(&SAMPLE.replace("v0.2.0\"", "nightly\"")).is_err());
        assert!(Release::parse("{}").is_err());
    }

    #[test]
    fn sums_are_found_by_file_name() {
        let a = "a".repeat(64);
        let b = "B".repeat(64);
        let sums = format!(
            "{a}  statscraft_0.2.0_amd64.deb\n{b} *statscraft-0.2.0-windows-x86_64-setup.exe\nxyz  short.tar.gz\n"
        );
        assert_eq!(
            sum_of(&sums, "statscraft_0.2.0_amd64.deb"),
            Some(a.as_str())
        );
        assert_eq!(
            sum_of(&sums, "statscraft-0.2.0-windows-x86_64-setup.exe"),
            Some(b.as_str())
        );
        assert_eq!(sum_of(&sums, "short.tar.gz"), None);
        assert_eq!(sum_of(&sums, "statscraft_0.2.0"), None);
    }

    fn temp(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "statscraft-update-test-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn the_installation_follows_from_the_program_path() {
        let v = Version::new(1, 2, 3);
        assert_eq!(
            Install::at(Path::new("/usr/bin/statscraft-gui"), false),
            Install::Deb
        );
        assert_eq!(
            Install::Deb.asset(&v).unwrap(),
            "statscraft_1.2.3_amd64.deb"
        );
        let root = temp("detect");
        // a tarball under a prefix: bin/ and the desktop file in share/
        let prefix = root.join("home/.local");
        std::fs::create_dir_all(prefix.join("bin")).unwrap();
        std::fs::create_dir_all(prefix.join("share/applications")).unwrap();
        let exe = prefix.join("bin/statscraft");
        assert_eq!(
            Install::at(&exe, false),
            Install::Unmanaged(prefix.join("bin")),
            "no desktop file, no tarball"
        );
        std::fs::write(prefix.join("share/applications/statscraft.desktop"), "").unwrap();
        let tarball = Install::at(&exe, false);
        assert_eq!(tarball, Install::Tarball(prefix.join("bin")));
        assert_eq!(
            tarball.asset(&v).unwrap(),
            "statscraft-1.2.3-linux-x86_64.tar.gz"
        );
        // a source build
        let target = root.join("target/release");
        std::fs::create_dir_all(&target).unwrap();
        let built = Install::at(&target.join("statscraft"), false);
        assert_eq!(built, Install::Unmanaged(target.clone()));
        assert_eq!(built.asset(&v), None);
        // the Windows installer leaves its uninstaller next to the programs
        assert_eq!(
            Install::at(&target.join("statscraft-gui.exe"), true),
            Install::Unmanaged(target.clone())
        );
        // the portable ZIP has LICENSE.txt next to the programs
        let portable = root.join("portable");
        std::fs::create_dir_all(&portable).unwrap();
        std::fs::write(portable.join("LICENSE.txt"), "").unwrap();
        let zip = Install::at(&portable.join("statscraft.exe"), true);
        assert_eq!(zip, Install::Portable(portable.clone()));
        assert_eq!(
            zip.asset(&v).unwrap(),
            "statscraft-1.2.3-windows-x86_64.zip"
        );
        // only on Windows
        assert_eq!(
            Install::at(&portable.join("statscraft"), false),
            Install::Unmanaged(portable.clone())
        );
        // the installer also ships LICENSE.txt, and stays the installer
        std::fs::write(target.join("LICENSE.txt"), "").unwrap();
        std::fs::write(target.join("unins000.exe"), "").unwrap();
        let windows = Install::at(&target.join("statscraft-gui.exe"), true);
        assert_eq!(windows, Install::Windows);
        assert_eq!(
            windows.asset(&v).unwrap(),
            "statscraft-1.2.3-windows-x86_64-setup.exe"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_mac_app_bundle_is_found_by_its_layout() {
        let app = Path::new("/Applications/Statscraft.app");
        let mac = Install::at(&app.join("Contents/MacOS/statscraft-gui"), false);
        assert_eq!(mac, Install::Mac(app.to_path_buf()));
        assert_eq!(
            mac.asset(&Version::new(1, 2, 3)).unwrap(),
            "statscraft-1.2.3-macos-universal.tar.gz"
        );
        // the command line from the tarball, outside the bundle
        assert_eq!(
            Install::at(Path::new("/usr/local/bin/statscraft"), false),
            Install::Unmanaged(PathBuf::from("/usr/local/bin"))
        );
        // Contents/MacOS that is not in a bundle
        assert_eq!(
            Install::at(Path::new("/tmp/Contents/MacOS/statscraft"), false),
            Install::Unmanaged(PathBuf::from("/tmp/Contents/MacOS"))
        );
    }

    #[test]
    fn replace_takes_the_place_of_an_old_program_moved_aside() {
        let root = temp("aside");
        let (new, bin) = (root.join("new"), root.join("bin"));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        for p in PROGRAMS {
            std::fs::write(new.join(program(p)), "new").unwrap();
            std::fs::write(bin.join(program(p)), "old").unwrap();
        }
        // left from an earlier update whose old program was still running
        std::fs::write(aside(&bin, &program("statscraft")), "older").unwrap();
        replace(&new, &bin).unwrap();
        let mut names: Vec<_> = std::fs::read_dir(&bin)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        let mut expected = PROGRAMS.map(program);
        expected.sort();
        assert_eq!(names, expected);
        for p in PROGRAMS {
            assert_eq!(
                std::fs::read_to_string(bin.join(program(p))).unwrap(),
                "new"
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    /// Windows does not let a running program be overwritten; replace renames it aside.
    #[cfg(windows)]
    #[test]
    fn replace_works_while_the_old_program_runs() {
        let root = temp("running");
        let (new, bin) = (root.join("new"), root.join("bin"));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        for p in PROGRAMS {
            std::fs::write(new.join(program(p)), "new").unwrap();
        }
        // a copy of ping stands in for the running command line
        let ping = Path::new(&std::env::var("SystemRoot").unwrap()).join("System32\\PING.EXE");
        let old = bin.join(program("statscraft"));
        std::fs::copy(&ping, &old).unwrap();
        std::fs::write(bin.join(program("statscraft-gui")), "old").unwrap();
        let mut running = Command::new(&old)
            .args(["-n", "30", "127.0.0.1"])
            .stdout(std::process::Stdio::null())
            .spawn()
            .unwrap();
        assert!(running.try_wait().unwrap().is_none());
        replace(&new, &bin).unwrap();
        assert_eq!(std::fs::read_to_string(&old).unwrap(), "new");
        assert!(aside(&bin, &program("statscraft")).is_file());
        running.kill().unwrap();
        running.wait().unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replace_swaps_both_programs_or_neither() {
        let root = temp("replace");
        let (new, bin) = (root.join("new"), root.join("bin"));
        std::fs::create_dir_all(&new).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        for p in PROGRAMS {
            std::fs::write(bin.join(program(p)), "old").unwrap();
        }
        // only one new program: nothing changes and nothing is left behind
        std::fs::write(new.join(program("statscraft")), "new").unwrap();
        assert!(replace(&new, &bin).is_err());
        let mut names: Vec<_> = std::fs::read_dir(&bin)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        let mut expected = PROGRAMS.map(program);
        expected.sort();
        assert_eq!(names, expected);
        for p in PROGRAMS {
            assert_eq!(
                std::fs::read_to_string(bin.join(program(p))).unwrap(),
                "old"
            );
        }
        std::fs::write(new.join(program("statscraft-gui")), "new gui").unwrap();
        replace(&new, &bin).unwrap();
        assert_eq!(
            std::fs::read_to_string(bin.join(program("statscraft"))).unwrap(),
            "new"
        );
        assert_eq!(
            std::fs::read_to_string(bin.join(program("statscraft-gui"))).unwrap(),
            "new gui"
        );
        assert_eq!(std::fs::read_dir(&bin).unwrap().count(), 2);
        std::fs::remove_dir_all(root).unwrap();
    }
}
