//! ADP on small invented data. Each expected value below was worked out by hand from the
//! published rules, and each test runs the written preparation syntax on the data to check that
//! it makes those values.

use statscraft_data::{Measure, Values};
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;
use std::sync::atomic::{AtomicUsize, Ordering};

fn run(source: &str) -> (Engine, Vec<Table>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}\n{source}", command.first_line),
        }
    }
    (engine, tables)
}

fn fails(source: &str) -> String {
    let mut engine = Engine::default();
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.to_string();
        }
    }
    panic!("{source} ran without an error");
}

static RUNS: AtomicUsize = AtomicUsize::new(0);

/// Runs ADP (`options` after /FIELDS) on `data`, then the written syntax on the same data:
/// the ADP table, the syntax and the engine after the syntax ran.
fn prepare(data: &str, options: &str) -> (Table, String, Engine) {
    let dir = std::env::temp_dir().join(format!(
        "statscraft-adp-{}-{}",
        std::process::id(),
        RUNS.fetch_add(1, Ordering::SeqCst)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let (engine, tables) = run(&format!(
        "{data}CD '{}'.\nADP {options}\n  /OUTFILE PREPXML='prep.sps'.",
        dir.display()
    ));
    // ADP itself leaves the data alone
    let fresh = run(data).0;
    assert_eq!(
        engine.dataset().unwrap().variable_count(),
        fresh.dataset().unwrap().variable_count()
    );
    let syntax = std::fs::read_to_string(dir.join("prep.sps")).unwrap();
    std::fs::remove_dir_all(&dir).unwrap();
    let (prepared, _) = run(&format!("{data}{syntax}"));
    assert_eq!(tables.len(), 1);
    (tables.into_iter().next().unwrap(), syntax, prepared)
}

fn numbers(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let Values::Numeric(values) = data.variable(name).unwrap().values() else {
        panic!("{name} is not numeric")
    };
    (0..data.case_count()).map(|c| values.get(c)).collect()
}

fn texts(engine: &Engine, name: &str) -> Vec<String> {
    let data = engine.dataset().unwrap();
    let Values::String(values) = data.variable(name).unwrap().values() else {
        panic!("{name} is not a string")
    };
    (0..data.case_count())
        .map(|c| values.get(c).trim_end().to_string())
        .collect()
}

fn measure(engine: &Engine, name: &str) -> Measure {
    engine.dataset().unwrap().variable(name).unwrap().measure()
}

fn close(got: &[Option<f64>], want: &[f64]) {
    assert_eq!(got.len(), want.len(), "{got:?} {want:?}");
    for (g, w) in got.iter().zip(want) {
        let g = g.unwrap_or_else(|| panic!("missing in {got:?}"));
        assert!((g - w).abs() < 1e-9, "{got:?} against {want:?}");
    }
}

fn text(cell: &Cell) -> String {
    match cell {
        Cell::Text(s) => s.clone(),
        Cell::Number(n) => n.to_string(),
        Cell::Empty => String::new(),
    }
}

/// The row of a field: role, level, prepared field, prepared level, actions.
fn row(table: &Table, field: &str) -> Vec<String> {
    let row = table
        .rows
        .iter()
        .find(|r| text(&r[0]) == field)
        .unwrap_or_else(|| panic!("no row {field}"));
    row[1..].iter().map(text).collect()
}

const PLAIN: &str = "/OUTLIERHANDLING INPUT=NO TARGET=NO /RESCALE INPUT=NONE TARGET=NONE";

#[test]
fn dates_and_times_become_durations_and_parts() {
    // with the reference 2020-01-01: d1 is 1, 10, 30 and 40 days back (least under 31: days);
    // d2 61, 214, 365 and 365 days (months of 30.4375 days); d3 730, 1095, 1278 and 1461
    // days (years of 365.25 days). From 12:00:00, t is 7200, 5400, 3600 and 60 seconds
    // back (least 60: minutes).
    let data = "DATA LIST FREE /d1 (ADATE10) d2 (ADATE10) d3 (ADATE10) t (TIME8).
BEGIN DATA.
12/31/2019 11/01/2019 01/01/2018 10:00:00
12/22/2019 06/01/2019 01/01/2017 10:30:00
12/02/2019 01/01/2019 07/02/2016 11:00:00
11/22/2019 01/01/2019 01/01/2016 11:59:00
END DATA.
";
    let (table, syntax, engine) = prepare(
        data,
        &format!(
            "/FIELDS INPUT=d1 d2 d3 t
  /PREPDATETIME DATEDURATION=YES(REFERENCE=YMD('2020-01-01'))
    TIMEDURATION=YES(REFERENCE=HMS('12:00:00')) EXTRACTYEAR=YES EXTRACTHOUR=YES
  /ADJUSTLEVEL INPUT=NO {PLAIN}"
        ),
    );
    close(&numbers(&engine, "d1_days"), &[1.0, 10.0, 30.0, 40.0]);
    close(
        &numbers(&engine, "d2_months"),
        &[61.0, 214.0, 365.0, 365.0].map(|d| d / 30.4375),
    );
    close(
        &numbers(&engine, "d3_years"),
        &[730.0, 1095.0, 1278.0, 1461.0].map(|d| d / 365.25),
    );
    close(&numbers(&engine, "t_minutes"), &[120.0, 90.0, 60.0, 1.0]);
    close(
        &numbers(&engine, "d3_year"),
        &[2018.0, 2017.0, 2016.0, 2016.0],
    );
    close(&numbers(&engine, "t_hour"), &[10.0, 10.0, 11.0, 11.0]);
    assert_eq!(measure(&engine, "d3_year"), Measure::Ordinal);
    assert_eq!(measure(&engine, "d1_days"), Measure::Scale);
    assert!(syntax.contains("COMPUTE d1_days = (DATE.DMY(1,1,2020) - d1) / 86400.\n"));
    assert!(syntax.contains("COMPUTE t_minutes = (TIME.HMS(12,0,0) - t) / 60.\n"));
    assert!(row(&table, "d1")[4].starts_with("Excluded: a date or time field"));
    // every value of d1 falls in 2019, and dates have no hour
    assert_eq!(row(&table, "d1_year")[4], "Excluded: a constant field");
    assert_eq!(row(&table, "d1_hour")[4], "Excluded: a constant field");
    assert_eq!(row(&table, "d1_days")[0], "Input from d1");
    assert_eq!(row(&table, "d1_days")[4], "days since 2020-01-01");
    // fixed units and suffixes
    let (_, _, engine) = prepare(
        data,
        &format!(
            "/FIELDS INPUT=d1
  /PREPDATETIME DATEDURATION=YES(REFERENCE=YMD('2020-01-01') UNIT=MONTHS(SUFFIX='_m'))
  /ADJUSTLEVEL INPUT=NO {PLAIN}"
        ),
    );
    close(
        &numbers(&engine, "d1_m"),
        &[1.0, 10.0, 30.0, 40.0].map(|d| d / 30.4375),
    );
}

#[test]
fn screening_drops_unsuitable_fields() {
    // m: 3 of 5 missing (60% > 50); c: one value; s: 4 of 5 in one category (80% > 79);
    // u: 4 categories (> 3)
    let data = "DATA LIST FREE /y m c (F8.0) s u (A1) x (F8.0).
BEGIN DATA.
1 1 5 a a 1
2 . 5 a b 2
3 . 5 a c 3
4 . 5 a d 4
5 2 5 b a 5
END DATA.
VARIABLE LEVEL y m c x (SCALE).
";
    let options = "/FIELDS TARGET=y INPUT=m c s u x
  /SCREENING SINGLECAT=YES(MAXPCT=79) UNIQUECAT=YES(MAXCAT=3)";
    let (table, _, _) = prepare(data, &format!("{options} {PLAIN}"));
    assert_eq!(
        row(&table, "m")[4],
        "Excluded: 60% missing values, more than 50"
    );
    assert_eq!(row(&table, "c")[4], "Excluded: a constant field");
    assert_eq!(
        row(&table, "s")[4],
        "Excluded: 80% of the values in one category, more than 79"
    );
    assert_eq!(row(&table, "u")[4], "Excluded: 4 categories, more than 3");
    assert_eq!(row(&table, "x")[2..4], ["x", "Continuous"]);
    // at the limits nothing is screened
    let (table, _, _) = prepare(
        data,
        &format!(
            "/FIELDS TARGET=y INPUT=m c s u x
  /SCREENING PCTMISSING=YES(MAXPCT=60) SINGLECAT=YES(MAXPCT=80) UNIQUECAT=YES(MAXCAT=4)
  /TRANSFORM MERGESUPERVISED=NO {PLAIN}"
        ),
    );
    for field in ["m", "s", "u"] {
        assert!(!row(&table, field)[4].starts_with("Excluded"), "{field}");
    }
    assert!(
        fails(&format!(
            "{data}ADP /FIELDS TARGET=m INPUT=x /OUTFILE PREPXML='p.sps'."
        ))
        .contains("target m is screened out")
    );
    assert!(
        fails(&format!(
            "{data}ADP /FIELDS INPUT=m c /OUTFILE PREPXML='p.sps'."
        ))
        .contains("every input is screened out")
    );
}

#[test]
fn levels_follow_the_number_of_values() {
    // x has 3 values (< 5): ordinal; o has 12 categories (> 10): continuous
    let data = "DATA LIST FREE /x o.
BEGIN DATA.
1 1 2 2 3 3 1 4 2 5 3 6 1 7 2 8 3 9 1 10 2 11 3 12
END DATA.
VARIABLE LEVEL x (SCALE) o (ORDINAL).
";
    let (table, syntax, engine) = prepare(data, &format!("/FIELDS INPUT=x o {PLAIN}"));
    assert_eq!(
        row(&table, "x"),
        [
            "Input",
            "Continuous",
            "x_transformed",
            "Ordinal",
            "continuous to ordinal (3 values)"
        ]
    );
    assert_eq!(
        row(&table, "o")[3..],
        ["Continuous", "ordinal to continuous (12 values)"]
    );
    assert_eq!(measure(&engine, "x_transformed"), Measure::Ordinal);
    assert_eq!(measure(&engine, "o_transformed"), Measure::Scale);
    assert_eq!(numbers(&engine, "x_transformed"), numbers(&engine, "x"));
    assert!(syntax.contains("RECODE x (MISSING=SYSMIS) (ELSE=COPY) INTO x_transformed.\n"));
    // ADJUSTLEVEL INPUT=NO keeps them
    let (table, _, _) = prepare(
        data,
        &format!("/FIELDS INPUT=x o /ADJUSTLEVEL INPUT=NO {PLAIN}"),
    );
    assert_eq!(row(&table, "x")[2..], ["x", "Continuous", "Unchanged"]);
}

#[test]
fn outliers_by_the_robust_mean_and_sd() {
    // 1 to 39 and 200: mean 24.5, SD sqrt(36530 / 39) = 30.605. Intervals one SD wide from
    // the mean: 1 to 24 in (m - sd, m], 25 to 39 in (m, m + sd], 200 beyond 3 SD. The empty
    // left tails go, then 200 (1/40 = 2.5% < 5%) and the empty intervals down to (m, m + sd];
    // dropping 15 more would pass 5%. Robust mean 20 and SD sqrt(130) from 1 to 39; the cutoff
    // 20 + 3 sqrt(130) = 54.2053.
    let values: String = (1..=39).map(|i| format!("{i} 1\n")).collect();
    let data = format!("DATA LIST FREE /x w.\nBEGIN DATA.\n{values}200 1\nEND DATA.\n");
    let hi = 20.0 + 3.0 * 130f64.sqrt();
    let mut want: Vec<f64> = (1..=39).map(f64::from).collect();
    want.push(hi);
    let (table, syntax, engine) = prepare(&data, "/FIELDS INPUT=x /RESCALE INPUT=NONE");
    close(&numbers(&engine, "x_transformed"), &want);
    assert!(row(&table, "x")[4].starts_with("1 outliers moved to -14.2053 and 54.2053"));
    assert!(syntax.contains("IF (x_transformed > 54.205"));
    // the same with an analysis weight of 1 everywhere, through its own syntax
    let (_, syntax, engine) = prepare(
        &data,
        "/FIELDS INPUT=x ANALYSISWEIGHT=w /RESCALE INPUT=NONE",
    );
    close(&numbers(&engine, "x_transformed"), &want);
    assert!(syntax.contains("SQRT(w)"));
    // as missing values, then the mean of 1 to 39
    let (_, _, engine) = prepare(
        &data,
        "/FIELDS INPUT=x /OUTLIERHANDLING REPLACEWITH=MISSING /RESCALE INPUT=NONE",
    );
    want[39] = 20.0;
    close(&numbers(&engine, "x_transformed"), &want);
    // with a cutoff of 1.5 SD: 20 +- 1.5 sqrt(130) = 2.8733 and 37.1267
    let (_, _, engine) = prepare(
        &data,
        "/FIELDS INPUT=x /OUTLIERHANDLING CUTOFF=SD(1.5) /RESCALE INPUT=NONE",
    );
    let reach = 1.5 * 130f64.sqrt();
    let got = numbers(&engine, "x_transformed");
    assert!((got[0].unwrap() - (20.0 - reach)).abs() < 1e-9);
    assert_eq!(got[10], Some(11.0));
    assert!((got[39].unwrap() - (20.0 + reach)).abs() < 1e-9);
}

#[test]
fn missing_values_take_the_mean_median_or_mode() {
    // c: mean of 2, 4, 6, 8 is 5; o: 1 holds 2 of 4 valid values, half, so the median is 1;
    // s: a and b tie, the least is a ('x' is user-missing)
    let data = "DATA LIST FREE /c o (F8.0) s (A1).
BEGIN DATA.
2 1 b
4 1 a
6 2 b
. 3 a
8 . x
END DATA.
VARIABLE LEVEL c (SCALE) o (ORDINAL) s (NOMINAL).
MISSING VALUES s ('x').
";
    let (table, _, engine) = prepare(
        data,
        &format!("/FIELDS INPUT=c o s /ADJUSTLEVEL INPUT=NO {PLAIN}"),
    );
    close(
        &numbers(&engine, "c_transformed"),
        &[2.0, 4.0, 6.0, 5.0, 8.0],
    );
    close(
        &numbers(&engine, "o_transformed"),
        &[1.0, 1.0, 2.0, 3.0, 1.0],
    );
    assert_eq!(texts(&engine, "s_transformed"), ["b", "a", "b", "a", "a"]);
    assert_eq!(
        row(&table, "o")[4],
        "missing values replaced by the median 1"
    );
    // EXCLUDE leaves those levels alone
    let (table, _, _) = prepare(
        data,
        &format!(
            "/FIELDS INPUT=c o s /ADJUSTLEVEL INPUT=NO
  /REPLACEMISSING INPUT=YES(EXCLUDE(CONTINUOUS NOMINAL)) {PLAIN}"
        ),
    );
    assert_eq!(row(&table, "c")[2..], ["c", "Continuous", "Unchanged"]);
    assert_eq!(row(&table, "s")[4], "Unchanged");
    assert_eq!(row(&table, "o")[2], "o_transformed");
}

#[test]
fn rescaling_by_z_score_and_min_max() {
    // 1 to 5: mean 3, SD sqrt(2.5)
    let data = "DATA LIST FREE /x.\nBEGIN DATA.\n1 2 3 4 5\nEND DATA.\nVARIABLE LEVEL x (SCALE).\n";
    let (_, _, engine) = prepare(data, "/FIELDS INPUT=x /RESCALE INPUT=ZSCORE(MEAN=50 SD=10)");
    let sd = 2.5f64.sqrt();
    close(
        &numbers(&engine, "x_transformed"),
        &[1.0, 2.0, 3.0, 4.0, 5.0].map(|x| (x - 3.0) * 10.0 / sd + 50.0),
    );
    let (_, _, engine) = prepare(
        data,
        "/FIELDS INPUT=x /RESCALE INPUT=MINMAX(MIN=0 MAX=1) /CRITERIA SUFFIX(INPUT='_r')",
    );
    close(&numbers(&engine, "x_r"), &[0.0, 0.25, 0.5, 0.75, 1.0]);
}

#[test]
fn box_cox_target() {
    // y = 2^k, k = 0 to 7, so c = 0; the log-likelihood peaks at lambda 0 (-23.109, against
    // -24.978 at -0.5 and 0.5), so the target becomes ln y = k ln 2 at mean 0 and SD 1:
    // (k - 3.5) / sqrt(6)
    let data = "DATA LIST FREE /y x.
BEGIN DATA.
1 0 2 1.1 4 2 8 3.1 16 4 32 5.1 64 6 128 7.1
END DATA.
VARIABLE LEVEL y x (SCALE).
";
    let (table, syntax, engine) = prepare(data, "/FIELDS TARGET=y INPUT=x");
    close(
        &numbers(&engine, "y_transformed"),
        &(0..8)
            .map(|k| (k as f64 - 3.5) / 6f64.sqrt())
            .collect::<Vec<_>>(),
    );
    assert!(syntax.contains("COMPUTE y_transformed = LN(y_transformed - 0).\n"));
    assert_eq!(
        row(&table, "y")[4],
        "Box-Cox with lambda 0, then mean 0 and SD 1"
    );
}

#[test]
fn supervised_merging_with_a_continuous_target() {
    // a: 1, 2, 3; b: 1.5, 2.5, 3.5; c: 10, 11, 12. For a and b: means 2 and 2.5, joint 2.25,
    // F = (3 * 0.0625 * 2) / ((2 + 2) / 4) = 0.375 on 1 and 4 df, p = 0.573 > 0.05: merged.
    // The pairs with c have p near 0, so a and b form category 1 and c category 2.
    let data = "DATA LIST FREE /g (A1) y (F8.1).
BEGIN DATA.
a 1 a 2 a 3 b 1.5 b 2.5 b 3.5 c 10 c 11 c 12
END DATA.
";
    let (table, syntax, engine) = prepare(data, &format!("/FIELDS TARGET=y INPUT=g {PLAIN}"));
    close(
        &numbers(&engine, "g_transformed"),
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
    assert_eq!(
        row(&table, "g")[4],
        "supervised merging: 3 categories into 2"
    );
    assert!(syntax.contains(
        "RECODE g (MISSING=SYSMIS) ('a'=1) ('b'=1) ('c'=2) (ELSE=SYSMIS) INTO g_transformed.\n"
    ));
    // an ordinal input merges only neighbours: here 1 and 3 are alike but 2 lies between
    let data = "DATA LIST FREE /o y.
BEGIN DATA.
1 1 1 2 1 3 2 10 2 11 2 12 3 1.5 3 2.5 3 3.5
END DATA.
";
    let (table, _, _) = prepare(
        &format!("{data}VARIABLE LEVEL o (ORDINAL).\n"),
        &format!("/FIELDS TARGET=y INPUT=o {PLAIN}"),
    );
    assert_eq!(row(&table, "o")[4], "Unchanged");
    let (_, _, engine) = prepare(
        &format!("{data}VARIABLE LEVEL o (NOMINAL).\n"),
        &format!("/FIELDS TARGET=y INPUT=o {PLAIN}"),
    );
    close(
        &numbers(&engine, "o_transformed"),
        &[1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 1.0, 1.0, 1.0],
    );
    // ordinal merges cut at the largest value of each merged category
    let data = "DATA LIST FREE /o y.
BEGIN DATA.
1 1 1 2 1 3 2 1.5 2 2.5 2 3.5 3 10 3 11 3 12
END DATA.
VARIABLE LEVEL o (ORDINAL).
";
    let (_, syntax, engine) = prepare(data, &format!("/FIELDS TARGET=y INPUT=o {PLAIN}"));
    assert!(
        syntax
            .contains("RECODE o (MISSING=SYSMIS) (LOWEST THRU 2=1) (ELSE=2) INTO o_transformed.\n")
    );
    close(
        &numbers(&engine, "o_transformed"),
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
}

#[test]
fn supervised_merging_with_a_categorical_target() {
    // g against t: a 10/10, b 11/9, c 18/2. a with b: expected 10.5 and 9.5 in each row,
    // chi-square 0.10025 on 1 df, p = 0.75: merged. Then (a, b) 21/19 against c 18/2:
    // expected 26, 14, 13, 7, chi-square 8.24, p = 0.004: kept apart.
    // h: 20/11 against 19/10, nearly the same: merged into one category, so h is dropped.
    let data = "DATA LIST FREE /g (A1) h (A1) t n.
BEGIN DATA.
a a 0 5 a b 0 5 a a 1 5 a b 1 5
b a 0 6 b b 0 5 b a 1 5 b b 1 4
c a 0 9 c b 0 9 c a 1 1 c b 1 1
END DATA.
VARIABLE LEVEL t (NOMINAL).
WEIGHT BY n.
";
    let (table, _, engine) = prepare(data, "/FIELDS TARGET=t INPUT=g h");
    close(
        &numbers(&engine, "g_transformed"),
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0],
    );
    assert_eq!(
        row(&table, "h")[4],
        "Excluded: supervised merging leaves a single category"
    );
    // an ordinal target: a and b have the same profile (5, 5, 5 and 10, 10, 10), so the row
    // effects model fits them no better than independence (p = 1); c (20, 5, 1) differs
    let data = "DATA LIST FREE /g (A1) t n.
BEGIN DATA.
a 1 5 a 2 5 a 3 5 b 1 10 b 2 10 b 3 10 c 1 20 c 2 5 c 3 1
END DATA.
VARIABLE LEVEL t (ORDINAL).
WEIGHT BY n.
";
    let (_, _, engine) = prepare(data, "/FIELDS TARGET=t INPUT=g");
    close(
        &numbers(&engine, "g_transformed"),
        &[1.0, 1.0, 1.0, 1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
}

#[test]
fn unsupervised_merging_by_counts() {
    // by ascending count: e 3, d 5, c 12, b 30, a 50, at least 10 of 100 each: e + d + c = 20
    // is the first group, then b and a alone. Groups numbered by their least value.
    let data = "DATA LIST FREE /n (A1) w.
BEGIN DATA.
a 50 b 30 c 12 d 5 e 3
END DATA.
WEIGHT BY w.
";
    let options = "/FIELDS INPUT=n /TRANSFORM MERGEUNSUPERVISED=YES(NOMINAL MINPCT=10)";
    let (table, _, engine) = prepare(data, options);
    close(
        &numbers(&engine, "n_transformed"),
        &[1.0, 2.0, 3.0, 3.0, 3.0],
    );
    assert_eq!(
        row(&table, "n")[4],
        "unsupervised merging: 5 categories into 3"
    );
    // renumbered by count from 0: (c, d, e) 20, b 30, a 50
    let (_, _, engine) = prepare(data, &format!("{options} /REORDERNOMINAL INPUT=YES"));
    close(
        &numbers(&engine, "n_transformed"),
        &[2.0, 1.0, 0.0, 0.0, 0.0],
    );
    // NOMINAL not given: nothing merged
    let (table, _, _) = prepare(
        data,
        "/FIELDS INPUT=n /TRANSFORM MERGEUNSUPERVISED=YES(ORDINAL)",
    );
    assert_eq!(row(&table, "n")[4], "Unchanged");
    // ordinal by value: 1 (5) + 2 (40) is the first group; 3 (3) + 4 (2) reach the end
    // with only one group made, so 3, 4 and 5 form the second
    let data = "DATA LIST FREE /o w.
BEGIN DATA.
1 5 2 40 3 3 4 2 5 50
END DATA.
VARIABLE LEVEL o (ORDINAL).
WEIGHT BY w.
";
    let (_, syntax, engine) = prepare(
        data,
        "/FIELDS INPUT=o /TRANSFORM MERGEUNSUPERVISED=YES(ORDINAL NOMINAL)",
    );
    close(
        &numbers(&engine, "o_transformed"),
        &[1.0, 1.0, 2.0, 2.0, 2.0],
    );
    assert!(syntax.contains("(LOWEST THRU 2=1) (ELSE=2)"));
}

#[test]
fn supervised_binning_against_a_categorical_target() {
    // means 2 and 8, each sum of squares 2, N = 6, J = 2: critical difference
    // sqrt(2 F(0.95; 1, 4)) * sqrt(4 / 4) * sqrt((1/3 + 1/3) / 2) = 2.267 < 6, two subsets
    // with SD 1 each, cut halfway at 5
    let data = "DATA LIST FREE /x t.
BEGIN DATA.
1 0 2 0 3 0 7 1 8 1 9 1
END DATA.
VARIABLE LEVEL t (NOMINAL) x (SCALE).
";
    let (table, syntax, engine) = prepare(data, "/FIELDS TARGET=t INPUT=x");
    close(
        &numbers(&engine, "x_transformed"),
        &[1.0, 1.0, 1.0, 2.0, 2.0, 2.0],
    );
    assert!(syntax.contains("RECODE x_transformed (MISSING=SYSMIS) (LOWEST THRU 5=1) (ELSE=2).\n"));
    assert_eq!(row(&table, "x")[3..], ["Ordinal", "2 bins, cut at 5"]);
    // three classes, means 2, 2.5 and 11 (each sum of squares 2, N = 9): the critical
    // difference sqrt(4 F(0.95; 2, 6)) sqrt(1/3) = 2.619 joins the first two (mean 2.25, sum
    // of squares 4.375, SD sqrt(4.375 / 5)); 11 stands alone with SD 1. The cut:
    // 2.25 + sd / (sd + 1) * 8.75
    let data = "DATA LIST FREE /x t.
BEGIN DATA.
1 1 2 1 3 1 1.5 2 2.5 2 3.5 2 10 3 11 3 12 3
END DATA.
VARIABLE LEVEL t (NOMINAL) x (SCALE).
";
    let (table, _, _) = prepare(data, "/FIELDS TARGET=t INPUT=x /OUTLIERHANDLING INPUT=NO");
    let sd = (4.375f64 / 5.0).sqrt();
    let cut = 2.25 + sd / (sd + 1.0) * 8.75;
    assert_eq!(row(&table, "x")[4], format!("2 bins, cut at {cut:.3}"));
}

#[test]
fn selection_drops_inputs_unrelated_to_a_continuous_target() {
    // x2 is symmetric around the middle of y, so its correlation with y is 0 and p = 1
    let data = "DATA LIST FREE /y x1 x2.
BEGIN DATA.
1 2 1 2 4 2 3 6 3 4 8 4 5 10 5 6 12 5 7 14 4 8 16 3 9 18 2 10 20 1
END DATA.
VARIABLE LEVEL y x1 x2 (SCALE).
";
    let (table, _, engine) = prepare(data, &format!("/FIELDS TARGET=y INPUT=x1 x2 {PLAIN}"));
    assert_eq!(
        row(&table, "x2")[4],
        "Excluded: not correlated with the target (p = 1)"
    );
    close(
        &numbers(&engine, "x1"),
        &(1..=10).map(|i| 2.0 * i as f64).collect::<Vec<_>>(),
    );
    assert_eq!(row(&table, "x1")[4], "Unchanged");
    let (table, _, _) = prepare(
        data,
        &format!("/FIELDS TARGET=y INPUT=x1 x2 /TRANSFORM SELECTION=NO {PLAIN}"),
    );
    assert_eq!(row(&table, "x2")[4], "Unchanged");
}

#[test]
fn errors_and_warnings() {
    let data = "DATA LIST FREE /y x (F8.0) s (A1).\nBEGIN DATA.\n1 1 a 2 2 b 3 3 a 4 4 b 5 5 a\nEND DATA.\nVARIABLE LEVEL y x (SCALE).\n";
    assert!(
        fails(&format!(
            "{data}ADP /FIELDS TARGET=y INPUT=x y /OUTFILE PREPXML='p.sps'."
        ))
        .contains("cannot also be an input")
    );
    assert!(
        fails(&format!(
            "{data}ADP /FIELDS INPUT=x ANALYSISWEIGHT=s /OUTFILE PREPXML='p.sps'."
        ))
        .contains("must be numeric")
    );
    assert!(
        fails(&format!(
            "{data}ADP /FIELDS INPUT=x /CRITERIA SUFFIX(INPUT='_{}') /OUTFILE PREPXML='p.sps'.",
            "z".repeat(70)
        ))
        .contains("not a valid variable name")
    );
    let dir = std::env::temp_dir().join(format!("statscraft-adp-split-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (engine, _) = run(&format!(
        "{data}CD '{}'.\nSPLIT FILE BY s.\nADP /FIELDS INPUT=x /OUTFILE PREPXML='p.sps'.",
        dir.display()
    ));
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(
        engine
            .warnings()
            .join("\n")
            .contains("does not use SPLIT FILE")
    );
}

/// Forty cases: x1, x2 and x3 follow one source with different noise, x4 is a second source,
/// x5 is noise, y depends on the two sources. The expectations are an R implementation of the
/// manual's steps (cor.test for the p-values, eigen of the covariance matrix, the partial
/// correlation formula): x5 is dropped for its p with the target, x1 and x2 form the group at
/// the level 0.9 (x3 is less correlated with them), their component is the new predictor,
/// x3 is then dropped (partial correlation p 0.414), x4 stays.
const FEATURES: &str = "DATA LIST FREE /x1 x2 x3 x4 x5 y.\n\
BEGIN DATA.\n\
-0.453 -0.691 -0.138 -2.621 -0.114 -3.566 \n\
1.184 0.862 1.915 2.246 -0.295 4.483 \n\
-1.274 -0.963 -2.046 0.093 0.989 -2.478 \n\
0.544 0.063 -0.916 1.627 -0.775 2.472 \n\
1.597 1.914 2.347 -0.511 0.276 1.979 \n\
-0.462 -0.816 -0.429 -0.659 0.411 -2.212 \n\
-0.694 0.244 -0.712 -0.04 0.611 -0.654 \n\
-0.75 -0.777 0.11 -0.119 0.937 -1.493 \n\
-0.467 -0.309 -1.106 -0.02 -0.368 -1.398 \n\
0.121 -0.018 -0.727 -0.486 0.74 0.174 \n\
1.593 1.505 2.037 -1.44 1.219 1.122 \n\
-0.755 -1.121 -1.989 0.144 0.629 -1.675 \n\
-0.825 -0.913 -1.825 -1.235 0.528 -3.034 \n\
-0.305 0.113 -0.22 -1.753 -0.472 -1.992 \n\
-1.1 -0.775 -0.632 -0.035 0.824 -3.463 \n\
-0.37 -0.024 0.134 0.332 -0.428 -1.092 \n\
-0.409 -0.701 -0.424 1.572 -0.143 -0.347 \n\
-2.212 -2.346 -2.828 -1.069 1.419 -5.82 \n\
0.225 0.186 0.63 0.916 0.487 1.759 \n\
-0.201 -0.277 -0.08 -0.595 0.603 -1.016 \n\
0.616 0.302 0.423 2.182 0.211 2.527 \n\
1.156 1.282 0.924 -0.684 -0.033 1.402 \n\
1.323 1.671 2.776 0.75 2.025 4.224 \n\
0.831 0.769 1.281 0.974 -0.371 1.775 \n\
0.629 0.802 0.636 -1.264 -1.578 0.679 \n\
-0.379 -0.025 -0.545 -0.277 -0.122 0.729 \n\
0.893 1.35 1.479 -0.189 -1.797 3.242 \n\
1.423 0.909 1.361 -0.384 -0.476 2.313 \n\
-0.975 -0.883 -1.506 0.741 -0.884 -1.843 \n\
-0.923 -0.469 -1.088 -1.168 -3.498 -3.152 \n\
0.265 0.03 0.884 0.668 -0.382 0.917 \n\
1.053 1.596 1.561 0.366 0.978 1.823 \n\
2.302 2.996 1.905 -0.515 -0.558 3.507 \n\
1.225 1.259 1.702 0.451 -0.626 3.82 \n\
1.583 1.074 1.111 -0.188 -0.53 4.109 \n\
0.913 1.191 1.695 1.339 1.898 2.977 \n\
-0.766 -1.476 -1.212 0.816 1.396 -2.307 \n\
-1.97 -1.861 -1.283 0.082 -0.746 -3.829 \n\
-1.715 -1.746 -2.028 -0.651 -0.306 -3.842 \n\
-0.283 -0.203 -0.031 0.726 1.17 0.21 \n\
END DATA.\n\
VARIABLE LEVEL x1 x2 x3 x4 x5 y (SCALE).\n\
";

#[test]
fn construction_groups_correlated_inputs_into_a_principal_component() {
    let (table, syntax, engine) = prepare(
        FEATURES,
        &format!("/FIELDS TARGET=y INPUT=x1 x2 x3 x4 x5 /TRANSFORM CONSTRUCTION=YES {PLAIN}"),
    );
    close(
        &numbers(&engine, "feature1"),
        &[
            -0.5205257189,
            0.9203125454,
            -1.0068100261,
            0.2675557044,
            1.5916456886,
            -0.5827490229,
            -0.1900880863,
            -0.6906607007,
            -0.3485431243,
            0.0445850728,
            1.3991930132,
            -0.8532441497,
            -0.7869098717,
            -0.0808514178,
            -0.8429697372,
            -0.1731885210,
            -0.5059236900,
            -2.0623383835,
            0.1852372487,
            -0.2171591391,
            0.4105164091,
            1.1038834508,
            1.3583776615,
            0.7223950767,
            0.6493394650,
            -0.1775952808,
            1.0204392100,
            1.0468689756,
            -0.8385969531,
            -0.6227969798,
            0.1300136273,
            1.2051930587,
            2.4048139510,
            1.1233698936,
            1.1938555656,
            0.9550601406,
            -1.0235834839,
            -1.7302454998,
            -1.5649778944,
            -0.2185580263,
        ],
    );
    assert_eq!(
        row(&table, "x1")[4],
        "Replaced by the constructed predictor feature1"
    );
    assert_eq!(
        row(&table, "x2")[4],
        "Replaced by the constructed predictor feature1"
    );
    assert!(
        row(&table, "x3")[4].starts_with(
            "Excluded: not correlated with the target given the constructed predictor"
        )
    );
    assert!(row(&table, "x5")[4].starts_with("Excluded: not correlated with the target"));
    assert_eq!(row(&table, "x4")[4], "Unchanged");
    assert_eq!(row(&table, "feature1")[0], "Input from x1, x2");
    assert!(syntax.contains("COMPUTE feature1 ="), "{syntax}");
    assert_eq!(measure(&engine, "feature1"), Measure::Scale);
    // the root is chosen, and without CONSTRUCTION nothing is built
    let (_, _, named) = prepare(
        FEATURES,
        &format!(
            "/FIELDS TARGET=y INPUT=x1 x2 x3 x4 x5 /TRANSFORM CONSTRUCTION=YES(ROOT=pc) {PLAIN}"
        ),
    );
    assert_eq!(numbers(&named, "pc1").len(), 40);
    let (plain, _, _) = prepare(
        FEATURES,
        &format!("/FIELDS TARGET=y INPUT=x1 x2 x3 x4 x5 {PLAIN}"),
    );
    assert!(plain.rows.iter().all(|r| text(&r[0]) != "feature1"));
}
