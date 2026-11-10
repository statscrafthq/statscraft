//! Dictionary record writers for SAV/ZSAV files.

use std::io::Write;

use indexmap::IndexMap;

use super::layout::{CaseLayout, WriteVariable};
use crate::constants::*;
use crate::encoding::{code_page_of, encode};
use crate::error::{Result, SavError};
use crate::io_utils::{self, SavWriteExt};
use crate::metadata::{MissingSpec, MrType, SavMetadata, Value};
use crate::variable::MissingValues;

// ---------------------------------------------------------------------------
// Date/time helpers
// ---------------------------------------------------------------------------

/// Return the current date and time in the file header format.
/// Date: "DD MMM YY" (9 bytes), Time: "HH:MM:SS" (8 bytes).
pub(super) fn current_date_time() -> (String, String) {
    use std::time::{SystemTime, UNIX_EPOCH};

    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert epoch seconds to date/time components.
    // Simple civil-time calculation (no leap seconds, good enough for timestamps).
    let secs_per_day = 86400u64;
    let time_of_day = secs % secs_per_day;
    let hh = time_of_day / 3600;
    let mm = (time_of_day % 3600) / 60;
    let ss = time_of_day % 60;

    // Days since 1970-01-01
    let mut days = (secs / secs_per_day) as i64;

    // Compute year/month/day from days since epoch
    let mut year = 1970i32;
    loop {
        let days_in_year = if is_leap(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let months_days: [i64; 12] = if is_leap(year) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 0usize;
    for (i, &md) in months_days.iter().enumerate() {
        if days < md {
            month = i;
            break;
        }
        days -= md;
    }
    let day = days + 1;

    const MONTH_ABBR: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];

    let yy = year % 100;
    let date = format!("{:02} {} {:02}", day, MONTH_ABBR[month], yy);
    let time = format!("{:02}:{:02}:{:02}", hh, mm, ss);
    (date, time)
}

fn is_leap(y: i32) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

// ---------------------------------------------------------------------------
// Dictionary writers
// ---------------------------------------------------------------------------

pub(super) fn write_header<W: Write>(
    w: &mut W,
    layout: &CaseLayout,
    meta: &SavMetadata,
    compression: Compression,
    nrows: i32,
) -> Result<()> {
    // Magic: "$FL2" for sav, "$FL3" for zsav
    let magic = if compression == Compression::Zlib {
        b"$FL3"
    } else {
        b"$FL2"
    };
    w.write_all(magic)?;

    // Product name (60 bytes, space-padded)
    let product = format!("@(#) StatsCraft data file {}", env!("CARGO_PKG_VERSION"));
    w.write_fixed_string(&product, 60)?;

    // Layout code (2 = little-endian)
    w.write_i32_le(2)?;

    // Nominal case size (total slots per row)
    w.write_i32_le(layout.slots_per_row as i32)?;

    // Compression
    w.write_i32_le(compression.to_i32())?;

    // Weight index (0 = no weight, else 1-based slot index)
    let weight_index = meta
        .weight_variable
        .as_ref()
        .and_then(|wv| {
            layout
                .write_vars
                .iter()
                .scan(0usize, |slot, var| {
                    let this_slot = *slot;
                    *slot += var.total_slots();
                    Some((var, this_slot))
                })
                .find(|(v, _)| v.long_name == *wv)
                .map(|(_, slot)| (slot + 1) as i32) // 1-based
        })
        .unwrap_or(0);
    w.write_i32_le(weight_index)?;

    // Number of cases
    w.write_i32_le(nrows)?;

    // Bias
    w.write_f64_le(DEFAULT_BIAS)?;

    // File header: date (9 bytes "DD MMM YY") + time (8 bytes "HH:MM:SS").
    // meta.creation_time is now a full datetime like "2026-02-21 12:38:47".
    // Preserve on roundtrip; use current UTC for new files.
    let (date_now, time_now) = current_date_time();
    if let Some((date_part, time_part)) =
        crate::metadata::parse_iso_to_sav_parts(&meta.creation_time)
    {
        w.write_fixed_string(&date_part, 9)?;
        w.write_fixed_string(&time_part, 8)?;
    } else {
        w.write_fixed_string(&date_now, 9)?;
        w.write_fixed_string(&time_now, 8)?;
    }

    // File label: the header has exactly 64 bytes for it and no other record holds one
    let label = encode(&meta.file_label, layout.encoding)?;
    if label.len() > 64 {
        return Err(SavError::WriteError(format!(
            "the file label takes {} bytes in {}; a file label holds at most 64",
            label.len(),
            layout.encoding.name()
        )));
    }
    w.write_fixed_bytes(&label, 64)?;

    // Padding (3 bytes)
    w.write_zero_padding(3)?;

    Ok(())
}

pub(super) fn write_variable_records<W: Write>(w: &mut W, layout: &CaseLayout) -> Result<()> {
    for rec in &layout.slot_records {
        // Record type = 2
        w.write_i32_le(RECORD_TYPE_VARIABLE)?;

        // Raw type
        w.write_i32_le(rec.raw_type)?;

        // Has variable label
        let has_label = if rec.label.is_some() && !rec.is_ghost {
            1
        } else {
            0
        };
        w.write_i32_le(has_label)?;

        // Number of missing values
        let n_missing = missing_count(&rec.missing_values);
        w.write_i32_le(n_missing)?;

        // Print format and write format
        if rec.is_ghost {
            w.write_i32_le(0)?;
            w.write_i32_le(0)?;
        } else {
            w.write_i32_le(rec.print_format.to_packed())?;
            w.write_i32_le(rec.write_format.to_packed())?;
        }

        // Short name (8 bytes)
        if rec.is_ghost {
            w.write_all(&[b' '; 8])?;
        } else {
            w.write_fixed_string(&rec.short_name, 8)?;
        }

        // Variable label (if present and not ghost)
        if has_label == 1 {
            let label_bytes = rec.label.as_ref().unwrap();
            let label_len = label_bytes.len();
            w.write_i32_le(label_len as i32)?;
            w.write_all(label_bytes)?;
            // Pad to 4-byte boundary
            let padded_len = io_utils::round_up(label_len, 4);
            if padded_len > label_len {
                w.write_zero_padding(padded_len - label_len)?;
            }
        }

        // Missing values
        write_missing_values(w, &rec.missing_values)?;
    }
    Ok(())
}

fn missing_count(mv: &MissingValues) -> i32 {
    match mv {
        MissingValues::None => 0,
        MissingValues::DiscreteNumeric(vals) => vals.len() as i32,
        MissingValues::Range { .. } => -2,
        MissingValues::RangeAndValue { .. } => -3,
        MissingValues::DiscreteString(vals) => vals.len() as i32,
    }
}

fn write_missing_values<W: Write>(w: &mut W, mv: &MissingValues) -> Result<()> {
    match mv {
        MissingValues::None => {}
        MissingValues::DiscreteNumeric(vals) => {
            for &v in vals {
                w.write_f64_le(v)?;
            }
        }
        MissingValues::Range { low, high } => {
            w.write_f64_le(*low)?;
            w.write_f64_le(*high)?;
        }
        MissingValues::RangeAndValue { low, high, value } => {
            w.write_f64_le(*low)?;
            w.write_f64_le(*high)?;
            w.write_f64_le(*value)?;
        }
        MissingValues::DiscreteString(vals) => {
            for v in vals {
                let mut buf = [b' '; 8];
                let len = v.len().min(8);
                buf[..len].copy_from_slice(&v[..len]);
                w.write_all(&buf)?;
            }
        }
    }
    Ok(())
}

pub(super) fn write_value_label_records<W: Write>(
    w: &mut W,
    layout: &CaseLayout,
    meta: &SavMetadata,
) -> Result<()> {
    // Track cumulative slot index for each variable
    let mut var_slot_indices: Vec<(usize, &WriteVariable)> = Vec::new();
    let mut slot_idx = 0usize;
    for var in &layout.write_vars {
        var_slot_indices.push((slot_idx, var));
        slot_idx += var.total_slots();
    }

    // Group variables that share the same value label set
    // In the format, Type 3+4 pairs can reference multiple variables with the same labels.
    // For simplicity (and correctness), emit one Type 3+4 pair per variable.
    for &(slot, var) in &var_slot_indices {
        let labels = match meta.variable_value_labels.get(&var.long_name) {
            Some(labels) if !labels.is_empty() => labels,
            _ => continue,
        };

        // Type 3+4 only works for short strings (width <= 8) and numerics.
        // Long string value labels go to subtype 21.
        let is_long_string = matches!(&var.var_type, VarType::String(w) if *w > 8);
        if is_long_string {
            continue;
        }

        // Type 3: value labels
        w.write_i32_le(RECORD_TYPE_VALUE_LABEL)?;
        w.write_i32_le(labels.len() as i32)?;

        for (val, label) in labels {
            // Write 8-byte value
            match val {
                Value::Numeric(v) => {
                    w.write_f64_le(*v)?;
                }
                Value::String(s) => w.write_fixed_bytes(&encode(s, layout.encoding)?, 8)?,
            }

            // Label length (1 byte) + label text + padding to 8-byte boundary
            let label_bytes = encode(label, layout.encoding)?;
            let label_len = label_bytes.len();
            if label_len > 255 {
                return Err(SavError::WriteError(format!(
                    "variable '{}': value label {label:?} is longer than 255 bytes",
                    var.long_name
                )));
            }
            w.write_all(&[label_len as u8])?;
            w.write_all(&label_bytes)?;
            // Pad: total = 1 + label_len, round up to nearest multiple of 8
            let total = 1 + label_len;
            let padded = io_utils::round_up(total, 8);
            if padded > total {
                w.write_zero_padding(padded - total)?;
            }
        }

        // Type 4: variable indices
        w.write_i32_le(RECORD_TYPE_VALUE_LABEL_VARS)?;
        w.write_i32_le(1)?; // one variable
        w.write_i32_le((slot + 1) as i32)?; // 1-based slot index
    }

    Ok(())
}

pub(super) fn write_document_record<W: Write>(
    w: &mut W,
    meta: &SavMetadata,
    layout: &CaseLayout,
) -> Result<()> {
    if meta.notes.is_empty() {
        return Ok(());
    }

    // Split notes into 80-byte lines
    let mut lines: Vec<String> = Vec::new();
    for note in &meta.notes {
        // Each note can be one line
        lines.push(note.clone());
    }

    w.write_i32_le(RECORD_TYPE_DOCUMENT)?;
    w.write_i32_le(lines.len() as i32)?;

    for line in &lines {
        w.write_fixed_bytes(&encode(line, layout.encoding)?, 80)?;
    }

    Ok(())
}

fn write_info_record_header<W: Write>(
    w: &mut W,
    subtype: i32,
    elem_size: i32,
    count: i32,
) -> Result<()> {
    w.write_i32_le(RECORD_TYPE_INFO)?;
    w.write_i32_le(subtype)?;
    w.write_i32_le(elem_size)?;
    w.write_i32_le(count)?;
    Ok(())
}

pub(super) fn write_info_integer<W: Write>(
    w: &mut W,
    _compression: Compression,
    layout: &CaseLayout,
) -> Result<()> {
    write_info_record_header(w, INFO_INTEGER, 4, 8)?;
    w.write_i32_le(28)?; // version_major
    w.write_i32_le(0)?; // version_minor
    w.write_i32_le(0)?; // version_revision
    w.write_i32_le(-1)?; // machine_code
    w.write_i32_le(1)?; // floating_point_rep (IEEE 754)
    w.write_i32_le(1)?; // compression_code: always 1 per PSPP spec, regardless of actual compression
    w.write_i32_le(2)?; // endianness (LE)
    w.write_i32_le(code_page_of(layout.encoding))?;
    Ok(())
}

pub(super) fn write_info_float<W: Write>(w: &mut W) -> Result<()> {
    write_info_record_header(w, INFO_FLOAT, 8, 3)?;
    w.write_f64_le(f64::from_bits(SYSMIS_BITS))?;
    w.write_f64_le(f64::from_bits(HIGHEST_BITS))?;
    w.write_f64_le(f64::from_bits(LOWEST_BITS))?;
    Ok(())
}

pub(super) fn write_info_var_display<W: Write>(w: &mut W, layout: &CaseLayout) -> Result<()> {
    // Count non-ghost slot records
    let n_vars = layout.slot_records.iter().filter(|r| !r.is_ghost).count();
    write_info_record_header(w, INFO_VAR_DISPLAY, 4, (n_vars * 3) as i32)?;

    // Emit measure/width/alignment for each non-ghost record
    let mut var_idx = 0;
    for rec in &layout.slot_records {
        if rec.is_ghost {
            continue;
        }
        // Find the corresponding WriteVariable for this non-ghost record
        // For VLS segments beyond the first, use defaults
        let (measure, display_width, alignment) = if var_idx < layout.write_vars.len() {
            let var = &layout.write_vars[var_idx];
            // Only advance var_idx for the primary segment
            if rec.short_name == var.short_name {
                var_idx += 1;
                (var.measure, var.display_width, var.alignment)
            } else {
                // VLS continuation segment
                (Measure::Unknown, 255, Alignment::Left)
            }
        } else {
            (Measure::Unknown, 8, Alignment::Left)
        };
        w.write_i32_le(measure.to_i32())?;
        w.write_i32_le(display_width as i32)?;
        w.write_i32_le(alignment.to_i32())?;
    }
    Ok(())
}

pub(super) fn write_info_long_names<W: Write>(w: &mut W, layout: &CaseLayout) -> Result<()> {
    // Format: "SHORT=LongName\tSHORT2=LongName2\t..."
    let mut payload = String::new();
    for (short, long) in &layout.short_to_long {
        if !payload.is_empty() {
            payload.push('\t');
        }
        payload.push_str(short);
        payload.push('=');
        payload.push_str(long);
    }

    if payload.is_empty() {
        return Ok(());
    }

    let payload = encode(&payload, layout.encoding)?;
    write_info_record_header(w, INFO_LONG_NAMES, 1, payload.len() as i32)?;
    w.write_all(&payload)?;
    Ok(())
}

pub(super) fn write_info_very_long_strings<W: Write>(w: &mut W, layout: &CaseLayout) -> Result<()> {
    if layout.very_long_strings.is_empty() {
        return Ok(());
    }

    // Format: "SHORTNAME=WIDTH\0\tSHORTNAME2=WIDTH2\0\t"
    let mut payload = String::new();
    for (short_name, width) in &layout.very_long_strings {
        payload.push_str(short_name);
        payload.push('=');
        payload.push_str(&format!("{:05}", width));
        payload.push('\0');
        payload.push('\t');
    }

    write_info_record_header(w, INFO_VERY_LONG_STRINGS, 1, payload.len() as i32)?;
    w.write_all(payload.as_bytes())?;
    Ok(())
}

pub(super) fn write_info_encoding<W: Write>(w: &mut W, layout: &CaseLayout) -> Result<()> {
    let encoding = layout.encoding.name().as_bytes();
    write_info_record_header(w, INFO_ENCODING, 1, encoding.len() as i32)?;
    w.write_all(encoding)?;
    Ok(())
}

pub(super) fn write_info_long_string_labels<W: Write>(
    w: &mut W,
    layout: &CaseLayout,
    meta: &SavMetadata,
) -> Result<()> {
    // Collect long string variables (width > 8) that have value labels
    let mut entries: Vec<(&str, usize, &IndexMap<Value, String>)> = Vec::new();
    for var in &layout.write_vars {
        if matches!(&var.var_type, VarType::String(w) if *w > 8)
            && let Some(labels) = meta.variable_value_labels.get(&var.long_name)
            && !labels.is_empty()
        {
            entries.push((&var.long_name, var.storage_width, labels));
        }
    }
    if entries.is_empty() {
        return Ok(());
    }

    // Build payload in pascal-string format
    let mut payload = Vec::new();
    for (var_name, var_width, labels) in &entries {
        // Variable name: length-prefixed
        let name_bytes = encode(var_name, layout.encoding)?;
        payload.extend_from_slice(&(name_bytes.len() as i32).to_le_bytes());
        payload.extend_from_slice(&name_bytes);

        // Variable width
        payload.extend_from_slice(&(*var_width as i32).to_le_bytes());

        // Number of labels
        payload.extend_from_slice(&(labels.len() as i32).to_le_bytes());

        for (val, label) in *labels {
            // Value: length-prefixed, space-padded to var_width
            let val_str = match val {
                Value::String(s) => s.clone(),
                Value::Numeric(n) => n.to_string(),
            };
            let val_bytes = encode(&val_str, layout.encoding)?;
            let padded_len = *var_width;
            payload.extend_from_slice(&(padded_len as i32).to_le_bytes());
            let copy_len = val_bytes.len().min(padded_len);
            payload.extend_from_slice(&val_bytes[..copy_len]);
            // Pad with spaces to var_width
            if copy_len < padded_len {
                payload.extend(std::iter::repeat_n(b' ', padded_len - copy_len));
            }

            // Label: length-prefixed
            let label_bytes = encode(label, layout.encoding)?;
            if label_bytes.len() > 255 {
                return Err(SavError::WriteError(format!(
                    "variable '{var_name}': value label {label:?} is longer than 255 bytes"
                )));
            }
            payload.extend_from_slice(&(label_bytes.len() as i32).to_le_bytes());
            payload.extend_from_slice(&label_bytes);
        }
    }

    write_info_record_header(w, INFO_LONG_STRING_LABELS, 1, payload.len() as i32)?;
    w.write_all(&payload)?;
    Ok(())
}

pub(super) fn write_info_long_string_missing<W: Write>(
    w: &mut W,
    layout: &CaseLayout,
    meta: &SavMetadata,
) -> Result<()> {
    // Collect long string variables (width > 8) that have missing values.
    // Per PSPP spec: use SHORT name, value_len = 8, each value is 8 bytes space-padded.
    let mut entries: Vec<(&str, &Vec<MissingSpec>)> = Vec::new();
    for var in &layout.write_vars {
        if matches!(&var.var_type, VarType::String(w) if *w > 8)
            && let Some(specs) = meta.variable_missing_values.get(&var.long_name)
            && !specs.is_empty()
        {
            entries.push((&var.short_name, specs));
        }
    }
    if entries.is_empty() {
        return Ok(());
    }

    // Build payload
    let mut payload = Vec::new();
    for (var_name, specs) in &entries {
        let name_bytes = var_name.as_bytes();
        payload.extend_from_slice(&(name_bytes.len() as i32).to_le_bytes());
        payload.extend_from_slice(name_bytes);

        // Number of missing values
        let n_missing: u8 = specs.len() as u8;
        payload.push(n_missing);

        // Value width: always 8 per PSPP spec (string missing values are max 8 chars)
        payload.extend_from_slice(&8_i32.to_le_bytes());

        for spec in *specs {
            if let MissingSpec::StringValue(s) = spec {
                // Each value is exactly 8 bytes, space-padded
                let val_bytes = encode(s, layout.encoding)?;
                let mut buf = [b' '; 8];
                let len = val_bytes.len().min(8);
                buf[..len].copy_from_slice(&val_bytes[..len]);
                payload.extend_from_slice(&buf);
            }
        }
    }

    write_info_record_header(w, INFO_LONG_STRING_MISSING, 1, payload.len() as i32)?;
    w.write_all(&payload)?;
    Ok(())
}

pub(super) fn write_info_mr_sets<W: Write>(
    w: &mut W,
    meta: &SavMetadata,
    layout: &CaseLayout,
) -> Result<()> {
    let long_to_short: IndexMap<&str, &str> = layout
        .short_to_long
        .iter()
        .map(|(short, long)| (long.as_str(), short.as_str()))
        .collect();
    // One line per set (lengths count bytes in the file encoding):
    //   $name=C label_len label vars                     category set
    //   $name=Dn value label_len label vars              dichotomy set, n = value length
    //   $name=E 1 n value label_len label vars           CATEGORYLABELS=COUNTEDVALUES
    //   $name=E 11 n value label_len label vars          ... with LABELSOURCE=VARLABEL
    // with the variables by short name in lower case.
    let (mut classic, mut extended) = (Vec::new(), Vec::new());
    for (name, mr) in &meta.mr_sets {
        let counted = encode(mr.counted_value.as_deref().unwrap_or("1"), layout.encoding)?;
        let label = encode(&mr.label, layout.encoding)?;
        let mut line = b"$".to_vec();
        line.extend_from_slice(&encode(name, layout.encoding)?);
        let payload = match mr.mr_type {
            MrType::MultipleCategory => {
                line.extend_from_slice(b"=C ");
                &mut classic
            }
            MrType::MultipleDichotomy if mr.counted_value_labels => {
                let code = if mr.label_from_variables { 11 } else { 1 };
                line.extend_from_slice(format!("=E {code} {} ", counted.len()).as_bytes());
                line.extend_from_slice(&counted);
                line.push(b' ');
                &mut extended
            }
            MrType::MultipleDichotomy => {
                line.extend_from_slice(format!("=D{} ", counted.len()).as_bytes());
                line.extend_from_slice(&counted);
                line.push(b' ');
                &mut classic
            }
        };
        line.extend_from_slice(format!("{} ", label.len()).as_bytes());
        line.extend_from_slice(&label);
        for var in &mr.variables {
            let short = long_to_short.get(var.as_str()).copied().unwrap_or(var);
            line.push(b' ');
            line.extend_from_slice(&encode(&short.to_lowercase(), layout.encoding)?);
        }
        line.push(b'\n');
        payload.extend_from_slice(&line);
    }
    for (subtype, payload) in [(INFO_MR_SETS, classic), (INFO_MR_SETS_V2, extended)] {
        if !payload.is_empty() {
            write_info_record_header(w, subtype, 1, payload.len() as i32)?;
            w.write_all(&payload)?;
        }
    }
    Ok(())
}

/// Subtype 5: one line per variable set, `name= var1 var2`, the variables by short name.
pub(super) fn write_info_variable_sets<W: Write>(
    w: &mut W,
    meta: &SavMetadata,
    layout: &CaseLayout,
) -> Result<()> {
    let long_to_short: IndexMap<&str, &str> = layout
        .short_to_long
        .iter()
        .map(|(short, long)| (long.as_str(), short.as_str()))
        .collect();
    let mut payload = String::new();
    for (name, members) in &meta.variable_sets {
        payload.push_str(name);
        payload.push('=');
        for member in members {
            payload.push(' ');
            payload.push_str(
                long_to_short
                    .get(member.as_str())
                    .copied()
                    .unwrap_or(member),
            );
        }
        payload.push('\n');
    }
    if payload.is_empty() {
        return Ok(());
    }
    let payload = encode(&payload, layout.encoding)?;
    write_info_record_header(w, INFO_VARIABLE_SETS, 1, payload.len() as i32)?;
    w.write_all(&payload)?;
    Ok(())
}

/// `name('value'\n'value'\n)` for each attribute, the text of subtypes 17 and 18.
fn attribute_text(payload: &mut String, attributes: &IndexMap<String, Vec<String>>) {
    for (name, values) in attributes {
        if values.is_empty() {
            continue; // the grammar wants at least one value
        }
        payload.push_str(name);
        payload.push('(');
        for value in values {
            payload.push('\'');
            payload.push_str(value);
            payload.push_str("'\n");
        }
        payload.push(')');
    }
}

/// Subtype 17: the data file attributes.
pub(super) fn write_info_file_attributes<W: Write>(
    w: &mut W,
    meta: &SavMetadata,
    layout: &CaseLayout,
) -> Result<()> {
    let mut payload = String::new();
    attribute_text(&mut payload, &meta.file_attributes);
    if payload.is_empty() {
        return Ok(());
    }
    let payload = encode(&payload, layout.encoding)?;
    write_info_record_header(w, INFO_FILE_ATTRIBUTES, 1, payload.len() as i32)?;
    w.write_all(&payload)?;
    Ok(())
}

pub(super) fn write_info_var_attributes<W: Write>(
    w: &mut W,
    meta: &SavMetadata,
    layout: &CaseLayout,
) -> Result<()> {
    if meta.variable_roles.is_empty() && meta.variable_attributes.is_empty() {
        return Ok(());
    }

    // Collect all variable names that have either a role or custom attributes
    let mut all_vars: IndexMap<&str, ()> = IndexMap::new();
    for name in meta.variable_roles.keys() {
        all_vars.insert(name.as_str(), ());
    }
    for name in meta.variable_attributes.keys() {
        all_vars.insert(name.as_str(), ());
    }

    // Build text payload in subtype 18 format (uses LONG names per PSPP spec):
    //   varname:CustomAttr('val'\n)$@Role('code'\n)/varname2:...
    let mut payload = String::new();
    for (long_name, _) in &all_vars {
        if !payload.is_empty() {
            payload.push('/');
        }
        payload.push_str(long_name);
        payload.push(':');

        // Write custom attributes first
        if let Some(attrs) = meta.variable_attributes.get(*long_name) {
            attribute_text(&mut payload, attrs);
        }

        // Write $@Role
        if let Some(role) = meta.variable_roles.get(*long_name) {
            payload.push_str("$@Role('");
            payload.push_str(role.to_code());
            payload.push_str("'\n)");
        }
    }

    let payload = encode(&payload, layout.encoding)?;
    write_info_record_header(w, INFO_VAR_ATTRIBUTES, 1, payload.len() as i32)?;
    w.write_all(&payload)?;
    Ok(())
}

pub(super) fn write_dict_termination<W: Write>(w: &mut W) -> Result<()> {
    w.write_i32_le(RECORD_TYPE_DICT_TERMINATION)?;
    w.write_i32_le(0)?; // filler
    Ok(())
}
