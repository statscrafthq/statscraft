//! A write that is killed half way (Ctrl-C of the command line, a crash, the power) leaves the
//! file it was writing as it was; the part of the write stays beside it until the next write of
//! the same file, which removes it.

use std::io::{BufRead, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// The child process, killed when the test ends however it ends.
struct Child(std::process::Child);

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// The name of the test in the test binary, which the child is started with.
const TEST: &str = "kill::a_killed_write_leaves_the_old_file_and_the_next_write_removes_the_part";

fn parts(dir: &Path) -> Vec<String> {
    let names = std::fs::read_dir(dir).unwrap();
    let names = names.map(|e| e.unwrap().file_name().to_string_lossy().into_owned());
    names.filter(|n| n.ends_with(".part")).collect()
}

#[test]
fn a_killed_write_leaves_the_old_file_and_the_next_write_removes_the_part() {
    // as the child: write a megabyte, say so, and stay in the middle of the write
    if let Some(file) = std::env::var_os("DATA_KILL_CHILD") {
        statscraft_data::write_file(Path::new(&file), |part| {
            part.write_all(&vec![b'x'; 1 << 20]).unwrap();
            part.flush().unwrap();
            println!("@@writing");
            std::io::stdout().flush().unwrap();
            std::thread::sleep(std::time::Duration::from_secs(600));
            Ok(())
        })
        .unwrap();
        return;
    }
    let dir = std::env::temp_dir().join(format!("statscraft-kill-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("data.sav");
    std::fs::write(&file, b"the old file").unwrap();
    let mut child = Child(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", TEST, "--nocapture"])
            .env("DATA_KILL_CHILD", &file)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let stdout = child.0.stdout.take().unwrap();
    let mut lines = std::io::BufReader::new(stdout).lines();
    let said = lines
        .by_ref()
        .map_while(Result::ok)
        .any(|l| l == "@@writing");
    assert!(
        said,
        "the child never started to write (it runs {TEST}: is that still the name of this test?)"
    );
    // killed where nothing can be done about it
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"the old file");
    let left = parts(&dir);
    assert_eq!(left.len(), 1, "{left:?}");
    assert_eq!(
        std::fs::metadata(dir.join(&left[0])).unwrap().len(),
        1 << 20
    );
    // the next write of the file takes the stale part away (where processes can be looked up)
    statscraft_data::write_bytes(&file, b"new").unwrap();
    assert_eq!(std::fs::read(&file).unwrap(), b"new");
    if cfg!(target_os = "linux") {
        assert!(parts(&dir).is_empty(), "{:?}", parts(&dir));
    }
    std::fs::remove_dir_all(&dir).ok();
}
