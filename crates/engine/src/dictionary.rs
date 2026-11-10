//! The dictionary commands: ADD VALUE LABELS, FILE LABEL, documents, roles, widths,
//! alignments, attributes, ALTER TYPE, APPLY DICTIONARY, and KEEP, DROP and RENAME of GET
//! and SAVE.
//!
//! Choices where the manual is silent:
//! - Each DOCUMENT and ADD DOCUMENT appends its lines and then `(Entered YYYY-MM-DD)`, the date
//!   in the form that sorts.
//! - ALTER TYPE from string to numeric reads F, COMMA and DOT values (thousands separators
//!   allowed); other numeric formats are refused for now.

use crate::{Engine, Error, ctables::format_value};
use statscraft_data::{Dataset, MissingValues, NewValues, Values, cut_to};
use statscraft_syntax::{
    AlterTarget, AlterType, ApplyDictionary, AttributeChange, Command, FileOption, FormatType,
};

/// Today as YYYY-MM-DD, from the system clock (UTC).
fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    // days since 1970-01-01 to a civil date
    let z = (secs / 86_400) as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

/// Sets or deletes attributes in a list of (name, values), arrays being several values.
fn change_attributes(attributes: &mut Vec<(String, Vec<String>)>, change: &AttributeChange) {
    let find = |list: &Vec<(String, Vec<String>)>, name: &str| {
        list.iter().position(|(n, _)| n.eq_ignore_ascii_case(name))
    };
    for (name, index, value) in &change.set {
        let at = match find(attributes, name) {
            Some(at) => at,
            None => {
                attributes.push((name.clone(), Vec::new()));
                attributes.len() - 1
            }
        };
        let values = &mut attributes[at].1;
        match index {
            None => *values = vec![value.clone()],
            Some(i) => {
                if values.len() < *i {
                    values.resize(*i, String::new());
                }
                values[i - 1] = value.clone();
            }
        }
    }
    for (name, index) in &change.delete {
        let Some(at) = find(attributes, name) else {
            continue;
        };
        match index {
            None => {
                attributes.remove(at);
            }
            Some(i) => {
                let values = &mut attributes[at].1;
                if *i <= values.len() {
                    values.remove(i - 1);
                }
                if values.is_empty() {
                    attributes.remove(at);
                }
            }
        }
    }
}

/// A number written in a string, for ALTER TYPE to a numeric format; None when it is none.
fn read_number(text: &str, comma_decimal: bool) -> Option<f64> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let (group, decimal) = if comma_decimal {
        ('.', ',')
    } else {
        (',', '.')
    };
    let cleaned: String = text
        .chars()
        .filter(|&c| c != group)
        .map(|c| if c == decimal { '.' } else { c })
        .collect();
    cleaned.parse::<f64>().ok().filter(|v| v.is_finite())
}

impl Engine {
    /// The commands of this module; `execute_session` hands them over.
    pub(crate) fn execute_dictionary(&mut self, command: &Command) -> Result<(), Error> {
        match command {
            Command::AddValueLabels(groups) => {
                for group in groups {
                    for name in self.expand(&group.vars)? {
                        let data = self.active_mut()?;
                        let mut labels = data.variable(&name)?.value_labels().to_vec();
                        for (value, label) in &group.labels {
                            let value = crate::value(value);
                            labels.retain(|(v, _)| *v != value);
                            labels.push((value, label.clone()));
                        }
                        data.set_value_labels(&name, labels)?;
                    }
                }
            }
            Command::FileLabel(label) => self.active_mut()?.set_file_label(Some(label)),
            Command::AddDocument(lines) => {
                let data = self.active_mut()?;
                let mut documents = data.documents().to_vec();
                documents.extend(lines.iter().cloned());
                documents.push(format!("(Entered {})", today()));
                data.set_documents(documents);
            }
            Command::DropDocuments => self.active_mut()?.set_documents(Vec::new()),
            Command::VariableRole(groups) => {
                for (role, vars) in groups {
                    for name in self.expand(vars)? {
                        let data = self.active_mut()?;
                        let v = data.variable(&name)?;
                        let (width, alignment) = (v.display_width(), v.alignment());
                        data.set_display(&name, width, alignment, Some(role))?;
                    }
                }
            }
            Command::VariableAlignment(groups) => {
                for (vars, alignment) in groups {
                    for name in self.expand(vars)? {
                        let data = self.active_mut()?;
                        let v = data.variable(&name)?;
                        let (width, role) = (v.display_width(), v.role());
                        data.set_display(&name, width, Some(alignment), role)?;
                    }
                }
            }
            Command::Attribute(changes) => {
                for change in changes {
                    match &change.variables {
                        None => {
                            let data = self.active_mut()?;
                            let mut attributes = data.attributes().to_vec();
                            change_attributes(&mut attributes, change);
                            data.set_attributes(attributes);
                        }
                        Some(vars) => {
                            for name in self.expand(vars)? {
                                let data = self.active_mut()?;
                                let mut attributes = data.variable(&name)?.attributes().to_vec();
                                change_attributes(&mut attributes, change);
                                data.set_variable_attributes(&name, attributes)?;
                            }
                        }
                    }
                }
            }
            Command::AlterType(groups) => self.alter_type(groups)?,
            Command::ApplyDictionary(apply) => self.apply_dictionary(apply)?,
            _ => unreachable!("only dictionary commands come here"),
        }
        Ok(())
    }

    fn alter_type(&mut self, groups: &[AlterType]) -> Result<(), Error> {
        // a variable named in several groups takes the last one
        let mut plan: Vec<(String, &AlterType)> = Vec::new();
        for group in groups {
            for name in self.expand(&group.vars)? {
                plan.retain(|(n, _)| *n != name);
                plan.push((name, group));
            }
        }
        if plan.is_empty() {
            return Ok(());
        }
        self.begin_pass()?;
        let comma = self.setting("DECIMAL").eq_ignore_ascii_case("COMMA");
        for (name, group) in plan {
            let data = self.active_mut()?;
            let v = data.variable(&name)?;
            let width = v.string_width();
            if let Some((kind, only_width)) = group.only {
                let is_string = matches!(kind, FormatType::A | FormatType::Ahex);
                let same_kind = match width {
                    Some(_) => is_string,
                    None => !is_string && v.format().to_uppercase().starts_with(kind.name()),
                };
                let same_width = only_width.is_none_or(|w| match width {
                    Some(sw) => sw == usize::from(w),
                    None => v.format().contains(&w.to_string()),
                });
                if !(same_kind && same_width) {
                    continue;
                }
            }
            let format = match &group.to {
                AlterTarget::Amin { plus, percent } => {
                    let Values::String(values) = v.values() else {
                        continue; // AMIN changes strings only
                    };
                    let longest = values.iter().map(str::len).max().unwrap_or(0).max(1);
                    let extra = if *percent {
                        (longest as f64 * plus / 100.0).round()
                    } else {
                        *plus
                    };
                    statscraft_syntax::Format {
                        kind: FormatType::A,
                        width: (longest + extra.max(0.0) as usize) as u16,
                        decimals: 0,
                    }
                }
                AlterTarget::Format(format) => *format,
            };
            let mut text = format!("{}{}", format.kind.name(), format.width);
            if format.decimals > 0 {
                text.push_str(&format!(".{}", format.decimals));
            }
            let to_string = matches!(format.kind, FormatType::A | FormatType::Ahex);
            let new_width = usize::from(format.width).max(1);
            match (v.values(), to_string) {
                (Values::Numeric(_), false) => data.set_format(&name, &text)?,
                (Values::String(values), true) => {
                    let values = values
                        .iter()
                        .map(|s| cut_to(s, new_width).to_string())
                        .collect();
                    data.retype_variable(
                        &name,
                        NewValues::String {
                            width: new_width,
                            values,
                        },
                        &text,
                    )?;
                }
                (Values::Numeric(values), true) => {
                    let old = v.format().to_string();
                    let values = values
                        .iter()
                        .map(|x| match x {
                            Some(x) => cut_to(&format_value(x, &old, true), new_width).to_string(),
                            None => String::new(),
                        })
                        .collect();
                    data.retype_variable(
                        &name,
                        NewValues::String {
                            width: new_width,
                            values,
                        },
                        &text,
                    )?;
                }
                (Values::String(values), false) => {
                    if !matches!(
                        format.kind,
                        FormatType::F | FormatType::Comma | FormatType::Dot
                    ) {
                        return Err(Error(format!(
                            "ALTER TYPE from string to {} is not supported yet; use F, COMMA or DOT",
                            format.kind.name()
                        )));
                    }
                    let comma = comma || format.kind == FormatType::Dot;
                    let values = values.iter().map(|s| read_number(s, comma)).collect();
                    data.retype_variable(&name, NewValues::Numeric(values), &text)?;
                }
            }
        }
        Ok(())
    }

    fn apply_dictionary(&mut self, apply: &ApplyDictionary) -> Result<(), Error> {
        if apply.newvars {
            return Err(Error(
                "APPLY DICTIONARY /NEWVARS is not supported yet".into(),
            ));
        }
        let source: Dataset = match &apply.from {
            None => self.active()?.clone(),
            Some(name) => match self.named_dataset(name) {
                Some(data) => data.clone(),
                None => Dataset::read_sav(self.path(name))?,
            },
        };
        let target_names: Vec<String> = match &apply.target {
            Some(items) => self.expand(items)?,
            None => self
                .active()?
                .variable_names()
                .map(str::to_string)
                .collect(),
        };
        let source_names: Vec<String> = match &apply.source {
            Some(items) => Engine::with_dataset(Some(source.clone())).expand(items)?,
            None => source.variable_names().map(str::to_string).collect(),
        };
        let one_to_many = apply.source.is_some() && apply.target.is_some();
        if one_to_many && source_names.len() != 1 {
            return Err(Error(
                "APPLY DICTIONARY with SOURCE and TARGET takes one SOURCE variable".into(),
            ));
        }
        let varinfo: Vec<(String, Option<String>)> = match &apply.varinfo {
            Some(list) if list.iter().any(|(k, _)| k == "ALL") => [
                "ALIGNMENT",
                "ATTRIBUTES",
                "FORMATS",
                "LEVEL",
                "MISSING",
                "ROLE",
                "VALLABELS",
                "VARLABEL",
                "WIDTH",
            ]
            .iter()
            .map(|k| {
                (
                    k.to_string(),
                    list.iter().find(|(n, _)| n == k).and_then(|e| e.1.clone()),
                )
            })
            .collect(),
            Some(list) => list.clone(),
            None => [
                "ALIGNMENT",
                "FORMATS",
                "LEVEL",
                "MISSING",
                "ROLE",
                "VALLABELS",
                "VARLABEL",
                "WIDTH",
            ]
            .iter()
            .map(|k| (k.to_string(), None))
            .collect(),
        };
        let wants = |key: &str| {
            varinfo
                .iter()
                .find(|(k, _)| k == key)
                .map(|e| e.1.as_deref())
        };
        let data = self
            .dataset
            .as_mut()
            .ok_or_else(|| Error("no active dataset".into()))?;
        let pairs: Vec<(String, String)> = if one_to_many {
            target_names
                .iter()
                .map(|t| (source_names[0].clone(), t.clone()))
                .collect()
        } else {
            source_names
                .iter()
                .filter_map(|s| {
                    let t = target_names.iter().find(|t| t.eq_ignore_ascii_case(s))?;
                    Some((s.clone(), t.clone()))
                })
                .collect()
        };
        for (s, t) in pairs {
            let from = source.variable(&s)?;
            let to = data.variable(&t)?;
            let to_string = to.is_string();
            let same_type = from.is_string() == to_string
                && (!to_string || from.string_width() == to.string_width() || !one_to_many);
            if wants("VARLABEL").is_some() && from.label().is_some() {
                data.set_variable_label(&t, from.label())?;
            }
            if !same_type {
                continue;
            }
            if let Some(mode) = wants("VALLABELS")
                && !from.value_labels().is_empty()
            {
                let mut labels = match mode {
                    Some("MERGE") => data.variable(&t)?.value_labels().to_vec(),
                    _ => Vec::new(),
                };
                for (value, label) in from.value_labels() {
                    labels.retain(|(v, _)| v != value);
                    labels.push((value.clone(), label.clone()));
                }
                data.set_value_labels(&t, labels)?;
            }
            if wants("MISSING").is_some() && *from.missing_values() != MissingValues::None {
                data.set_missing_values(&t, from.missing_values().clone())?;
            }
            if wants("LEVEL").is_some() {
                data.set_measure(&t, from.measure())?;
            }
            if wants("FORMATS").is_some() && !to_string {
                data.set_format(&t, from.format())?;
            }
            let to = data.variable(&t)?;
            let width = if wants("WIDTH").is_some() {
                from.display_width().or(to.display_width())
            } else {
                to.display_width()
            };
            let alignment = if wants("ALIGNMENT").is_some() {
                from.alignment().or(to.alignment())
            } else {
                to.alignment()
            };
            let role = if wants("ROLE").is_some() {
                from.role().or(to.role())
            } else {
                to.role()
            };
            data.set_display(&t, width, alignment, role)?;
            if let Some(mode) = wants("ATTRIBUTES")
                && !from.attributes().is_empty()
            {
                let mut attributes = match mode {
                    Some("MERGE") => data.variable(&t)?.attributes().to_vec(),
                    _ => Vec::new(),
                };
                for (name, values) in from.attributes() {
                    attributes.retain(|(n, _)| n != name);
                    attributes.push((name.clone(), values.clone()));
                }
                data.set_variable_attributes(&t, attributes)?;
            }
        }
        // file information, not when the source is the active dataset
        if apply.from.is_none() {
            return Ok(());
        }
        let fileinfo = &apply.fileinfo;
        let all = fileinfo.iter().any(|(k, _)| k == "ALL");
        let file_wants = |key: &str| {
            fileinfo
                .iter()
                .find(|(k, _)| k == key)
                .map(|e| e.1.as_deref())
                .or_else(|| all.then_some(None))
        };
        if (file_wants("WEIGHT").is_some() || fileinfo.is_empty())
            && let Some(weight) = source.weight_variable()
            && data.variable_index(weight).is_some()
        {
            data.set_weight_variable(Some(weight))?;
        }
        if let Some(mode) = file_wants("ATTRIBUTES")
            && !source.attributes().is_empty()
        {
            let mut attributes = match mode {
                Some("MERGE") => data.attributes().to_vec(),
                _ => Vec::new(),
            };
            for (name, values) in source.attributes() {
                attributes.retain(|(n, _)| n != name);
                attributes.push((name.clone(), values.clone()));
            }
            data.set_attributes(attributes);
        }
        if let Some(mode) = file_wants("DOCUMENTS")
            && !source.documents().is_empty()
        {
            let mut documents = match mode {
                Some("MERGE") => data.documents().to_vec(),
                _ => Vec::new(),
            };
            documents.extend(source.documents().iter().cloned());
            data.set_documents(documents);
        }
        if file_wants("FILELABEL").is_some() && source.file_label().is_some() {
            data.set_file_label(source.file_label());
        }
        let present =
            |data: &Dataset, vars: &[String]| vars.iter().all(|v| data.variable_index(v).is_some());
        if let Some(mode) = file_wants("MRSETS")
            && !source.multiple_response_sets().is_empty()
        {
            if mode != Some("MERGE") {
                let names: Vec<String> = data
                    .multiple_response_sets()
                    .iter()
                    .map(|s| s.name.clone())
                    .collect();
                let names: Vec<&str> = names.iter().map(String::as_str).collect();
                data.delete_multiple_response_sets(&names)?;
            }
            for set in source.multiple_response_sets() {
                if present(data, &set.variables) {
                    data.set_multiple_response_set(set.clone())?;
                }
            }
        }
        if let Some(mode) = file_wants("VARSETS")
            && !source.variable_sets().is_empty()
        {
            let mut sets = match mode {
                Some("MERGE") => data.variable_sets().to_vec(),
                _ => Vec::new(),
            };
            for set in source.variable_sets() {
                if present(data, &set.variables) {
                    sets.retain(|s| s.name != set.name);
                    sets.push(set.clone());
                }
            }
            data.set_variable_sets(sets)?;
        }
        Ok(())
    }

    /// A copy of the active dataset with GET's or SAVE's KEEP, DROP and RENAME applied.
    pub(crate) fn file_options(
        &self,
        data: Dataset,
        options: &[FileOption],
    ) -> Result<Dataset, Error> {
        if options.is_empty() {
            return Ok(data);
        }
        let mut engine = Engine::with_dataset(Some(data));
        for option in options {
            match option {
                FileOption::Rename(groups) => engine.rename_variables(groups)?,
                FileOption::Keep(items) => {
                    let mut names = engine.expand(items)?;
                    let mut seen = Vec::new();
                    names.retain(|n| {
                        let fresh = !seen.contains(n);
                        seen.push(n.clone());
                        fresh
                    });
                    let names: Vec<&str> = names.iter().map(String::as_str).collect();
                    engine.active_mut()?.keep_variables(&names)?;
                }
                FileOption::Drop(items) => {
                    let names = engine.expand(items)?;
                    let names: Vec<&str> = names.iter().map(String::as_str).collect();
                    engine.active_mut()?.delete_variables(&names)?;
                }
            }
        }
        Ok(engine.dataset.expect("set above"))
    }
}
