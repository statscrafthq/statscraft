//! EXPORT and IMPORT: portable files, and the MAP tables that they and GET TRANSLATE show.

use crate::{Cell, Engine, Error, Table};
use statscraft_data::{Dataset, portable_names};
use statscraft_syntax::{Export, FileOption, Import};

impl Engine {
    pub(crate) fn export_portable(&mut self, command: &Export) -> Result<Vec<Table>, Error> {
        self.begin_pass()?;
        let mut data = self.active()?.clone();
        if command.delete_unselected
            && let Some(filter) = data.filter_variable()
        {
            let variable = data.variable(filter)?;
            let keep: Vec<bool> = (0..data.case_count()).map(|c| variable.keeps(c)).collect();
            data.select_cases(&keep)?;
        }
        let names: Vec<String> = data.variable_names().map(str::to_string).collect();
        let tables = self.map_tables(&data, &names, &command.options, false, true)?;
        let data = self.file_options(data, &command.options, false)?;
        if !self.dry_run {
            let warnings = data.write_por(self.path(&command.outfile), command.digits)?;
            warnings.into_iter().for_each(|w| self.warn(w));
        }
        let before = self.dataset.clone();
        self.end_pass(before.as_ref());
        Ok(tables)
    }

    pub(crate) fn import_portable(&mut self, command: &Import) -> Result<Vec<Table>, Error> {
        let data = Dataset::read_por(self.path(&command.file))?;
        data.warnings().iter().for_each(|w| self.warn(w.clone()));
        let names: Vec<String> = data.variable_names().map(str::to_string).collect();
        let tables = self.map_tables(&data, &names, &command.options, false, false)?;
        let data = self.file_options(data, &command.options, false)?;
        self.make_active(data);
        Ok(tables)
    }

    /// One table for every `/MAP` among `options`: the variables as the options before it
    /// leave them, each with its name on the other side (`file_names`, one per variable of
    /// `data`; for EXPORT the names of the active dataset, and the file gets the portable ones).
    pub(crate) fn map_tables(
        &self,
        data: &Dataset,
        file_names: &[String],
        options: &[FileOption],
        keep_order: bool,
        export: bool,
    ) -> Result<Vec<Table>, Error> {
        let maps = options
            .iter()
            .enumerate()
            .filter(|(_, o)| **o == FileOption::Map);
        if maps.clone().next().is_none() {
            return Ok(Vec::new());
        }
        // the labels carry the other names through KEEP, DROP and RENAME
        let mut labelled = data.clone();
        for (name, other) in data.variable_names().zip(file_names) {
            labelled.set_variable_label(name, Some(other))?;
        }
        let mut tables = Vec::new();
        for (at, _) in maps {
            let shaped = self.file_options(labelled.clone(), &options[..at], keep_order)?;
            let names: Vec<String> = match export {
                true => portable_names(shaped.variable_names()),
                false => shaped.variable_names().map(str::to_string).collect(),
            };
            let rows = shaped
                .variables()
                .zip(names)
                .map(|(v, name)| {
                    vec![
                        Cell::Text(name),
                        Cell::Text(v.label().unwrap_or_default().into()),
                    ]
                })
                .collect();
            let (title, columns) = match export {
                true => ("Variables in the file", ["Name in the file", "Variable"]),
                false => (
                    "Variables in the active dataset",
                    ["Variable", "Name in the file"],
                ),
            };
            tables.push(Table {
                title: title.into(),
                columns: columns.map(String::from).to_vec(),
                rows,
                ..Table::default()
            });
        }
        Ok(tables)
    }
}
