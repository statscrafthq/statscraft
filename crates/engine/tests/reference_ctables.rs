//! Compares CTABLES with GNU PSPP, run as a black box on the same file.
//!
//! The tests are ignored by default. They run when STATSCRAFT_PSPP names the PSPP executable:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_ctables -- --ignored
//! ```
//!
//! `check(fixture, syntax)` runs the syntax after loading the fixture in both programs. PSPP
//! writes its tables as CSV, with every number already formatted. Our tables are laid out the
//! same way and every number is formatted with the cell's own display format by
//! `format_value`, then the two grids are compared as text, cell by cell: titles, header
//! labels, row labels and values. Comparing formatted text rather than numbers with a
//! tolerance also checks the formats, and the format decides the tolerance anyway: a cell
//! shown as 45.2% matches any value that rounds to it. Where PSPP repeats nothing, in spanned
//! headers and in row labels of the same group, the grid shows a blank, so ours blanks a
//! label that repeats the one before it under the same parents. PSPP shows a statistic it
//! cannot compute as "."; that matches our empty cell.

use statscraft_engine::{Cell, Engine, Table, format_value};
use statscraft_syntax::parse;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static RUNS: AtomicUsize = AtomicUsize::new(0);

/// A table as text: title, rows of cells, caption.
#[derive(Debug, PartialEq)]
struct Grid {
    title: String,
    rows: Vec<Vec<String>>,
    caption: Option<String>,
}

/// Records of a CSV text: quoted fields may hold commas, doubled quotes and line breaks.
fn csv_records(text: &str) -> Vec<Vec<String>> {
    let mut records = Vec::new();
    let (mut record, mut field) = (Vec::new(), String::new());
    let mut chars = text.chars().peekable();
    let mut quoted = false;
    while let Some(c) = chars.next() {
        match (quoted, c) {
            (true, '"') if chars.peek() == Some(&'"') => {
                field.push('"');
                chars.next();
            }
            (true, '"') => quoted = false,
            (true, c) => field.push(c),
            (false, '"') => quoted = true,
            (false, ',') => record.push(std::mem::take(&mut field)),
            (false, '\r') => {}
            (false, '\n') => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
            }
            (false, c) => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
}

fn pspp_grids(csv: &str) -> Vec<Grid> {
    let mut grids: Vec<Grid> = Vec::new();
    let mut open = false;
    for record in csv_records(csv) {
        let single = record.len() == 1;
        if single && let Some(title) = record[0].strip_prefix("Table: ") {
            grids.push(Grid {
                title: title.to_string(),
                rows: Vec::new(),
                caption: None,
            });
            open = true;
        } else if single && record[0].is_empty() {
            open = false;
        } else if single && open && record[0].starts_with("Caption: ") {
            grids.last_mut().unwrap().caption = Some(record[0][9..].to_string());
        } else if open {
            let cells = record.iter().map(|cell| {
                let cell = cell.trim();
                // a statistic that cannot be computed: "." aligned like the format's decimals
                if cell == "." {
                    String::new()
                } else {
                    cell.to_string()
                }
            });
            grids.last_mut().unwrap().rows.push(cells.collect());
        }
    }
    grids
}

/// Blanks every label that repeats the label before it under the same parents, the way a
/// spanned label shows in PSPP's CSV.
fn blank_repeats(labels: &[Vec<String>]) -> Vec<Vec<String>> {
    labels
        .iter()
        .enumerate()
        .map(|(i, path)| {
            (0..path.len())
                .map(|level| {
                    let repeated = i > 0 && labels[i - 1][..=level] == path[..=level];
                    if repeated {
                        String::new()
                    } else {
                        path[level].clone()
                    }
                })
                .collect()
        })
        .collect()
}

fn our_grid(table: &Table) -> Grid {
    let width = table.columns.len();
    let levels = table.column_groups.first().map_or(0, Vec::len) + 1;
    let header = |c: usize, level: usize| {
        if level + 1 == levels {
            table.columns[c].clone()
        } else {
            table.column_groups[c][level].clone()
        }
    };
    let data_paths: Vec<Vec<String>> = (table.row_headers..width)
        .map(|c| (0..levels).map(|level| header(c, level)).collect())
        .collect();
    let data_paths = blank_repeats(&data_paths);
    let mut rows = Vec::new();
    for level in 0..levels {
        let mut row: Vec<String> = (0..table.row_headers).map(|c| header(c, level)).collect();
        row.extend(data_paths.iter().map(|path| path[level].clone()));
        if row.iter().any(|cell| !cell.is_empty()) {
            rows.push(row);
        }
    }
    let text = |cell: &Cell, format: &str| match cell {
        Cell::Text(t) => t.clone(),
        Cell::Number(n) => format_value(*n, format, false),
        Cell::Empty => String::new(),
    };
    let labels: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|row| {
            row[..table.row_headers]
                .iter()
                .map(|c| text(c, ""))
                .collect()
        })
        .collect();
    for (r, labels) in blank_repeats(&labels).into_iter().enumerate() {
        let mut row = labels;
        for c in table.row_headers..width {
            row.push(text(&table.rows[r][c], &table.formats[r][c]));
        }
        rows.push(row);
    }
    Grid {
        title: table.title.clone(),
        rows,
        caption: table.caption.clone(),
    }
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures")
}

/// Runs the job in PSPP; returns its tables and its messages.
fn run_pspp(fixture: &str, syntax: &str) -> (Vec<Grid>, String) {
    let pspp = std::env::var("STATSCRAFT_PSPP").expect("STATSCRAFT_PSPP names the PSPP executable");
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("statscraft-ctables-{}-{run}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(fixture_dir().join(fixture), dir.join(fixture)).unwrap();
    // PSPP here may be a Windows program, so it gets relative paths and runs in the directory
    std::fs::write(
        dir.join("job.sps"),
        format!("GET FILE='{fixture}'.\n{syntax}\n"),
    )
    .unwrap();
    let run = || {
        Command::new(&pspp)
            .current_dir(&dir)
            .args(["job.sps", "-o", "out.csv", "-O", "format=csv"])
            .output()
            .unwrap_or_else(|e| panic!("cannot run {pspp}: {e}"))
    };
    let mut output = run();
    // a Windows program started from WSL now and then fails to start when several start at once
    for _ in 0..3 {
        if !String::from_utf8_lossy(&output.stderr).contains("UtilAcceptVsock") {
            break;
        }
        output = run();
    }
    let csv = std::fs::read_to_string(dir.join("out.csv")).unwrap_or_default();
    std::fs::remove_dir_all(&dir).unwrap();
    let messages = String::from_utf8_lossy(&output.stderr).to_string()
        + &String::from_utf8_lossy(&output.stdout);
    (pspp_grids(&csv), messages)
}

fn run_ours(fixture: &str, syntax: &str) -> Result<Vec<Table>, String> {
    let path = fixture_dir().join(fixture);
    let source = format!("GET FILE='{}'.\n{syntax}\n", path.display());
    let (commands, errors) = parse(&source);
    if let Some(error) = errors.first() {
        return Err(error.to_string());
    }
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for parsed in &commands {
        let result = engine.execute(&parsed.command);
        tables.extend(result.map_err(|e| format!("line {}: {e}", parsed.first_line))?);
    }
    Ok(tables)
}

fn show(grid: &Grid) -> String {
    let rows: Vec<String> = grid.rows.iter().map(|r| r.join(" | ")).collect();
    format!("{}\n{}\n{:?}", grid.title, rows.join("\n"), grid.caption)
}

fn check(fixture: &str, syntax: &str) {
    let (theirs, messages) = run_pspp(fixture, syntax);
    let ours = run_ours(fixture, syntax).unwrap_or_else(|e| panic!("{e}\n{syntax}"));
    let ours: Vec<Grid> = ours.iter().map(our_grid).collect();
    assert_eq!(
        ours.len(),
        theirs.len(),
        "number of tables for:\n{syntax}\nPSPP said:\n{messages}"
    );
    for (mine, pspp) in ours.iter().zip(&theirs) {
        assert!(
            mine == pspp,
            "tables differ for:\n{syntax}\n--- ours\n{}\n--- PSPP\n{}",
            show(mine),
            show(pspp)
        );
    }
}

/// Both programs reject the syntax; ours with a message that contains `fragment`.
fn check_rejected(fixture: &str, syntax: &str, fragment: &str) {
    let (theirs, messages) = run_pspp(fixture, syntax);
    assert!(
        theirs.is_empty() && messages.contains("error"),
        "PSPP accepted:\n{syntax}"
    );
    match run_ours(fixture, syntax) {
        Ok(_) => panic!("we accepted:\n{syntax}"),
        Err(e) => assert!(e.contains(fragment), "{e}"),
    }
}

/// A layered table. PSPP writes only its first layer, with a "Layer: category" line above the
/// header that names one of its layer categories or summaries, so that table is compared with
/// our first one, whose title names them all.
fn check_first_layer(fixture: &str, syntax: &str) {
    let (theirs, messages) = run_pspp(fixture, syntax);
    let ours = run_ours(fixture, syntax).unwrap_or_else(|e| panic!("{e}\n{syntax}"));
    let mut pspp = theirs
        .into_iter()
        .next()
        .unwrap_or_else(|| panic!("no table from PSPP:\n{messages}"));
    let mine = our_grid(&ours[0]);
    assert!(mine.title.starts_with(&format!("{} (", pspp.title)));
    while let Some(layer) = pspp.rows[0][0].strip_prefix("Layer: ") {
        assert!(
            mine.title.contains(&format!(" {layer}")) || mine.title.contains(&format!("({layer}")),
            "{} is not layer {layer}",
            mine.title
        );
        pspp.rows.remove(0);
    }
    pspp.title = mine.title.clone();
    assert!(
        mine == pspp,
        "tables differ for:\n{syntax}\n--- ours\n{}\n--- PSPP\n{}",
        show(&mine),
        show(&pspp)
    );
}

fn survey(cases: &[&str]) {
    for syntax in cases {
        check("survey_basic.sav", syntax);
    }
}

/// The reference program rounds case weights in CTABLES and we do not, so the weighted cases use
/// a whole-number weight derived from the file's weight, where both rules give the same result.
fn weighted(cases: &[&str]) {
    for syntax in cases {
        let syntax = format!("COMPUTE w = RND(weight * 3).\nWEIGHT BY w.\n{syntax}");
        check("survey_basic_phpwriter.sav", &syntax);
    }
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn counts_and_layout() {
    survey(&[
        "CTABLES /TABLE gender BY region.",
        "CTABLES /TABLE gender.",
        "CTABLES /TABLE q1_satisfaction.",
        "CTABLES /TABLE gender > region.",
        "CTABLES /TABLE gender + region.",
        "CTABLES /TABLE region BY gender > q5_1.",
        "CTABLES /TABLE gender > region + age BY q5_1.",
        "CTABLES /TABLE gender BY region + age.",
        "CTABLES /TABLE plan BY gender.",
        "CTABLES /TABLE (gender + plan) > q5_1 BY region.",
        "CTABLES /TABLE gender BY region /TABLE q5_1 + q5_2 BY gender.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn percentages() {
    survey(&[
        "CTABLES /TABLE gender [COUNT, ROWPCT.COUNT, COLPCT.COUNT, TABLEPCT.COUNT].",
        "CTABLES /TABLE gender [COUNT, COLPCT.COUNT] BY region.",
        "CTABLES /TABLE gender BY region [COLPCT.COUNT].",
        // PSPP leaves cases with a hidden missing column value out of ROWPCT.TOTALN, where the
        // manual counts them, so that function is checked in ctables.rs instead
        "CTABLES /TABLE gender [COUNT, ROWPCT.COUNT, ROWPCT.VALIDN] BY q1_satisfaction.",
        "CTABLES /TABLE gender BY region > q5_1 [ROWPCT.COUNT, COLPCT.COUNT].",
        "CTABLES /TABLE gender [COUNT, TABLEPCT.COUNT, LAYERPCT.COUNT, LAYERROWPCT.COUNT, LAYERCOLPCT.COUNT, SUBTABLEPCT.VALIDN, ROWPCT.TOTALN] + region [TABLEPCT.COUNT] BY q5_1.",
        "CTABLES /TABLE gender [COUNT, TABLEPCT.COUNT, TABLEPCT.VALIDN, TABLEPCT.TOTALN] BY q1_satisfaction\n\
         /CATEGORIES VARIABLES=q1_satisfaction MISSING=INCLUDE.",
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT.COUNT, COLPCT.VALIDN, COLPCT.TOTALN] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction TOTAL=YES MISSING=INCLUDE.",
        "CTABLES /TABLE region > gender [COUNT, ROWPCT.COUNT, COLPCT.COUNT, SUBTABLEPCT.COUNT] BY q5_1 > q5_2.",
        // a percentage without its .COUNT suffix, commas left out
        "CTABLES /TABLE gender [COUNT ROWPCT COLPCT TABLEPCT SUBTABLEPCT] BY region.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn totals() {
    survey(&[
        "CTABLES /TABLE gender BY region /CATEGORIES VARIABLES=gender region TOTAL=YES.",
        // PSPP leaves a percentage blank in the total of a variable that decides its base, where
        // the manual shows it (Figure 25: column percentages in a Total column), so these cases
        // keep only percentages whose base no totaled variable decides; ctables.rs checks the rest
        "CTABLES /TABLE gender [COUNT, TABLEPCT.COUNT] BY region\n\
         /CATEGORIES VARIABLES=gender region TOTAL=YES.",
        "CTABLES /TABLE gender [COUNT, COLPCT.COUNT] BY region /CATEGORIES VARIABLES=gender TOTAL=YES.",
        "CTABLES /TABLE gender [COUNT, ROWPCT.COUNT] BY region /CATEGORIES VARIABLES=region TOTAL=YES.",
        // PSPP leaves a count of 0 blank in a total or beside a scale variable, where the manual
        // shows 0, so the third gender, which has empty cells, is left out
        "CTABLES /TABLE gender > region [COUNT, TABLEPCT.COUNT] BY q5_1\n\
         /CATEGORIES VARIABLES=gender [1, 2] TOTAL=YES /CATEGORIES VARIABLES=region q5_1 TOTAL=YES.",
        "CTABLES /TABLE gender > region [COUNT, COLPCT.COUNT, SUBTABLEPCT.COUNT] BY q5_1\n\
         /CATEGORIES VARIABLES=gender [1, 2] /CATEGORIES VARIABLES=region TOTAL=YES.",
        "CTABLES /TABLE q1_satisfaction BY gender /CATEGORIES VARIABLES=q1_satisfaction MISSING=INCLUDE TOTAL=YES.",
        "CTABLES /TABLE region /CATEGORIES VARIABLES=region KEY=LABEL ORDER=D TOTAL=YES LABEL='All regions' POSITION=BEFORE.",
        // PSPP leaves a count of 0 blank in a total or beside a scale variable, where the manual
        // shows 0, so the third gender, which has empty cells, is left out
        "CTABLES /TABLE gender > age [MEAN, COUNT, VALIDN] BY region /CATEGORIES VARIABLES=gender [1, 2] TOTAL=YES\n\
         /CATEGORIES VARIABLES=region TOTAL=YES.",
        "CTABLES /TABLE age [MEAN] BY gender /CATEGORIES VARIABLES=gender TOTAL=YES LABEL='All'.",
        "CTABLES /TABLE q6_mention1 /CATEGORIES VARIABLES=q6_mention1 MISSING=INCLUDE TOTAL=YES.",
        "CTABLES /TABLE q6_mention1 [COUNT, COLPCT.COUNT, COLPCT.VALIDN] /CATEGORIES VARIABLES=q6_mention1 TOTAL=YES.",
        "CTABLES /TABLE region BY gender /CATEGORIES VARIABLES=region [3, 1, 5] TOTAL=YES\n\
         /CATEGORIES VARIABLES=gender TOTAL=YES POSITION=BEFORE.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn scale_summaries() {
    survey(&[
        "CTABLES /TABLE age BY gender.",
        "CTABLES /TABLE age [MEAN, MEDIAN, MODE, MINIMUM, MAXIMUM, RANGE, SUM, STDDEV, VARIANCE, VALIDN, TOTALN, MISSING, SEMEAN, COUNT] BY gender.",
        "CTABLES /TABLE age [MEAN F12.6, MEDIAN F12.6, MODE F12.6, STDDEV F12.6, VARIANCE F12.6, SEMEAN F12.6] BY gender.",
        "CTABLES /TABLE monthly_spend [MEAN, MEDIAN, VALIDN, TOTALN, MISSING, COUNT, SUM, ROWPCT.SUM, COLPCT.VALIDN, COLPCT.TOTALN, COLPCT.COUNT] BY region.",
        "CTABLES /TABLE monthly_spend [MEDIAN F12.6, MODE F12.6, MINIMUM, MAXIMUM, RANGE] BY region.",
        "CTABLES /TABLE q2_recommend [MEAN, VALIDN, TOTALN, MISSING, COUNT] BY gender.",
        "CTABLES /TABLE gender > age BY region.",
        "CTABLES /TABLE gender BY age + monthly_spend.",
        "CTABLES /TABLE age + monthly_spend BY gender.",
        "CTABLES /TABLE age [MEAN, COUNT] + monthly_spend [MEAN F8.1, VALIDN] BY gender.",
        "CTABLES /TABLE age [MEAN, MEAN 'Avg' F8.2, SUM] + monthly_spend [MEAN 'Avg', VALIDN] BY gender.",
        "CTABLES /TABLE q5_1 [S] [SUM, MEAN PCT8.1] + q5_2 [S] [SUM, MEAN PCT8.1] BY gender.",
        // PSPP leaves a count of 0 blank in a total or beside a scale variable, where the manual
        // shows 0, so the third gender, which has empty cells, is left out
        "CTABLES /TABLE gender > monthly_spend [COUNT, COLPCT.COUNT, COLPCT.VALIDN, COLPCT.TOTALN, VALIDN, TOTALN, TABLEPCT.VALIDN] BY region\n\
         /CATEGORIES VARIABLES=gender [1, 2].",
        "CTABLES /TABLE monthly_spend [COLPCT.SUM, TABLEPCT.SUM, SUBTABLEPCT.SUM, ROWPCT.SUM] BY gender.",
        "CTABLES /TABLE q1_satisfaction [S] [MEAN, COUNT] BY gender.",
        // PSPP leaves a count of 0 blank in a total or beside a scale variable, where the manual
        // shows 0, so the third gender, which has empty cells, is left out
        "CTABLES /TABLE region > monthly_spend [MEAN F8.2, UCOUNT, UMEAN F8.3] BY gender /CATEGORIES VARIABLES=gender [1, 2].",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn categories() {
    survey(&[
        "CTABLES /TABLE region /CATEGORIES VARIABLES=region ORDER=D.",
        "CTABLES /TABLE region /CATEGORIES VARIABLES=region KEY=LABEL.",
        "CTABLES /TABLE region /CATEGORIES VARIABLES=region [3, 1, 5].",
        "CTABLES /TABLE q1_satisfaction /CATEGORIES VARIABLES=q1_satisfaction [5, 4, 99, 1 THRU 3].",
        "CTABLES /TABLE gender BY q2_recommend [C].",
        "CTABLES /TABLE q2_recommend [C] /CATEGORIES VARIABLES=q2_recommend EMPTY=EXCLUDE.",
        "CTABLES /TABLE q2_recommend [C] BY gender /CATEGORIES VARIABLES=q2_recommend MISSING=INCLUDE.",
        "CTABLES /TABLE monthly_spend [C] /CATEGORIES VARIABLES=monthly_spend [12.5, 1, 2].",
        "CTABLES /TABLE plan /CATEGORIES VARIABLES=plan ['PLUS', 'BASIC', 'NONE'].",
        "CTABLES /TABLE age [C] /CATEGORIES VARIABLES=age [18 THRU 20, LO THRU 19, 83 THRU HI].",
        "CTABLES /TABLE plan BY region /CATEGORIES VARIABLES=plan ORDER=D TOTAL=YES.",
        "MISSING VALUES region (1).\nCTABLES /TABLE region /CATEGORIES VARIABLES=region MISSING=INCLUDE.",
        "MISSING VALUES region (1).\nCTABLES /TABLE region BY gender.",
        "VALUE LABELS gender 1 'Male' 2 'Female' 3 'Other' 4 'Not said'.\n\
         CTABLES /TABLE gender BY region /CATEGORIES VARIABLES=gender TOTAL=YES.",
        "VALUE LABELS gender 1 'Male' 2 'Female' 3 'Other' 4 'Not said'.\n\
         CTABLES /TABLE gender BY region /CATEGORIES VARIABLES=gender EMPTY=EXCLUDE.",
        "COMPUTE ms = monthly_spend.\nFORMATS ms (F6.1).\nVALUE LABELS ms 12.5 'Twelve and a half'.\n\
         CTABLES /TABLE ms [C] /CATEGORIES VARIABLES=ms [12.5, 100 THRU 101].",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn labels_titles_and_formats() {
    survey(&[
        "CTABLES /VLABELS VARIABLES=gender DISPLAY=NAME /VLABELS VARIABLES=region DISPLAY=BOTH /TABLE gender BY region.",
        "CTABLES /VLABELS VARIABLES=gender DISPLAY=NONE /VLABELS VARIABLES=region DISPLAY=LABEL\n\
         /TABLE gender BY region /SLABELS VISIBLE=NO.",
        "CTABLES /TABLE gender [COUNT, COLPCT.COUNT] BY region /SLABELS POSITION=ROW.",
        "CTABLES /TABLE gender > region + age [MEAN, SUM] BY q5_1 /SLABELS POSITION=ROW.",
        "CTABLES /TABLE gender BY region /TITLES TITLE='My title' CAPTION='My caption' CORNER='Corner text'.",
        "CTABLES /TABLE q1_satisfaction [COUNT 'N' F8.2, COLPCT.VALIDN 'Valid col pct' PCT8.3, TABLEPCT.COUNT] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction MISSING=INCLUDE TOTAL=YES.",
        // PSPP leaves column percentages blank in the Total column, where the manual shows them
        // (Figure 25), so only row percentages go with the total of region
        "CTABLES /FORMAT MISSING='x' EMPTY=BLANK /TABLE gender [COUNT, ROWPCT.COUNT] BY region\n\
         /CATEGORIES VARIABLES=region TOTAL=YES.",
        "CTABLES /FORMAT MISSING='x' /TABLE gender > age [MEAN, STDDEV] BY region.",
        // PSPP leaves a count of 0 blank in a total or beside a scale variable, where the manual
        // shows 0, so the third gender, which has empty cells, is left out
        "CTABLES /FORMAT EMPTY='none' /TABLE gender > monthly_spend [MEAN, COUNT] BY region /CATEGORIES VARIABLES=gender [1, 2].",
        "CTABLES /FORMAT EMPTY='none' /TABLE gender BY region.",
        "CTABLES /TABLE monthly_spend [SUM COMMA12.2, MEAN DOLLAR10.2] BY gender.",
        "CTABLES /HIDESMALLCOUNTS COUNT=10 /TABLE gender [COUNT, COLPCT] BY region.",
        "CTABLES /HIDESMALLCOUNTS /TABLE gender > q5_1 [COUNT, ROWPCT] BY region.",
        "VARIABLE LABELS region 'Where they live'.\nCTABLES /TABLE region [COUNT, COLPCT.COUNT PCT5.0] BY gender.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn derived_variables() {
    survey(&[
        "RECODE age (LOWEST THRU 29=1) (30 THRU 49=2) (50 THRU HIGHEST=3) INTO agegroup.\n\
         VALUE LABELS agegroup 1 '18-29' 2 '30-49' 3 '50+'.\n\
         VARIABLE LEVEL agegroup (ORDINAL).\n\
         CTABLES /TABLE agegroup [COUNT, COLPCT.COUNT] BY gender /CATEGORIES VARIABLES=agegroup TOTAL=YES.",
        "RECODE q2_recommend (0 THRU 6=1) (7, 8=2) (9, 10=3) INTO nps.\n\
         VALUE LABELS nps 1 'Detractor' 2 'Passive' 3 'Promoter'.\n\
         VARIABLE LEVEL nps (NOMINAL).\n\
         CTABLES /TABLE region BY nps [COLPCT.COUNT] /CATEGORIES VARIABLES=region TOTAL=YES.",
        "COMPUTE channels = SUM(q5_1 TO q5_5).\nVARIABLE LEVEL channels (SCALE).\n\
         CTABLES /TABLE region > channels [MEAN F8.2, MEDIAN, MAXIMUM] BY gender.",
        "COMPUTE spend_k = monthly_spend / 1000.\nFORMATS spend_k (F8.3).\nVARIABLE LEVEL spend_k (SCALE).\n\
         CTABLES /TABLE spend_k [MEAN, SUM] BY plan /CATEGORIES VARIABLES=plan TOTAL=YES.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn weighted_tables() {
    weighted(&[
        "CTABLES /TABLE gender BY region.",
        // PSPP leaves a percentage blank in the total of a variable that decides its base, where
        // the manual shows it, so the totals here go with percentages of other bases
        "CTABLES /TABLE gender [COUNT, COLPCT.COUNT, UCOUNT] BY region /CATEGORIES VARIABLES=gender TOTAL=YES.",
        "CTABLES /TABLE gender [COUNT F12.4, COLPCT.COUNT PCT12.4, UCOLPCT.COUNT PCT12.4, ROWPCT.COUNT PCT12.4] BY region.",
        "CTABLES /TABLE age [MEAN F12.6, MEDIAN F12.6, MODE F12.6, STDDEV F12.6, VARIANCE F12.6, SEMEAN F12.6, SUM F12.4, UMEDIAN F12.6, USTDDEV F12.6, USEMEAN F12.6, UMODE F12.4] BY gender.",
        "CTABLES /TABLE monthly_spend [MEAN F12.6, MEDIAN F12.6, VALIDN F12.4, TOTALN F12.4, MISSING F12.4, COUNT F12.4, UCOUNT, UVALIDN, UTOTALN, UMISSING] BY region.",
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT.COUNT, COLPCT.VALIDN, UCOUNT] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction MISSING=INCLUDE TOTAL=YES.",
        // PSPP leaves a count of 0 blank in a total or beside a scale variable, where the manual
        // shows 0, so the third gender, which has empty cells, is left out
        "CTABLES /TABLE gender > region [COUNT, SUBTABLEPCT.COUNT] BY q5_1 /CATEGORIES VARIABLES=region q5_1 TOTAL=YES\n\
         /CATEGORIES VARIABLES=gender [1, 2].",
        "CTABLES /TABLE age + monthly_spend [MEAN, VALIDN] BY gender /CATEGORIES VARIABLES=gender TOTAL=YES.",
        "CTABLES /TABLE plan [COUNT F8.2, TABLEPCT.COUNT PCT8.2] BY q5_3.",
        "CTABLES /TABLE monthly_spend [ROWPCT.SUM, UCOLPCT.SUM, TABLEPCT.SUM] BY gender > q5_1.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn moved_category_labels() {
    // with column variables PSPP puts moved categories inside the summary labels, where the
    // manual's Figure 26 has them outside, so these cases have no column variable or have the
    // summaries in the rows
    survey(&[
        "CTABLES /TABLE (q5_1 + q5_2 + q5_3) [COUNT ROWPCT TABLEPCT] /CLABELS ROWLABELS=OPPOSITE.",
        "CTABLES /TABLE (region > q5_1 + gender > q5_2) [COUNT ROWPCT COLPCT SUBTABLEPCT] /CLABELS ROWLABELS=OPPOSITE.",
        "CTABLES /TABLE gender BY q5_1 + q5_2 [COUNT, COLPCT, ROWPCT] /CLABELS COLLABELS=OPPOSITE.",
        "CTABLES /TABLE region > q5_1 [COUNT, ROWPCT] BY gender /SLABELS POSITION=ROW /CLABELS ROWLABELS=OPPOSITE.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn computed_categories() {
    // PSPP leaves out a computed category beside a scale variable and refuses a range unless the
    // list holds it as written, where the manual computes every summary and totals the listed
    // categories in a range, so those are checked in ctables.rs
    survey(&[
        "CTABLES /PCOMPUTE &x=EXPR([2]-[1]) /PPROPERTIES &x LABEL='Female - Male'\n\
         /TABLE gender /CATEGORIES VARIABLES=gender [1, 2, &x].",
        "CTABLES /PCOMPUTE &x=EXPR([2]/[1]) /PPROPERTIES &x LABEL='Female / Male' FORMAT=COUNT F8.3\n\
         /TABLE gender /CATEGORIES VARIABLES=gender [1, 2, &x].",
        "CTABLES /PCOMPUTE &top=EXPR([4] + [5]) /PPROPERTIES &top LABEL='Top two'\n\
         /TABLE q1_satisfaction [COUNT, COLPCT] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, 3, 4, 5, &top] TOTAL=YES.",
        "CTABLES /PCOMPUTE &share=EXPR([1] / TOTAL * 100) /PPROPERTIES &share LABEL='Share' HIDESOURCECATS=YES\n\
         /TABLE region BY gender /CATEGORIES VARIABLES=region [1, 2, 3, &share] TOTAL=YES.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn first_layer() {
    for syntax in [
        "CTABLES /TABLE gender [COUNT, COLPCT, ROWPCT, SUBTABLEPCT, LAYERPCT, LAYERROWPCT, LAYERCOLPCT, TABLEPCT] BY region BY q5_1.",
        // an explicit list puts the other category first
        "CTABLES /TABLE q5_2 > gender [COUNT, COLPCT, LAYERPCT, LAYERCOLPCT, TABLEPCT] BY region BY q5_1\n\
         /CATEGORIES VARIABLES=q5_1 [1, 0].",
        "CTABLES /TABLE gender [COUNT, COLPCT] BY region BY q5_1 /SLABELS POSITION=LAYER.",
        "CTABLES /TABLE gender [COUNT, COLPCT] BY BY q5_1.",
        // PSPP leaves a count of 0 blank beside a scale variable, where the manual shows 0, so
        // the third gender, which has empty cells, is left out
        "CTABLES /TABLE age [MEAN, COUNT, MEDIAN] BY gender BY q5_1 /CATEGORIES VARIABLES=gender [1, 2].",
    ] {
        check_first_layer("survey_basic.sav", syntax);
    }
    let weighted = "COMPUTE w = RND(weight * 3).\nWEIGHT BY w.\n\
                    CTABLES /TABLE gender [COUNT, COLPCT, LAYERPCT, TABLEPCT] BY region BY q5_1.";
    check_first_layer("survey_basic_phpwriter.sav", weighted);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn rejected_by_both() {
    check_rejected(
        "survey_basic.sav",
        "CTABLES /TABLE gender [MEAN] BY region.",
        "applies only to scale variables",
    );
    check_rejected(
        "survey_basic.sav",
        "CTABLES /TABLE gender [COUNT] BY region [COLPCT.COUNT].",
        "one axis only",
    );
    check_rejected(
        "survey_basic.sav",
        "CTABLES /TABLE gender [COUNT] > region.",
        "innermost",
    );
    check_rejected(
        "survey_basic.sav",
        "CTABLES /TABLE gender [COLPCT.COUNT] BY age.",
        "one axis only",
    );
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn othernm_and_missing_in_a_list() {
    survey(&[
        "CTABLES /TABLE q1_satisfaction BY gender /CATEGORIES VARIABLES=q1_satisfaction [5, OTHERNM, MISSING] TOTAL=YES.",
        "CTABLES /TABLE q1_satisfaction /CATEGORIES VARIABLES=q1_satisfaction [MISSING, 1, 2].",
        // PSPP stops on an internal error for [OTHERNM, 3], so ctables_categories.rs checks it
        "CTABLES /TABLE plan /CATEGORIES VARIABLES=plan ['PLUS', OTHERNM].",
        // PSPP does not sort by a summary (KEY=COUNT), so ctables_categories.rs checks that
        // PSPP refuses [OTHERNM] in a computed category, which the manual allows (PCOMPUTE, page
        // 505), so ctables_categories.rs checks that too
    ]);
}

// Effective base weights (/WEIGHT, ECOUNT) are not compared: PSPP leaves COUNT unweighted and
// shows the weighted count as ECOUNT, where the manual weights every summary by the effective base
// weight and makes ECOUNT the effective base (w^2 / q). ctables.rs and ctables_sigtest.rs check
// them against the raw values.

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn labels_in_the_layers_and_an_empty_row_axis() {
    survey(&[
        "CTABLES /TABLE BY region.",
        "CTABLES /TABLE BY region [COUNT, ROWPCT] /CATEGORIES VARIABLES=region TOTAL=YES.",
        "CTABLES /TABLE BY gender > region [COUNT, ROWPCT].",
    ]);
    for syntax in [
        "CTABLES /TABLE BY BY region.",
        "CTABLES /TABLE gender [COUNT, COLPCT, ROWPCT] BY region /CLABELS ROWLABELS=LAYER.",
        "CTABLES /TABLE gender BY region [COUNT, COLPCT, ROWPCT] /CLABELS COLLABELS=LAYER.",
        "CTABLES /TABLE q5_1 + q5_2 [COUNT, COLPCT] BY gender /CLABELS ROWLABELS=LAYER.",
    ] {
        check_first_layer("survey_basic.sav", syntax);
    }
}

// SMISSING LISTWISE is not compared: PSPP gives the stacked scale variables of
// (a + b) [MEAN] BY c the cases valid on each, where the manual's example (SMISSING, page 503)
// leaves out of both the cases missing on either. ctables.rs checks it against the raw values.

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn subtotals() {
    survey(&[
        "CTABLES /TABLE q1_satisfaction /CATEGORIES VARIABLES=q1_satisfaction [1, 2, SUBTOTAL='Low', 3, 4, 5, SUBTOTAL='High'].",
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, SUBTOTAL='Low', 3, 4, 5, SUBTOTAL='High'] TOTAL=YES.",
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, HSUBTOTAL='Low', 3, 4, 5, HSUBTOTAL='High'] TOTAL=YES.",
        // PSPP leaves a percentage blank in a total or subtotal of a variable that decides its
        // base, where the manual shows it, so these cases take percentages of other bases;
        // ctables.rs checks the rest
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT] BY region\n\
         /CATEGORIES VARIABLES=q1_satisfaction [SUBTOTAL='Low', 1, 2, SUBTOTAL='Rest', 3, 4, 5] TOTAL=YES POSITION=BEFORE.",
        "CTABLES /TABLE gender BY region [COUNT, ROWPCT]\n\
         /CATEGORIES VARIABLES=region [1, 2, SUBTOTAL='North and south', 3, 4, 5, SUBTOTAL].",
        "CTABLES /TABLE region > age [MEAN, COUNT] BY q5_1\n\
         /CATEGORIES VARIABLES=region [1, 2, HSUBTOTAL='A', 3, 4, 5, HSUBTOTAL='B'].",
        "CTABLES /PCOMPUTE &x = EXPR(SUBTOTAL[1] - SUBTOTAL[2]) /PPROPERTIES &x LABEL='Low minus high'\n\
         /TABLE q1_satisfaction BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, HSUBTOTAL='Low', 3, 4, 5, HSUBTOTAL='High', &x].",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn totals_summaries() {
    survey(&[
        "CTABLES /TABLE gender [COUNT, COLPCT TOTALS [COUNT, MEAN F8.4, MEDIAN 'Med' F8.3]] BY region\n\
         /CATEGORIES VARIABLES=gender TOTAL=YES.",
        "CTABLES /TABLE q1_satisfaction [COUNT TOTALS [MEAN F8.5, STDDEV F8.5, VALIDN]]\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, SUBTOTAL='Low', 3, 4, 5, SUBTOTAL='High'] TOTAL=YES.",
        "CTABLES /TABLE region [COUNT TOTALS [COUNT, MINIMUM, MAXIMUM, MODE]] BY gender\n\
         /CATEGORIES VARIABLES=region TOTAL=YES.",
        // PSPP stops on PTILE in TOTALS (an assertion on the percentile), so MEDIAN stands in
        "CTABLES /TABLE gender > q1_satisfaction [COUNT, ROWPCT TOTALS [MEAN F8.4, MEDIAN F8.2]]\n\
         /CATEGORIES VARIABLES=q1_satisfaction TOTAL=YES.",
        "CTABLES /TABLE q5_1 BY q1_satisfaction [COUNT TOTALS [MEAN F8.4, SEMEAN F8.5, VARIANCE F8.5]]\n\
         /CATEGORIES VARIABLES=q1_satisfaction TOTAL=YES /SLABELS POSITION=ROW.",
        // totals given after a parenthesis go to every variable in it
        "CTABLES /TABLE (gender + region) [COUNT TOTALS [SUM F8.0, RANGE F8.0]]\n\
         /CATEGORIES VARIABLES=gender region TOTAL=YES.",
    ]);
    weighted(&[
        "CTABLES /TABLE q1_satisfaction [COUNT, COLPCT TOTALS [COUNT, MEAN F8.4, MEDIAN F8.2]] BY gender\n\
         /CATEGORIES VARIABLES=q1_satisfaction [1, 2, 3, SUBTOTAL='Up to 3', 4, 5] TOTAL=YES.",
    ]);
}

#[test]
#[ignore = "needs STATSCRAFT_PSPP"]
fn item_labels_of_computed_categories() {
    survey(&[
        "CTABLES /PCOMPUTE &x = EXPR([1] + [2])\n\
         /PPROPERTIES &x LABEL=\")LABEL[1] and )LABEL[2], then )LABEL[4]\"\n\
         /TABLE region /CATEGORIES VARIABLES=region [1, 2, SUBTOTAL='Sub', 3 THRU 5, &x].",
        "CTABLES /PCOMPUTE &d = EXPR([2] - [1]) /PPROPERTIES &d LABEL=\")LABEL[2] - )LABEL[1]\"\n\
         /TABLE gender [COUNT, COLPCT] BY q5_1 /CATEGORIES VARIABLES=gender [1, 2, &d] TOTAL=YES.",
    ]);
}
