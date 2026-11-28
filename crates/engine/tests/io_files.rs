//! Every writer of a data file shares one safe path: a failed write leaves the old file and no
//! part, a stale part is replaced, a symbolic link is written through, a read-only file and a
//! missing folder are refused with a plain message.

use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-io-{tag}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn session(dir: &Path) -> Session {
    let mut session = Session::default();
    session.set_base_dir(Some(dir.to_path_buf()));
    let source = format!(
        "CD '{}'.\nDATA LIST LIST /x (F3) s (A6).\nBEGIN DATA\n1 ab\n2 cd\n3 ef\nEND DATA.",
        dir.display()
    );
    session
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    session
}

fn run(session: &mut Session, syntax: &str) {
    session
        .run(syntax, None)
        .unwrap_or_else(|e| panic!("{syntax}: {e:?}"));
}

fn error(session: &mut Session, syntax: &str) -> String {
    session.run(syntax, None).unwrap_err()[0].message.clone()
}

/// The writers, each as a command that writes `{f}` from the active dataset.
const WRITERS: [(&str, &str); 10] = [
    ("SAVE OUTFILE='{f}'.", "w.sav"),
    ("SAVE OUTFILE='{f}' /ZCOMPRESSED.", "w.zsav"),
    ("EXPORT OUTFILE='{f}'.", "w.por"),
    ("SAVE TRANSLATE OUTFILE='{f}' /TYPE=CSV /REPLACE.", "w.csv"),
    ("SAVE TRANSLATE OUTFILE='{f}' /TYPE=TAB /REPLACE.", "w.tab"),
    (
        "SAVE TRANSLATE OUTFILE='{f}' /TYPE=XLSX /REPLACE.",
        "w.xlsx",
    ),
    (
        "SAVE TRANSLATE OUTFILE='{f}' /TYPE=STATA /REPLACE.",
        "w.dta",
    ),
    ("SAVE TRANSLATE OUTFILE='{f}' /TYPE=DB4 /REPLACE.", "w.dbf"),
    (
        "SAVE TRANSLATE OUTFILE='{f}' /TYPE=SAS /VERSION=X /REPLACE.",
        "w.xpt",
    ),
    ("WRITE OUTFILE='{f}' /x s.\nEXECUTE.", "w.txt"),
];

fn command(template: &str, file: &str) -> String {
    template.replace("{f}", file)
}

fn parts(dir: &Path) -> Vec<String> {
    let names = std::fs::read_dir(dir).unwrap();
    let names = names.map(|e| e.unwrap().file_name().to_string_lossy().into_owned());
    names.filter(|n| n.contains(".part")).collect()
}

#[test]
fn a_missing_folder_is_named_and_nothing_is_made() {
    let dir = temp_dir("folder");
    let mut s = session(&dir);
    for (template, name) in WRITERS {
        let file = format!("nowhere/{name}");
        let message = error(&mut s, &command(template, &file));
        assert!(message.contains("does not exist"), "{name}: {message}");
        assert!(!dir.join("nowhere").exists(), "{name}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_stale_part_goes_and_a_symbolic_link_is_written_through() {
    let dir = temp_dir("link");
    let mut s = session(&dir);
    for (template, name) in WRITERS {
        run(&mut s, &command(template, name));
        let before = std::fs::read(dir.join(name)).unwrap();
        // a part left by a killed write goes with the next write of the file
        // (no process has the number 4294967295; where processes cannot be looked up, a part
        // is never taken for stale)
        std::fs::write(
            dir.join(format!("{name}.4294967295-0.part")),
            b"left by a kill",
        )
        .unwrap();
        run(&mut s, &command(template, name));
        if cfg!(target_os = "linux") {
            assert!(parts(&dir).is_empty(), "{name}: {:?}", parts(&dir));
        } else {
            std::fs::remove_file(dir.join(format!("{name}.4294967295-0.part"))).unwrap();
        }
        assert_eq!(std::fs::read(dir.join(name)).unwrap(), before, "{name}");
        #[cfg(unix)]
        {
            // the link stays a link and the file it points to changes
            let link = format!("link-{name}");
            std::os::unix::fs::symlink(name, dir.join(&link)).unwrap();
            run(&mut s, "COMPUTE x = x + 40.");
            run(&mut s, &command(template, &link));
            let meta = std::fs::symlink_metadata(dir.join(&link)).unwrap();
            assert!(meta.file_type().is_symlink(), "{name}");
            let after = std::fs::read(dir.join(name)).unwrap();
            assert_ne!(after, before, "{name}: the real file was not written");
            assert!(parts(&dir).is_empty(), "{name}");
            run(&mut s, "COMPUTE x = x - 40.");
        }
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_read_only_file_is_refused_and_kept() {
    use std::os::unix::fs::PermissionsExt;
    let dir = temp_dir("readonly");
    let mut s = session(&dir);
    for (template, name) in WRITERS {
        run(&mut s, &command(template, name));
        let file = dir.join(name);
        let before = std::fs::read(&file).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
        // a user who may write to anything (root) cannot be refused by permissions
        if std::fs::OpenOptions::new().write(true).open(&file).is_ok() {
            continue;
        }
        let message = error(&mut s, &command(template, name));
        assert!(message.contains("read-only"), "{name}: {message}");
        assert_eq!(std::fs::read(&file).unwrap(), before, "{name}");
        assert!(parts(&dir).is_empty(), "{name}");
    }
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn a_failed_text_write_leaves_the_old_file_and_no_part() {
    let dir = temp_dir("failed");
    let mut s = session(&dir);
    run(&mut s, "SAVE TRANSLATE OUTFILE='a.csv' /TYPE=CSV.");
    let before = std::fs::read(dir.join("a.csv")).unwrap();
    run(&mut s, "COMPUTE s = 'Ωmega'.");
    let message = error(
        &mut s,
        "SAVE TRANSLATE OUTFILE='a.csv' /TYPE=CSV /ENCODING='windows-1252' /REPLACE.",
    );
    assert!(message.contains("cannot hold"), "{message}");
    assert_eq!(std::fs::read(dir.join("a.csv")).unwrap(), before);
    assert!(parts(&dir).is_empty());
    std::fs::remove_dir_all(&dir).unwrap();
}

#[cfg(unix)]
#[test]
fn a_device_is_written_to_as_it_is() {
    let dir = temp_dir("device");
    let mut s = session(&dir);
    run(
        &mut s,
        "SAVE TRANSLATE OUTFILE='/dev/null' /TYPE=CSV /REPLACE.\n\
         WRITE OUTFILE='/dev/null' /x s.\nEXECUTE.",
    );
    assert!(std::path::Path::new("/dev/null").exists());
    std::fs::remove_dir_all(&dir).unwrap();
}
