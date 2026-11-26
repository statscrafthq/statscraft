//! SAVE CODEPAGE: the active dataset as a data file whose text is in a code page.
//!
//! The data file writer already writes text in the encoding of the dictionary; this command
//! gives a copy of the dataset the encoding asked for and writes that copy, so the active
//! dataset stays as it is. Choices where the manual is silent:
//! - LOCALE is the legacy code page of the system, as for syntax files (Windows-1252 where
//!   the locale names none).
//! - An encoding that data files cannot be written in (UTF-16) is refused; UTF-8 is accepted
//!   and gives the file SAVE writes.
//! - Text the code page cannot hold is an error and nothing is written.
//! - A password is refused: Statscraft does not encrypt files.
//! - The file is written as SAVE writes it, bytecode compressed, zlib blocks for .zsav.

use crate::{Engine, Error};
use statscraft_syntax::{SaveCodepage, encoding_for};

impl Engine {
    pub(crate) fn save_codepage(&mut self, command: &SaveCodepage) -> Result<(), Error> {
        if command.password {
            return Err(Error(
                "SAVE CODEPAGE: PASSPROTECT with a password is not supported: Statscraft does not encrypt files; use PASSWORD=NONE"
                    .into(),
            ));
        }
        let name = command.encoding.as_deref().unwrap_or("LOCALE");
        let encoding = encoding_for(name)
            .ok_or_else(|| Error(format!("SAVE CODEPAGE: unknown encoding {name}")))?;
        let mut data = self.active()?.clone();
        if command.delete_unselected
            && let Some(filter) = data.filter_variable()
        {
            let variable = data.variable(filter)?;
            let keep: Vec<bool> = (0..data.case_count()).map(|c| variable.keeps(c)).collect();
            data.select_cases(&keep)?;
        }
        let mut data = self.file_options(data, &command.options, false)?;
        data.set_encoding(encoding.name())
            .map_err(|e| Error(format!("SAVE CODEPAGE: {e}")))?;
        if !self.dry_run {
            data.write_sav_as(self.path(&command.outfile), None)
                .map_err(|e| Error(format!("SAVE CODEPAGE: {e}")))?;
        }
        Ok(())
    }
}
