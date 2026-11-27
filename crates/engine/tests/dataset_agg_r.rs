//! AGGREGATE MODE=ADDVARIABLES and the DATASET commands against R (dplyr) on 20 invented cases with a
//! missing key, a group whose x are all missing, an unsorted file and two key variables, a number and a string.
//! ADDVARIABLES: every case gets the value of its group (N, SUM, MEAN, SD, MIN, MAX, NMISS, FIRST
//! and LAST), the file keeps its order, a missing key is a group of its own, no break makes the
//! whole file one group, SELECT IF before it leaves only the cases that remain to be grouped, and
//! the new variables can be used in the next COMPUTE.
//! DATASET: NAME, DECLARE, COPY, ACTIVATE and CLOSE with AGGREGATE into a dataset of its own: the
//! datasets are saved as .sav files; the engine's read-back of each file equals what R made with
//! dplyr; haven was shown the very files and read the same values (set `SAV_DIR` to keep them for
//! that check; the test itself does not run R).
//! What R computes itself: the group statistics (`n`, `sum`, `mean`, `sd`, `min`, `max`, `first`,
//! `last` in `group_by` with `mutate` or `summarise`). What follows the manual, written out in the R
//! script: a missing key is a group (it sorts first in the aggregated file, R puts it last), SUM,
//! MEAN and SD of a group with no valid value are missing and NMISS counts its cases.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

const DATA: &str = r####"DATA LIST LIST /id (F3) g (F2) h (A1) x (F3) y (F5.1).
BEGIN DATA
1 2 'a' 5 1.5
2 1 'b' 3 2.5
3 3 'a' . 3.5
4 2 'a' 8 4.5
5 1 'b' 1 5.5
6 . 'a' 2 6.5
7 3 'b' . 7.5
8 2 'a' 9 8.5
9 1 'b' 4 9.5
10 . 'a' 6 10.5
11 3 'a' 7 11.5
12 2 'b' 2 12.5
13 1 'a' 5 13.5
14 3 'b' . 14.5
15 2 'a' 10 15.5
16 . 'b' 4 16.5
17 1 'a' 3 17.5
18 3 'b' 1 18.5
19 4 'a' . 19.5
20 4 'b' . 20.5
END DATA.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"ADDVARIABLES by one key (a missing key is a group): every case gets its group's value and the file keeps its order"####,
        command: r####"AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=g /n = N /sx = SUM(x) /mx = MEAN(x) /sdx = SD(x) /mn = MIN(y) /mxy = MAX(y) /nm = NMISS(x) /first = FIRST(y) /last = LAST(y)."####,
        columns: &[
            (
                r####"id"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"g"####,
                r####"2 1 3 2 1 . 3 2 1 . 3 2 1 3 2 . 1 3 4 4"####,
            ),
            (
                r####"n"####,
                r####"5 5 5 5 5 3 5 5 5 3 5 5 5 5 5 3 5 5 2 2"####,
            ),
            (
                r####"sx"####,
                r####"34 16 8 34 16 12 8 34 16 12 8 34 16 8 34 12 16 8 . ."####,
            ),
            (
                r####"mx"####,
                r####"6.8 3.2 4 6.8 3.2 4 4 6.8 3.2 4 4 6.8 3.2 4 6.8 4 3.2 4 . ."####,
            ),
            (
                r####"sdx"####,
                r####"3.27108544675922 1.48323969741913 4.24264068711928 3.27108544675922 1.48323969741913 2 4.24264068711928 3.27108544675922 1.48323969741913 2 4.24264068711928 3.27108544675922 1.48323969741913 4.24264068711928 3.27108544675922 2 1.48323969741913 4.24264068711928 . ."####,
            ),
            (
                r####"mn"####,
                r####"1.5 2.5 3.5 1.5 2.5 6.5 3.5 1.5 2.5 6.5 3.5 1.5 2.5 3.5 1.5 6.5 2.5 3.5 19.5 19.5"####,
            ),
            (
                r####"mxy"####,
                r####"15.5 17.5 18.5 15.5 17.5 16.5 18.5 15.5 17.5 16.5 18.5 15.5 17.5 18.5 15.5 16.5 17.5 18.5 20.5 20.5"####,
            ),
            (
                r####"nm"####,
                r####"0 0 3 0 0 0 3 0 0 0 3 0 0 3 0 0 0 3 2 2"####,
            ),
            (
                r####"first"####,
                r####"1.5 2.5 3.5 1.5 2.5 6.5 3.5 1.5 2.5 6.5 3.5 1.5 2.5 3.5 1.5 6.5 2.5 3.5 19.5 19.5"####,
            ),
            (
                r####"last"####,
                r####"15.5 17.5 18.5 15.5 17.5 16.5 18.5 15.5 17.5 16.5 18.5 15.5 17.5 18.5 15.5 16.5 17.5 18.5 20.5 20.5"####,
            ),
        ],
    },
    Case {
        label: r####"ADDVARIABLES by two keys, a number and a string, on an unsorted file"####,
        command: r####"AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=g h /n = N /sx = SUM(x) /mx = MEAN(x) /sdx = SD(x) /mn = MIN(y) /mxy = MAX(y) /nm = NMISS(x) /first = FIRST(y) /last = LAST(y)."####,
        columns: &[
            (
                r####"id"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"n"####,
                r####"4 3 2 4 3 2 3 4 3 2 2 1 2 3 4 1 2 3 1 1"####,
            ),
            (
                r####"sx"####,
                r####"32 8 7 32 8 8 1 32 8 8 7 2 8 1 32 4 8 1 . ."####,
            ),
            (
                r####"mx"####,
                r####"8 2.66666666666667 7 8 2.66666666666667 4 1 8 2.66666666666667 4 7 2 4 1 8 4 4 1 . ."####,
            ),
            (
                r####"sdx"####,
                r####"2.16024689946929 1.52752523165195 . 2.16024689946929 1.52752523165195 2.82842712474619 . 2.16024689946929 1.52752523165195 2.82842712474619 . . 1.4142135623731 . 2.16024689946929 . 1.4142135623731 . . ."####,
            ),
            (
                r####"mn"####,
                r####"1.5 2.5 3.5 1.5 2.5 6.5 7.5 1.5 2.5 6.5 3.5 12.5 13.5 7.5 1.5 16.5 13.5 7.5 19.5 20.5"####,
            ),
            (
                r####"mxy"####,
                r####"15.5 9.5 11.5 15.5 9.5 10.5 18.5 15.5 9.5 10.5 11.5 12.5 17.5 18.5 15.5 16.5 17.5 18.5 19.5 20.5"####,
            ),
            (
                r####"nm"####,
                r####"0 0 1 0 0 0 2 0 0 0 1 0 0 2 0 0 0 2 1 1"####,
            ),
            (
                r####"first"####,
                r####"1.5 2.5 3.5 1.5 2.5 6.5 7.5 1.5 2.5 6.5 3.5 12.5 13.5 7.5 1.5 16.5 13.5 7.5 19.5 20.5"####,
            ),
            (
                r####"last"####,
                r####"15.5 9.5 11.5 15.5 9.5 10.5 18.5 15.5 9.5 10.5 11.5 12.5 17.5 18.5 15.5 16.5 17.5 18.5 19.5 20.5"####,
            ),
        ],
    },
    Case {
        label: r####"ADDVARIABLES without a break: the whole file is one group"####,
        command: r####"AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /all_n = N /all_mx = MEAN(x) /all_first = FIRST(y) /all_last = LAST(y)."####,
        columns: &[
            (
                r####"id"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"all_n"####,
                r####"20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20 20"####,
            ),
            (
                r####"all_mx"####,
                r####"4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667 4.66666666666667"####,
            ),
            (
                r####"all_first"####,
                r####"1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5 1.5"####,
            ),
            (
                r####"all_last"####,
                r####"20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5 20.5"####,
            ),
        ],
    },
    Case {
        label: r####"SELECT IF before ADDVARIABLES: only the cases that remain are grouped"####,
        command: r####"SELECT IF (id <> 5 AND id <> 9).
AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=g /n = N /sx = SUM(x) /mx = MEAN(x) /sdx = SD(x) /mn = MIN(y) /mxy = MAX(y) /nm = NMISS(x) /first = FIRST(y) /last = LAST(y)."####,
        columns: &[
            (
                r####"id"####,
                r####"1 2 3 4 6 7 8 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (r####"n"####, r####"5 3 5 5 3 5 5 3 5 5 3 5 5 3 3 5 2 2"####),
            (
                r####"sx"####,
                r####"34 11 8 34 12 8 34 12 8 34 11 8 34 12 11 8 . ."####,
            ),
            (
                r####"mx"####,
                r####"6.8 3.66666666666667 4 6.8 4 4 6.8 4 4 6.8 3.66666666666667 4 6.8 4 3.66666666666667 4 . ."####,
            ),
            (
                r####"first"####,
                r####"1.5 2.5 3.5 1.5 6.5 3.5 1.5 6.5 3.5 1.5 2.5 3.5 1.5 6.5 2.5 3.5 19.5 19.5"####,
            ),
            (
                r####"last"####,
                r####"15.5 17.5 18.5 15.5 16.5 18.5 15.5 16.5 18.5 15.5 17.5 18.5 15.5 16.5 17.5 18.5 20.5 20.5"####,
            ),
        ],
    },
    Case {
        label: r####"a new variable computed from the added ones"####,
        command: r####"AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=g /ysum = SUM(y) /xmean = MEAN(x).
COMPUTE share = y / ysum.
COMPUTE dev = x - xmean."####,
        columns: &[
            (
                r####"id"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"share"####,
                r####"0.0352941176470588 0.0515463917525773 0.0630630630630631 0.105882352941176 0.11340206185567 0.194029850746269 0.135135135135135 0.2 0.195876288659794 0.313432835820896 0.207207207207207 0.294117647058824 0.278350515463918 0.261261261261261 0.364705882352941 0.492537313432836 0.360824742268041 0.333333333333333 0.4875 0.5125"####,
            ),
            (
                r####"dev"####,
                r####"-1.8 -0.2 . 1.2 -2.2 -2 . 2.2 0.8 2 3 -4.8 1.8 . 3.2 0 -0.2 -3 . ."####,
            ),
        ],
    },
];

const DATASET_COMMANDS: &str = r####"DATASET NAME main.
DATASET DECLARE agg.
AGGREGATE /OUTFILE=agg /BREAK=g /n = N /mx = MEAN(x) /ymax = MAX(y).
DATASET COPY copy.
DATASET ACTIVATE copy.
DELETE VARIABLES h y.
SAVE OUTFILE='{OUT}/copy.sav'.
DATASET ACTIVATE agg.
SAVE OUTFILE='{OUT}/agg.sav'.
DATASET CLOSE agg.
DATASET CLOSE copy.
DATASET ACTIVATE main.
SAVE OUTFILE='{OUT}/main.sav'."####;

const DATASET_FILES: &[(&str, &[(&str, &str)])] = &[
    (
        r####"agg.sav"####,
        &[
            (r####"g"####, r####". 1 2 3 4"####),
            (r####"n"####, r####"3 5 5 5 2"####),
            (r####"mx"####, r####"4 3.2 6.8 4 ."####),
            (r####"ymax"####, r####"16.5 17.5 15.5 18.5 20.5"####),
        ],
    ),
    (
        r####"copy.sav"####,
        &[
            (
                r####"id"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"g"####,
                r####"2 1 3 2 1 . 3 2 1 . 3 2 1 3 2 . 1 3 4 4"####,
            ),
            (
                r####"x"####,
                r####"5 3 . 8 1 2 . 9 4 6 7 2 5 . 10 4 3 1 . ."####,
            ),
        ],
    ),
    (
        r####"main.sav"####,
        &[
            (
                r####"id"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"g"####,
                r####"2 1 3 2 1 . 3 2 1 . 3 2 1 3 2 . 1 3 4 4"####,
            ),
            (
                r####"h"####,
                r####"a|b|a|a|b|a|b|a|b|a|a|b|a|b|a|b|a|b|a|b"####,
            ),
            (
                r####"x"####,
                r####"5 3 . 8 1 2 . 9 4 6 7 2 5 . 10 4 3 1 . ."####,
            ),
            (
                r####"y"####,
                r####"1.5 2.5 3.5 4.5 5.5 6.5 7.5 8.5 9.5 10.5 11.5 12.5 13.5 14.5 15.5 16.5 17.5 18.5 19.5 20.5"####,
            ),
        ],
    ),
];

fn show(engine: &Engine, name: &str) -> Option<String> {
    let data = engine.dataset()?;
    Some(match data.variable(name).ok()?.values() {
        Values::Numeric(values) => {
            let text = values
                .iter()
                .map(|v| v.map_or(".".into(), |v| v.to_string()));
            text.collect::<Vec<_>>().join(" ")
        }
        Values::String(values) => values
            .iter()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("|"),
    })
}

/// The same values: numbers to 12 significant digits (R shows 15), anything else exactly.
fn same(got: &str, want: &str) -> bool {
    let (got_numbers, want_numbers): (Vec<&str>, Vec<&str>) =
        (got.split(' ').collect(), want.split(' ').collect());
    let number = |t: &&str| *t == "." || t.parse::<f64>().is_ok();
    if got_numbers.len() == want_numbers.len() && got_numbers.iter().all(number) {
        return got_numbers.iter().zip(&want_numbers).all(|(g, w)| {
            match (g.parse::<f64>(), w.parse::<f64>()) {
                (Ok(g), Ok(w)) => (g - w).abs() <= 1e-12 * w.abs().max(1.0),
                _ => g == w,
            }
        });
    }
    got == want
}

fn execute_all(engine: &mut Engine, source: &str) -> Result<(), String> {
    let (commands, errors) = parse(source);
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    for command in &commands {
        engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
    }
    Ok(())
}

#[test]
fn aggregate_addvariables_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        let source = format!("{DATA}{}\nEXECUTE.", case.command);
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Set `SAV_DIR` to a folder to keep the files there, for R to read with haven.
#[test]
fn datasets_made_copied_and_saved_hold_what_r_finds() {
    let kept = std::env::var_os("SAV_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-datasets-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    // the files of an earlier run must not stand in for these
    for (file, _) in DATASET_FILES {
        let _ = std::fs::remove_file(dir.join(file));
    }
    let mut failures = Vec::new();
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}{}",
        DATASET_COMMANDS.replace("{OUT}", &dir.display().to_string())
    );
    if let Err(error) = execute_all(&mut engine, &source) {
        failures.push(format!("error {error}"));
    }
    for (file, columns) in DATASET_FILES {
        let mut again = Engine::default();
        let path = dir.join(file);
        if !path.exists() {
            failures.push(format!("{file} was not written"));
            continue;
        }
        if let Err(error) = execute_all(&mut again, &format!("GET FILE='{}'.", path.display())) {
            failures.push(format!("{file}: {error}"));
            continue;
        }
        let names: Vec<String> = again
            .dataset()
            .map(|d| d.variable_names().map(str::to_string).collect())
            .unwrap_or_default();
        let wanted: Vec<&str> = columns.iter().map(|(n, _)| *n).collect();
        if names != wanted {
            failures.push(format!(
                "{file}: the variables are {names:?}, R has {wanted:?}"
            ));
            continue;
        }
        for (name, want) in *columns {
            let got = show(&again, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!("{file}: {name} is [{got}], R has [{want}]"));
            }
        }
    }
    if kept.is_none() {
        std::fs::remove_dir_all(&dir).ok();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
