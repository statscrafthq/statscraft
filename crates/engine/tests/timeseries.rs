//! TSET, TSHOW, CREATE, ACF, PACF, CCF and SEASON on invented data, with the TSET settings
//! that CURVEFIT, WLS and PPLOT read. The expected numbers come from an independent general
//! statistics package (autocorrelations, partial autocorrelations, cross-correlations,
//! Box-Ljung tests, linear filters, differences and running medians) and from the published
//! formulas computed by hand where that package has no equivalent (the medial-average
//! seasonal factors, the 4253H smoother, the missing-value forms).

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Twenty quarters of visitors to a small museum (a rising trend with a strong season) and
/// the number of guided tours offered.
const MUSEUM: &str = "DATA LIST LIST /visitors tours (2F5.0).
BEGIN DATA
112 54
98 61
131 49
140 58
118 57
104 66
139 52
151 60
127 61
110 70
146 55
160 64
133 63
117 74
155 59
171 67
141 66
124 77
163 62
180 71
END DATA.
";

struct Run {
    tables: Vec<Table>,
    warnings: Vec<String>,
    engine: Engine,
}

fn run(source: &str) -> Run {
    let (commands, errors) = parse(&format!("{MUSEUM}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        tables = engine
            .execute(&command.command)
            .unwrap_or_else(|e| panic!("line {}: {e}", command.first_line));
    }
    let warnings = engine.warnings().to_vec();
    Run {
        tables,
        warnings,
        engine,
    }
}

fn error(source: &str) -> String {
    let (commands, errors) = parse(&format!("{MUSEUM}{source}"));
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(e) = engine.execute(&command.command) {
            return e.to_string();
        }
    }
    panic!("no error")
}

fn find<'a>(tables: &'a [Table], title: &str) -> &'a Table {
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no table {title}: {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn num(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(v) => *v,
        other => panic!("not a number: {other:?}"),
    }
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= 1e-8 * expected.abs().max(1.0),
        "{actual} is not {expected}"
    );
}

/// A column of a table as numbers.
fn column(table: &Table, c: usize) -> Vec<f64> {
    table.rows.iter().map(|r| num(&r[c])).collect()
}

fn values(run: &Run, name: &str) -> Vec<Option<f64>> {
    let data = run.engine.dataset().unwrap();
    let v = data.variable(name).unwrap();
    let n = v.numeric().unwrap();
    (0..data.case_count()).map(|k| n.get(k)).collect()
}

fn same(actual: &[Option<f64>], expected: &[Option<f64>]) {
    assert_eq!(actual.len(), expected.len());
    for (a, e) in actual.iter().zip(expected) {
        match (a, e) {
            (Some(a), Some(e)) => close(*a, *e),
            _ => assert_eq!(a, e),
        }
    }
}

const ACF: [f64; 8] = [
    0.292114332049463,
    -0.371072369754713,
    0.173829312791405,
    0.698763429961484,
    0.124062436651125,
    -0.423575917291709,
    -0.000506790999392,
    0.414250962902899,
];

#[test]
fn acf_with_white_noise_errors_and_box_ljung() {
    let r = run("ACF visitors /MXAUTO=8 /PACF.");
    let t = find(&r.tables, "Autocorrelations");
    assert_eq!(column(t, 0), (1..=8).map(f64::from).collect::<Vec<_>>());
    for (a, e) in column(t, 1).iter().zip(ACF) {
        close(*a, e);
    }
    let se = [
        0.207802353648,
        0.202259958739,
        0.196561348277,
        0.190692517849,
        0.184637236469,
        0.178376517003,
        0.171887912738,
        0.165144564769,
    ];
    for (a, e) in column(t, 2).iter().zip(se) {
        assert!((a - e).abs() < 1e-11);
    }
    let q = [
        1.976081290265,
        5.3419518225962,
        6.124029304575,
        19.5514634085,
        20.002947062001,
        25.6417531614,
        25.6417618544,
        31.9339033975,
    ];
    let sig = [
        0.159803844097,
        0.0691846742475,
        0.105729405653,
        6.12210020864e-04,
        0.00124814016325,
        2.59580995873e-04,
        5.83533109686e-04,
        9.57149046702e-05,
    ];
    for k in 0..8 {
        assert!((num(&t.rows[k][3]) - q[k]).abs() < 1e-9);
        assert_eq!(num(&t.rows[k][4]), k as f64 + 1.0);
        assert!((num(&t.rows[k][5]) - sig[k]).abs() < 1e-9 * sig[k].max(1e-3));
    }
    let p = find(&r.tables, "Partial Autocorrelations");
    let pacf = [
        0.2921143320495,
        -0.4989816474143,
        0.7017749354511,
        0.1652040913681,
        -0.1909194686560,
        -0.0891847377956,
        -0.1356226057589,
        -0.0759885927976,
    ];
    for (a, e) in column(p, 1).iter().zip(pacf) {
        assert!((a - e).abs() < 1e-11);
    }
    close(num(&p.rows[0][2]), 1.0 / 20f64.sqrt());
    // one chart for the autocorrelations, one for the partial ones, limits at 2 SE
    let charts = r.engine.clone().take_charts();
    assert_eq!(charts.len(), 2);
    assert!((charts[0].series[1].values[0] - 2.0 * se[0]).abs() < 1e-11);
}

#[test]
fn bartlett_errors_from_serror_and_from_tset() {
    let bartlett = [
        0.223606797750,
        0.241936103752,
        0.268891332435,
        0.274452567226,
        0.352351024920,
    ];
    for source in [
        "ACF visitors /MXAUTO=5 /SERROR=MA.",
        "TSET ACFSE=MA MXAUTO=5.\nACF visitors.",
    ] {
        let r = run(source);
        let t = find(&r.tables, "Autocorrelations");
        assert_eq!(t.rows.len(), 5);
        for (a, e) in column(t, 2).iter().zip(bartlett) {
            assert!((a - e).abs() < 1e-11, "{source}");
        }
    }
    // DEFAULT puts both back
    let r = run("TSET ACFSE=MA MXAUTO=5.\nTSET DEFAULT.\nACF visitors.");
    let t = find(&r.tables, "Autocorrelations");
    assert_eq!(t.rows.len(), 16);
    assert!((num(&t.rows[0][2]) - 0.207802353648).abs() < 1e-11);
}

#[test]
fn acf_with_a_missing_value() {
    let r = run("MISSING VALUES visitors (139).\nACF visitors /MXAUTO=5.");
    let t = find(&r.tables, "Autocorrelations");
    let rk = [
        0.297468162327,
        -0.362978422272,
        0.181114865188,
        0.697756397465,
        0.128315505061,
    ];
    let se = [
        0.206413459568,
        0.200250469729,
        0.193891683582,
        0.187317162316,
        0.180503334136,
    ];
    let q = [
        2.07684916068,
        5.36244795313,
        6.23499696395,
        20.11062068477,
        20.61596550529,
    ];
    for k in 0..5 {
        assert!((num(&t.rows[k][1]) - rk[k]).abs() < 1e-11);
        assert!((num(&t.rows[k][2]) - se[k]).abs() < 1e-11);
        assert!((num(&t.rows[k][3]) - q[k]).abs() < 1e-9);
    }
    let cases = find(&r.tables, "Case Processing Summary");
    assert_eq!(num(&cases.rows[1][1]), 1.0);
    assert_eq!(num(&cases.rows[3][1]), 19.0);
    // TSET MISSING=INCLUDE counts the user-missing value as valid again
    let r = run("MISSING VALUES visitors (139).\nTSET MISSING=INCLUDE.\nACF visitors /MXAUTO=5.");
    let t = find(&r.tables, "Autocorrelations");
    close(num(&t.rows[0][1]), ACF[0]);
}

#[test]
fn ccf_lags_both_ways() {
    let r = run("CCF visitors WITH tours /MXCROSS=4.");
    let t = find(&r.tables, "Cross Correlations");
    assert_eq!(column(t, 0), vec![-4., -3., -2., -1., 0., 1., 2., 3., 4.]);
    let expected = [
        -0.1142044850786,
        0.3366643160852,
        0.5896609517747,
        0.2467626973242,
        0.0262648164631,
        0.4702984364750,
        0.5073539787380,
        0.1498993199827,
        -0.1306279064573,
    ];
    for (a, e) in column(t, 1).iter().zip(expected) {
        assert!((a - e).abs() < 1e-11);
    }
    close(num(&t.rows[0][2]), 1.0 / 16f64.sqrt());
    close(num(&t.rows[4][2]), 1.0 / 20f64.sqrt());
    // without WITH the pairs are the same
    let r2 = run("TSET MXCROSS=4.\nCCF visitors tours.");
    assert_eq!(find(&r2.tables, "Cross Correlations").rows, t.rows);
}

#[test]
fn periods_from_date_and_tset() {
    // DATE gives period 4: SEASONAL shows lags 4 and 8
    let r = run("DATE YEAR 2001 QUARTER 1.\nACF visitors /MXAUTO=8 /SEASONAL.");
    let t = find(&r.tables, "Autocorrelations");
    assert_eq!(column(t, 0), vec![4.0, 8.0]);
    close(num(&t.rows[0][1]), ACF[3]);
    // TSET PERIOD wins over DATE
    let r = run("DATE YEAR 2001 QUARTER 1.\nTSET PERIOD=2.\nACF visitors /MXAUTO=4 /SEASONAL.");
    assert_eq!(
        column(find(&r.tables, "Autocorrelations"), 0),
        vec![2.0, 4.0]
    );
    // no period anywhere: SEASONAL is left out with a warning
    let r = run("ACF visitors /MXAUTO=3 /SEASONAL.");
    assert_eq!(find(&r.tables, "Autocorrelations").rows.len(), 3);
    assert!(r.warnings.iter().any(|w| w.contains("no period")));
    // a monthly DATE stepped by 3 is quarterly
    let r = run("DATE YEAR 2001 MONTH 1 BY 3.\nCREATE s = SDIFF(visitors, 1).");
    assert_eq!(values(&r, "s")[4], Some(6.0));
}

#[test]
fn acf_after_log_and_differences_and_apply() {
    let r = run("CREATE d = DIFF(visitors, 1).\nACF d /MXAUTO=3.\nACF visitors /DIFF /MXAUTO=3.");
    let first = run("CREATE d = DIFF(visitors, 1).\nACF d /MXAUTO=3.");
    assert_eq!(
        find(&r.tables, "Autocorrelations").rows,
        find(&first.tables, "Autocorrelations").rows
    );
    // APPLY repeats the previous ACF on another series
    let r = run("ACF visitors /DIFF=1 /MXAUTO=3.\nCOMPUTE v2 = visitors.\nACF v2 /APPLY.");
    assert_eq!(
        find(&r.tables, "Autocorrelations").rows,
        find(&first.tables, "Autocorrelations").rows
    );
    // LN of a series with a value not above 0 leaves it out
    let r = run("COMPUTE z = visitors - 100.\nACF visitors z /LN /MXAUTO=2.");
    assert_eq!(
        r.tables
            .iter()
            .filter(|t| t.title == "Autocorrelations")
            .count(),
        1
    );
    assert!(r.warnings.iter().any(|w| w.contains("cannot be logged")));
    assert!(error("ACF APPLY.").contains("no earlier ACF"));
}

fn some(v: &[f64]) -> Vec<Option<f64>> {
    v.iter().map(|x| Some(*x)).collect()
}

#[test]
fn create_functions() {
    let r = run(
        "CREATE total = CSUM(visitors) /d1 = DIFF(visitors 1) /s1 = SDIFF(visitors, 1, 4)
  /lag1 TO lag2 = LAG(visitors, 1, 2) /next = LEAD(visitors, 1)
  /ma4 = MA(visitors, 4) /ma3 = MA(visitors, 3, 1) /rm = RMED(visitors, 3) /prior = PMA(visitors, 3)
  /smooth = T4253H(visitors).",
    );
    let x = values(&r, "visitors");
    let total: Vec<f64> = vec![
        112., 210., 341., 481., 599., 703., 842., 993., 1120., 1230., 1376., 1536., 1669., 1786.,
        1941., 2112., 2253., 2377., 2540., 2720.,
    ];
    same(&values(&r, "total"), &some(&total));
    let d1 = values(&r, "d1");
    assert_eq!(d1[0], None);
    assert_eq!(d1[1], Some(-14.0));
    let s1 = values(&r, "s1");
    assert_eq!(&s1[..5], &[None, None, None, None, Some(6.0)]);
    assert_eq!(s1[19], Some(9.0));
    assert_eq!(values(&r, "lag2")[2], x[0]);
    assert_eq!(values(&r, "lag1")[0], None);
    assert_eq!(values(&r, "next")[0], x[1]);
    assert_eq!(values(&r, "next")[19], None);
    let ma4 = values(&r, "ma4");
    assert_eq!(&ma4[..2], &[None, None]);
    close(ma4[2].unwrap(), 121.0);
    close(ma4[17].unwrap(), 150.875);
    assert_eq!(ma4[18], None);
    let ma3 = values(&r, "ma3");
    assert_eq!(ma3[0], Some(112.0));
    close(ma3[1].unwrap(), 113.666666666667);
    assert_eq!(ma3[19], Some(180.0));
    let rm = values(&r, "rm");
    assert_eq!(rm[0], None);
    assert_eq!(rm[1], Some(112.0));
    assert_eq!(rm[18], Some(163.0));
    let prior = values(&r, "prior");
    assert_eq!(prior[2], None);
    close(prior[3].unwrap(), 113.666666666667);
    let smooth = [
        112.0,
        115.45703125,
        121.18359375,
        124.953125,
        126.80859375,
        128.81640625,
        130.9375,
        133.015625,
        134.875,
        136.5,
        137.9375,
        139.5625,
        141.65625,
        143.90625,
        146.0,
        147.3828125,
        148.1484375,
        151.46875,
        161.40625,
        177.53125,
    ];
    same(&values(&r, "smooth"), &some(&smooth));
    let created = find(&r.tables, "Created Series");
    assert_eq!(created.rows[0][4], Cell::Text("CSUM(visitors)".into()));
    assert_eq!(num(&created.rows[2][1]), 5.0);
    assert_eq!(num(&created.rows[2][3]), 16.0);
}

#[test]
fn create_fourier_round_trip_and_replacement() {
    let r = run("CREATE re im = FFT(visitors) /back = IFFT(re im) /visitors = DIFF(visitors, 1).");
    let re = values(&r, "re");
    close(re[0].unwrap(), 2720.0 / 20.0);
    assert!(re[9].is_some());
    assert_eq!(re[10], None);
    let back = values(&r, "back");
    let original = [
        112., 98., 131., 140., 118., 104., 139., 151., 127., 110., 146., 160., 133., 117., 155.,
        171., 141., 124., 163., 180.,
    ];
    for (a, e) in back.iter().zip(original) {
        assert!((a.unwrap() - e).abs() < 1e-8);
    }
    // the last equation replaced the series it read
    assert_eq!(values(&r, "visitors")[1], Some(-14.0));
    // an odd length is padded with a warning; a constant series has no autocorrelations
    let r =
        run("SELECT IF $CASENUM < 20.\nCREATE a b = FFT(visitors).\nCOMPUTE flat = 3.\nACF flat.");
    assert!(r.warnings.iter().any(|w| w.contains("does not vary")));
    let r = run("SELECT IF $CASENUM < 20.\nCREATE a b = FFT(visitors).");
    assert!(r.warnings.iter().any(|w| w.contains("odd number")));
}

#[test]
fn create_under_split_file_and_errors() {
    let r = run("COMPUTE half = $CASENUM > 10.\nSPLIT FILE BY half.\nCREATE l = LAG(visitors, 1).");
    let l = values(&r, "l");
    assert_eq!(l[0], None);
    assert_eq!(l[10], None);
    assert_eq!(l[11], Some(146.0));
    assert!(error("CREATE a b = LAG(visitors, 1).").contains("makes 1 series"));
    assert!(error("CREATE a = SDIFF(visitors, 1).").contains("needs a period"));
}

#[test]
fn season_multiplicative_and_additive() {
    let r = run("SEASON visitors /PERIOD=4.");
    let f = find(&r.tables, "Seasonal Factors");
    let factors = [94.9707580268, 82.1071911592, 107.335563383, 115.586487431];
    for (a, e) in column(f, 1).iter().zip(factors) {
        assert!((a - e).abs() < 1e-8);
    }
    let d = find(&r.tables, "Seasonal Decomposition");
    close(num(&d.rows[2][2]), 121.0);
    // saved under TSET NEWVAR=CURRENT: factors as ratios, SAS = series / SAF
    let saf = values(&r, "SAF_1");
    assert!((saf[0].unwrap() - 0.949707580268).abs() < 1e-10);
    let sas = values(&r, "SAS_1");
    assert!((sas[9].unwrap() - 133.971213053).abs() < 1e-8);
    let stc = values(&r, "STC_1");
    assert!((stc[0].unwrap() - 119.151822285).abs() < 1e-8);
    assert!((stc[19].unwrap() - 153.894425157).abs() < 1e-8);
    let err = values(&r, "ERR_1");
    assert!((err[19].unwrap() - 1.01191154591).abs() < 1e-10);
    // a second SEASON replaces them; BRIEF leaves out the casewise table
    let r = run(
        "TSET PRINT=BRIEF PERIOD=4.\nSEASON visitors.\nSEASON visitors /MODEL=ADDITIVE /MA=EQUAL.",
    );
    assert!(r.tables.iter().all(|t| t.title != "Seasonal Decomposition"));
    let f = find(&r.tables, "Seasonal Factors");
    let additive = [-6.41666666667, -24.6666666667, 10.125, 20.9583333333];
    for (a, e) in column(f, 1).iter().zip(additive) {
        assert!((a - e).abs() < 1e-8);
    }
    assert!(r.engine.dataset().unwrap().variable("SAF_2").is_err());
    assert!((values(&r, "SAF_1")[1].unwrap() + 24.6666666667).abs() < 1e-8);
    // ALL keeps the earlier ones; NONE saves nothing
    let r = run("TSET NEWVAR=ALL PERIOD=4.\nSEASON visitors.\nSEASON visitors.");
    assert!(r.engine.dataset().unwrap().variable("SAF_2").is_ok());
    let r = run("TSET NEWVAR=NONE PERIOD=4.\nSEASON visitors.");
    assert!(r.engine.dataset().unwrap().variable("SAF_1").is_err());
    assert!(error("SEASON visitors.").contains("needs a period"));
    assert!(error("SEASON visitors /PERIOD=6.").contains("four full periods"));
}

#[test]
fn tset_reaches_curvefit_wls_and_pplot() {
    // NEWVAR=CURRENT (the default) saves all four variables, numbered from 1, replacing
    let r = run("CURVEFIT visitors.\nCURVEFIT visitors /MODEL=QUADRATIC.");
    let data = r.engine.dataset().unwrap();
    assert!(data.variable("UCL_1").is_ok() && data.variable("FIT_2").is_err());
    let label = data.variable("FIT_1").unwrap().label().unwrap().to_string();
    assert!(label.contains("Quadratic"), "{label}");
    // CIN from TSET
    let wide = run("TSET NEWVAR=NONE.\nCURVEFIT visitors /SAVE=CIN.");
    let narrow = run("TSET NEWVAR=NONE CIN=80.\nCURVEFIT visitors /SAVE=CIN.");
    let (w, n) = (values(&wide, "UCL_1")[0], values(&narrow, "UCL_1")[0]);
    assert!(w.unwrap() > n.unwrap());
    let r = run("TSET NEWVAR=NONE.\nCURVEFIT visitors.");
    assert!(r.engine.dataset().unwrap().variable("FIT_1").is_err());
    // NOCONSTANT from TSET
    let r = run("TSET NOCONSTANT NEWVAR=NONE.\nCURVEFIT visitors.");
    let estimates = &r.tables[1];
    assert!(estimates.rows[0].iter().any(|c| matches!(c, Cell::Empty)));
    // WLS saves WGT_1 by default
    let r = run("WLS visitors WITH tours.");
    assert!(r.engine.dataset().unwrap().variable("WGT_1").is_ok());
    // MXNEWVARS caps a procedure's new variables
    let r = run("TSET MXNEWVARS=3.\nCURVEFIT visitors.");
    assert!(r.warnings.iter().any(|w| w.contains("MXNEWVARS")));
    // PPLOT takes the period of TSET for SDIFF
    let r = run("TSET PERIOD=4.\nPPLOT visitors /SDIFF.");
    assert!(r.warnings.iter().all(|w| !w.contains("SDIFF")));
}

#[test]
fn tshow_lists_the_settings() {
    let r = run("DATE YEAR 2001 QUARTER 1.\nTSET MXAUTO 12 /PRINT DETAILED.\nTSHOW.");
    let t = find(&r.tables, "Time Series Settings");
    let get = |name: &str| {
        t.rows
            .iter()
            .find(|row| row[0] == Cell::Text(name.into()))
            .unwrap()[1]
            .clone()
    };
    assert_eq!(get("MXAUTO"), Cell::Number(12.0));
    assert_eq!(get("PRINT"), Cell::Text("DETAILED".into()));
    assert_eq!(get("PERIOD"), Cell::Text("None".into()));
    assert_eq!(get("Period of the DATE variables"), Cell::Number(4.0));
    assert_eq!(get("Period in use"), Cell::Number(4.0));
}
