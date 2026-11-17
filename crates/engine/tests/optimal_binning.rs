//! OPTIMAL BINNING on small invented data. Every cut point and every MDLP decision below was
//! worked out by hand from the published formulas (entropies in bits):
//! Ent(S) = -sum p log2 p, E(T) = |S1|/N Ent(S1) + |S2|/N Ent(S2), gain = Ent(S) - E(T),
//! accept when gain > (log2(N - 1) + delta) / N with
//! delta = log2(3^k - 2) - (k Ent(S) - k1 Ent(S1) - k2 Ent(S2)).

use statscraft_data::Values;
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// x with a string guide g and frequency weights. Rounded weights: 6, 6, 0 (left out), 6, 6,
/// 1, 2, 2, 1, so the classes per value are 1: 6a, 2: 6a, 3: 6b, 4: 6b, 5: 1a 2b, 6: 2a 1b.
const WEIGHTED: &str = "DATA LIST FREE /x (F8.1) g (A1) w (F8.1).
BEGIN DATA.
1 a 6
2 a 5.6
1 b 0.4
3 b 6
4 b 6
5 a 1
5 b 2
6 a 2
6 b 1
END DATA.
WEIGHT BY w.
";

/// Three guide classes: 1: 10 of class 1, 2: 4 of class 2 and 1 of class 3, 3: 20 of class 3.
const THREE: &str = "DATA LIST FREE /x g w.
BEGIN DATA.
1 1 10
2 2 4
2 3 1
3 3 20
END DATA.
WEIGHT BY w.
";

/// x = 1 to 10 once each, z missing in case 10, x user-missing (99) in case 11 and
/// system-missing in case 12.
const PLAIN: &str = "DATA LIST FREE /x z.
BEGIN DATA.
1 1 2 1 3 1 4 1 5 1 6 1 7 1 8 1 9 1 10 . 99 1 . 1
END DATA.
MISSING VALUES x (99).
";

fn run(source: &str) -> (Engine, Vec<Table>) {
    let mut engine = Engine::default();
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
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

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn cell(cell: &Cell) -> Option<f64> {
    match cell {
        Cell::Number(n) => Some(*n),
        Cell::Empty => None,
        other => panic!("not a number: {other:?}"),
    }
}

/// The rows of a table from column `from` on, empty cells as None.
fn numbers(table: &Table, from: usize) -> Vec<Vec<Option<f64>>> {
    table
        .rows
        .iter()
        .map(|r| r[from..].iter().map(cell).collect())
        .collect()
}

/// The inner end points of an End Points table: the upper ends of all but the last bin of
/// each variable, in order.
fn ends(table: &Table) -> Vec<f64> {
    numbers(table, 3).iter().filter_map(|r| r[0]).collect()
}

fn saved(engine: &Engine, name: &str) -> Vec<Option<f64>> {
    let data = engine.dataset().unwrap();
    let Values::Numeric(values) = data.variable(name).unwrap().values() else {
        panic!("{name} is not numeric")
    };
    (0..data.case_count()).map(|c| values.get(c)).collect()
}

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn mdlp_accepts_the_first_cut_and_rejects_the_second() {
    // Whole set: 15a 15b, N = 30, Ent = 1. Candidates after 2 (E = 18/30 Ent(3a,15b)
    // = 0.390013), after 4 (E = 1) and after 5 (E = 0.990939); the cut between 2 and 3 wins.
    // gain = 0.609987; k = 2, k1 = 1, k2 = 2; delta = log2 7 - (2 - 0 - 2 * 0.650022)
    // = 2.107400; threshold = (log2 29 + 2.107400) / 30 = 0.232179: accepted.
    // Below: 1 and 2 are both pure a, no candidate. Above: 3a 15b, N = 18, Ent = 0.650022;
    // candidates after 4 (E = 6/18 Ent(3,3) = 0.333333) and after 5 (0.447515); gain =
    // 0.316689; k1 = 1, k2 = 2; delta = log2 7 - (2 * 0.650022 - 2) = 3.507311; threshold
    // = (log2 17 + 3.507311) / 18 = 0.421932: rejected. One cut, between 2 and 3.
    let (engine, tables) = run(&format!(
        "{WEIGHTED}OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x SAVE=YES /PRINT ENDPOINTS DESCRIPTIVES ENTROPY."
    ));
    let end_points = find(&tables, "End Points");
    assert_eq!(
        end_points.columns,
        ["", "Bin", "Lower", "Upper", "a", "b", "Total"]
    );
    // INCLUSIVE: the end point is the lowest value of the upper bin
    assert_eq!(
        numbers(end_points, 1),
        vec![
            vec![
                Some(1.0),
                None,
                Some(3.0),
                Some(12.0),
                Some(0.0),
                Some(12.0)
            ],
            vec![
                Some(2.0),
                Some(3.0),
                None,
                Some(3.0),
                Some(15.0),
                Some(18.0)
            ],
        ]
    );
    // N, missing, distinct values, minimum, maximum; the weight 0.4 case counts for nothing
    let descriptives = find(&tables, "Descriptive Statistics");
    assert_eq!(
        numbers(descriptives, 1),
        vec![vec![Some(30.0), Some(0.0), Some(6.0), Some(1.0), Some(6.0)]]
    );
    let guide = find(&tables, "Guide Variable Distribution");
    assert_eq!(
        numbers(guide, 1),
        vec![vec![Some(15.0), Some(15.0), Some(30.0)]]
    );
    // model entropy: (12 * 0 + 18 * Ent(3, 15)) / 30
    let entropy = cell(&find(&tables, "Model Entropy").rows[0][1]).unwrap();
    assert!(close(entropy, 0.39001345298901247), "{entropy}");
    // every case with a valid value is binned, also the one of weight 0
    assert_eq!(
        saved(&engine, "g_x_bin"),
        [1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0].map(Some)
    );
    let data = engine.dataset().unwrap();
    let variable = data.variable("g_x_bin").unwrap();
    assert_eq!(variable.label(), Some("x (binned)"));

    // EXCLUSIVE: the end point is the highest value of the lower bin, the same bins
    let (_, tables) = run(&format!(
        "{WEIGHTED}OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x /CRITERIA LOWERLIMIT=EXCLUSIVE."
    ));
    assert_eq!(ends(find(&tables, "End Points")), [2.0]);
}

#[test]
fn rejected_cut_leaves_one_bin() {
    // 1 case of class 1 at x = 1 and 7 of class 2 at x = 2 (weight 6.6 rounded): N = 8,
    // Ent = 1/8 log2 8 + 7/8 log2(8/7) = 0.543564 = gain of the only candidate;
    // delta = log2 7 - 2 * 0.543564 = 1.720226; threshold = (log2 7 + 1.720226) / 8 =
    // 0.565948 > gain: rejected, although the classes separate perfectly.
    let (_, tables) = run("DATA LIST FREE /x g w.
BEGIN DATA.
1 1 1
2 2 6.6
END DATA.
WEIGHT BY w.
OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x.");
    assert_eq!(
        numbers(find(&tables, "End Points"), 1),
        vec![vec![Some(1.0), None, None, Some(1.0), Some(7.0), Some(8.0)]]
    );
}

#[test]
fn forced_merging_joins_by_entropy() {
    // MDLP: S = (10, 4, 21), N = 35, Ent = 1.316199. Candidates after 1 (E = 25/35 *
    // Ent(4, 21) = 0.453078) and after 2 (E = 15/35 Ent(10, 4, 1) = 0.496688); after 1 wins:
    // gain = 0.863121, k = 3, k1 = 1, k2 = 2, delta = log2 25 - (3 * 1.316199 - 2 * 0.634310)
    // = 1.963879, threshold = (log2 34 + 1.963879) / 35 = 0.201467: accepted. Then (0, 4, 21),
    // N = 25: the only candidate after 2 has E = 5/25 Ent(4, 1) = 0.144386, gain = 0.489924,
    // k1 = 2, k2 = 1, delta = log2 7 - (2 * 0.634310 - 2 * 0.721928) = 2.982592, threshold =
    // (log2 24 + 2.982592) / 25 = 0.302702: accepted. Bins of 10, 5 and 20 cases.
    let (_, tables) = run(&format!(
        "{THREE}OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x /PRINT ENDPOINTS ENTROPY."
    ));
    assert_eq!(ends(find(&tables, "End Points")), [2.0, 3.0]);
    let entropy = cell(&find(&tables, "Model Entropy").rows[0][1]).unwrap();
    assert!(close(entropy, 0.1031325849839089), "{entropy}");

    // 5 / min(10, 20) = 0.5 is not below 0.5: no merging
    let (_, tables) = run(&format!(
        "{THREE}OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x /CRITERIA FORCEMERGE=0.5."
    ));
    assert_eq!(ends(find(&tables, "End Points")), [2.0, 3.0]);

    // below 0.6 it merges. With the left bin: 15 Ent(10, 4, 1) + 0 = 17.384; with the right:
    // 0 + 25 Ent(4, 21) = 15.858, lower, so the middle bin joins the larger right neighbour.
    let (_, tables) = run(&format!(
        "{THREE}OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x /CRITERIA FORCEMERGE=0.6 /PRINT ENDPOINTS ENTROPY."
    ));
    assert_eq!(
        numbers(find(&tables, "End Points"), 1),
        vec![
            vec![
                Some(1.0),
                None,
                Some(2.0),
                Some(10.0),
                Some(0.0),
                Some(0.0),
                Some(10.0)
            ],
            vec![
                Some(2.0),
                Some(2.0),
                None,
                Some(0.0),
                Some(4.0),
                Some(21.0),
                Some(25.0)
            ],
        ]
    );
    let entropy = cell(&find(&tables, "Model Entropy").rows[0][1]).unwrap();
    assert!(close(entropy, 0.4530782533146901), "{entropy}");
}

#[test]
fn preprocessing_limits_the_cuts() {
    // x = 1 to 8, weight 5 each, class 1 up to 3 and class 2 from 4.
    let data = "DATA LIST FREE /x g.
BEGIN DATA.
1 1 2 1 3 1 4 2 5 2 6 2 7 2 8 2
END DATA.
COMPUTE w = 5.
WEIGHT BY w.
";
    // Without preprocessing the only candidate lies between 3 and 4: E = 0, gain = Ent(15, 25)
    // = 0.954434, delta = log2 7 - 2 * 0.954434 = 0.898487, threshold = (log2 39 + 0.898487)
    // / 40 = 0.154597: accepted. 8 distinct values are fewer than the default 1000, so the
    // default preprocessing changes nothing.
    for criteria in ["PREPROCESS=NONE", "PREPROCESS=EQUALFREQ"] {
        let (_, tables) = run(&format!(
            "{data}OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x /CRITERIA {criteria}."
        ));
        assert_eq!(ends(find(&tables, "End Points")), [4.0], "{criteria}");
    }
    // Two equal frequency groups: the median at W p = 20, exactly the weight up to 4, is
    // (4 + 5) / 2 = 4.5, so the groups are 1 to 4 (15 + 5 cases) and 5 to 8 (20). MDLP between
    // them: E = 20/40 Ent(15, 5) = 0.405639, gain = 0.954434 - 0.405639 = 0.548795, k1 = 2,
    // k2 = 1, delta = log2 7 - (2 * 0.954434 - 2 * 0.811278) = 2.521043, threshold =
    // (log2 39 + 2.521043) / 40 = 0.195161: accepted, at 5.
    let (_, tables) = run(&format!(
        "{data}OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x /CRITERIA PREPROCESS=EQUALFREQ(BINS=2)."
    ));
    assert_eq!(
        numbers(find(&tables, "End Points"), 1),
        vec![
            vec![
                Some(1.0),
                None,
                Some(5.0),
                Some(15.0),
                Some(5.0),
                Some(20.0)
            ],
            vec![
                Some(2.0),
                Some(5.0),
                None,
                Some(0.0),
                Some(20.0),
                Some(20.0)
            ],
        ]
    );
}

#[test]
fn equal_frequency_with_missing_values() {
    // N = 10, four bins. 25%: W p = 2.5 lies inside the third value, so 3; 50%: W p = 5 is the
    // weight up to 5 exactly, so (5 + 6) / 2 = 5.5; 75%: W p = 7.5, so 8. Bins hold the values
    // up to each: 1-3, 4-5, 6-8, 9-10, with end points 4, 6 and 9.
    let (engine, tables) = run(&format!(
        "{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x z SAVE=YES /CRITERIA METHOD=EQUALFREQ(BINS=4) /PRINT ENDPOINTS DESCRIPTIVES."
    ));
    let end_points = find(&tables, "End Points");
    assert_eq!(end_points.columns, ["", "Bin", "Lower", "Upper", "Total"]);
    // z has one distinct value and so one unbounded bin
    assert_eq!(
        numbers(end_points, 1),
        vec![
            vec![Some(1.0), None, Some(4.0), Some(3.0)],
            vec![Some(2.0), Some(4.0), Some(6.0), Some(2.0)],
            vec![Some(3.0), Some(6.0), Some(9.0), Some(3.0)],
            vec![Some(4.0), Some(9.0), None, Some(2.0)],
            vec![Some(1.0), None, None, Some(11.0)],
        ]
    );
    // pairwise: x misses the user-missing and the system-missing case, z one case
    assert_eq!(
        numbers(find(&tables, "Descriptive Statistics"), 1),
        vec![
            vec![Some(10.0), Some(2.0), Some(10.0), Some(1.0), Some(10.0)],
            vec![Some(11.0), Some(1.0), Some(1.0), Some(1.0), Some(1.0)],
        ]
    );
    assert!(
        tables
            .iter()
            .all(|t| t.title != "Guide Variable Distribution")
    );
    // user-missing becomes system-missing
    let mut want: Vec<Option<f64>> = [1., 1., 1., 2., 2., 3., 3., 3., 4., 4.].map(Some).to_vec();
    want.extend([None, None]);
    assert_eq!(saved(&engine, "x_bin"), want);

    // LISTWISE also drops case 10 (z missing): N = 9; 25%: 2.25, so 3; 50%: 4.5, so 5; 75%:
    // 6.75, so 7. End points 4, 6, 8.
    let (_, tables) = run(&format!(
        "{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x z /CRITERIA METHOD=EQUALFREQ(BINS=4) /MISSING SCOPE=LISTWISE."
    ));
    assert_eq!(ends(find(&tables, "End Points")), [4.0, 6.0, 8.0]);

    // EXCLUSIVE end points are the highest values of the lower bins
    let (_, tables) = run(&format!(
        "{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x /CRITERIA METHOD=EQUALFREQ(BINS=4) LOWERLIMIT=EXCLUSIVE."
    ));
    assert_eq!(ends(find(&tables, "End Points")), [3.0, 5.0, 8.0]);

    // bins of 3, 2, 3 and 2: the second is 2/3 of its smaller neighbour, below 0.7, and joins
    // the lower one (no guide, so equal entropy and equal neighbours); the third is then
    // 3 / min(5, 2) = 1.5, kept.
    let (_, tables) = run(&format!(
        "{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x /CRITERIA METHOD=EQUALFREQ(BINS=4) FORCEMERGE=0.7."
    ));
    assert_eq!(ends(find(&tables, "End Points")), [6.0, 9.0]);

    // at most as many distinct values as bins: one bin per value
    let (_, tables) = run(&format!(
        "{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x /CRITERIA METHOD=EQUALFREQ(BINS=12)."
    ));
    assert_eq!(
        ends(find(&tables, "End Points")),
        [2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]
    );
}

#[test]
fn rules_file_bins_other_data_the_same_way() {
    let dir = std::env::temp_dir().join(format!("statscraft-optbin-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let (engine, tables) = run(&format!(
        "{WEIGHTED}CD '{}'.
OPTIMAL BINNING /VARIABLES GUIDE=g BIN=x SAVE=YES (INTO=xb)
  /CRITERIA LOWEREND=OBSERVED UPPEREND=OBSERVED /OUTFILE RULES='rules.sps'.",
        dir.display()
    ));
    // observed ends: the minimum and maximum, both inside their bins
    assert_eq!(
        numbers(find(&tables, "End Points"), 2)
            .iter()
            .map(|r| (r[0], r[1]))
            .collect::<Vec<_>>(),
        [(Some(1.0), Some(3.0)), (Some(3.0), Some(6.0))]
    );
    assert_eq!(
        saved(&engine, "xb"),
        [1.0, 1.0, 1.0, 2.0, 2.0, 2.0, 2.0, 2.0, 2.0].map(Some)
    );
    let rules = std::fs::read_to_string(dir.join("rules.sps")).unwrap();
    assert_eq!(
        rules,
        "* Bins of x into xb.
COMPUTE xb = $SYSMIS.
IF (NOT MISSING(x) AND x >= 1 AND x < 3) xb = 1.
IF (NOT MISSING(x) AND x >= 3 AND x <= 6) xb = 2.
VARIABLE LABELS xb 'x (binned)'.
VALUE LABELS xb
  1 '>= 1 and < 3'
  2 '>= 3 and <= 6'.
FORMATS xb (F8.0).
VARIABLE LEVEL xb (ORDINAL).
"
    );
    // new values: below the observed minimum, inside, on the cut, at the maximum, above it,
    // user-missing and system-missing
    let (engine, _) = run(&format!(
        "DATA LIST FREE /x.
BEGIN DATA.
0.5 1 2.9 3 6 7 99 .
END DATA.
MISSING VALUES x (99).
{rules}EXECUTE."
    ));
    assert_eq!(
        saved(&engine, "xb"),
        [
            None,
            Some(1.0),
            Some(1.0),
            Some(2.0),
            Some(2.0),
            None,
            None,
            None
        ]
    );
    std::fs::remove_dir_all(&dir).unwrap();
}

#[test]
fn errors_and_warnings() {
    let string = "DATA LIST FREE /x s (A1).\nBEGIN DATA.\n1 a\nEND DATA.\n";
    assert!(fails(&format!("{string}OPTIMAL BINNING /VARIABLES BIN=s.")).contains("string"));
    assert!(
        fails(&format!(
            "{WEIGHTED}OPTIMAL BINNING /VARIABLES GUIDE=x BIN=x."
        ))
        .contains("guide variable x cannot also be binned")
    );
    assert!(fails(&format!("{WEIGHTED}OPTIMAL BINNING /VARIABLES BIN=w.")).contains("weight"));
    assert!(
        fails(&format!(
            "{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x z SAVE=YES (INTO=a)."
        ))
        .contains("1 names for 2")
    );
    assert!(
        fails(&format!(
            "{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x SAVE=YES (INTO=z)."
        ))
        .contains("already exists")
    );
    assert!(fails(&format!("{PLAIN}OPTIMAL BINNING /VARIABLES BIN=x x.")).contains("twice"));
    // 257 guide values
    let many: String = (0..257).map(|i| format!("{i} {i}\n")).collect();
    assert!(
        fails(&format!(
            "DATA LIST FREE /x g.\nBEGIN DATA.\n{many}END DATA.\nOPTIMAL BINNING /VARIABLES GUIDE=g BIN=x."
        ))
        .contains("at most 256")
    );
    // a default name in use gets a free suffix
    let (engine, _) = run(&format!(
        "{PLAIN}COMPUTE x_bin = 0.\nOPTIMAL BINNING /VARIABLES BIN=x SAVE=YES /PRINT NONE."
    ));
    assert_eq!(saved(&engine, "x_bin_1")[0], Some(1.0));
    // ENTROPY without MDLP, and SPLIT FILE
    let (engine, tables) = run(&format!(
        "{PLAIN}SORT CASES BY z.\nSPLIT FILE BY z.\nOPTIMAL BINNING /VARIABLES BIN=x /PRINT ENTROPY."
    ));
    assert!(tables.iter().all(|t| t.title != "Model Entropy"));
    let warnings = engine.warnings().join("\n");
    assert!(warnings.contains("PRINT ENTROPY needs MDLP"), "{warnings}");
    assert!(warnings.contains("does not use SPLIT FILE"), "{warnings}");
}
