//! Writing a file without losing the old one. Every writer of the data files shares this path:
//! the bytes go to a part file beside the file (`<name>.<process>-<n>.part`) and replace it only
//! when complete, so a failed write leaves the old file as it was and no part behind. A part
//! left by a process that was killed is removed by the next write of the same file (where the
//! system lets us tell that the process is gone: Linux). A symbolic link is written through to
//! the file it points to, and the file keeps its permissions. A device or a pipe is written to
//! directly, and so is a file in a folder that cannot be written to.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

/// The file a path stands for: a symbolic link (also one that points nowhere yet) is followed
/// to its target.
fn real_path(path: &Path) -> Result<PathBuf, String> {
    let mut current = path.to_path_buf();
    for _ in 0..40 {
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let to = std::fs::read_link(&current).map_err(|e| e.to_string())?;
                current = match current.parent() {
                    Some(parent) => parent.join(to),
                    None => to,
                };
            }
            _ => return Ok(current),
        }
    }
    Err("too many symbolic links".to_string())
}

static COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A part file name of `target` for this write.
fn part_of(target: &Path) -> PathBuf {
    let n = COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let mut part = target.as_os_str().to_owned();
    part.push(format!(".{}-{n}.part", std::process::id()));
    PathBuf::from(part)
}

/// Removes the parts of `target` that processes now gone left behind.
fn sweep(target: &Path) {
    let (Some(folder), Some(name)) = (target.parent(), target.file_name()) else {
        return;
    };
    let folder = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    };
    let prefix = format!("{}.", name.to_string_lossy());
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let file = entry.file_name().to_string_lossy().into_owned();
        let Some(rest) = file
            .strip_prefix(&prefix)
            .and_then(|r| r.strip_suffix(".part"))
        else {
            continue;
        };
        let Some((process, count)) = rest.split_once('-') else {
            continue;
        };
        let (Ok(process), Ok(_)) = (process.parse::<u32>(), count.parse::<usize>()) else {
            continue;
        };
        if process != std::process::id() && process_is_gone(process) {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(target_os = "linux")]
fn process_is_gone(process: u32) -> bool {
    !Path::new(&format!("/proc/{process}")).exists()
}

/// Where processes cannot be looked up, a part is never taken for stale.
#[cfg(not(target_os = "linux"))]
fn process_is_gone(_process: u32) -> bool {
    false
}

fn describe(error: &std::io::Error) -> String {
    match error.kind() {
        std::io::ErrorKind::StorageFull => "the disk is full".to_string(),
        std::io::ErrorKind::PermissionDenied => "permission denied".to_string(),
        _ => error.to_string(),
    }
}

/// Writes `path`: `fill` writes the whole content to the file it is given. The error says what
/// went wrong in plain words (the caller adds the file's name).
pub fn write_file<F>(path: &Path, fill: F) -> Result<(), String>
where
    F: FnOnce(&mut File) -> Result<(), String>,
{
    let target = real_path(path)?;
    let folder = match target.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    if !folder.is_dir() {
        return Err(format!("the folder {} does not exist", folder.display()));
    }
    let (permissions, regular) = match std::fs::metadata(&target) {
        Ok(meta) if meta.is_dir() => return Err("it is a folder".to_string()),
        Ok(meta) => {
            // the old file is replaced only where it could be written to
            OpenOptions::new()
                .write(true)
                .open(&target)
                .map_err(|e| match e.kind() {
                    std::io::ErrorKind::PermissionDenied => {
                        "the file is read-only or not yours to change".to_string()
                    }
                    _ => describe(&e),
                })?;
            (Some(meta.permissions()), meta.is_file())
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (None, true),
        Err(e) => return Err(describe(&e)),
    };
    // a device or a pipe is written to as it is
    let in_place = |fill: F| -> Result<(), String> {
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&target)
            .map_err(|e| describe(&e))?;
        fill(&mut file)
    };
    if !regular {
        return in_place(fill);
    }
    sweep(&target);
    let part = part_of(&target);
    let mut file = match File::create(&part) {
        Ok(file) => file,
        // a file that may be written to in a folder that may not: written where it stands
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied && permissions.is_some() => {
            return in_place(fill);
        }
        Err(e) => return Err(describe(&e)),
    };
    let result = (|| {
        fill(&mut file)?;
        // a file system that cannot sync is no reason to refuse the file
        match file.sync_all() {
            Err(e)
                if !matches!(
                    e.kind(),
                    std::io::ErrorKind::Unsupported | std::io::ErrorKind::InvalidInput
                ) =>
            {
                return Err(describe(&e));
            }
            _ => {}
        }
        drop(file);
        if let Some(permissions) = permissions {
            std::fs::set_permissions(&part, permissions).map_err(|e| describe(&e))?;
        }
        std::fs::rename(&part, &target).map_err(|e| describe(&e))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    result
}

/// Writes `bytes` to `path`, as `write_file` does.
pub fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    write_file(path, |file| file.write_all(bytes).map_err(|e| describe(&e)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(tag: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("statscraft-files-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn parts(dir: &Path) -> Vec<String> {
        let names = std::fs::read_dir(dir).unwrap();
        let names = names.map(|e| e.unwrap().file_name().to_string_lossy().into_owned());
        names.filter(|n| n.ends_with(".part")).collect()
    }

    #[test]
    fn a_failure_leaves_the_old_file_and_no_part() {
        let dir = folder("fail");
        let file = dir.join("a.txt");
        std::fs::write(&file, b"old").unwrap();
        let result = write_file(&file, |f| {
            use std::io::Write;
            f.write_all(b"half").unwrap();
            Err("broke".to_string())
        });
        assert_eq!(result, Err("broke".to_string()));
        assert_eq!(std::fs::read(&file).unwrap(), b"old");
        assert!(parts(&dir).is_empty());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_missing_folder_is_named_and_a_folder_is_refused() {
        let dir = folder("missing");
        let message = write_bytes(&dir.join("none").join("b.txt"), b"x").unwrap_err();
        assert!(message.starts_with("the folder "), "{message}");
        assert!(message.ends_with("does not exist"), "{message}");
        std::fs::create_dir(dir.join("sub")).unwrap();
        assert_eq!(
            write_bytes(&dir.join("sub"), b"x"),
            Err("it is a folder".into())
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn the_part_of_a_process_that_is_gone_is_removed_by_the_next_write() {
        let dir = folder("stale");
        let file = dir.join("a.txt");
        // no process has this number; one of a live process is not touched
        std::fs::write(dir.join("a.txt.4294967295-0.part"), b"left by a kill").unwrap();
        let alive = format!("a.txt.{}-99.part", std::process::id() + 1);
        std::fs::write(dir.join(&alive), b"maybe live").unwrap();
        write_bytes(&file, b"new").unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"new");
        let left = parts(&dir);
        assert!(left.len() <= 1, "{left:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_full_disk_is_named() {
        let full = std::io::Error::from(std::io::ErrorKind::StorageFull);
        assert_eq!(describe(&full), "the disk is full");
        let dir = folder("full");
        let file = dir.join("x.out");
        let message = write_file(&file, |_| Err(describe(&full))).unwrap_err();
        assert_eq!(message, "the disk is full");
        assert!(parts(&dir).is_empty() && !file.exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_link_is_written_through_and_a_read_only_file_and_a_device_are_handled() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = folder("link");
        let real = dir.join("real.txt");
        std::fs::write(&real, b"old").unwrap();
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o640)).unwrap();
        let link = dir.join("link.txt");
        symlink("real.txt", &link).unwrap();
        write_bytes(&link, b"through").unwrap();
        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(std::fs::read(&real).unwrap(), b"through");
        let mode = std::fs::metadata(&real).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
        // a link to a file that is not there yet creates the file it points to
        let dangling = dir.join("dangling.txt");
        symlink("fresh.txt", &dangling).unwrap();
        write_bytes(&dangling, b"made").unwrap();
        assert_eq!(std::fs::read(dir.join("fresh.txt")).unwrap(), b"made");
        // a device is written to as it is, not replaced
        write_bytes(Path::new("/dev/null"), b"nothing").unwrap();
        {
            use std::os::unix::fs::FileTypeExt;
            assert!(
                std::fs::metadata("/dev/null")
                    .unwrap()
                    .file_type()
                    .is_char_device()
            );
        }
        // a read-only file stays as it is (not for a user who may write anything)
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o444)).unwrap();
        if std::fs::OpenOptions::new().write(true).open(&real).is_err() {
            let message = write_bytes(&real, b"no").unwrap_err();
            assert!(message.contains("read-only"), "{message}");
            assert_eq!(std::fs::read(&real).unwrap(), b"through");
            assert!(parts(&dir).is_empty());
        }
        // a file that may be written to, in a folder that may not, is written where it stands
        std::fs::set_permissions(&real, std::fs::Permissions::from_mode(0o644)).unwrap();
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
        if File::create(dir.join("probe")).is_err() {
            write_bytes(&real, b"in place").unwrap();
            assert_eq!(std::fs::read(&real).unwrap(), b"in place");
        }
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
