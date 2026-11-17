//! OMS, OMSEND, OMSINFO and OMSLOG: which output each request takes, what stays in the
//! results, and the files the requests write, read back value by value.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use statscraft_engine::{Table, cell_text};
use std::path::PathBuf;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-oms-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Invented scores of five people in two groups.
const DATA: &str = "DATA LIST FREE / score grp.\nBEGIN DATA\n1 1 2 1 2 2 3 2 5 1\nEND DATA.\n";

fn run(session: &mut Session, source: &str) -> (Vec<Table>, Vec<String>) {
    let result = session
        .run(source, None)
        .unwrap_or_else(|e| panic!("{source}\n{e:?}"));
    let tables = result.commands.iter().flat_map(|c| c.tables.clone());
    let warnings = result.commands.iter().flat_map(|c| c.warnings.clone());
    (tables.collect(), warnings.collect())
}

fn titles(tables: &[Table]) -> Vec<&str> {
    tables.iter().map(|t| t.title.as_str()).collect()
}

/// Every case of a data file as name=value pairs.
fn cases(path: &std::path::Path) -> Vec<String> {
    let data = Dataset::read_sav(path).unwrap();
    (0..data.case_count())
        .map(|case| {
            let fields: Vec<String> = data
                .variables()
                .map(|v| {
                    let value = match v.values() {
                        Values::Numeric(n) => n.get(case).map_or(".".into(), |x| format!("{x}")),
                        Values::String(s) => s.get(case).trim_end().to_string(),
                    };
                    format!("{}={value}", v.name())
                })
                .collect();
            fields.join(" ")
        })
        .collect()
}

fn path_text(path: &std::path::Path) -> String {
    path.display().to_string()
}

#[test]
fn frequencies_and_descriptives_tables_become_data_files() {
    let dir = folder("sav");
    let (freq, desc) = (dir.join("freq.sav"), dir.join("desc.sav"));
    let mut s = Session::default();
    run(&mut s, DATA);
    let (tables, _) = run(
        &mut s,
        &format!(
            "OMS /SELECT TABLES /IF COMMANDS=['Frequencies'] SUBTYPES=['Frequencies']\n\
             \x20 /DESTINATION FORMAT=SAV OUTFILE='{}'.\n\
             OMS /IF SUBTYPES=['descriptive*'] /DESTINATION FORMAT=SAV OUTFILE='{}' NUMBERED.\n\
             FREQUENCIES grp /STATISTICS=MEAN.\n\
             DESCRIPTIVES score.\n\
             OMSEND.",
            path_text(&freq),
            path_text(&desc)
        ),
    );
    // the results keep everything
    assert_eq!(
        titles(&tables),
        ["Statistics", "grp", "Descriptive Statistics"]
    );
    assert_eq!(
        cases(&freq),
        [
            "Command_=FREQUENCIES Subtype_=Frequencies Label_=grp Var1=valid Var2=1 Var3= Frequency=3 Percent=60 ValidPercent=60 CumulativePercent=60",
            "Command_=FREQUENCIES Subtype_=Frequencies Label_=grp Var1=valid Var2=2 Var3= Frequency=2 Percent=40 ValidPercent=40 CumulativePercent=100",
            "Command_=FREQUENCIES Subtype_=Frequencies Label_=grp Var1=total Var2= Var3= Frequency=5 Percent=100 ValidPercent=. CumulativePercent=.",
        ]
    );
    // score 1 2 2 3 5: squared deviations from 2.6 sum to 9.2, so the SD is sqrt(9.2 / 4)
    let desc_cases = cases(&desc);
    assert_eq!(
        desc_cases,
        [
            "Command_=DESCRIPTIVES Subtype_=Descriptive Statistics Label_=Descriptive Statistics TableNumber_=1 Var1=score Var2= N=5 Mean=2.6 StdDev=1.51657508881031 Minimum=1 Maximum=5",
            "Command_=DESCRIPTIVES Subtype_=Descriptive Statistics Label_=Descriptive Statistics TableNumber_=1 Var1=Valid N (listwise) Var2= N=5 Mean=. StdDev=. Minimum=. Maximum=.",
            "Command_=DESCRIPTIVES Subtype_=Descriptive Statistics Label_=Descriptive Statistics TableNumber_=1 Var1=Missing N (listwise) Var2= N=0 Mean=. StdDev=. Minimum=. Maximum=.",
        ]
    );
    let data = Dataset::read_sav(&desc).unwrap();
    let Values::Numeric(sd) = data.variable("StdDev").unwrap().values() else {
        panic!()
    };
    assert!((sd.get(0).unwrap() - (9.2f64 / 4.0).sqrt()).abs() < 1e-12);
    assert_eq!(data.variable("StdDev").unwrap().label(), Some("Std Dev"));
}

#[test]
fn viewer_no_hides_only_the_chosen_tables() {
    let mut s = Session::default();
    run(&mut s, DATA);
    let (tables, _) = run(
        &mut s,
        "OMS /SELECT TABLES /IF SUBTYPES=['Statistics'] /DESTINATION VIEWER=NO.\n\
         FREQUENCIES grp /STATISTICS=MEAN.\nDESCRIPTIVES score.",
    );
    assert_eq!(titles(&tables), ["grp", "Descriptive Statistics"]);
    // a later request that shows the same tables decides
    let (tables, _) = run(
        &mut s,
        "OMS /IF COMMANDS=['Frequencies'] /DESTINATION VIEWER=YES.\nFREQUENCIES grp /STATISTICS=MEAN.",
    );
    assert_eq!(titles(&tables), ["Statistics", "grp"]);
    // after OMSEND everything shows again
    let (tables, _) = run(
        &mut s,
        "OMSEND.\nOMS /SELECT CHARTS /DESTINATION VIEWER=NO.\nFREQUENCIES grp /STATISTICS=MEAN.",
    );
    assert_eq!(titles(&tables), ["Statistics", "grp"]);
}

#[test]
fn exceptif_instances_and_labels_narrow_the_choice() {
    let dir = folder("narrow");
    let (one, two) = (dir.join("one.sav"), dir.join("two.sav"));
    let mut s = Session::default();
    run(&mut s, DATA);
    run(
        &mut s,
        &format!(
            "OMS /IF COMMANDS=['freq*'] /EXCEPTIF SUBTYPES=['Statistics'] /DESTINATION FORMAT=SAV OUTFILE='{}'.\n\
             OMS /IF INSTANCES=[LAST] LABELS=['s*'] /DESTINATION FORMAT=SAV OUTFILE='{}'.\n\
             FREQUENCIES score grp /STATISTICS=MEAN.\nOMSEND.",
            path_text(&one),
            path_text(&two)
        ),
    );
    let labels = |path: &PathBuf| -> Vec<String> {
        let data = Dataset::read_sav(path).unwrap();
        let Values::String(s) = data.variable("Label_").unwrap().values() else {
            panic!()
        };
        let mut all: Vec<String> = s.iter().map(|t| t.trim_end().to_string()).collect();
        all.dedup();
        all
    };
    // EXCEPTIF leaves out the statistics; the two frequency tables stay
    assert_eq!(labels(&one), ["score", "grp"]);
    // the last of the tables whose label starts with s: the frequency table of score
    assert_eq!(labels(&two), ["score"]);
}

#[test]
fn requests_with_tags_end_one_by_one_and_omsinfo_lists_them() {
    let dir = folder("tags");
    let (a, b) = (dir.join("a.sav"), dir.join("b.txt"));
    let mut s = Session::default();
    run(&mut s, DATA);
    let (tables, _) = run(
        &mut s,
        &format!(
            "OMS /SELECT TABLES /IF COMMANDS=['Descriptives'] /DESTINATION FORMAT=SAV OUTFILE='{}' VIEWER=NO /TAG='first'.\n\
             OMS /SELECT TABLES WARNINGS /DESTINATION FORMAT=TEXT OUTFILE='{}'.\n\
             OMSINFO.",
            path_text(&a),
            path_text(&b)
        ),
    );
    let info = &tables[0];
    assert_eq!(info.title, "OMS Requests");
    let rows: Vec<Vec<String>> = (0..info.rows.len())
        .map(|r| (0..5).map(|c| cell_text(info, r, c)).collect())
        .collect();
    assert_eq!(
        rows,
        [
            vec![
                "first".to_string(),
                "TABLES".into(),
                "SAV".into(),
                path_text(&a),
                "No".into()
            ],
            vec![
                "$Oms2".to_string(),
                "TABLES WARNINGS".into(),
                "TEXT".into(),
                path_text(&b),
                "Yes".into()
            ],
        ]
    );
    // the OMSINFO table itself went to the text file
    assert!(
        std::fs::read_to_string(&b)
            .unwrap()
            .contains("OMS Requests")
    );
    let (_, warnings) = run(
        &mut s,
        "DESCRIPTIVES score.\nOMSEND TAG=['first' 'none'].\nDESCRIPTIVES score.",
    );
    assert_eq!(warnings, ["OMSEND: no OMS request has the tag 'none'"]);
    // the first request took one table (three rows) before it ended
    let first = cases(&a);
    assert_eq!(first.len(), 3);
    assert!(
        first[0].contains("Var1=score Var2= N=5 Mean=2.6"),
        "{first:?}"
    );
    let text = std::fs::read_to_string(&b).unwrap();
    assert_eq!(text.matches("Descriptive Statistics").count(), 2);
    let (tables, _) = run(&mut s, "OMSINFO.");
    assert_eq!(tables[0].rows.len(), 1);
    assert_eq!(cell_text(&tables[0], 0, 0), "$Oms2");
    let (tables, _) = run(&mut s, "OMSEND.\nOMSINFO.");
    assert!(tables[0].rows.is_empty());
    assert_eq!(
        tables[0].caption.as_deref(),
        Some("No OMS request is in force.")
    );
}

#[test]
fn a_declared_dataset_and_an_html_page_take_the_output() {
    let dir = folder("dataset");
    let (page, log) = (dir.join("out.html"), dir.join("log.xml"));
    let mut s = Session::default();
    run(&mut s, DATA);
    let (_, warnings) = run(
        &mut s,
        &format!(
            "DATASET DECLARE stats.\nOMSLOG FILE='{}'.\n\
             OMS /SELECT TABLES /IF SUBTYPES=['Statistics'] /DESTINATION FORMAT=SAV OUTFILE=stats /NOWARN.\n\
             OMS /SELECT TABLES /DESTINATION FORMAT=HTML OUTFILE='{}' IMAGEMAP=YES.\n\
             FREQUENCIES grp /STATISTICS=MEAN.\nOMSEND.\nDATASET ACTIVATE stats.",
            path_text(&log),
            path_text(&page)
        ),
    );
    assert_eq!(warnings[0], "OMS: IMAGEMAP changes nothing in Statscraft");
    let data = s.dataset().unwrap();
    assert_eq!(data.case_count(), 3);
    assert_eq!(data.variable_names().nth(5), Some("grp"));
    let html = std::fs::read_to_string(&page).unwrap();
    assert!(html.contains("<h2>FREQUENCIES</h2>") && html.contains("Statistics"));
    let xml = std::fs::read_to_string(&log).unwrap();
    assert_eq!(xml.matches("<event name=\"start\"").count(), 2);
    assert_eq!(xml.matches("<event name=\"end\"").count(), 2);
    assert!(xml.ends_with("</omsLog>\n"));
}

#[test]
fn a_format_statscraft_does_not_write_warns_and_hides_nothing() {
    let dir = folder("oxml");
    let xml = dir.join("out.xml");
    let mut s = Session::default();
    run(&mut s, DATA);
    let (tables, warnings) = run(
        &mut s,
        &format!(
            "OMS /DESTINATION FORMAT=OXML OUTFILE='{}' VIEWER=NO.\nDESCRIPTIVES score.",
            path_text(&xml)
        ),
    );
    assert_eq!(titles(&tables), ["Descriptive Statistics"]);
    assert!(warnings[0].contains("FORMAT=OXML"), "{warnings:?}");
    assert!(!xml.exists());
}

#[test]
fn columns_pivot_the_file_and_the_log_records_start_and_end() {
    let dir = folder("columns");
    let (data, log) = (dir.join("pivot.sav"), dir.join("log.txt"));
    let mut s = Session::default();
    run(&mut s, DATA);
    run(
        &mut s,
        &format!(
            "OMSLOG FILE='{}' /FORMAT=TEXT.\n\
             OMS /IF SUBTYPES=['Descriptive Statistics'] /DESTINATION FORMAT=SAV OUTFILE='{}'\n\
             \x20 /COLUMNS DIMNAMES=['variable'] /TAG='piv'.\n\
             DESCRIPTIVES score grp.\nOMSEND.",
            path_text(&log),
            path_text(&data)
        ),
    );
    let all = cases(&data);
    // the variables went to the columns, the statistics to the rows (Var2)
    let start =
        "Command_=DESCRIPTIVES Subtype_=Descriptive Statistics Label_=Descriptive Statistics Var1=";
    assert_eq!(
        all,
        [
            format!("{start} Var2=N score=5 grp=5 ValidNlistwise=5 MissingNlistwise=0"),
            format!("{start} Var2=Mean score=2.6 grp=1.4 ValidNlistwise=. MissingNlistwise=."),
            format!(
                "{start} Var2=Std Dev score=1.51657508881031 grp=0.5477225575051661 ValidNlistwise=. MissingNlistwise=."
            ),
            format!("{start} Var2=Minimum score=1 grp=1 ValidNlistwise=. MissingNlistwise=."),
            format!("{start} Var2=Maximum score=5 grp=2 ValidNlistwise=. MissingNlistwise=."),
        ]
    );
    let lines: Vec<Vec<String>> = std::fs::read_to_string(&log)
        .unwrap()
        .lines()
        .map(|l| l.split('\t').skip(1).map(String::from).collect())
        .collect();
    assert_eq!(
        lines,
        [
            vec!["start".to_string(), "piv".into(), path_text(&data)],
            vec!["end".to_string(), "piv".into(), path_text(&data)],
        ]
    );
}
