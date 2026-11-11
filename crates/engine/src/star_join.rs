//! STAR JOIN: every case of the FROM source with the fields of the JOIN sources whose keys
//! match, as an SQL left outer join.
//!
//! Rules from the Command Syntax Reference (STAR JOIN):
//! - Every case of FROM is kept; a JOIN adds values only to the cases whose keys it holds. The
//!   files need not be sorted, and every JOIN has keys of its own.
//! - The keys of a JOIN are unique there (the combination, with several); a repeated key is an
//!   error. A key pair is numeric with numeric or string with string; string widths may differ.
//! - The result holds the SELECT fields and every key. SELECT * is every field that is not a
//!   key. A JOIN key named on SELECT is ignored there; one with another name than its FROM key
//!   is kept as a field of its own.
//! - A name that two sources bring gets the alias and a period in front: t1.name, t2.name.
//! - IN=name is 1 where the JOIN has the case, 0 where not.
//! - A variable in several files (a key of the same name) takes its dictionary from the first
//!   file with a variable label, value labels, missing values or attributes for it; the file
//!   attributes come from the first file with some; the documents of every file are kept and
//!   no file label is.
//! - OUTFILE=* replaces the active dataset, a dataset name fills that dataset, a quoted name
//!   writes a data file. PASSPROTECT is ignored unless the output is a file.
//!
//! Choices where the manual is silent:
//! - A system-missing key matches nothing, as NULL in SQL; user-missing values match as values.
//! - Keys not named on SELECT come first (FROM keys, then JOIN keys of their own name), then
//!   the SELECT fields in order, then the IN variables. SELECT * keeps every field in its
//!   source's order, FROM first.
//! - The dictionary of the result (weight, filter, split, multiple response sets) is that of
//!   FROM, as far as its variables remain.
//! - PASSPROTECT with a password for an output file is an error: Statscraft writes no
//!   encrypted files, and a file believed to be protected must not be written in the clear.
//! - After TEMPORARY, STAR JOIN is refused, as MATCH FILES is.

use crate::{Engine, Error};
use statscraft_data::{Dataset, MissingValues, Values};
use statscraft_syntax::StarJoin;
use std::collections::HashMap;

/// A key value that can be looked up: numbers by their bits, strings without trailing blanks.
#[derive(Hash, PartialEq, Eq)]
enum KeyPart {
    Number(u64),
    Text(String),
}

/// The key of `case` in the `names` variables of `data`; None when a number is system-missing.
fn key(data: &Dataset, names: &[&str], case: usize) -> Result<Option<Vec<KeyPart>>, Error> {
    let mut parts = Vec::with_capacity(names.len());
    for name in names {
        let part = match data.variable(name)?.values() {
            // + 0.0 makes -0 and 0 one key
            Values::Numeric(values) => match values.get(case) {
                Some(v) => KeyPart::Number((v + 0.0).to_bits()),
                None => return Ok(None),
            },
            Values::String(values) => KeyPart::Text(values.get(case).trim_end().to_string()),
        };
        parts.push(part);
    }
    Ok(Some(parts))
}

/// One variable of the result: where it comes from and its name there and in the result.
struct Field {
    source: usize,
    name: String,
    out: String,
}

/// Renames the variables of `data` from the first to the second name of each pair, by way of
/// scratch names so that swapped names do not collide.
fn rename_all(data: &mut Dataset, pairs: &[(&str, &str)]) -> Result<(), Error> {
    for (i, (from, _)) in pairs.iter().enumerate() {
        data.rename_variable(from, &format!("#starjoin{i}"))?;
    }
    for (i, (_, to)) in pairs.iter().enumerate() {
        data.rename_variable(&format!("#starjoin{i}"), to)?;
    }
    Ok(())
}

impl Engine {
    pub(crate) fn star_join(&mut self, join: &StarJoin) -> Result<(), Error> {
        let sources = &join.sources;
        let data: Vec<Dataset> = sources
            .iter()
            .map(|s| match &s.file {
                None => self.active().cloned(),
                Some(file) => self.data_file(file),
            })
            .collect::<Result<_, _>>()?;
        let alias = |s: usize| sources[s].alias.as_str();
        let check = |s: usize, name: &str| {
            data[s].variable(name).map(|_| ()).map_err(|_| {
                let what = sources[s].file.as_deref().unwrap_or("the active dataset");
                Error(format!(
                    "{}.{name}: {what} has no variable {name}",
                    alias(s)
                ))
            })
        };

        // the keys, checked, and for every JOIN the matching case of each FROM case
        let mut matches: Vec<Vec<Option<usize>>> = vec![Vec::new()];
        for (j, source) in sources.iter().enumerate().skip(1) {
            for (from, to) in &source.on {
                check(0, from)?;
                check(j, to)?;
                let (a, b) = (data[0].variable(from)?, data[j].variable(to)?);
                if a.is_string() != b.is_string() {
                    let kind = |string: bool| if string { "a string" } else { "numeric" };
                    return Err(Error(format!(
                        "the key {}.{from} is {} but {}.{to} is {}; a key pair must be of one type",
                        alias(0),
                        kind(a.is_string()),
                        alias(j),
                        kind(b.is_string())
                    )));
                }
            }
            let (from, to): (Vec<&str>, Vec<&str>) = source
                .on
                .iter()
                .map(|(a, b)| (a.as_str(), b.as_str()))
                .unzip();
            let mut index: HashMap<Vec<KeyPart>, usize> = HashMap::new();
            for case in 0..data[j].case_count() {
                let Some(k) = key(&data[j], &to, case)? else {
                    continue;
                };
                if let Some(first) = index.insert(k, case) {
                    return Err(Error(format!(
                        "the keys of {} ({}) must be unique there, but cases {} and {} hold the same key",
                        alias(j),
                        to.join(", "),
                        first + 1,
                        case + 1
                    )));
                }
            }
            let found = (0..data[0].case_count())
                .map(|case| Ok(key(&data[0], &from, case)?.and_then(|k| index.get(&k).copied())))
                .collect::<Result<Vec<_>, Error>>()?;
            matches.push(found);
        }

        // the fields: unlisted keys first, then SELECT, each from its source once
        let same_key = |s: usize, name: &str| {
            sources[s]
                .on
                .iter()
                .any(|(f, t)| t.eq_ignore_ascii_case(name) && f.eq_ignore_ascii_case(name))
        };
        let mut keys: Vec<(usize, &str)> = Vec::new();
        for source in &sources[1..] {
            for (from, _) in &source.on {
                keys.push((0, from));
            }
        }
        for (j, source) in sources.iter().enumerate().skip(1) {
            for (_, to) in &source.on {
                if !same_key(j, to) {
                    keys.push((j, to));
                }
            }
        }
        let mut listed: Vec<(usize, String, Option<String>)> = match &join.select {
            Some(fields) => fields
                .iter()
                .map(|f| (f.source, f.name.clone(), f.rename.clone()))
                .collect(),
            None => (0..sources.len())
                .flat_map(|s| {
                    data[s]
                        .variable_names()
                        .map(move |n| (s, n.to_string(), None))
                })
                .collect(),
        };
        // a JOIN key of the name of its FROM key is that key
        listed.retain(|(s, name, _)| *s == 0 || !same_key(*s, name));
        for (s, name, _) in &listed {
            check(*s, name)?;
        }
        let is_listed = |s: usize, name: &str, listed: &[(usize, String, Option<String>)]| {
            listed
                .iter()
                .any(|(t, n, _)| *t == s && n.eq_ignore_ascii_case(name))
        };
        let mut first: Vec<(usize, String, Option<String>)> = Vec::new();
        for (s, name) in keys {
            if !is_listed(s, name, &listed) && !is_listed(s, name, &first) {
                first.push((s, name.to_string(), None));
            }
        }
        first.extend(listed);
        // the name in the source as the dictionary writes it
        let mut fields: Vec<Field> = first
            .into_iter()
            .map(|(source, name, rename)| {
                let name = data[source].variable(&name).map(|v| v.name().to_string())?;
                Ok(Field {
                    source,
                    out: rename.unwrap_or_default(),
                    name,
                })
            })
            .collect::<Result<_, Error>>()?;
        // the names the sources bring into the result without AS
        let brought: Vec<(usize, String)> = fields
            .iter()
            .filter(|f| f.out.is_empty())
            .map(|f| (f.source, f.name.to_lowercase()))
            .collect();
        for field in &mut fields {
            if field.out.is_empty() {
                let lower = field.name.to_lowercase();
                let twice = brought
                    .iter()
                    .any(|(s, n)| *s != field.source && *n == lower);
                field.out = match twice {
                    true => format!("{}.{}", alias(field.source), field.name),
                    false => field.name.clone(),
                };
            }
        }
        let flags: Vec<(usize, &str)> = (1..sources.len())
            .filter_map(|j| sources[j].flag.as_deref().map(|f| (j, f)))
            .collect();
        let mut names: Vec<&str> = fields.iter().map(|f| f.out.as_str()).collect();
        names.extend(flags.iter().map(|(_, f)| *f));
        for (i, name) in names.iter().enumerate() {
            if names[..i].iter().any(|n| n.eq_ignore_ascii_case(name)) {
                return Err(Error(format!(
                    "STAR JOIN would make two variables named {name}; give one another name with AS"
                )));
            }
        }

        // the FROM source with its fields, then the fields of each JOIN
        let mut out = data[0].clone();
        let own: Vec<&Field> = fields.iter().filter(|f| f.source == 0).collect();
        let kept: Vec<&str> = own.iter().map(|f| f.name.as_str()).collect();
        out.keep_variables(&kept)?;
        let pairs: Vec<(&str, &str)> = own
            .iter()
            .map(|f| (f.name.as_str(), f.out.as_str()))
            .collect();
        rename_all(&mut out, &pairs)?;
        for j in 1..sources.len() {
            let theirs: Vec<&Field> = fields.iter().filter(|f| f.source == j).collect();
            if theirs.is_empty() {
                continue;
            }
            let mut lookup = data[j].clone();
            let kept: Vec<&str> = theirs.iter().map(|f| f.name.as_str()).collect();
            lookup.keep_variables(&kept)?;
            let pairs: Vec<(&str, &str)> = theirs
                .iter()
                .map(|f| (f.name.as_str(), f.out.as_str()))
                .collect();
            rename_all(&mut lookup, &pairs)?;
            for field in theirs {
                out.copy_variable(&lookup, &field.out, &matches[j])?;
            }
        }
        for (j, flag) in &flags {
            let values = matches[*j]
                .iter()
                .map(|m| Some(f64::from(u8::from(m.is_some()))))
                .collect();
            out.set_numeric_values(flag, values)?;
            out.set_format(flag, "F1.0")?;
        }
        out.keep_variables(&names)?;

        // a FROM key without dictionary entries takes those of the first JOIN key of its name
        for field in fields.iter().filter(|f| f.source == 0) {
            let bare = |v: &statscraft_data::Variable| {
                v.label().is_none()
                    && v.value_labels().is_empty()
                    && *v.missing_values() == MissingValues::None
                    && v.attributes().is_empty()
            };
            if !bare(&out.variable(&field.out)?) {
                continue;
            }
            let donor = (1..sources.len())
                .filter(|&j| same_key(j, &field.name))
                .filter_map(|j| data[j].variable(&field.name).ok())
                .find(|v| !bare(v));
            if let Some(v) = donor {
                let label = v.label().map(str::to_string);
                out.set_variable_label(&field.out, label.as_deref())?;
                out.set_value_labels(&field.out, v.value_labels().to_vec())?;
                out.set_missing_values(&field.out, v.missing_values().clone())?;
                out.set_variable_attributes(&field.out, v.attributes().to_vec())?;
            }
        }
        // documents of every file, each file once; file attributes from the first with some
        let mut documents = Vec::new();
        for (s, source) in sources.iter().enumerate() {
            if sources[..s].iter().all(|o| o.file != source.file) {
                documents.extend(data[s].documents().iter().cloned());
            }
        }
        out.set_documents(documents);
        if let Some(d) = data.iter().find(|d| !d.attributes().is_empty()) {
            out.set_attributes(d.attributes().to_vec());
        }
        out.set_file_label(None);

        match &join.outfile {
            Some(name) if !self.is_active_name(name) => {
                if join.encrypt && !self.is_dataset_name(name) {
                    return Err(Error(format!(
                        "/PASSPROTECT: Statscraft cannot write encrypted data files, so {name} was not \
                         written; leave PASSPROTECT out or give PASSWORD=NONE"
                    )));
                }
                self.write_data_file(name, out)
            }
            _ => {
                self.dataset = Some(out);
                Ok(())
            }
        }
    }
}
