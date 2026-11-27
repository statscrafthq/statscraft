//! The data management commands at their limits, against R: a file with no case, one case, only
//! missing values, zero weights, strings of 32767 bytes, and 1200 variables.
//!
//! The small cases run on invented files; what R gives (data frames, `aggregate`, `merge`, `order`,
//! weighted formulas) is the table of values, and the guide's rules that R lacks are written out in the
//! oracle (a sum of nothing but missing values is missing, MAX and FIRST take no weight, a group of
//! zero weights has N 0). The 1200 variables are built from a formula (`value(i, j)`, the same in the
//! R script), and AGGREGATE by a break, COMPUTE over a list (SUM, NVALID, a loop over a vector), RECODE
//! over a list, SORT CASES by 100 keys and ADD FILES agree with what R makes of the same matrix.
//! Choices where the guide is silent: AGGREGATE of a file with no case gives no case, also without a
//! break (R's `summarise` gives one row with N 0; an output case needs an input case); a group of
//! zero weights has N 0 and sum 0 (its values are there, each weighing nothing) and no mean.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    data: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

const DATA_A: &str = r####"DATA LIST LIST /g (F2) x (F5.1) y (F3).
BEGIN DATA
2 5 1
1 3 2
3 . 3
1 8 4
2 2 5
4 . 6
3 7 7
1 1 8
4 9 9
2 4 10
3 6 11
1 . 12
END DATA.
"####;
const DATA_EMPTY: &str = r####"DATA LIST LIST /g (F2) x (F5.1) y (F3).
BEGIN DATA
END DATA.
"####;
const DATA_ONE: &str = r####"DATA LIST LIST /g (F2) x (F5.1) y (F3).
BEGIN DATA
3 . 3
END DATA.
"####;
const DATA_ALLMISSING: &str = r####"DATA LIST LIST /g (F2) x (F5.1) y (F3).
BEGIN DATA
1 . 1
1 . 2
1 . 3
2 . 4
2 . 5
2 . 6
END DATA.
"####;
const DATA_ZERO_WEIGHTS: &str = r####"DATA LIST LIST /g (F2) x (F5.1) w (F3.1).
BEGIN DATA
1 2 0
1 4 0
2 6 1
2 8 2
3 . 1
END DATA.
WEIGHT BY w.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"the file of the other cases, by a break: counts, means over valid values, the largest, a sum"####,
        data: DATA_A,
        command: r####"SORT CASES BY g.
AGGREGATE OUTFILE=* /BREAK=g /n=N /nm=NMISS(x) /m=MEAN(x) /mx=MAX(x) /s=SUM(y)."####,
        columns: &[
            (r####"g"####, r####"1 2 3 4"####),
            (r####"n"####, r####"4 3 3 2"####),
            (r####"nm"####, r####"1 0 1 1"####),
            (r####"m"####, r####"4 3.66666666666667 6.5 9"####),
            (r####"mx"####, r####"8 5 7 9"####),
            (r####"s"####, r####"26 16 21 15"####),
        ],
    },
    Case {
        label: r####"SORT CASES of a file with no case"####,
        data: DATA_EMPTY,
        command: r####"SORT CASES BY g."####,
        columns: &[(r####"g"####, r####""####), (r####"x"####, r####""####)],
    },
    Case {
        label: r####"AGGREGATE by a break of a file with no case gives no case"####,
        data: DATA_EMPTY,
        command: r####"AGGREGATE OUTFILE=* /BREAK=g /n=N /m=MEAN(x)."####,
        columns: &[
            (r####"g"####, r####""####),
            (r####"n"####, r####""####),
            (r####"m"####, r####""####),
        ],
    },
    Case {
        label: r####"AGGREGATE without a break of a file with no case gives no case (R's summarise gives one with N 0; the guide is silent, an output case needs an input case)"####,
        data: DATA_EMPTY,
        command: r####"AGGREGATE OUTFILE=* /n=N /nu=NU /m=MEAN(x) /s=SUM(x) /mx=MAX(x)."####,
        columns: &[
            (r####"n"####, r####""####),
            (r####"nu"####, r####""####),
            (r####"m"####, r####""####),
            (r####"s"####, r####""####),
            (r####"mx"####, r####""####),
        ],
    },
    Case {
        label: r####"COMPUTE and RECODE on a file with no case make their variables and no value"####,
        data: DATA_EMPTY,
        command: r####"COMPUTE z = x * 2.
RECODE x (SYSMIS=0) INTO r.
COMPUTE t = LAG(x)."####,
        columns: &[
            (r####"z"####, r####""####),
            (r####"r"####, r####""####),
            (r####"t"####, r####""####),
        ],
    },
    Case {
        label: r####"ADD FILES of a file with no case and one with cases is the cases"####,
        data: DATA_EMPTY,
        command: r####"DATASET NAME e.
DATA LIST LIST /g (F2) x (F5.1) y (F3).
BEGIN DATA
2 5 1
1 3 2
3 . 3
1 8 4
2 2 5
4 . 6
3 7 7
1 1 8
4 9 9
2 4 10
3 6 11
1 . 12
END DATA.
DATASET NAME full.
DATASET ACTIVATE e.
ADD FILES /FILE=* /FILE=full."####,
        columns: &[
            (r####"g"####, r####"2 1 3 1 2 4 3 1 4 2 3 1"####),
            (r####"x"####, r####"5 3 . 8 2 . 7 1 9 4 6 ."####),
            (r####"y"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
        ],
    },
    Case {
        label: r####"MATCH FILES by a key with a file that has no case first"####,
        data: DATA_EMPTY,
        command: r####"DATASET NAME e.
DATA LIST LIST /g (F2) x (F5.1) y (F3).
BEGIN DATA
2 5 1
1 3 2
3 . 3
1 8 4
2 2 5
4 . 6
3 7 7
1 1 8
4 9 9
2 4 10
3 6 11
1 . 12
END DATA.
DATASET NAME full.
SORT CASES BY g.
DATASET ACTIVATE e.
MATCH FILES /FILE=* /FILE=full /BY g."####,
        columns: &[
            (r####"g"####, r####"1 1 1 1 2 2 2 3 3 3 4 4"####),
            (r####"x"####, r####"3 8 1 . 5 2 4 . 7 6 . 9"####),
            (r####"y"####, r####"2 4 8 12 1 5 10 3 7 11 6 9"####),
        ],
    },
    Case {
        label: r####"AGGREGATE of one case: the standard deviation of one value is missing"####,
        data: DATA_ONE,
        command: r####"AGGREGATE OUTFILE=* /BREAK=g /n=N /m=MEAN(x) /sd=SD(x) /mx=MAX(x) /s=SUM(x) /f=FIRST(x) /l=LAST(x)."####,
        columns: &[
            (r####"g"####, r####"3"####),
            (r####"n"####, r####"1"####),
            (r####"m"####, r####"."####),
            (r####"sd"####, r####"."####),
            (r####"mx"####, r####"."####),
            (r####"s"####, r####"."####),
            (r####"f"####, r####"."####),
            (r####"l"####, r####"."####),
        ],
    },
    Case {
        label: r####"one case with a value: AGGREGATE"####,
        data: DATA_ONE,
        command: r####"COMPUTE x = 7.
AGGREGATE OUTFILE=* /BREAK=g /n=N /m=MEAN(x) /sd=SD(x) /mx=MAX(x) /s=SUM(x)."####,
        columns: &[
            (r####"g"####, r####"3"####),
            (r####"n"####, r####"1"####),
            (r####"m"####, r####"7"####),
            (r####"sd"####, r####"."####),
            (r####"mx"####, r####"7"####),
            (r####"s"####, r####"7"####),
        ],
    },
    Case {
        label: r####"LAG of the one case is missing, and the case number is 1"####,
        data: DATA_ONE,
        command: r####"COMPUTE t = LAG(y).
COMPUTE k = $CASENUM.
SORT CASES BY g."####,
        columns: &[(r####"t"####, r####"."####), (r####"k"####, r####"1"####)],
    },
    Case {
        label: r####"AGGREGATE of groups with only missing values: counts count, the rest is missing"####,
        data: DATA_ALLMISSING,
        command: r####"AGGREGATE OUTFILE=* /BREAK=g /n=N /nu=NU /nm=NMISS(x) /m=MEAN(x) /s=SUM(x) /sd=SD(x) /mx=MAX(x) /mn=MIN(x) /f=FIRST(x) /my=MEAN(y)."####,
        columns: &[
            (r####"g"####, r####"1 2"####),
            (r####"n"####, r####"3 3"####),
            (r####"nu"####, r####"3 3"####),
            (r####"nm"####, r####"3 3"####),
            (r####"m"####, r####". ."####),
            (r####"s"####, r####". ."####),
            (r####"sd"####, r####". ."####),
            (r####"mx"####, r####". ."####),
            (r####"mn"####, r####". ."####),
            (r####"f"####, r####". ."####),
            (r####"my"####, r####"2 5"####),
        ],
    },
    Case {
        label: r####"AGGREGATE ADDVARIABLES on only missing values"####,
        data: DATA_ALLMISSING,
        command: r####"AGGREGATE OUTFILE=* MODE=ADDVARIABLES /BREAK=g /m=MEAN(x) /my=SUM(y)."####,
        columns: &[
            (r####"m"####, r####". . . . . ."####),
            (r####"my"####, r####"6 6 6 15 15 15"####),
        ],
    },
    Case {
        label: r####"COMPUTE functions over missing and valid values"####,
        data: DATA_ALLMISSING,
        command: r####"COMPUTE a = SUM(x, y).
COMPUTE b = MEAN(x, y).
COMPUTE c = NVALID(x, y).
COMPUTE d = x + y.
COMPUTE e = MAX(x, y).
COMPUTE f = SUM.2(x, y).
COMPUTE h = MEAN(x)."####,
        columns: &[
            (r####"a"####, r####"1 2 3 4 5 6"####),
            (r####"b"####, r####"1 2 3 4 5 6"####),
            (r####"c"####, r####"1 1 1 1 1 1"####),
            (r####"d"####, r####". . . . . ."####),
            (r####"e"####, r####"1 2 3 4 5 6"####),
            (r####"f"####, r####". . . . . ."####),
            (r####"h"####, r####". . . . . ."####),
        ],
    },
    Case {
        label: r####"RECODE of missing values"####,
        data: DATA_ALLMISSING,
        command: r####"RECODE x (SYSMIS=-1) (ELSE=COPY) INTO r.
RECODE y (MISSING=0) (1 THRU 3=1) (ELSE=2) INTO q."####,
        columns: &[
            (r####"r"####, r####"-1 -1 -1 -1 -1 -1"####),
            (r####"q"####, r####"1 1 1 2 2 2"####),
        ],
    },
    Case {
        label: r####"AGGREGATE weighted: a group of zero weights has N 0, sum 0 and no mean or deviation; MAX and FIRST take no weight, NU counts the cases"####,
        data: DATA_ZERO_WEIGHTS,
        command: r####"AGGREGATE OUTFILE=* /BREAK=g /n=N /nu=NU /m=MEAN(x) /s=SUM(x) /sd=SD(x) /mx=MAX(x) /f=FIRST(x) /nm=NMISS(x)."####,
        columns: &[
            (r####"g"####, r####"1 2 3"####),
            (r####"n"####, r####"0 3 1"####),
            (r####"nu"####, r####"2 2 1"####),
            (r####"m"####, r####". 7.33333333333333 ."####),
            (r####"s"####, r####"0 22 ."####),
            (r####"sd"####, r####". 1.15470053837925 ."####),
            (r####"mx"####, r####"4 8 ."####),
            (r####"f"####, r####"2 6 ."####),
            (r####"nm"####, r####"0 0 1"####),
        ],
    },
    Case {
        label: r####"SORT, SELECT IF and the case number ignore the weights"####,
        data: DATA_ZERO_WEIGHTS,
        command: r####"SORT CASES BY x (D).
SELECT IF (x > 3).
COMPUTE k = $CASENUM."####,
        columns: &[
            (r####"x"####, r####"8 6 4"####),
            (r####"k"####, r####"1 2 3"####),
        ],
    },
    Case {
        label: r####"strings of 32767 bytes: length, upper case, a cut at the width, a position at the last letter"####,
        data: DATA_EMPTY,
        command: r####"DATASET CLOSE ALL.
DATA LIST LIST /id (F2).
BEGIN DATA
1
2
END DATA.
STRING a b (A32767).
COMPUTE a = CHAR.LPAD('x', 32767, 'x').
COMPUTE b = CONCAT(a, 'yy').
IF (id = 2) a = CONCAT(CHAR.LPAD('a', 32766, 'a'), 'z').
COMPUTE n1 = LENGTH(a).
COMPUTE n2 = LENGTH(RTRIM(b)).
COMPUTE n3 = LENGTH(UPCASE(a)).
COMPUTE p = CHAR.INDEX(a, 'z').
COMPUTE q = CHAR.RINDEX(a, 'a').
COMPUTE l = CHAR.LENGTH(REPLACE(a, 'a', 'bb'))."####,
        columns: &[
            (r####"n1"####, r####"32767 32767"####),
            (r####"n2"####, r####"32767 32767"####),
            (r####"n3"####, r####"32767 32767"####),
            (r####"p"####, r####"0 32767"####),
            (r####"q"####, r####"0 32766"####),
        ],
    },
];

const MEAN_V1: &str = r####"3.14 3.78 6 4.85"####;
const MEAN_V600: &str = r####"5.65 3.64 2.64 6.8"####;
const MEAN_V1200: &str = r####"3.56 6.14 5.45 4.3"####;
const MEAN_TOTAL: &str = r####"5761.35 5759.22666666667 5759.02666666667 5759.01333333333"####;
const ROW_SUM: &str = r####"5249.7 5227.2 5228.4 5254.4 5226.5 5242.8 5244 5211.8 5252.6 5229.3 5235.9 5256.5 5214.6 5244.9 5241.8 5223.6 5259.3 5221.7 5238 5247.8 5207 5252.4 5224.5 5231.1"####;
const ROW_VALID: &str = r####"1091 1091 1091 1091 1091 1091 1091 1091 1090 1091 1091 1091 1091 1091 1091 1091 1091 1091 1091 1090 1091 1091 1091 1091"####;
const RECODED_TOTAL: &str = r####"1622 1618 1618 1624 1617 1620 1621 1614 1623 1618 1619 1624 1615 1622 1620 1617 1625 1617 1619 1621 1613 1623 1618 1618"####;
const SORT_ORDER: &str = r####"9 20 13 21 8 16 3 24 11 19 6 14 1 22 17 4 12 7 15 2 23 10 18 5"####;

fn show(engine: &Engine, name: &str) -> Option<String> {
    let data = engine.dataset()?;
    Some(match data.variable(name).ok()?.values() {
        Values::Numeric(values) => values
            .iter()
            .map(|v| v.map_or(".".into(), |v| v.to_string()))
            .collect::<Vec<_>>()
            .join(" "),
        Values::String(values) => values
            .iter()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("|"),
    })
}

/// The same values: numbers to 12 significant digits (R shows 15), anything else exactly.
fn same(got: &str, want: &str) -> bool {
    let (g, w): (Vec<&str>, Vec<&str>) = (got.split(' ').collect(), want.split(' ').collect());
    let number = |t: &&str| *t == "." || t.parse::<f64>().is_ok();
    if g.len() == w.len() && !got.is_empty() && g.iter().all(number) {
        return g
            .iter()
            .zip(&w)
            .all(|(g, w)| match (g.parse::<f64>(), w.parse::<f64>()) {
                (Ok(g), Ok(w)) => (g - w).abs() <= 1e-9 * w.abs().max(1.0),
                _ => g == w,
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
fn small_edge_cases_agree_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        let source = format!("{}{}\nEXECUTE.", case.data, case.command);
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

const NV: usize = 1200;
const NC: usize = 24;

fn value(i: usize, j: usize) -> Option<f64> {
    (!(i + 2 * j).is_multiple_of(11)).then(|| ((i * 37 + j * 101) % 97) as f64 / 10.0)
}

/// 24 cases of id, g and 1200 variables v1 to v1200, from the formula.
fn big_data() -> String {
    data_of(NV)
}

fn data_of(variables: usize) -> String {
    let names: String = (1..=variables).map(|j| format!(" v{j} (F6.2)")).collect();
    let mut s = format!("DATA LIST LIST /id (F3) g (F2){names}.\nBEGIN DATA\n");
    for i in 1..=NC {
        s += &format!("{i} {}", i % 4 + 1);
        for j in 1..=variables {
            s += &value(i, j).map_or(" .".to_string(), |v| format!(" {v}"));
        }
        s += "\n";
    }
    s + "END DATA.\n"
}

fn column(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    data.variable(name)
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .collect()
}

fn expect(got: Vec<Option<f64>>, want: &str, what: &str) {
    let got = got
        .iter()
        .map(|v| v.map_or(".".into(), |v| v.to_string()))
        .collect::<Vec<_>>()
        .join(" ");
    assert!(same(&got, want), "{what}: [{got}] against R's [{want}]");
}

#[test]
fn a_thousand_variables_in_aggregate_compute_recode_and_sort_agree_with_r() {
    let mut engine = Engine::default();
    let aggregates: String = (1..=NV).map(|j| format!(" /m{j}=MEAN(v{j})")).collect();
    let source = format!(
        "{}COMPUTE tot = SUM(v1 TO v{NV}).\nCOMPUTE valid = NVALID(v1 TO v{NV}).\n\
         VECTOR vv = v1 TO v{NV}.\nCOMPUTE loop_sum = 0.\nCOMPUTE seen = 0.\n\
         LOOP #i = 1 TO {NV}.\nDO IF NOT MISSING(vv(#i)).\nCOMPUTE loop_sum = loop_sum + vv(#i).\n\
         COMPUTE seen = seen + 1.\nEND IF.\nEND LOOP.\n\
         DATASET COPY means.\nDATASET COPY recoded.\nDATASET COPY sorted.\n\
         DATASET ACTIVATE means.\nSORT CASES BY g.\nAGGREGATE OUTFILE=* /BREAK=g{aggregates}.\n",
        big_data()
    );
    execute_all(&mut engine, &source).unwrap();
    // the means: three variables and the total over all 1200, by group
    expect(column(&engine, "m1"), MEAN_V1, "mean of v1");
    expect(column(&engine, "m600"), MEAN_V600, "mean of v600");
    expect(column(&engine, "m1200"), MEAN_V1200, "mean of v1200");
    let total: Vec<Option<f64>> = (0..4)
        .map(|k| {
            Some(
                (1..=NV)
                    .map(|j| column(&engine, &format!("m{j}"))[k].unwrap_or(0.0))
                    .sum(),
            )
        })
        .collect();
    expect(total, MEAN_TOTAL, "sum of the 1200 means");
    // COMPUTE over the list, and the loop over a vector, on the dataset before the break
    execute_all(&mut engine, "DATASET ACTIVATE recoded.").unwrap();
    expect(column(&engine, "tot"), ROW_SUM, "SUM(v1 TO v1200)");
    expect(column(&engine, "valid"), ROW_VALID, "NVALID(v1 TO v1200)");
    expect(
        column(&engine, "loop_sum"),
        ROW_SUM,
        "the loop over the vector",
    );
    expect(
        column(&engine, "seen"),
        ROW_VALID,
        "valid values seen in the loop",
    );
    // RECODE over the list: below 5 is 1, 5 and over is 2, missing is 0
    execute_all(
        &mut engine,
        &format!(
            "RECODE v1 TO v{NV} (SYSMIS=0) (LOWEST THRU 4.99=1) (ELSE=2).\nCOMPUTE rt = SUM(v1 TO v{NV})."
        ),
    )
    .unwrap();
    expect(
        column(&engine, "rt"),
        RECODED_TOTAL,
        "the recoded variables",
    );
    // SORT CASES by 100 keys, missing values first
    execute_all(&mut engine, "DATASET ACTIVATE sorted.").unwrap();
    execute_all(&mut engine, "SORT CASES BY v1 TO v100.").unwrap();
    expect(
        column(&engine, "id"),
        SORT_ORDER,
        "the order of SORT CASES by 100 keys",
    );
}

#[test]
fn add_files_of_files_with_a_thousand_variables_stacks_them() {
    let mut engine = Engine::default();
    let source = format!(
        "{}DATASET NAME one.\nCOMPUTE tot = SUM(v1 TO v{NV}).\nDATASET COPY two.\nDATASET ACTIVATE one.\nADD FILES /FILE=* /FILE=two.\n",
        big_data()
    );
    execute_all(&mut engine, &source).unwrap();
    let both = format!("{ROW_SUM} {ROW_SUM}");
    expect(
        column(&engine, "tot"),
        &both,
        "the totals of two stacked files",
    );
    assert_eq!(engine.dataset().unwrap().variable_count(), NV + 3);
}

/// Files with no case: a text file with no byte, one with a header only, and the .sav, .xlsx and CSV
/// the engine writes from a file with no case. Set `EDGE_DIR` to keep the written files, for
/// R to read with haven, readxl and read.csv (0 cases, two names).
#[test]
fn files_with_no_case_read_and_write() {
    let kept = std::env::var_os("EDGE_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-edge-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("zero.txt"), b"").unwrap();
    std::fs::write(dir.join("header.csv"), b"id;name\r\n").unwrap();
    let d = dir.display();
    let cases = |engine: &Engine| engine.dataset().unwrap().case_count();
    let mut engine = Engine::default();
    execute_all(
        &mut engine,
        &format!("DATA LIST FILE='{d}/zero.txt' LIST /a (F2) b (A3)."),
    )
    .unwrap();
    assert_eq!(
        (cases(&engine), engine.dataset().unwrap().variable_count()),
        (0, 2)
    );
    let mut engine = Engine::default();
    let source = format!(
        "GET DATA /TYPE=TXT /FILE='{d}/header.csv' /DELIMITERS=';' /FIRSTCASE=2 /VARIABLES=id F2 name A5.\n\
         SAVE OUTFILE='{d}/none.sav'.\n\
         SAVE TRANSLATE OUTFILE='{d}/none.xlsx' /TYPE=XLSX /FIELDNAMES /REPLACE.\n\
         SAVE TRANSLATE OUTFILE='{d}/none.csv' /TYPE=CSV /FIELDNAMES /REPLACE."
    );
    execute_all(&mut engine, &source).unwrap();
    assert_eq!(cases(&engine), 0);
    // back from the .sav and the workbook: the names stay, no case
    for read in [
        format!("GET FILE='{d}/none.sav'."),
        format!("GET DATA /TYPE=XLSX /FILE='{d}/none.xlsx'."),
    ] {
        let mut engine = Engine::default();
        execute_all(&mut engine, &read).unwrap();
        let data = engine.dataset().unwrap();
        assert_eq!(data.case_count(), 0, "{read}");
        assert!(
            data.variable("id").is_ok() && data.variable("name").is_ok(),
            "{read}"
        );
    }
    if kept.is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// A string of 32767 bytes, the widest there is, written to a .sav file and a workbook (a cell holds
/// 32767 characters) and read back. With `EDGE_DIR`, R reads the files with haven and readxl.
#[test]
fn a_string_of_32767_bytes_survives_sav_and_workbook() {
    let kept = std::env::var_os("EDGE_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-edge2-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    let d = dir.display();
    let mut engine = Engine::default();
    let source = format!(
        "DATA LIST LIST /id (F2).\nBEGIN DATA\n1\n2\nEND DATA.\nSTRING a (A32767).\nCOMPUTE a = CHAR.LPAD('x', 32767, 'x').\nIF (id = 2) a = CONCAT('õ', CHAR.LPAD('y', 16382, 'y'), 'ä').\n\
         SAVE OUTFILE='{d}/long.sav'.\nSAVE TRANSLATE OUTFILE='{d}/long.xlsx' /TYPE=XLSX /FIELDNAMES /REPLACE.\nGET FILE='{d}/long.sav'.\nCOMPUTE n = LENGTH(RTRIM(a)).\nCOMPUTE c = CHAR.LENGTH(a)."
    );
    execute_all(&mut engine, &source).unwrap();
    assert_eq!(show(&engine, "n").unwrap(), "32767 16386");
    assert_eq!(show(&engine, "c").unwrap(), "32767 16384");
    let mut engine = Engine::default();
    execute_all(
        &mut engine,
        &format!("GET DATA /TYPE=XLSX /FILE='{d}/long.xlsx'.\nCOMPUTE n = LENGTH(RTRIM(a))."),
    )
    .unwrap();
    assert_eq!(show(&engine, "n").unwrap(), "32767 16386");
    if kept.is_none() {
        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// 3000 summaries in one AGGREGATE took 14 seconds when a variable was found by scanning the
/// dictionary (the time grew with the square of their number); now about one second.
#[test]
fn three_thousand_summaries_in_one_aggregate_stay_fast() {
    let variables = 3000;
    let aggregates: String = (1..=variables)
        .map(|j| format!(" /m{j}=MEAN(v{j})"))
        .collect();
    let source = format!(
        "{}SORT CASES BY g.\nAGGREGATE OUTFILE=* /BREAK=g{aggregates}.\n",
        data_of(variables)
    );
    let mut engine = Engine::default();
    let start = std::time::Instant::now();
    execute_all(&mut engine, &source).unwrap();
    let seconds = start.elapsed().as_secs_f64();
    assert!(seconds < 8.0, "3000 summaries took {seconds} seconds");
    assert_eq!(engine.dataset().unwrap().variable_count(), variables + 1);
    // the mean of v2999 over the cases of each group, from the formula
    let want: Vec<Option<f64>> = (1..=4)
        .map(|g| {
            let values: Vec<f64> = (1..=NC)
                .filter(|i| i % 4 + 1 == g)
                .filter_map(|i| value(i, 2999))
                .collect();
            (!values.is_empty()).then(|| values.iter().sum::<f64>() / values.len() as f64)
        })
        .collect();
    let got = column(&engine, "m2999");
    for (g, w) in got.iter().zip(&want) {
        let close = match (g, w) {
            (Some(g), Some(w)) => (g - w).abs() < 1e-9,
            (None, None) => true,
            _ => false,
        };
        assert!(close, "{got:?} against {want:?}");
    }
}

/// A guard that stacking wide files stays quick: ADD FILES looked a variable up by name for every
/// cell, which with 200 variables and 500,000 cases a file took 66 seconds instead of 11. Two files of
/// 200 variables and 5000 cases here, with a generous time limit, and the values against the formula.
#[test]
fn add_files_of_wide_files_takes_no_time_for_lookups() {
    let dir = std::env::temp_dir().join(format!("statscraft-wide-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let d = dir.display();
    let make = |from: usize, to: usize, file: &str| {
        format!(
            "INPUT PROGRAM.\nLOOP #i = {from} TO {to}.\nCOMPUTE id = #i.\nEND CASE.\nEND LOOP.\nEND FILE.\nEND INPUT PROGRAM.\n\
             VECTOR x(200).\nLOOP #j = 1 TO 200.\nCOMPUTE x(#j) = MOD(id * #j, 97).\nEND LOOP.\nSAVE OUTFILE='{d}/{file}'.\n"
        )
    };
    let mut engine = Engine::default();
    execute_all(&mut engine, &make(1, 5000, "a.sav")).unwrap();
    execute_all(&mut engine, &make(5001, 10000, "b.sav")).unwrap();
    let start = std::time::Instant::now();
    execute_all(
        &mut engine,
        &format!("ADD FILES /FILE='{d}/a.sav' /FILE='{d}/b.sav' /IN=second."),
    )
    .unwrap();
    let seconds = start.elapsed().as_secs_f64();
    assert!(seconds < 10.0, "stacking took {seconds} seconds");
    assert_eq!(engine.dataset().unwrap().case_count(), 10000);
    // the sum of x7 over the stacked file, from the formula
    let want: f64 = (1..=10000u64).map(|i| ((i * 7) % 97) as f64).sum();
    let got: f64 = column(&engine, "x7").iter().flatten().sum();
    assert_eq!(got, want);
    assert_eq!(
        column(&engine, "second").iter().flatten().sum::<f64>(),
        5000.0
    );
    let _ = std::fs::remove_dir_all(&dir);
}
