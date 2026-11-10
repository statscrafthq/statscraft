//! RANK on small inline datasets. The expected values follow the Algorithms manual, worked out
//! by hand; `reference_data.rs` compares the same functions with the reference program.

use statscraft_data::Dataset;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// x: 10 20 20 30 40 50 and 99 (user-missing) and system-missing; g groups; w weights.
const DATA: &str = "DATA LIST LIST /x g w.
BEGIN DATA
10 1 1
20 1 2
20 2 1
30 . 1
40 2 3
50 1 1
99 2 1
. 1 1
END DATA.
MISSING VALUES x (99).
";

fn run(source: &str) -> Result<(Engine, Vec<Table>), String> {
    let (commands, errors) = parse(&format!("{DATA}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
    }
    Ok((engine, tables))
}

fn dataset(source: &str) -> Dataset {
    run(source).unwrap().0.dataset().unwrap().clone()
}

fn column(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    let values = data.variable(name).unwrap().numeric().unwrap();
    (0..data.case_count()).map(|c| values.get(c)).collect()
}

fn same(got: &[Option<f64>], want: &[Option<f64>], what: &str) {
    assert_eq!(got.len(), want.len(), "{what}");
    for (g, w) in got.iter().zip(want) {
        match (g, w) {
            (Some(g), Some(w)) => assert!((g - w).abs() < 1e-9, "{what}: {got:?}, want {want:?}"),
            _ => assert_eq!(g, w, "{what}: {got:?}, want {want:?}"),
        }
    }
}

#[test]
fn every_function_with_default_names() {
    let data =
        dataset("RANK x /RANK /RFRACTION /PERCENT /PROPORTION /N /NTILES(3) /SAVAGE /NORMAL.");
    let names: Vec<&str> = data.variable_names().skip(3).collect();
    // R and N are taken by RANK and N, so RFRACTION, NTILES and NORMAL number theirs
    assert_eq!(
        names,
        [
            "Rx", "RFR001", "Px", "PRO001", "Nx", "NTI001", "Sx", "NOR001"
        ]
    );
    let r = [1.0, 2.5, 2.5, 4.0, 5.0, 6.0];
    let with_missing = |values: Vec<f64>| -> Vec<Option<f64>> {
        let mut v: Vec<Option<f64>> = values.into_iter().map(Some).collect();
        v.extend([None, None]);
        v
    };
    same(&column(&data, "Rx"), &with_missing(r.to_vec()), "rank");
    same(
        &column(&data, "RFR001"),
        &with_missing(r.iter().map(|r| r / 6.0).collect()),
        "rfraction",
    );
    same(
        &column(&data, "Px"),
        &with_missing(r.iter().map(|r| 100.0 * r / 6.0).collect()),
        "percent",
    );
    let blom: Vec<f64> = r.iter().map(|r| (r - 0.375) / 6.25).collect();
    same(
        &column(&data, "PRO001"),
        &with_missing(blom.clone()),
        "proportion",
    );
    same(&column(&data, "Nx"), &with_missing(vec![6.0; 6]), "n");
    same(
        &column(&data, "NTI001"),
        &with_missing(vec![1.0, 2.0, 2.0, 2.0, 3.0, 3.0]),
        "ntiles",
    );
    // l_j = 1/6, 1/6 + 1/5, ...; tied values share the mean of their l
    let l: Vec<f64> = (1..=6)
        .scan(0.0, |sum, j| {
            *sum += 1.0 / (7 - j) as f64;
            Some(*sum)
        })
        .collect();
    let tied = (l[1] + l[2]) / 2.0 - 1.0;
    same(
        &column(&data, "Sx"),
        &with_missing(vec![
            l[0] - 1.0,
            tied,
            tied,
            l[3] - 1.0,
            l[4] - 1.0,
            l[5] - 1.0,
        ]),
        "savage",
    );
    // normal scores: the quantiles of Blom's proportions, symmetric around 0.5 here
    let normal = column(&data, "NOR001");
    assert!((normal[0].unwrap() + 1.281_551_565_544_600_5).abs() < 1e-9);
    assert!((normal[0].unwrap() + normal[5].unwrap()).abs() < 1e-12);
    let x = data.variable("Rx").unwrap();
    assert_eq!(x.label(), Some("RANK of x"));
    assert_eq!(x.format(), "F9.3");
    assert_eq!(
        data.variable("NOR001").unwrap().label(),
        Some("NORMAL of x")
    );
}

#[test]
fn ties_directions_and_groups() {
    let data = dataset(
        "RANK x (D) BY g /RANK INTO low /TIES=LOW /PRINT=NO.\n\
         RANK x /RANK INTO high /TIES=HIGH.\n\
         RANK x /RANK INTO condensed /TIES=CONDENSE.\n\
         RANK x BY g /RANK INTO grouped.",
    );
    // descending within g: g = 1 holds 10 20 50 (and a missing x), g = 2 holds 20 40,
    // g missing holds 30 alone
    same(
        &column(&data, "low"),
        &[
            Some(3.0),
            Some(2.0),
            Some(2.0),
            Some(1.0),
            Some(1.0),
            Some(1.0),
            None,
            None,
        ],
        "low",
    );
    same(
        &column(&data, "high"),
        &[
            Some(1.0),
            Some(3.0),
            Some(3.0),
            Some(4.0),
            Some(5.0),
            Some(6.0),
            None,
            None,
        ],
        "high",
    );
    same(
        &column(&data, "condensed"),
        &[
            Some(1.0),
            Some(2.0),
            Some(2.0),
            Some(3.0),
            Some(4.0),
            Some(5.0),
            None,
            None,
        ],
        "condensed",
    );
    assert_eq!(
        data.variable("grouped").unwrap().label(),
        Some("RANK of x by g")
    );
}

#[test]
fn weights_and_fractions() {
    // weights 1 2 1 1 3 1: W = 9, 20 weighs 3
    let data = dataset(
        "WEIGHT BY w.\nRANK x /RANK INTO r /PROPORTION INTO p /NORMAL INTO z /NTILES(4) INTO q /N INTO n /FRACTION=TUKEY.",
    );
    let r = [1.0, 3.0, 3.0, 5.0, 7.0, 9.0];
    let mut want: Vec<Option<f64>> = r.iter().map(|&r| Some(r)).collect();
    want.extend([None, None]);
    same(&column(&data, "r"), &want, "weighted mean ranks");
    let tukey = |r: f64| (r - 1.0 / 3.0) / (9.0 + 1.0 / 3.0);
    let mut want: Vec<Option<f64>> = r.iter().map(|&r| Some(tukey(r))).collect();
    want.extend([None, None]);
    same(&column(&data, "p"), &want, "tukey");
    let q: Vec<Option<f64>> = column(&data, "q");
    assert_eq!(
        &q[..6],
        [
            Some(1.0),
            Some(2.0),
            Some(2.0),
            Some(3.0),
            Some(3.0),
            Some(4.0)
        ]
    );
    assert_eq!(column(&data, "n")[0], Some(9.0));
    assert_eq!(data.variable("n").unwrap().format(), "F8.2");
    // RANKIT on the smallest rank of one case with a weight below 1 is negative: missing
    let data =
        dataset("COMPUTE w = w / 4.\nWEIGHT BY w.\nRANK x /PROPORTION INTO p /FRACTION=RANKIT.");
    assert_eq!(column(&data, "p")[0], None);
}

#[test]
fn missing_values_and_include() {
    let data = dataset("RANK x /RANK INTO r /MISSING=INCLUDE /PRINT=NO.");
    assert_eq!(column(&data, "r")[6], Some(7.0));
    assert_eq!(column(&data, "r")[7], None);
}

#[test]
fn the_summary_table() {
    let (_, tables) = run("RANK x BY g /NTILES(4) /NORMAL /FRACTION=VW.").unwrap();
    let table = &tables[0];
    assert_eq!(table.title, "Created Variables");
    let text = |c: &Cell| match c {
        Cell::Text(t) => t.clone(),
        _ => String::new(),
    };
    let rows: Vec<Vec<String>> = table
        .rows
        .iter()
        .map(|r| r.iter().map(text).collect())
        .collect();
    assert_eq!(
        rows,
        [
            ["x", "NTILES(4)", "Nx", "NTILES of x by g"],
            ["x", "NORMAL (VW)", "NOR001", "NORMAL of x by g"],
        ]
    );
    assert!(table.caption.as_ref().unwrap().contains("mean rank"));
}

#[test]
fn errors_and_warnings() {
    let error = |source: &str| run(source).err().unwrap();
    assert!(error("RANK x /RANK INTO a b.").contains("INTO gives 2 names for 1"));
    assert!(error("RANK x /RANK INTO g.").contains("already in use"));
    assert!(error("STRING s (A3).\nRANK s.").contains("string variable"));
    let (engine, _) = run("RANK x /RANK INTO r /FRACTION=VW.").unwrap();
    assert!(engine.warnings()[0].contains("FRACTION"));
    let (engine, _) = run("RANK x x /RANK INTO r.").unwrap();
    assert!(engine.warnings()[0].contains("ranked once"));
    // when R plus the name is taken, RAN001 and on
    let data = dataset("RANK x /RANK INTO Rx.\nRANK x.\nRANK x.");
    let names: Vec<&str> = data.variable_names().skip(3).collect();
    assert_eq!(names, ["Rx", "RAN001", "RAN002"]);
    // the default name keeps the first seven bytes of the variable name
    let data = dataset("COMPUTE abcdefghij = x.\nRANK abcdefghij /SAVAGE.");
    assert!(data.variable("Sabcdefg").is_ok());
}
