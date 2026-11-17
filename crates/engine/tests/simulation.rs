//! SIMPLAN, SIMRUN and BEGIN EXPR on invented data. The fitted parameters are compared with
//! maximum likelihood fits of R's MASS::fitdistr run as a black box on the same numbers, the
//! Anderson-Darling and Kolmogorov-Smirnov statistics with their textbook formulas evaluated in
//! R; the simulated moments with the exact moments of targets whose distribution is known.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

/// Twenty invented positive values x, the same over 7 as y (inside 0 to 1), and twenty
/// invented counts k.
const DATA: &str = "DATA LIST FREE /x y k.
BEGIN DATA.
2.3 0.328571428571429 0 4.1 0.585714285714286 3 1.7 0.242857142857143 1 3.3 0.471428571428571 5
5.9 0.842857142857143 2 2.8 0.4 0 3.6 0.514285714285714 7 1.2 0.171428571428571 2
4.4 0.628571428571429 1 2.5 0.357142857142857 4 3.1 0.442857142857143 9 6.2 0.885714285714286 0
2.2 0.314285714285714 3 3.9 0.557142857142857 2 2.7 0.385714285714286 6 4.8 0.685714285714286 1
1.9 0.271428571428571 0 3.4 0.485714285714286 4 2.6 0.371428571428571 2 5.1 0.728571428571429 11
END DATA.
";

fn run_on(engine: &mut Engine, source: &str) -> Vec<Table> {
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

fn fails(engine: &mut Engine, source: &str) -> String {
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
    tables.iter().find(|t| t.title == title).unwrap_or_else(|| {
        panic!(
            "no {title} in {:?}",
            tables.iter().map(|t| &t.title).collect::<Vec<_>>()
        )
    })
}

fn number(cell: &Cell) -> f64 {
    match cell {
        Cell::Number(n) => *n,
        other => panic!("{other:?}"),
    }
}

fn text(cell: &Cell) -> &str {
    match cell {
        Cell::Text(t) => t,
        other => panic!("{other:?}"),
    }
}

/// A folder of its own for the files a test writes.
fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-sim-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn plan_json(dir: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("p.splan")).unwrap()).unwrap()
}

/// The values of the parameter `name` of input `i` in a plan.
fn param(plan: &serde_json::Value, i: usize, name: &str) -> f64 {
    let params = plan["inputs"][i]["params"].as_array().unwrap();
    let found = params.iter().find(|p| p[0] == name).unwrap();
    found[1][0].as_f64().unwrap()
}

fn near(got: f64, want: f64, relative: f64) {
    assert!(
        (got - want).abs() <= relative * want.abs(),
        "got {got}, want {want}"
    );
}

/// Maximum likelihood fits against fitdistr: gamma (shape, rate), Weibull (scale, shape),
/// lognormal (exp(meanlog), sdlog), beta and the negative binomial (size, size / (size + mu)).
/// fitdistr's optimizer stops near 1e-7 relative, so the tolerance is 1e-5.
#[test]
fn fits_agree_with_maximum_likelihood() {
    let dir = folder("fits");
    let source = format!(
        "{DATA}CD '{}'.\nSIMPLAN CREATE /SIMINPUT INPUT=g(MAPTO=x) TYPE=AUTOFIT(GAMMA)
          /SIMINPUT INPUT=w(MAPTO=x) TYPE=AUTOFIT(WEIBULL) /SIMINPUT INPUT=l(MAPTO=x) TYPE=AUTOFIT(LNORMAL)
          /SIMINPUT INPUT=b(MAPTO=y) TYPE=AUTOFIT(BETA) /SIMINPUT INPUT=n(MAPTO=k) TYPE=AUTOFIT(NEGBIN)
          /SIMINPUT INPUT=m(MAPTO=x) TYPE=AUTOFIT(NORMAL)
          /CORRELATIONS VARORDER=g w l b n m CORRMATRIX=1; 0, 1; 0, 0, 1; 0, 0, 0, 1; 0, 0, 0, 0, 1; 0, 0, 0, 0, 0, 1
          /PLAN FILE='p.splan'.",
        dir.display()
    );
    run_on(&mut Engine::default(), &source);
    let plan = plan_json(&dir);
    near(param(&plan, 0, "SHAPE"), 6.27716420036292, 1e-5);
    near(param(&plan, 0, "SCALE"), 1.85440655641568, 1e-5);
    near(param(&plan, 1, "A"), 3.81404377530355, 1e-5);
    near(param(&plan, 1, "B"), 2.73353491453808, 1e-5);
    near(param(&plan, 2, "A"), 3.119242483188267, 1e-9);
    near(param(&plan, 2, "B"), 0.414479417297809, 1e-9);
    near(param(&plan, 3, "SHAPE1"), 2.89509707090623, 1e-5);
    near(param(&plan, 3, "SHAPE2"), 3.01023170449529, 1e-5);
    near(param(&plan, 4, "THRESHOLD"), 1.452255949344724, 1e-5);
    near(param(&plan, 4, "PROB"), 0.315553047884655, 1e-5);
    near(param(&plan, 5, "MEAN"), 3.385, 1e-12);
    near(param(&plan, 5, "STDDEV"), 1.3368900478349, 1e-10);
    // the statistic of the normal fit: Anderson-Darling 0.27913212796869 in R
    near(
        plan["inputs"][5]["fit"][0]["statistic"].as_f64().unwrap(),
        0.27913212796869,
        1e-9,
    );
}

/// FIT=KS gives the Kolmogorov-Smirnov distance (0.119155604571404 in R); with the default
/// continuous set the best fit comes first in the Goodness of Fit table and is the one kept.
#[test]
fn autofit_orders_the_candidates() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!("{DATA}SIMPLAN CREATE /SIMINPUT INPUT=x TYPE=AUTOFIT(NORMAL) /AUTOFIT FIT=KS /PLAN FILE='*\\p'.
          SIMPLAN CREATE /SIMINPUT INPUT=x /PLAN FILE='*\\q'."),
    );
    let ks = find(&tables, "Goodness of Fit");
    assert_eq!(text(&ks.rows[0][2]), "Kolmogorov-Smirnov");
    near(number(&ks.rows[0][3]), 0.119155604571404, 1e-9);
    let all = tables
        .iter()
        .filter(|t| t.title == "Goodness of Fit")
        .nth(1)
        .unwrap();
    // beta needs values inside 0 to 1, so seven of the eight continuous families fit
    assert_eq!(all.rows.len(), 7);
    let stats: Vec<f64> = all.rows.iter().map(|r| number(&r[3])).collect();
    assert!(stats.windows(2).all(|w| w[0] <= w[1]), "{stats:?}");
    let inputs = tables
        .iter()
        .filter(|t| t.title == "Simulated Inputs")
        .nth(1)
        .unwrap();
    assert_eq!(text(&inputs.rows[0][1]), text(&all.rows[0][1]));
}

const MODEL: &str = "BEGIN EXPR /OUTFILE PREPXML='*\\m'.
NUMERIC a b c.
COMPUTE total = a + b + c.
END EXPR.
";

/// A sum of independent normals N(10, 2), N(5, 1.5) and N(-3, 1) is N(12, 2.5 + ...): the
/// variance is 4 + 2.25 + 1 = 7.25. With 20000 cases the standard error of the mean is
/// sqrt(7.25 / 20000) = 0.019 and of the standard deviation about 2.69 / 200 = 0.013; the
/// tolerances are five of them.
#[test]
fn a_sum_of_normals_has_the_exact_moments() {
    let source = format!(
        "{MODEL}SIMPLAN CREATE /MODEL FILE='*\\m' CUSTOM=YES
          /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=10 STDDEV=2)
          /SIMINPUT INPUT=b TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=5 STDDEV=1.5)
          /SIMINPUT INPUT=c TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=-3 STDDEV=1)
          /STOPCRITERIA MAXCASES=20000 /PLAN FILE='*\\p' DISPLAY=NO.
        SIMRUN /PLAN FILE='*\\p' /CRITERIA SEED=12345."
    );
    let tables = run_on(&mut Engine::default(), &source);
    let stats = find(&tables, "Target Statistics");
    let row = &stats.rows[0];
    assert_eq!(number(&row[1]), 20000.0);
    assert!((number(&row[2]) - 12.0).abs() < 5.0 * 0.019, "{row:?}");
    assert!(
        (number(&row[3]) - 7.25f64.sqrt()).abs() < 5.0 * 0.013,
        "{row:?}"
    );
    // the same seed gives the same numbers; another seed others
    let again = run_on(&mut Engine::default(), &source);
    assert_eq!(find(&again, "Target Statistics").rows, stats.rows);
    let other = run_on(&mut Engine::default(), &source.replace("12345", "54321"));
    assert_ne!(find(&other, "Target Statistics").rows, stats.rows);
}

/// Correlated inputs through the normal-to-anything transformation: a gamma and a uniform
/// input asked to correlate 0.6 reach it within the search tolerance 0.01 plus five standard
/// errors of a sample correlation over 20000 cases ((1 - 0.36) / sqrt(20000) = 0.0045). Two
/// normals asked for -0.4 and a sum with known variance: Var(a + b) = 4 + 2.25 + 2 (-0.4) 3.
#[test]
fn correlated_inputs_reach_the_planned_correlation() {
    let source =
        "SIMPLAN CREATE /SIMINPUT INPUT=g TYPE=MANUAL DISTRIBUTION=GAMMA(SHAPE=2 SCALE=0.5)
          /SIMINPUT INPUT=u TYPE=MANUAL DISTRIBUTION=UNIFORM(MIN=0 MAX=10)
          /CORRELATIONS VARORDER=g u CORRMATRIX=1; 0.6, 1
          /STOPCRITERIA MAXCASES=20000 /PLAN FILE='*\\p' DISPLAY=NO.
        SIMRUN /PLAN FILE='*\\p' /PRINT ASSOCIATIONS=YES.";
    let tables = run_on(&mut Engine::default(), source);
    let r = number(&find(&tables, "Correlations of Simulated Inputs").rows[0][2]);
    assert!((r - 0.6).abs() < 0.01 + 5.0 * 0.0045, "{r}");
    // the gamma keeps its mean shape / rate = 4 and the uniform 5
    let inputs = find(&tables, "Input Statistics");
    assert!((number(&inputs.rows[0][1]) - 4.0).abs() < 0.05);
    assert!((number(&inputs.rows[1][1]) - 5.0).abs() < 0.1);

    let source = "BEGIN EXPR /OUTFILE PREPXML='*\\m'.
NUMERIC a b.
COMPUTE s = a + b.
END EXPR.
SIMPLAN CREATE /MODEL FILE='*\\m' CUSTOM=YES
  /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=2)
  /SIMINPUT INPUT=b TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=1.5)
  /CORRELATIONS VARORDER=a b CORRMATRIX=1; -0.4, 1
  /STOPCRITERIA MAXCASES=20000 /PLAN FILE='*\\p' DISPLAY=NO.
SIMRUN /PLAN FILE='*\\p'.";
    let tables = run_on(&mut Engine::default(), source);
    let sd = number(&find(&tables, "Target Statistics").rows[0][3]);
    let exact = (4.0f64 + 2.25 - 2.4).sqrt();
    assert!((sd - exact).abs() < 5.0 * exact / 200.0, "{sd} {exact}");
}

/// target = 10 a + b + 0.1 c with independent standard normals: the tornado puts a first,
/// then b, then c. One standard deviation of the drawn values moves the target by exactly 10,
/// 1 and 0.1 times that standard deviation; a's share of the variance is 100 / 101.01 (the
/// tolerance covers the Sobol estimate over 20000 cases).
#[test]
fn tornado_orders_the_inputs() {
    let source = "BEGIN EXPR /OUTFILE PREPXML='*\\m'.
NUMERIC a b c.
COMPUTE t = 10 * a + b + 0.1 * c.
END EXPR.
SIMPLAN CREATE /MODEL FILE='*\\m' CUSTOM=YES
  /SIMINPUT INPUT=c TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=1)
  /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=1)
  /SIMINPUT INPUT=b TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=1)
  /STOPCRITERIA MAXCASES=20000 /PLAN FILE='*\\p' DISPLAY=NO.
SIMRUN /PLAN FILE='*\\p' /TORNADO TYPE=CORR DELTASTDDEV(1) VARIANCE /PRINT DESCRIPTIVES=NO.";
    let mut engine = Engine::default();
    let tables = run_on(&mut engine, source);
    let order = |title: &str| -> Vec<String> {
        find(&tables, title)
            .rows
            .iter()
            .map(|r| text(&r[1]).to_string())
            .collect()
    };
    assert_eq!(order("Tornado: Correlation"), ["a", "b", "c"]);
    assert_eq!(order("Tornado: Change in Target"), ["a", "b", "c"]);
    assert_eq!(order("Tornado: Contribution to Variance"), ["a", "b", "c"]);
    let change = find(&tables, "Tornado: Change in Target");
    // the standard deviation of 20000 standard normal draws is 1 within 0.01
    assert!((number(&change.rows[0][3]) - 10.0).abs() < 0.1);
    assert!((number(&change.rows[0][2]) + number(&change.rows[0][3])).abs() < 1e-9);
    let share = number(&find(&tables, "Tornado: Contribution to Variance").rows[0][2]);
    assert!((share - 100.0 / 101.01).abs() < 0.03, "{share}");
    assert!(
        engine
            .take_charts()
            .iter()
            .any(|c| c.title.starts_with("Tornado: Correlation"))
    );
}

/// FIXEDINPUT ITERATIONS runs one simulation per value; the stopping rule on the mean stops at
/// a hundred cases where the 95% interval is at most 0.5 wide: for a standard deviation of 2
/// that needs about (2 * 1.96 * 2 / 0.5)^2 = 246 cases, so 300.
#[test]
fn sensitivity_iterations_and_stopping() {
    let source = "BEGIN EXPR /OUTFILE PREPXML='*\\m'.
NUMERIC a f.
COMPUTE t = a + f.
END EXPR.
SIMPLAN CREATE /MODEL FILE='*\\m' CUSTOM=YES
  /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=2)
  /FIXEDINPUT INPUT=f ITERATIONS=VALUES(0 100)
  /STOPCRITERIA MAXCASES=5000 TARGET=t TYPE=MEAN(THRESH=0.5 UNIT=VALUE)
  /PLAN FILE='*\\p' DISPLAY=NO.
SIMRUN /PLAN FILE='*\\p' /OUTFILE FILE=*.";
    let mut engine = Engine::default();
    let tables = run_on(&mut engine, source);
    let stats = find(&tables, "Target Statistics");
    assert_eq!(stats.rows.len(), 2);
    for (row, mean) in stats.rows.iter().zip([0.0, 100.0]) {
        let n = number(&row[2]);
        assert_eq!(n % 100.0, 0.0);
        assert!((200.0..=400.0).contains(&n), "{n}");
        assert!((number(&row[3]) - mean).abs() < 0.5);
        assert!(number(&row[9]) - number(&row[8]) <= 0.5);
    }
    let data = engine.dataset().unwrap();
    let names: Vec<&str> = data.variable_names().collect();
    assert_eq!(names, ["Iteration", "t", "a", "f"]);
    let n: f64 = stats.rows.iter().map(|r| number(&r[2])).sum();
    assert_eq!(data.case_count() as f64, n);
}

/// Without a model the plan only generates the inputs: a fitted Poisson keeps the mean of k
/// (3.15), and a CATEGORICAL input keeps its shares.
#[test]
fn generating_data_without_a_model() {
    let mut engine = Engine::default();
    run_on(
        &mut engine,
        &format!(
            "{DATA}VARIABLE LEVEL k (ORDINAL).
SIMPLAN CREATE /SIMINPUT INPUT=k TYPE=AUTOFIT(POISSON)
  /SIMINPUT INPUT=kind TYPE=MANUAL DISTRIBUTION=CATEGORICAL(CATS=1 2 PROBS=0.25 0.75)
  /STOPCRITERIA MAXCASES=4000 /VALUELABELS VARIABLES=kind VALUES=1 'small' 2 'large'
  /PLAN FILE='*\\p' DISPLAY=NO.
SIMRUN /PLAN FILE='*\\p' /OUTFILE FILE=*."
        ),
    );
    let data = engine.dataset().unwrap();
    assert_eq!(data.case_count(), 4000);
    let k: Vec<f64> = data
        .variable("k")
        .unwrap()
        .numeric()
        .unwrap()
        .iter()
        .map(Option::unwrap)
        .collect();
    // Poisson(3.15): standard error of the mean sqrt(3.15 / 4000) = 0.028
    assert!((k.iter().sum::<f64>() / 4000.0 - 3.15).abs() < 5.0 * 0.028);
    let kind = data.variable("kind").unwrap();
    let large = kind
        .numeric()
        .unwrap()
        .iter()
        .filter(|v| *v == Some(2.0))
        .count() as f64;
    // binomial standard error sqrt(0.75 * 0.25 / 4000) = 0.0068
    assert!((large / 4000.0 - 0.75).abs() < 5.0 * 0.0068);
    assert_eq!(kind.value_labels().len(), 2);
}

#[test]
fn errors_name_what_is_missing_or_unsupported() {
    let mut engine = Engine::default();
    let dir = folder("errors");
    std::fs::write(dir.join("tree.xml"), "<PMML/>").unwrap();
    let cd = format!("CD '{}'.\n", dir.display());
    let error = fails(
        &mut engine,
        &format!("{cd}SIMPLAN CREATE /MODEL FILE='tree.xml' /SIMINPUT INPUT=a /PLAN FILE='p'."),
    );
    assert!(error.contains("not supported yet"), "{error}");
    let error = fails(
        &mut Engine::default(),
        &format!(
            "{MODEL}SIMPLAN CREATE /MODEL FILE='*\\m' CUSTOM=YES /SIMINPUT INPUT=a TYPE=MANUAL DISTRIBUTION=NORMAL(MEAN=0 STDDEV=1) /PLAN FILE='*\\p'."
        ),
    );
    assert!(error.contains("not given: b c"), "{error}");
    let error = fails(
        &mut Engine::default(),
        "BEGIN EXPR /OUTFILE PREPXML='*\\m'.\nNUMERIC a.\nCOMPUTE t = a.\nCOMPUTE t = 2 * a.\nEND EXPR.",
    );
    assert!(error.contains("computed twice"), "{error}");
    let error = fails(
        &mut Engine::default(),
        "BEGIN EXPR /OUTFILE PREPXML='*\\m'.\nCOMPUTE t = a.\nEND EXPR.",
    );
    assert!(error.contains("declared with NUMERIC"), "{error}");
    let error = fails(
        &mut Engine::default(),
        &format!(
            "{DATA}SIMPLAN CREATE /SIMINPUT INPUT=x TYPE=MANUAL DISTRIBUTION=UNIFORM(MIN=0 MAX=1) MINVAL=0.2 /PLAN FILE='*\\p'."
        ),
    );
    assert!(error.contains("MINVAL"), "{error}");
}

/// An input of the model that is a field of the active dataset needs no NUMERIC; the block
/// leaves the active dataset as it was, and MINVAL cuts the fitted distribution.
#[test]
fn model_inputs_from_the_active_dataset() {
    let mut engine = Engine::default();
    let tables = run_on(
        &mut engine,
        &format!(
            "{DATA}BEGIN EXPR /OUTFILE PREPXML='*\\m'.
NUMERIC price.
COMPUTE revenue = price * x.
END EXPR.
SIMPLAN CREATE /MODEL FILE='*\\m' CUSTOM=YES /SIMINPUT INPUT=x TYPE=AUTOFIT(NORMAL) MINVAL=1
  /FIXEDINPUT INPUT=price VALUE=2 /STOPCRITERIA MAXCASES=2000 /PLAN FILE='*\\p'.
SIMRUN /PLAN FILE='*\\p' /PRINT PERCENTILES=YES(NTILES=4)."
        ),
    );
    assert_eq!(engine.dataset().unwrap().variable_count(), 3);
    let x = find(&tables, "Input Statistics");
    assert!(number(&x.rows[0][3]) >= 1.0);
    let stats = find(&tables, "Target Statistics");
    assert!((number(&stats.rows[0][2]) - 2.0 * number(&x.rows[0][1])).abs() < 1e-9);
    assert_eq!(
        find(&tables, "Target Percentiles").columns,
        ["Target", "25", "50", "75"]
    );
    assert_eq!(text(&find(&tables, "Fixed Inputs").rows[0][1]), "2");
}
