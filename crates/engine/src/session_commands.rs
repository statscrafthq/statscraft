//! Commands about the session rather than the data: macros and inserted files (already
//! handled before parsing), program blocks, settings, named datasets and the working folder.

use crate::{Engine, Error, Table};
use statscraft_syntax::Command;
use std::path::PathBuf;

/// What the session commands keep between commands.
#[derive(Default, Clone)]
pub(crate) struct State {
    /// The folder CD named; relative file names start there. None: the current folder.
    pub cwd: Option<PathBuf>,
}

impl Engine {
    /// A file name as the commands use it: relative names start from the folder of the last CD.
    pub(crate) fn path(&self, file: &str) -> PathBuf {
        match &self.session.cwd {
            Some(dir) if PathBuf::from(file).is_relative() => dir.join(file),
            _ => PathBuf::from(file),
        }
    }

    /// The folder relative names of syntax files start from.
    fn syntax_folder(&self) -> PathBuf {
        self.session
            .cwd
            .clone()
            .or_else(|| self.base_dir.clone())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// The commands of this module; the main `execute` hands them over.
    pub(crate) fn execute_session(&mut self, command: &Command) -> Result<Vec<Table>, Error> {
        match command {
            Command::Define(_) => {}
            Command::Insert { file, include } => {
                // the commands of the file were read in already; here only a file that could
                // not be read is reported
                let path = self.syntax_folder().join(file);
                if let Err(e) = std::fs::metadata(&path) {
                    let name = if *include { "INCLUDE" } else { "INSERT" };
                    return Err(Error(format!("{name}: cannot read {file}: {e}")));
                }
            }
            Command::BeginProgram => self.warn(
                "the program block was not run: StatsCraft runs no programs in other languages",
            ),
            other => {
                let _ = other;
                return Err(Error("this command cannot run in the engine yet".into()));
            }
        }
        Ok(Vec::new())
    }
}
