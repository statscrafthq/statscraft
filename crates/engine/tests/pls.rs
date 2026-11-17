//! PLS on invented data. The expected numbers come from an independent NIPALS written in base
//! R for this purpose (no PLS package): frequency weights by repeating rows, the latent factors
//! of several dependents by iterating NIPALS to convergence instead of an eigenproblem, the
//! sign of a factor chosen so its Y weights add up to a positive number, and the tables and
//! casewise values written out from the formulas of the Algorithms manual.

use statscraft_data::{Dataset, Values};
use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

fn run(engine: &mut Engine, source: &str) -> Vec<Table> {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    tables
}

fn run_error(source: &str) -> String {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    for command in &commands {
        if let Err(error) = engine.execute(&command.command) {
            return error.0;
        }
    }
    panic!("no error");
}

fn find<'t>(tables: &'t [Table], title: &str) -> &'t Table {
    tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"))
}

fn cell(table: &Table, row: &str, column: &str) -> f64 {
    let r = table
        .rows
        .iter()
        .position(|r| r[0] == Cell::Text(row.into()))
        .unwrap_or_else(|| panic!("{}: no row {row}", table.title));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| {
            panic!(
                "{}: no column {column} in {:?}",
                table.title,
                table.column_titles()
            )
        });
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{}: {row} / {column} is {other:?}", table.title),
    }
}

fn close(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-9 * expected.abs().max(1.0),
        "{what}: {actual} against {expected}"
    );
}

fn numbers(data: &Dataset, name: &str) -> Vec<Option<f64>> {
    match data.variable(name).unwrap().values() {
        Values::Numeric(v) => (0..data.case_count()).map(|k| v.get(k)).collect(),
        Values::String(_) => panic!("{name} is a string"),
    }
}

fn texts(data: &Dataset, name: &str) -> Vec<String> {
    match data.variable(name).unwrap().values() {
        Values::String(v) => (0..data.case_count())
            .map(|k| v.get(k).trim_end().to_string())
            .collect(),
        Values::Numeric(_) => panic!("{name} is numeric"),
    }
}

fn named<'e>(engine: &'e Engine, name: &str) -> &'e Dataset {
    let (sets, _) = engine.named_datasets();
    sets.iter()
        .find(|(n, _)| n == name)
        .and_then(|(_, d)| d.as_ref())
        .unwrap_or_else(|| panic!("no dataset {name}"))
}

/// The results of one model as the reference program printed them: w, p, c, q, VIP, t and u
/// by latent factor, b and the predictions by dependent column.
struct Expected {
    w: &'static [f64],
    p: &'static [f64],
    c: &'static [f64],
    q: &'static [f64],
    b: &'static [f64],
    constant: &'static [f64],
    x_variance: &'static [f64],
    y_variance: &'static [f64],
    x_cumulative: &'static [f64],
    r_square: &'static [f64],
    adjusted: &'static [f64],
    vip: &'static [f64],
    t: &'static [f64],
    u: &'static [f64],
    dmodx: &'static [f64],
    predicted: &'static [f64],
}

fn check(tables: &[Table], e: &Expected, xs: &[&str], ys: &[&str]) {
    let (n, m) = (xs.len(), ys.len());
    let d = e.x_variance.len();
    let variance = find(tables, "Proportion of Variance Explained");
    assert_eq!(variance.rows.len(), d);
    for k in 0..d {
        let row = (k + 1).to_string();
        let at = |column| cell(variance, &row, column);
        close(at("X Variance"), e.x_variance[k], "X variance");
        close(
            at("Cumulative X Variance"),
            e.x_cumulative[k],
            "cumulative X",
        );
        close(at("Y Variance"), e.y_variance[k], "Y variance");
        close(
            at("Cumulative Y Variance (R-square)"),
            e.r_square[k],
            "R-square",
        );
        close(at("Adjusted R-square"), e.adjusted[k], "adjusted R-square");
    }
    let parameters = find(tables, "Parameters");
    for (r, y) in ys.iter().enumerate() {
        let column = format!("Dependent Variables / {y}");
        close(
            cell(parameters, "(Constant)", &column),
            e.constant[r],
            "constant",
        );
        for (j, x) in xs.iter().enumerate() {
            close(
                cell(parameters, x, &column),
                e.b[r * n + j],
                &format!("B {x} {y}"),
            );
        }
    }
    let (vip, weights, loadings) = (
        find(tables, "Variable Importance in the Projection"),
        find(tables, "Weights"),
        find(tables, "Loadings"),
    );
    for k in 0..d {
        let column = format!("Latent Factors / {}", k + 1);
        for (j, x) in xs.iter().enumerate() {
            close(
                cell(vip, x, &column),
                e.vip[k * n + j],
                &format!("VIP {x} {k}"),
            );
            close(
                cell(weights, x, &column),
                e.w[k * n + j],
                &format!("w {x} {k}"),
            );
            close(
                cell(loadings, x, &column),
                e.p[k * n + j],
                &format!("p {x} {k}"),
            );
        }
        for (r, y) in ys.iter().enumerate() {
            close(
                cell(weights, y, &column),
                e.c[k * m + r],
                &format!("c {y} {k}"),
            );
            close(
                cell(loadings, y, &column),
                e.q[k * m + r],
                &format!("q {y} {k}"),
            );
        }
    }
}

/// Fifteen invented cases of three predictors, a three-level group, two scale outcomes, a
/// frequency weight (the last one rounds to 0) and a three-category outcome, then a case
/// without outcomes and a case with a user-missing x3.
const DATA: &str = "DATA LIST FREE /x1 x2 x3 g y1 y2 wt cy.
BEGIN DATA.
2.1 10 0.5 1 5.2 3.3 1 1   3.4 14 0.2 2 7.9 2.1 2 2   1.8 9 0.9 3 4.1 4.0 1 1
4.6 17 0.4 1 9.8 2.9 3 3   5.2 21 0.1 2 11.5 1.6 1 3  3.9 15 0.7 3 8.0 3.8 1 2
2.7 12 0.6 1 6.3 3.1 2 1   6.1 22 0.3 2 12.9 1.9 1 3  4.4 16 0.8 3 9.1 3.6 1 2
3.1 13 0.2 1 6.8 2.4 2 1   5.8 20 0.5 2 12.2 2.2 1 3  2.2 11 0.9 3 4.9 4.4 1 1
4.9 18 0.3 1 10.4 2.0 2 3  3.6 15 0.6 2 7.7 3.0 1 2   4.0 16 0.4 3 8.8 2.7 0.4 2
3.0 13 0.5 1 . . 1 .       4.2 17 99 2 9.0 2.5 1 1
END DATA.
FORMATS g cy (F1).
MISSING VALUES x3 (99).
";

const XS: [&str; 3] = ["x1", "x2", "x3"];

#[test]
fn pls_one_dependent_tables_and_case_results() {
    let mut engine = Engine::default();
    let source = format!("{DATA}PLS y1 WITH x1 x2 x3 /OUTDATASET CASES=scores.");
    let tables = run(&mut engine, &source);
    check(&tables, &A, &XS, &["y1"]);
    // five factors asked for, three possible with three predictors
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("extracted 3 latent factors"))
    );
    let scores = named(&engine, "scores");
    // the fifteen cases of the fit and the one without outcomes; not the one missing x3
    assert_eq!(scores.case_count(), 16);
    let case = numbers(scores, "CaseNumber");
    assert_eq!(case[15], Some(16.0));
    let predicted = numbers(scores, "Predicted_1");
    let residual = numbers(scores, "Residual_1");
    let dmodx = numbers(scores, "DModX");
    let y1 = [
        5.2, 7.9, 4.1, 9.8, 11.5, 8.0, 6.3, 12.9, 9.1, 6.8, 12.2, 4.9, 10.4, 7.7, 8.8,
    ];
    for i in 0..15 {
        close(predicted[i].unwrap(), A.predicted[i], "predicted");
        close(residual[i].unwrap(), y1[i] - A.predicted[i], "residual");
        // three factors of three predictors leave no X residual
        assert!(dmodx[i].unwrap().abs() < 1e-7);
        for k in 0..3 {
            let t = numbers(scores, &format!("XScore_{}", k + 1));
            let u = numbers(scores, &format!("YScore_{}", k + 1));
            close(t[i].unwrap(), A.t[k * 15 + i], "X score");
            close(u[i].unwrap(), A.u[k * 15 + i], "Y score");
        }
    }
    // the case without outcomes has a prediction but no residual and no Y scores
    let expected = A.constant[0] + A.b[0] * 3.0 + A.b[1] * 13.0 + A.b[2] * 0.5;
    close(
        predicted[15].unwrap(),
        expected,
        "predicted without outcome",
    );
    assert_eq!(residual[15], None);
    assert_eq!(numbers(scores, "YScore_1")[15], None);
}

#[test]
fn pls_two_dependents() {
    let mut engine = Engine::default();
    let source = format!("{DATA}PLS y1 y2 WITH x1 TO x3 /CRITERIA LATENTFACTORS=3.");
    let tables = run(&mut engine, &source);
    check(&tables, &B, &XS, &["y1", "y2"]);
    assert!(engine.warnings().is_empty(), "{:?}", engine.warnings());
}

#[test]
fn pls_factor_predictor_with_weights_and_saved_datasets() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}WEIGHT BY wt.
PLS y1 y2 BY g WITH x1 x3 /CRITERIA LATENTFACTORS=3
  /OUTDATASET CASES=scores LATENTFACTORS=factors PREDICTORS=coefficients."
    );
    let tables = run(&mut engine, &source);
    let xs = ["[g=1]", "[g=2]", "[g=3]", "x1", "x3"];
    check(&tables, &C, &xs, &["y1", "y2"]);
    let scores = named(&engine, "scores");
    assert_eq!(scores.case_count(), 16);
    let dmodx = numbers(scores, "DModX");
    let predicted = numbers(scores, "Predicted_2");
    let t2 = numbers(scores, "XScore_2");
    for i in 0..15 {
        close(dmodx[i].unwrap(), C.dmodx[i], "DModX");
        close(predicted[i].unwrap(), C.predicted[15 + i], "predicted y2");
        close(t2[i].unwrap(), C.t[15 + i], "X score 2");
    }
    let factors = named(&engine, "factors");
    let rows = texts(factors, "Variable");
    assert_eq!(rows, ["[g=1]", "[g=2]", "[g=3]", "x1", "x3", "y1", "y2"]);
    let loading = numbers(factors, "Loading_3");
    let weight = numbers(factors, "Weight_1");
    close(loading[4].unwrap(), C.p[2 * 5 + 4], "loading");
    close(loading[6].unwrap(), C.q[2 * 2 + 1], "Y loading");
    close(weight[1].unwrap(), C.w[1], "weight");
    close(weight[5].unwrap(), C.c[0], "Y weight");
    let coefficients = named(&engine, "coefficients");
    assert_eq!(texts(coefficients, "Variable")[0], "(Constant)");
    let b2 = numbers(coefficients, "Coefficient_2");
    close(b2[0].unwrap(), C.constant[1], "constant");
    close(b2[4].unwrap(), C.b[5 + 3], "coefficient of x1");
    let vip = numbers(coefficients, "VIP_2");
    assert_eq!(vip[0], None);
    close(vip[2].unwrap(), C.vip[5 + 1], "VIP");
}

#[test]
fn pls_categorical_dependent_with_the_last_category_as_reference() {
    let mut engine = Engine::default();
    let source = format!("{DATA}PLS cy MLEVEL=N WITH x1 x2 x3 /CRITERIA LATENTFACTORS=2.");
    let tables = run(&mut engine, &source);
    check(&tables, &D, &XS, &["[cy=1]", "[cy=2]"]);
    // REFERENCE=FIRST leaves out the first category instead
    let source = "PLS cy MLEVEL=O REFERENCE=FIRST WITH x1 x2 x3 /CRITERIA LATENTFACTORS=2.";
    let titles = find(&run(&mut engine, source), "Parameters").column_titles();
    assert_eq!(
        titles[1..],
        [
            "Dependent Variables / [cy=2]",
            "Dependent Variables / [cy=3]"
        ]
    );
    // a value as the reference, the level set for every dependent with ALL
    let source = "PLS cy ALL MLEVEL=N REFERENCE=2 WITH x1 x2 x3 /CRITERIA LATENTFACTORS=2.";
    let titles = find(&run(&mut engine, source), "Parameters").column_titles();
    assert_eq!(
        titles[1..],
        [
            "Dependent Variables / [cy=1]",
            "Dependent Variables / [cy=3]"
        ]
    );
}

#[test]
fn pls_interactions_and_id() {
    let mut engine = Engine::default();
    let source = format!(
        "{DATA}PLS y1 BY g WITH x1 /MODEL g x1 g*x1 /ID VARIABLE=x2 /CRITERIA LATENTFACTORS=2 /OUTDATASET CASES=out."
    );
    let tables = run(&mut engine, &source);
    let names: Vec<String> = find(&tables, "Parameters")
        .rows
        .iter()
        .map(|r| match &r[0] {
            Cell::Text(t) => t.clone(),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(
        names,
        [
            "(Constant)",
            "[g=1]",
            "[g=2]",
            "[g=3]",
            "x1",
            "[g=1] * x1",
            "[g=2] * x1",
            "[g=3] * x1"
        ]
    );
    let out = named(&engine, "out");
    assert_eq!(numbers(out, "x2")[..3], [Some(10.0), Some(14.0), Some(9.0)]);
}

#[test]
fn pls_errors() {
    let err = |tail: &str| run_error(&format!("{DATA}{tail}"));
    assert!(err("PLS y1 BY y1.").contains("at least one predictor"));
    assert!(err("PLS y1 BY g WITH g x1.").contains("both a factor"));
    assert!(err("PLS y1 WITH x1 /ID VARIABLE=x1.").contains("ID variable"));
    assert!(err("PLS cy MLEVEL=N REFERENCE=7 WITH x1.").contains("REFERENCE value"));
    let string = "STRING s (A3).\nCOMPUTE s = 'a'.\nPLS s MLEVEL=S WITH x1.";
    assert!(err(string).contains("string dependent"));
    // a constant scale dependent after a categorical one: its own column is checked, not a
    // dummy of the categorical one
    let constant = "COMPUTE k = 5.\nPLS cy MLEVEL=N k WITH x1 x2.";
    assert!(
        err(constant).contains("the dependent k has the same value"),
        "{}",
        err(constant)
    );
    assert!(err("PLS y1 WITH x1 /MODEL x2.").contains("neither a factor nor a covariate"));
}
const A: Expected = Expected {
    w: &[
        0.657986860989567,
        0.654507098135486,
        -0.372389244279907,
        0.334519808695633,
        0.188991585459361,
        0.923243563863726,
        0.674647899541184,
        -0.732053713272341,
        -0.0945916091883704,
    ],
    p: &[
        0.629714269339388,
        0.638534107053592,
        -0.450418968069821,
        0.306080269337782,
        0.219851046231163,
        0.927231039099719,
        0.674647899541185,
        -0.73205371327234,
        -0.0945916091883705,
    ],
    c: &[0.639356177370093, 0.211253886844052, 0.4438266093396],
    q: &[1.0, 1.0, 1.0],
    b: &[1.62357188854858, 0.102009724740086, -0.785486468123585],
    constant: &[0.947327849368472],
    x_variance: &[0.793951886244513, 0.202002152753996, 0.00404596100149107],
    y_variance: &[0.966740656215462, 0.0269970062236189, 0.00239094518803864],
    x_cumulative: &[0.793951886244513, 0.995954038998509, 1.0],
    r_square: &[0.966740656215462, 0.993737662439081, 0.99612860762712],
    adjusted: &[0.964182245155113, 0.992693939512261, 0.995072773343607],
    vip: &[
        1.13966667394669,
        1.13363954788513,
        0.644997091284977,
        1.12812882877791,
        1.11943563517859,
        0.688613973407813,
        1.12822751945468,
        1.11981569812309,
        0.687833894615726,
    ],
    t: &[
        -1.75599649388049,
        -0.00701602125040096,
        -2.65576557128553,
        0.794918977115278,
        2.19795869995623,
        -0.323456229383382,
        -1.27150783883389,
        2.51754875352868,
        -0.0554299931770329,
        -0.322318654316115,
        1.74297058656579,
        -2.12400051937957,
        1.25633822126582,
        -0.32559993963675,
        0.331356022711363,
        -0.81882131425799,
        -1.23821548392675,
        0.430557975534547,
        0.000282876552181066,
        -0.624225050948953,
        0.720192943003106,
        -0.168396107929921,
        0.401663308961688,
        1.27896413174934,
        -1.38847343231783,
        0.888871927485208,
        0.672469941507462,
        -0.199368484602641,
        0.282377737243327,
        -0.237880968052776,
        0.0585589894589582,
        0.0636207585388194,
        -0.00241660831231841,
        0.0891551492294642,
        -0.269129974402841,
        0.0271231474065403,
        -0.0207764218198063,
        -0.0309052540924909,
        0.0800862504170691,
        0.0921058749103508,
        0.137054718455811,
        -0.163200238973143,
        0.083035211393635,
        -0.106231770934401,
        -0.0380798312756472,
    ],
    u: &[
        -1.18884356291807,
        -0.177327506233578,
        -1.60094269712286,
        0.534480089211066,
        1.17136056934575,
        -0.139863948578596,
        -0.776744428713277,
        1.69585037651548,
        0.272235185626197,
        -0.589426640438371,
        1.43360547293061,
        -1.30123423588301,
        0.759261435140953,
        -0.25225462154354,
        0.159844512661254,
        -0.0661363571153541,
        -0.172841769706574,
        0.0970374265253573,
        0.0262437306836971,
        -0.23391790307561,
        0.0669397897865068,
        0.0362019626196697,
        0.0862400291165417,
        0.307674694175515,
        -0.383350217719747,
        0.319226461435402,
        0.0567586169196024,
        -0.0439861674915074,
        -0.0440802887854542,
        -0.0520100073680411,
        0.106842828152401,
        0.0887360640234413,
        0.00608038068197791,
        0.0261839719125518,
        -0.102047934797217,
        -0.0852037687005563,
        0.0717762949492759,
        0.00138709389574165,
        0.0374885502093376,
        -0.0900298083629029,
        0.131448811847587,
        -0.0853032720096415,
        -0.00186880020499085,
        -0.103733683336335,
        -0.00175672826066641,
    ],
    dmodx: &[
        8.10288236676091e-18,
        9.16736490990966e-17,
        3.66694596396387e-16,
        1.94469176802262e-16,
        2.59292235736349e-16,
        4.58368245495483e-17,
        4.15070099805392e-16,
        3.66694596396387e-16,
        6.48230589340873e-17,
        3.17566835909752e-16,
        4.4565853017185e-17,
        5.79795065144377e-16,
        1.83347298198193e-16,
        9.16736490990966e-17,
        7.93917089774381e-17,
    ],
    predicted: &[
        4.98418282865955,
        7.73851112317012,
        4.08090695010546,
        9.83572927002395,
        11.4535572425505,
        8.2595635581227,
        6.08379676445651,
        12.8596843733596,
        9.09481058032472,
        7.14942983186546,
        12.0114960636901,
        4.93435515500506,
        10.503359208141,
        7.85104063837049,
        8.75957641215472,
    ],
};

const B: Expected = Expected {
    w: &[
        0.600013764689273,
        0.611058180214609,
        -0.51632488084172,
        0.407130481089527,
        0.322338781998152,
        0.854600772868687,
        0.688642326284999,
        -0.722983824170886,
        -0.0553727047923986,
    ],
    p: &[
        0.612752854253214,
        0.624982951540371,
        -0.48504133222042,
        0.406539350419007,
        0.322959391415327,
        0.854648304806332,
        0.688642326284999,
        -0.722983824170886,
        -0.0553727047923983,
    ],
    c: &[
        0.626187736351321,
        -0.562677505552912,
        0.313491675950388,
        0.530631827398638,
        0.453576539354808,
        -0.237469453405246,
    ],
    q: &[
        0.713253238702002,
        -0.702396863664394,
        0.400464981052159,
        0.924887955452489,
        0.342522942827699,
        -1.50174906359635,
    ],
    b: &[
        1.62357188854858,
        0.102009724740086,
        -0.785486468123585,
        -0.187464975928768,
        -0.00287309777739202,
        2.52840867716814,
    ],
    constant: &[0.947327849368473, 2.38554605257742],
    x_variance: &[0.795509247273647, 0.20045212922913, 0.00403862349722313],
    y_variance: &[0.844559103400411, 0.114211675994913, 0.00158792669480337],
    x_cumulative: &[0.795509247273647, 0.995961376502777, 1.0],
    r_square: &[0.844559103400411, 0.958770779395324, 0.960358706090128],
    adjusted: &[0.832602111354289, 0.951899242627879, 0.949547444114708],
    vip: &[
        1.0392543256825,
        1.05838381451228,
        0.894300926829805,
        1.00529924696028,
        1.01186410875095,
        0.982600859699776,
        1.00563805755795,
        1.01230868485436,
        0.981795917571219,
    ],
    t: &[],
    u: &[],
    dmodx: &[],
    predicted: &[],
};

const C: Expected = Expected {
    w: &[
        -0.0129924239168628,
        -0.379336090674919,
        0.45082461723269,
        -0.597075208697949,
        0.544236793029959,
        -0.273167755623821,
        -0.0916065155854191,
        0.44640814248511,
        0.753740108285374,
        0.38675992951014,
        0.70637806974761,
        -0.63530419786919,
        -0.155140193169821,
        0.270832095237549,
        -0.00030887197305974,
    ],
    p: &[
        -0.0341851782020713,
        -0.420170502476436,
        0.524097255849063,
        -0.491829739470356,
        0.570036298025045,
        -0.642978100575427,
        0.228440081613843,
        0.542011634279952,
        0.61230915645394,
        0.366649509614121,
        0.698646846985035,
        -0.65184844487701,
        -0.126522348659256,
        0.268537670866743,
        -0.0382479644390813,
    ],
    c: &[
        -0.526012571761548,
        0.622336696218411,
        0.556226618769408,
        0.13136075158802,
        0.23385073582213,
        0.005940432643164,
    ],
    q: &[
        -0.697242592167205,
        0.720026301504652,
        0.968429085878595,
        0.250162068728698,
        0.99839991331105,
        0.0756882079160125,
    ],
    b: &[
        -0.0562813281151816,
        0.131789481878666,
        -0.0850341197857785,
        1.87619423152435,
        -0.777590386617926,
        -0.0607745942762291,
        -0.402962823301476,
        0.623849009139795,
        -0.160918136609128,
        1.21720132423075,
    ],
    constant: &[1.51668550683595, 2.87801915640379],
    x_variance: &[0.467504568256073, 0.241095370423483, 0.246203124002645],
    y_variance: &[0.761408334743052, 0.155178541631739, 0.0335942354388875],
    x_cumulative: &[0.467504568256073, 0.708599938679555, 0.954803062682201],
    r_square: &[0.761408334743052, 0.916586876374791, 0.950181111813678],
    adjusted: &[0.748153242228777, 0.906773567713002, 0.940840070278743],
    vip: &[
        0.0290519430705992,
        0.848221285068142,
        1.00807449006262,
        1.33510075432849,
        1.21695046507147,
        0.252720470982009,
        0.77767270159768,
        1.00640937167455,
        1.4005839218948,
        1.16484436173757,
        0.387061369201239,
        0.809161474684758,
        0.990608057160609,
        1.38030691465067,
        1.14406717583107,
    ],
    t: &[
        0.957600533096488,
        -1.12663556126981,
        3.12703260649977,
        -0.47356410799001,
        -2.22013484497287,
        1.66360266205814,
        0.893390020056358,
        -2.20384214646023,
        1.64763005721564,
        -0.20042935731685,
        -1.608694549576,
        2.93408097370924,
        -0.843494768728578,
        -0.322243633082398,
        0.9397139454235,
        -1.54800272472727,
        -0.624684293644676,
        0.244396390987117,
        0.0025836432886269,
        0.45521934633879,
        1.39559782517028,
        -1.0141370114567,
        1.32122960372318,
        1.86222314872193,
        -1.26686895240927,
        1.38035518448054,
        0.51335794986279,
        0.0738814374883211,
        0.031489985621403,
        1.07156809001801,
        -0.0428741037692387,
        -1.94971725111599,
        -0.997109752253466,
        1.34719725260424,
        -0.965963062119914,
        0.0997189813585206,
        0.36645913824294,
        -0.321432506971587,
        0.451006913745356,
        0.35439483970385,
        -0.373445667327385,
        -0.764928513752091,
        1.46027179722015,
        -1.58938109482483,
        -0.0254198617964849,
    ],
    u: &[
        1.34741732308238,
        -0.548897966422738,
        2.32882514084785,
        -0.256627929453693,
        -1.98917849547827,
        1.10650406872711,
        0.860618531203872,
        -2.06017422006721,
        0.6197052768486,
        0.0368226306331659,
        -1.57952823447448,
        2.51436313863107,
        -1.30453609324497,
        0.393935585905368,
        -0.191785011150537,
        -0.756798628709045,
        -0.813588638922978,
        -0.183767628644666,
        0.417956378203635,
        0.0599195158059587,
        0.761397484057927,
        -0.404382292945232,
        0.709693678575773,
        1.13160836876433,
        -0.818252341035461,
        0.741320846602411,
        0.1809646392012,
        0.251186836541219,
        -0.32813453753989,
        0.483723940967788,
        0.0735810417259939,
        -0.447054128550626,
        -0.247019636696833,
        0.334391959202543,
        -0.160819931140589,
        -0.057439156582277,
        0.18309957731796,
        -0.0575511627777068,
        0.114295730717216,
        -0.0530402613085533,
        -0.0619703825608965,
        -0.157286728610358,
        0.320308708261249,
        -0.455593443122243,
        0.0438667466476027,
    ],
    dmodx: &[
        0.309802645844785,
        0.38829760576418,
        0.103265785512949,
        0.139558150412715,
        0.611776069409837,
        0.664295501833379,
        0.715797571937027,
        0.177399224968825,
        0.265905428343078,
        0.695246259577593,
        0.875316288124194,
        0.0728463751051853,
        0.197993102262058,
        1.06837534136209,
        1.73778808067323,
    ],
    predicted: &[
        5.01161687161293,
        7.87201729857381,
        4.10896965583786,
        9.77986148908559,
        11.3269259539794,
        8.20449561936257,
        6.05957437186575,
        12.8599826850277,
        9.06483369646296,
        7.12108821912266,
        12.1416063382469,
        4.8594473484476,
        10.4204787972047,
        7.93621999023151,
        8.62539215850038,
        3.08791713736377,
        2.17137493347743,
        4.30769671145484,
        2.56390166341788,
        1.76000215515793,
        3.72632835972952,
        3.11308638782137,
        1.85861609705586,
        3.76758942384803,
        2.56183860348542,
        2.15033180288475,
        4.24332945681119,
        2.39390609001206,
        2.62607183584791,
        3.34507614879938,
    ],
};

const D: Expected = Expected {
    w: &[
        -0.677657742860242,
        -0.667375161817187,
        0.308853325918704,
        -0.282434510993535,
        -0.151582841940971,
        -0.947234600840225,
    ],
    p: &[
        -0.639422111055484,
        -0.646801388514009,
        0.4372025904608,
        -0.260718837909209,
        -0.17490258483372,
        -0.949977725516147,
    ],
    c: &[
        0.512074107362754,
        0.0202664943002984,
        0.271208809338421,
        -0.194797663297276,
    ],
    q: &[
        1.01973999055512,
        -0.478990030607593,
        0.568671768783961,
        -0.922433471251812,
    ],
    b: &[
        -0.164374976655202,
        -0.0507326565897567,
        -0.167322634017891,
        0.0216851793851475,
        0.00418649795574716,
        0.349656657205942,
    ],
    constant: &[1.82378930009716, 0.0133619557565605],
    x_variance: &[0.790920647380607, 0.205036476746507],
    y_variance: &[0.305962868432416, 0.034257415628451],
    x_cumulative: &[0.790920647380607, 0.995957124127115],
    r_square: &[0.305962868432416, 0.340220284060867],
    adjusted: &[0.252575396773371, 0.230256998071011],
    vip: &[
        1.17373764077638,
        1.15592768797687,
        0.534949652577825,
        1.12384929231473,
        1.09934907369009,
        0.726907409743773,
    ],
    t: &[],
    u: &[],
    dmodx: &[],
    predicted: &[],
};
