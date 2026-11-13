//! MIXED on synthetic data from tests/fixtures/mixed_generate.py (pupils in classes in schools,
//! and people seen at up to four waves). The expected numbers come from R 4.3.3 run as a separate
//! program on the same files: nlme's lme and gls for the fits (their -2 log likelihoods, fixed
//! effects and covariance parameters, converted from standard deviations and correlations to
//! variances and covariances), and an independent dense calculation written for this check: the
//! -2 log likelihood with the constants of the documentation, maximized and then differentiated
//! numerically for the standard errors (2 H^-1) and the Satterthwaite degrees of freedom. A
//! balanced block design checks the degrees of freedom exactly against sums of squares by hand.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const PUPILS: &str = include_str!("../../../tests/fixtures/mixed_pupils.csv");
const WAVES: &str = include_str!("../../../tests/fixtures/mixed_waves.csv");

fn inline(csv: &str, list: &str) -> String {
    let body: Vec<&str> = csv.lines().skip(1).collect();
    format!(
        "DATA LIST LIST(\",\") /{list}.\nBEGIN DATA\n{}\nEND DATA.\n",
        body.join("\n")
    )
}

fn pupils() -> String {
    inline(
        PUPILS,
        "school room pupil girl (F4.0) track (A4) hours mark (F8.1)",
    )
}

fn waves() -> String {
    inline(WAVES, "person arm wave dose level")
}

fn run(source: &str) -> (Engine, Vec<Table>) {
    let (commands, errors) = parse(source);
    assert!(errors.is_empty(), "{errors:?}");
    let mut engine = Engine::default();
    let mut tables = Vec::new();
    for command in &commands {
        match engine.execute(&command.command) {
            Ok(more) => tables.extend(more),
            Err(error) => panic!("line {}: {error}", command.first_line),
        }
    }
    (engine, tables)
}

/// The row headers of every row joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let Cell::Text(t) = cell else { continue };
                if t.is_empty() && level == 0 {
                    continue;
                }
                carried[level] = t.clone();
                for deeper in carried.iter_mut().skip(level + 1) {
                    deeper.clear();
                }
            }
            let parts: Vec<&str> = carried
                .iter()
                .map(String::as_str)
                .filter(|s| !s.is_empty())
                .collect();
            parts.join(" / ")
        })
        .collect()
}

fn cell(tables: &[Table], title: &str, row: &str, column: &str) -> Option<f64> {
    let table = tables.iter().find(|t| t.title == title)?;
    let r = row_keys(table).iter().position(|k| k == row)?;
    let c = table.column_titles().iter().position(|t| t == column)?;
    match table.rows[r][c] {
        Cell::Number(n) => Some(n),
        _ => None,
    }
}

struct Case {
    name: &'static str,
    syntax: &'static str,
    expected: &'static [(&'static str, &'static str, &'static str, f64)],
}

const CASES: &[Case] = &[
    Case {
        name: "random intercept, REML",
        syntax: "MIXED mark BY girl track WITH hours /FIXED=girl track hours /RANDOM=INTERCEPT | SUBJECT(school) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                1129.44106225,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1133.44106225,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1133.50699631,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1141.8817739,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1139.8817739,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                20.4191812372,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                2.17220729964,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                9.40019916177,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Estimate",
                16.8673285527,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Std. Error",
                9.14017424846,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Wald Z",
                1.84540557917,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                49.2879425653,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.62592081006,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                13.4807338681,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                30.3138641565,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -0.78358722269,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.673382030011,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "df",
                177.373498644,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "t",
                -1.16365924211,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Estimate",
                1.33474025436,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Std. Error",
                0.818253447332,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "df",
                177.734101139,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "t",
                1.63120639297,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Estimate",
                3.73664682032,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Std. Error",
                0.833692314939,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "df",
                177.984676259,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "t",
                4.48204541815,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.7523230541,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.116253844049,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 177.497344465),
            ("Estimates of Fixed Effects", "hours", "t", 15.0732482735),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                10.5251150223,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1096.92923616,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "Denominator df",
                177.373498644,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "F",
                1.35410283174,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "Denominator df",
                177.695932132,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "F",
                10.3023927496,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                177.497344465,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                227.202813515,
            ),
        ],
    },
    Case {
        name: "random intercept, ML",
        syntax: "MIXED mark BY girl track WITH hours /FIXED=girl track hours /RANDOM=INTERCEPT | SUBJECT(school) /PRINT=SOLUTION TESTCOV /METHOD=ML.",
        expected: &[
            (
                "Information Criteria",
                "-2 Log Likelihood",
                "Value",
                1133.03175134,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1147.03175134,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1147.64713595,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1176.76091984,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1169.76091984,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                19.9725983015,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                2.10157793852,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                9.50362008252,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Estimate",
                14.8172738105,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Std. Error",
                7.63368642191,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Wald Z",
                1.94103778851,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                49.2845390727,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.54880435267,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                15.8885888968,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                31.8210230929,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -0.78072286447,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.665914434779,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "df",
                181.368295349,
            ),
            ("Estimates of Fixed Effects", "[girl=0]", "t", -1.1724071798),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Estimate",
                1.34472835075,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Std. Error",
                0.809135227058,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "df",
                181.775461698,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "t",
                1.66193277191,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Estimate",
                3.74551223273,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Std. Error",
                0.824370208175,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "df",
                182.059952572,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "t",
                4.54348325011,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.75249621725,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.114962454279,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 181.512916925),
            ("Estimates of Fixed Effects", "hours", "t", 15.2440744958),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                12.1595640673,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1221.79186611,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "Denominator df",
                181.368295349,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "F",
                1.37453859526,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "Denominator df",
                181.73424229,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "F",
                10.5794696867,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                181.512916925,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                232.381807233,
            ),
        ],
    },
    Case {
        name: "random slope UN, REML",
        syntax: "MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(UN) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                1130.79173042,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1138.79173042,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1139.01151064,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1155.71616489,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1151.71616489,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                18.4131768519,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                1.99471308496,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                9.23099015632,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (1,1)",
                "Estimate",
                36.2030814469,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (1,1)",
                "Std. Error",
                21.6645123867,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (1,1)",
                "Wald Z",
                1.67107760381,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,1)",
                "Estimate",
                -3.25411257596,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,1)",
                "Std. Error",
                2.5638180545,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,1)",
                "Wald Z",
                -1.26924473843,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,2)",
                "Estimate",
                0.617476199232,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,2)",
                "Std. Error",
                0.384135383437,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,2)",
                "Wald Z",
                1.60744421331,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                50.2180598755,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                2.13816593473,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                7.19105607899,
            ),
            ("Estimates of Fixed Effects", "Intercept", "t", 23.486511996),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -1.68764486774,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.64878888879,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "df",
                174.975558762,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "t",
                -2.60122344403,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.95032668923,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.289091536055,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 7.5454734258),
            ("Estimates of Fixed Effects", "hours", "t", 6.74639844476),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                6.99719014325,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                540.149533573,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "Denominator df",
                174.975558762,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "F",
                6.76636340575,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                7.5454734258,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                45.5138919754,
            ),
        ],
    },
    Case {
        name: "random slope UN, ML",
        syntax: "MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(UN) /METHOD=ML /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Log Likelihood",
                "Value",
                1133.62745992,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1147.62745992,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1148.24284453,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1177.35662842,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1170.35662842,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                18.3176615017,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                1.98011845688,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                9.25079074841,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (1,1)",
                "Estimate",
                31.0798471285,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (1,1)",
                "Std. Error",
                17.9939008277,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (1,1)",
                "Wald Z",
                1.72724343799,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,1)",
                "Estimate",
                -2.75268276754,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,1)",
                "Std. Error",
                2.1346781778,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,1)",
                "Wald Z",
                -1.28950714734,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,2)",
                "Estimate",
                0.529043095684,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,2)",
                "Std. Error",
                0.320013545509,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / UN (2,2)",
                "Wald Z",
                1.65318969496,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                50.2513334882,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.99938081223,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                7.96495666996,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                25.1334479059,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -1.67711313753,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.646508776759,
            ),
            ("Estimates of Fixed Effects", "[girl=0]", "df", 176.28338895),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "t",
                -2.59410730035,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.94467918995,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.271239284434,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 8.3950674753),
            ("Estimates of Fixed Effects", "hours", "t", 7.16960743357),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                7.71596259852,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                619.924267379,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "Denominator df",
                176.28338895,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "F",
                6.72939268572,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                8.3950674753,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                51.4032707514,
            ),
        ],
    },
    Case {
        name: "random slope DIAG, REML",
        syntax: "MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(DIAG) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                1134.48434603,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1140.48434603,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1140.61549357,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1153.17767188,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1150.17767188,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                18.7247788641,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                2.06103214356,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                9.08514645081,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Var(1)",
                "Estimate",
                25.7110752871,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Var(1)",
                "Std. Error",
                15.9936052322,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Var(1)",
                "Wald Z",
                1.60758471363,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Var(2)",
                "Estimate",
                0.433979851096,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Var(2)",
                "Std. Error",
                0.282893151824,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Var(2)",
                "Wald Z",
                1.53407690606,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                50.4534473588,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.84438846931,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                7.31817065644,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                27.3551088603,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -1.69334279234,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.651997582835,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "df",
                172.413745222,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "t",
                -2.59716115047,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.91103088429,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.250805984563,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 7.85987287736),
            ("Estimates of Fixed Effects", "hours", "t", 7.61955855089),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                7.02328177089,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                737.237408332,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "Denominator df",
                172.413745222,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "F",
                6.74524604151,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                7.85987287736,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                58.0576725104,
            ),
        ],
    },
    Case {
        name: "random slope VC, ML",
        syntax: "MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(VC) /METHOD=ML /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Log Likelihood",
                "Value",
                1137.37964417,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1149.37964417,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1149.83866057,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1174.86178861,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1168.86178861,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                18.7057987293,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                2.06050794262,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                9.07824635975,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Estimate",
                21.0408106145,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Std. Error",
                12.718549954,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Wald Z",
                1.65434036825,
            ),
            (
                "Estimates of Covariance Parameters",
                "hours [subject = school] / Variance",
                "Estimate",
                0.353607955526,
            ),
            (
                "Estimates of Covariance Parameters",
                "hours [subject = school] / Variance",
                "Std. Error",
                0.225844681063,
            ),
            (
                "Estimates of Covariance Parameters",
                "hours [subject = school] / Variance",
                "Wald Z",
                1.56571301064,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                50.5144931412,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.69656083583,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                8.2868762109,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                29.7746429567,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -1.67726981948,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.650672831965,
            ),
            ("Estimates of Fixed Effects", "[girl=0]", "df", 173.01482014),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "t",
                -2.57774681388,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.900501832,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.23190793284,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 8.97521856355),
            ("Estimates of Fixed Effects", "hours", "t", 8.19507038302),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                7.88117502929,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                877.208182799,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "Denominator df",
                173.01482014,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "F",
                6.64477863648,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                8.97521856355,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                67.1591785826,
            ),
        ],
    },
    Case {
        name: "random slope ID, REML",
        syntax: "MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(ID) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                1154.57483084,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1158.57483084,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1158.64004823,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1167.03704808,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1165.03704808,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                19.1987726807,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                2.24816907411,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                8.53973702501,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Variance",
                "Estimate",
                8.89414422252,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Variance",
                "Std. Error",
                6.46270171102,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept + hours [subject = school] / Variance",
                "Wald Z",
                1.3762269435,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                50.5911593673,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.24203452367,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                9.31021627706,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                40.7324904447,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -1.79336312626,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.661634838699,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "df",
                156.413748463,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "t",
                -2.71050286557,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.90565203782,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                1.00170483194,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 3.90516042796),
            ("Estimates of Fixed Effects", "hours", "t", 1.90240875061),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                8.35810462838,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1672.08923346,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "Denominator df",
                156.413748463,
            ),
            (
                "Type III Tests of Fixed Effects",
                "girl",
                "F",
                7.34682578427,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                3.90516042796,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                3.61915905441,
            ),
        ],
    },
    Case {
        name: "nested subjects, REML",
        syntax: "MIXED mark BY track WITH hours /FIXED=track hours /RANDOM=INTERCEPT | SUBJECT(school) /RANDOM=INTERCEPT | SUBJECT(school*room) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                1130.07084173,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1136.07084173,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1136.20270986,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1148.74808175,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1145.74808175,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                19.2884711278,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                2.18044324067,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                8.84612392929,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Estimate",
                15.9294896787,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Std. Error",
                8.96710607641,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Wald Z",
                1.7764359586,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school * room] / Variance",
                "Estimate",
                1.72934465187,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school * room] / Variance",
                "Std. Error",
                1.64276353723,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school * room] / Variance",
                "Wald Z",
                1.05270455101,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                48.9977479715,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.57140236376,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                12.2872093987,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                31.1809050956,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Estimate",
                1.45826292937,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Std. Error",
                0.809836361183,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "df",
                175.544049098,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "t",
                1.80068838505,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Estimate",
                3.95507909214,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Std. Error",
                0.819131143397,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "df",
                176.971792889,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "t",
                4.82838324976,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.7158858241,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.114960007213,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 175.178344113),
            ("Estimates of Fixed Effects", "hours", "t", 14.9259369906),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                10.478831574,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1132.57293105,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "Denominator df",
                174.623634878,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "F",
                11.9371155814,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                175.178344113,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                222.783595049,
            ),
        ],
    },
    Case {
        name: "nested subjects, ML",
        syntax: "MIXED mark BY track WITH hours /FIXED=track hours /RANDOM=INTERCEPT | SUBJECT(school) /RANDOM=INTERCEPT | SUBJECT(school*room) /PRINT=SOLUTION TESTCOV /METHOD=ML.",
        expected: &[
            (
                "Information Criteria",
                "-2 Log Likelihood",
                "Value",
                1132.55325528,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1146.55325528,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1147.1686399,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1176.28242379,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1169.28242379,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                18.9539121228,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                2.12540949428,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                8.91776957515,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Estimate",
                13.9065897973,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Std. Error",
                7.49337474749,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Wald Z",
                1.85585137083,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school * room] / Variance",
                "Estimate",
                1.74405730094,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school * room] / Variance",
                "Std. Error",
                1.63034977984,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school * room] / Variance",
                "Wald Z",
                1.06974424906,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                48.997667185,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.49470488044,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                14.4119729963,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                32.7808304007,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Estimate",
                1.46980322807,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Std. Error",
                0.802921651161,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "df",
                178.513151532,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "t",
                1.8305686812,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Estimate",
                3.96526594822,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Std. Error",
                0.812135074612,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "df",
                180.003753618,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "t",
                4.88252025085,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.7156446271,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.113978451581,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 178.139172124),
            ("Estimates of Fixed Effects", "hours", "t", 15.0523594882),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                12.1183079872,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1261.35880936,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "Denominator df",
                177.582056188,
            ),
            (
                "Type III Tests of Fixed Effects",
                "track",
                "F",
                12.1980509725,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "Denominator df",
                178.139172124,
            ),
            (
                "Type III Tests of Fixed Effects",
                "hours",
                "F",
                226.573526161,
            ),
        ],
    },
    Case {
        name: "repeated DIAG, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(DIAG) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                501.310659321,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                509.310659321,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                509.727325987,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                523.771141388,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                519.771141388,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(1)",
                "Estimate",
                3.95832918241,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(1)",
                "Std. Error",
                1.10782136454,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(1)",
                "Wald Z",
                3.57307532522,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(2)",
                "Estimate",
                5.15025695953,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(2)",
                "Std. Error",
                1.44898772718,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(2)",
                "Wald Z",
                3.55438273419,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(3)",
                "Estimate",
                10.3982312385,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(3)",
                "Std. Error",
                3.0139824991,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(3)",
                "Wald Z",
                3.44999721851,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(4)",
                "Estimate",
                10.0965928303,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(4)",
                "Std. Error",
                2.78308292679,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / Var(4)",
                "Wald Z",
                3.62784476637,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                26.9241115048,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.01432656043,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                82.2894489314,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                26.5438297243,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Estimate",
                -1.69904266472,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Std. Error",
                0.492195304921,
            ),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 87.772727393),
            ("Estimates of Fixed Effects", "[arm=1]", "t", -3.45196845182),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Estimate",
                -4.59941606733,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Std. Error",
                0.714188560185,
            ),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 45.098908321),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "t",
                -6.44005844359,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Estimate",
                -2.98649941329,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Std. Error",
                0.74672605874,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "df",
                49.0092250441,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "t",
                -3.99945787124,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Estimate",
                -1.29576269351,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Std. Error",
                0.8812892957,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "df",
                48.9033301059,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "t",
                -1.47030345181,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Estimate",
                1.26129564714,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Std. Error",
                0.215383692999,
            ),
            ("Estimates of Fixed Effects", "dose", "df", 86.8029973325),
            ("Estimates of Fixed Effects", "dose", "t", 5.85604058311),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                92.3040905844,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1018.23852794,
            ),
            (
                "Type III Tests of Fixed Effects",
                "arm",
                "Denominator df",
                87.772727393,
            ),
            ("Type III Tests of Fixed Effects", "arm", "F", 11.9160861923),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "Denominator df",
                44.8205961168,
            ),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "F",
                16.2876026436,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "Denominator df",
                86.8029973325,
            ),
            ("Type III Tests of Fixed Effects", "dose", "F", 34.293211311),
        ],
    },
    Case {
        name: "repeated CS, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(CS) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                493.181848295,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                497.181848295,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                497.304297275,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                504.412089329,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                502.412089329,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS diagonal offset",
                "Estimate",
                4.52757261482,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS diagonal offset",
                "Std. Error",
                0.746545274278,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS diagonal offset",
                "Wald Z",
                6.06469931673,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS covariance",
                "Estimate",
                2.90727970454,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS covariance",
                "Std. Error",
                1.14351236536,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS covariance",
                "Wald Z",
                2.54241212655,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                26.8258173699,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.97474152494,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                96.9546961708,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                27.5209547183,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Estimate",
                -1.88429984094,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Std. Error",
                0.753934555847,
            ),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 28.3154815923),
            ("Estimates of Fixed Effects", "[arm=1]", "t", -2.49928833521),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Estimate",
                -4.49418296168,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Std. Error",
                0.583825137407,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "df",
                76.2355354942,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "t",
                -7.69782366966,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Estimate",
                -2.7863991256,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Std. Error",
                0.587391974705,
            ),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 76.64446882),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "t",
                -4.74367925609,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Estimate",
                -1.35154271077,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Std. Error",
                0.595549247102,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "df",
                76.5204685607,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "t",
                -2.26940545613,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Estimate",
                1.306124605,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Std. Error",
                0.205706882312,
            ),
            ("Estimates of Fixed Effects", "dose", "df", 90.2897878833),
            ("Estimates of Fixed Effects", "dose", "t", 6.34944533855),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                99.6655611655,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                965.265612671,
            ),
            (
                "Type III Tests of Fixed Effects",
                "arm",
                "Denominator df",
                28.3154815923,
            ),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.24644218254),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "Denominator df",
                75.8195322215,
            ),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "F",
                21.5574310546,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "Denominator df",
                90.2897878833,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "F",
                40.3154561072,
            ),
        ],
    },
    Case {
        name: "repeated CS, ML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(CS) /METHOD=ML /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Log Likelihood",
                "Value",
                494.466131081,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                510.466131081,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                511.935518836,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                539.848761756,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                531.848761756,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS diagonal offset",
                "Estimate",
                4.30237019034,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS diagonal offset",
                "Std. Error",
                0.692212738319,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS diagonal offset",
                "Wald Z",
                6.21538719554,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS covariance",
                "Estimate",
                2.66724211172,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS covariance",
                "Std. Error",
                1.02588435627,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / CS covariance",
                "Wald Z",
                2.59994422902,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                26.8212094108,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.946011554347,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                103.375372889,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                28.3518835341,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Estimate",
                -1.88248191619,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Std. Error",
                0.726170416904,
            ),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 30.3727472407),
            ("Estimates of Fixed Effects", "[arm=1]", "t", -2.59234178695),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Estimate",
                -4.49508552962,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Std. Error",
                0.568993433186,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "df",
                80.1203415023,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "t",
                -7.90006574321,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Estimate",
                -2.78810849814,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Std. Error",
                0.572450142847,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "df",
                80.5591562691,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "t",
                -4.87048266644,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Estimate",
                -1.3506962232,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Std. Error",
                0.580405725983,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "df",
                80.4344176169,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "t",
                -2.32715868009,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Estimate",
                1.30734044694,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Std. Error",
                0.200233178016,
            ),
            ("Estimates of Fixed Effects", "dose", "df", 95.3624390899),
            ("Estimates of Fixed Effects", "dose", "t", 6.52909003339),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                105.919130015,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1023.45533195,
            ),
            (
                "Type III Tests of Fixed Effects",
                "arm",
                "Denominator df",
                30.3727472407,
            ),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.72023594037),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "Denominator df",
                79.6862787606,
            ),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "F",
                22.7113272215,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "Denominator df",
                95.3624390899,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "F",
                42.6290166641,
            ),
        ],
    },
    Case {
        name: "repeated AR1, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(AR1) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                477.45506756,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                481.45506756,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                481.577516539,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                488.685308593,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                486.685308593,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 diagonal",
                "Estimate",
                7.56466719346,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 diagonal",
                "Std. Error",
                1.37111479567,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 diagonal",
                "Wald Z",
                5.51716546081,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 rho",
                "Estimate",
                0.625259498732,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 rho",
                "Std. Error",
                0.0794001857836,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 rho",
                "Wald Z",
                7.87478634415,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                27.3605666164,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.899746921342,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                94.6970810387,
            ),
            ("Estimates of Fixed Effects", "Intercept", "t", 30.40918059),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Estimate",
                -1.99949751308,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Std. Error",
                0.788307085192,
            ),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 30.889332783),
            ("Estimates of Fixed Effects", "[arm=1]", "t", -2.53644493451),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Estimate",
                -4.58009566386,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Std. Error",
                0.644616201243,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "df",
                100.554206273,
            ),
            ("Estimates of Fixed Effects", "[wave=1]", "t", -7.1051513366),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Estimate",
                -2.7855429088,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Std. Error",
                0.583653558145,
            ),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 90.344989398),
            ("Estimates of Fixed Effects", "[wave=2]", "t", -4.7725964657),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Estimate",
                -1.44776955749,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Std. Error",
                0.476092825806,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "df",
                69.8348626339,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "t",
                -3.04093966347,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Estimate",
                1.16610349355,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Std. Error",
                0.173091130896,
            ),
            ("Estimates of Fixed Effects", "dose", "df", 74.3734017885),
            ("Estimates of Fixed Effects", "dose", "t", 6.73693381929),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                99.785839835,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1243.87928658,
            ),
            (
                "Type III Tests of Fixed Effects",
                "arm",
                "Denominator df",
                30.889332783,
            ),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.43355290582),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "Denominator df",
                70.1424411147,
            ),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "F",
                16.9174011834,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "Denominator df",
                74.3734017885,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "F",
                45.3862772855,
            ),
        ],
    },
    Case {
        name: "repeated AR1, ML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(AR1) /METHOD=ML /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Log Likelihood",
                "Value",
                477.788170498,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                493.788170498,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                495.257558253,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                523.170801173,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                515.170801173,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 diagonal",
                "Estimate",
                7.10287651133,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 diagonal",
                "Std. Error",
                1.24260308126,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 diagonal",
                "Wald Z",
                5.71612658816,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 rho",
                "Estimate",
                0.621549033355,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 rho",
                "Std. Error",
                0.0775522586528,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / AR1 rho",
                "Wald Z",
                8.01458325202,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                27.3561660324,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.873435640289,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                100.934109359,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                31.3201852209,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Estimate",
                -1.99838219987,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Std. Error",
                0.762124521167,
            ),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 33.0356290534),
            ("Estimates of Fixed Effects", "[arm=1]", "t", -2.62212032859),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Estimate",
                -4.57995194371,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Std. Error",
                0.626421835308,
            ),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 106.48574619),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "t",
                -7.31129038862,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Estimate",
                -2.78572654203,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Std. Error",
                0.567696069544,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "df",
                95.4743281169,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "t",
                -4.90707385779,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Estimate",
                -1.44727397018,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Std. Error",
                0.463571066178,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "df",
                73.5524022854,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "t",
                -3.12201100494,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Estimate",
                1.16727558298,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Std. Error",
                0.168499453732,
            ),
            ("Estimates of Fixed Effects", "dose", "df", 78.5242269183),
            ("Estimates of Fixed Effects", "dose", "t", 6.92747398951),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                105.985161614,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                1318.26568908,
            ),
            (
                "Type III Tests of Fixed Effects",
                "arm",
                "Denominator df",
                33.0356290534,
            ),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.87551501759),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "Denominator df",
                73.9588650348,
            ),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "F",
                17.9125850326,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "Denominator df",
                78.5242269183,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "F",
                47.9898958754,
            ),
        ],
    },
    Case {
        name: "repeated UN, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(UN) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                450.628608857,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                470.628608857,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                473.073053301,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                506.779814025,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                496.779814025,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (1,1)",
                "Estimate",
                3.84288938026,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (1,1)",
                "Std. Error",
                1.0507402582,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (1,1)",
                "Wald Z",
                3.65731621138,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,1)",
                "Estimate",
                3.73483414049,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,1)",
                "Std. Error",
                1.14082230829,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,1)",
                "Wald Z",
                3.27380882489,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,2)",
                "Estimate",
                5.46010705248,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,2)",
                "Std. Error",
                1.53454472853,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,2)",
                "Wald Z",
                3.55812831712,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,1)",
                "Estimate",
                2.50806781379,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,1)",
                "Std. Error",
                1.27364377527,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,1)",
                "Wald Z",
                1.96920666712,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,2)",
                "Estimate",
                3.45215525613,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,2)",
                "Std. Error",
                1.64645203707,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,2)",
                "Wald Z",
                2.09672385129,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,3)",
                "Estimate",
                9.73162215162,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,3)",
                "Std. Error",
                2.66972040581,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,3)",
                "Wald Z",
                3.64518401644,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,1)",
                "Estimate",
                1.57877975552,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,1)",
                "Std. Error",
                1.22166216382,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,1)",
                "Wald Z",
                1.29232107064,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,2)",
                "Estimate",
                0.651912552195,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,2)",
                "Std. Error",
                1.4614818497,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,2)",
                "Wald Z",
                0.446062708427,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,3)",
                "Estimate",
                6.58550106428,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,3)",
                "Std. Error",
                2.44620116228,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,3)",
                "Wald Z",
                2.69213389554,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,4)",
                "Estimate",
                10.2292624047,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,4)",
                "Std. Error",
                2.79075050061,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,4)",
                "Wald Z",
                3.66541631094,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                27.1757818563,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.798734719169,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                53.6230661171,
            ),
            ("Estimates of Fixed Effects", "Intercept", "t", 34.023538985),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Estimate",
                -1.77781801976,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Std. Error",
                0.680012725872,
            ),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 28.427914631),
            ("Estimates of Fixed Effects", "[arm=1]", "t", -2.61438933731),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Estimate",
                -4.62481665831,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Std. Error",
                0.618830750208,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "df",
                28.7380340023,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "t",
                -7.47347583609,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Estimate",
                -2.87474537238,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Std. Error",
                0.708722265172,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "df",
                30.4223896217,
            ),
            ("Estimates of Fixed Effects", "[wave=2]", "t", -4.0562368556),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Estimate",
                -1.55533862858,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Std. Error",
                0.515952530316,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "df",
                20.1905428619,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "t",
                -3.01449946883,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Estimate",
                1.2049844511,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Std. Error",
                0.112669199125,
            ),
            ("Estimates of Fixed Effects", "dose", "df", 35.7898402407),
            ("Estimates of Fixed Effects", "dose", "t", 10.6948878705),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                57.7367068799,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                2117.91500049,
            ),
            (
                "Type III Tests of Fixed Effects",
                "arm",
                "Denominator df",
                28.427914631,
            ),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.83503160706),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "Denominator df",
                25.6937892128,
            ),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "F",
                41.7238084139,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "Denominator df",
                35.7898402407,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "F",
                114.380626562,
            ),
        ],
    },
    Case {
        name: "repeated UN, ML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(UN) /METHOD=ML /PRINT=SOLUTION TESTCOV.",
        expected: &[
            (
                "Information Criteria",
                "-2 Log Likelihood",
                "Value",
                448.627597748,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                480.627597748,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                486.672042192,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                539.392859099,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                523.392859099,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (1,1)",
                "Estimate",
                3.58192365934,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (1,1)",
                "Std. Error",
                0.94962666118,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (1,1)",
                "Wald Z",
                3.77192828062,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,1)",
                "Estimate",
                3.49611540863,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,1)",
                "Std. Error",
                1.03786076731,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,1)",
                "Wald Z",
                3.36857844401,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,2)",
                "Estimate",
                5.13571371246,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,2)",
                "Std. Error",
                1.40917396945,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (2,2)",
                "Wald Z",
                3.64448522594,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,1)",
                "Estimate",
                2.31442858534,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,1)",
                "Std. Error",
                1.16343750889,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,1)",
                "Wald Z",
                1.98930201893,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,2)",
                "Estimate",
                3.22125161388,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,2)",
                "Std. Error",
                1.51756326372,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,2)",
                "Wald Z",
                2.12264733266,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,3)",
                "Estimate",
                9.23145263119,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,3)",
                "Std. Error",
                2.48032367501,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (3,3)",
                "Wald Z",
                3.72187417482,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,1)",
                "Estimate",
                1.4198475088,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,1)",
                "Std. Error",
                1.11755675022,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,1)",
                "Wald Z",
                1.27049253519,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,2)",
                "Estimate",
                0.511293577689,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,2)",
                "Std. Error",
                1.34932133489,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,2)",
                "Wald Z",
                0.378926475457,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,3)",
                "Estimate",
                6.22488530707,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,3)",
                "Std. Error",
                2.27828887683,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,3)",
                "Wald Z",
                2.7322633975,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,4)",
                "Estimate",
                9.73471704269,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,4)",
                "Std. Error",
                2.60328823126,
            ),
            (
                "Estimates of Covariance Parameters",
                "Repeated Measures / UN (4,4)",
                "Wald Z",
                3.73939271334,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                27.173821579,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                0.775319217776,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                56.1498745278,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                35.0485592979,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Estimate",
                -1.77612524101,
            ),
            (
                "Estimates of Fixed Effects",
                "[arm=1]",
                "Std. Error",
                0.655845512012,
            ),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 30.4792337702),
            ("Estimates of Fixed Effects", "[arm=1]", "t", -2.70814575762),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Estimate",
                -4.62706997898,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "Std. Error",
                0.605963421424,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "df",
                29.8160016533,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=1]",
                "t",
                -7.63588991578,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Estimate",
                -2.87631817898,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "Std. Error",
                0.694953207523,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "df",
                31.4720735333,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=2]",
                "t",
                -4.13886596658,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Estimate",
                -1.55660722827,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "Std. Error",
                0.504961843386,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "df",
                20.9441050029,
            ),
            (
                "Estimates of Fixed Effects",
                "[wave=3]",
                "t",
                -3.08262346681,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Estimate",
                1.20590546268,
            ),
            (
                "Estimates of Fixed Effects",
                "dose",
                "Std. Error",
                0.108706259266,
            ),
            ("Estimates of Fixed Effects", "dose", "df", 37.933582621),
            ("Estimates of Fixed Effects", "dose", "t", 11.0932477193),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "Denominator df",
                61.1368941857,
            ),
            (
                "Type III Tests of Fixed Effects",
                "Intercept",
                "F",
                2267.65828073,
            ),
            (
                "Type III Tests of Fixed Effects",
                "arm",
                "Denominator df",
                30.4792337702,
            ),
            ("Type III Tests of Fixed Effects", "arm", "F", 7.33405344449),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "Denominator df",
                26.9747997912,
            ),
            (
                "Type III Tests of Fixed Effects",
                "wave",
                "F",
                44.7082042848,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "Denominator df",
                37.933582621,
            ),
            (
                "Type III Tests of Fixed Effects",
                "dose",
                "F",
                123.060144961,
            ),
        ],
    },
    Case {
        name: "regression weights, REML",
        syntax: "COMPUTE w = 1 + MOD(pupil, 3).\nMIXED mark BY girl track WITH hours /FIXED=girl track hours /RANDOM=INTERCEPT | SUBJECT(school) /PRINT=SOLUTION TESTCOV /REGWGT=w.",
        expected: &[
            (
                "Information Criteria",
                "-2 Restricted Log Likelihood",
                "Value",
                1143.63148041,
            ),
            (
                "Information Criteria",
                "Akaike's Information Criterion (AIC)",
                "Value",
                1147.63148041,
            ),
            (
                "Information Criteria",
                "Hurvich and Tsai's Criterion (AICC)",
                "Value",
                1147.69741448,
            ),
            (
                "Information Criteria",
                "Bozdogan's Criterion (CAIC)",
                "Value",
                1156.07219206,
            ),
            (
                "Information Criteria",
                "Schwarz's Bayesian Criterion (BIC)",
                "Value",
                1154.07219206,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Estimate",
                40.0745815327,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Std. Error",
                4.26243438988,
            ),
            (
                "Estimates of Covariance Parameters",
                "Residual",
                "Wald Z",
                9.40180607305,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Estimate",
                15.5122485254,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Std. Error",
                8.39958747402,
            ),
            (
                "Estimates of Covariance Parameters",
                "Intercept [subject = school] / Variance",
                "Wald Z",
                1.84678694917,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Estimate",
                49.7850748666,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "Std. Error",
                1.57072508257,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "df",
                13.8936419356,
            ),
            (
                "Estimates of Fixed Effects",
                "Intercept",
                "t",
                31.6956005981,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Estimate",
                -0.993538678896,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "Std. Error",
                0.670170342219,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "df",
                177.751883817,
            ),
            (
                "Estimates of Fixed Effects",
                "[girl=0]",
                "t",
                -1.48251663242,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Estimate",
                1.14667293573,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "Std. Error",
                0.827355514983,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "df",
                177.654254991,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=arts]",
                "t",
                1.38594946787,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Estimate",
                3.61815142182,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "Std. Error",
                0.812179580481,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "df",
                178.154089366,
            ),
            (
                "Estimates of Fixed Effects",
                "[track=sci]",
                "t",
                4.45486627437,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Estimate",
                1.71165299202,
            ),
            (
                "Estimates of Fixed Effects",
                "hours",
                "Std. Error",
                0.114953600984,
            ),
            ("Estimates of Fixed Effects", "hours", "df", 177.694424759),
            ("Estimates of Fixed Effects", "hours", "t", 14.8899467034),
        ],
    },
    Case {
        name: "repeated CSH, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(CSH) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 481.682291719),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 504.757894303),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Estimate", 3.71472090183),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Std. Error", 0.997185412938),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Estimate", 5.26208343521),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Std. Error", 1.47728809633),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Estimate", 9.60534695139),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Std. Error", 2.62623108213),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Estimate", 11.6088865388),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Std. Error", 3.33123796745),
            ("Estimates of Covariance Parameters", "Repeated Measures / CSH rho", "Estimate", 0.42630691606),
            ("Estimates of Covariance Parameters", "Repeated Measures / CSH rho", "Std. Error", 0.106026541623),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.1580765388),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.983195641122),
            ("Estimates of Fixed Effects", "Intercept", "df", 74.2839825892),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.71832086349),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.664429442626),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 32.0249560183),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.53769834217),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.598144912241),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 36.4521705098),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.85288118218),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.617805488011),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 43.3629168348),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.35212841692),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.68959683124),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 46.4863519222),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.18188490434),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.180966843605),
            ("Estimates of Fixed Effects", "dose", "df", 73.0611872998),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 93.332072965),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1192.29089351),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 32.0249560183),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.68822367133),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 36.4255483602),
            ("Type III Tests of Fixed Effects", "wave", "F", 23.071766246),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 73.0611872998),
            ("Type III Tests of Fixed Effects", "dose", "F", 42.6532716039),
        ],
    },
    Case {
        name: "repeated CSR, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(CSR) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 493.181848295),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 502.412089329),
            ("Estimates of Covariance Parameters", "Repeated Measures / CSR diagonal", "Estimate", 7.43485230869),
            ("Estimates of Covariance Parameters", "Repeated Measures / CSR diagonal", "Std. Error", 1.24709507533),
            ("Estimates of Covariance Parameters", "Repeated Measures / CSR rho", "Estimate", 0.391033953146),
            ("Estimates of Covariance Parameters", "Repeated Measures / CSR rho", "Std. Error", 0.107928862388),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 26.8258173691),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.97474152466),
            ("Estimates of Fixed Effects", "Intercept", "df", 96.9551460093),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.88429984062),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.75393455461),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 28.3151317007),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.49418296184),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.583825137678),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 76.2349872937),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.78639912591),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.587391974974),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 76.6439283223),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.35154271062),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.595549247375),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 76.5199257126),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.30612460521),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.205706882362),
            ("Estimates of Fixed Effects", "dose", "df", 90.2896028667),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 99.6660222378),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 965.265612999),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 28.3151317007),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.24644220091),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 75.8189764461),
            ("Type III Tests of Fixed Effects", "wave", "F", 21.5574310372),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 90.2896028667),
            ("Type III Tests of Fixed Effects", "dose", "F", 40.3154561007),
        ],
    },
    Case {
        name: "repeated ARH1, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(ARH1) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 463.460577606),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 486.53618019),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Estimate", 3.40784712182),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Std. Error", 0.873114418462),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Estimate", 5.62031629186),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Std. Error", 1.57870678992),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Estimate", 11.1000330123),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Std. Error", 3.11850127447),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Estimate", 11.5239616765),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Std. Error", 3.18439014175),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARH1 rho", "Estimate", 0.66767257025),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARH1 rho", "Std. Error", 0.0760060685275),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.5252090157),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.903845428531),
            ("Estimates of Fixed Effects", "Intercept", "df", 68.7846356962),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.83064978071),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.673632290213),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 33.9214099806),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.60059624228),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.632121014459),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 43.2125637297),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.84011009109),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.602906696489),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 48.1255827015),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.44666711114),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.548124238695),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 44.7569359262),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.09292205218),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.14755030244),
            ("Estimates of Fixed Effects", "dose", "df", 60.9813676735),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 88.9429325478),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1505.9481413),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 33.9214099806),
            ("Type III Tests of Fixed Effects", "arm", "F", 7.38524219787),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 33.0467586883),
            ("Type III Tests of Fixed Effects", "wave", "F", 20.0044161828),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 60.9813676735),
            ("Type III Tests of Fixed Effects", "dose", "F", 54.8653519925),
        ],
    },
    Case {
        name: "repeated ARMA11, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(ARMA11) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 473.681661788),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 487.527023339),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARMA11 diagonal", "Estimate", 7.42851567984),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARMA11 diagonal", "Std. Error", 1.26450660124),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARMA11 rho", "Estimate", 0.312343393986),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARMA11 rho", "Std. Error", 0.221306914992),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARMA11 gamma", "Estimate", 0.616112350623),
            ("Estimates of Covariance Parameters", "Repeated Measures / ARMA11 gamma", "Std. Error", 0.0729614515212),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.3307511044),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.836640080298),
            ("Estimates of Fixed Effects", "Intercept", "df", 78.5154828778),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -2.03884702858),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.720720196516),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 31.5042806164),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.63614926086),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.705125311529),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 77.9816424196),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.77129809752),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.654862834422),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 52.1051794421),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.47089360615),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.470795795087),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 69.5195991091),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.1868319486),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.153995273974),
            ("Estimates of Fixed Effects", "dose", "df", 44.7356419647),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 62.9905075535),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1516.44852355),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 31.5042806164),
            ("Type III Tests of Fixed Effects", "arm", "F", 8.00268827791),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 36.9179576889),
            ("Type III Tests of Fixed Effects", "wave", "F", 15.7820093838),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 44.7356419647),
            ("Type III Tests of Fixed Effects", "dose", "F", 59.3968853069),
        ],
    },
    Case {
        name: "repeated TP, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(TP) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 471.605295846),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 490.065777913),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP diagonal", "Estimate", 7.46985322018),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP diagonal", "Std. Error", 1.3113846725),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP rho(1)", "Estimate", 0.619555153188),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP rho(1)", "Std. Error", 0.0757699621342),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP rho(2)", "Estimate", 0.246821337557),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP rho(2)", "Std. Error", 0.146746736647),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP rho(3)", "Estimate", 0.311534700264),
            ("Estimates of Covariance Parameters", "Repeated Measures / TP rho(3)", "Std. Error", 0.172349110524),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.1014287098),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.828427005596),
            ("Estimates of Fixed Effects", "Intercept", "df", 78.4374369247),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -2.01779759086),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.768373222042),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 28.4293444239),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.61089903732),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.609384133547),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 33.1270921335),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.75392192611),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.634079233508),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 58.5887234577),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.48837984483),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.467894775331),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 67.3574862827),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.25296458405),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.146380113364),
            ("Estimates of Fixed Effects", "dose", "df", 40.5073584209),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 65.9433786101),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1538.69367053),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 28.4293444239),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.89621441436),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 37.9904715),
            ("Type III Tests of Fixed Effects", "wave", "F", 22.4705178222),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 40.5073584209),
            ("Type III Tests of Fixed Effects", "dose", "F", 73.267847486),
        ],
    },
    Case {
        name: "repeated TPH, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(TPH) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 457.660102824),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 489.965946442),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Estimate", 3.41225895088),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Std. Error", 0.857623655523),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Estimate", 5.5254086624),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Std. Error", 1.5037965887),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Estimate", 11.8872427181),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Std. Error", 3.28759291255),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Estimate", 10.2876089225),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Std. Error", 2.66047754698),
            ("Estimates of Covariance Parameters", "Repeated Measures / TPH rho(1)", "Estimate", 0.661203598163),
            ("Estimates of Covariance Parameters", "Repeated Measures / TPH rho(1)", "Std. Error", 0.0746469684702),
            ("Estimates of Covariance Parameters", "Repeated Measures / TPH rho(2)", "Estimate", 0.297740285931),
            ("Estimates of Covariance Parameters", "Repeated Measures / TPH rho(2)", "Std. Error", 0.145579318838),
            ("Estimates of Covariance Parameters", "Repeated Measures / TPH rho(3)", "Estimate", 0.302831448637),
            ("Estimates of Covariance Parameters", "Repeated Measures / TPH rho(3)", "Std. Error", 0.1618642625),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.2531371928),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.826763584252),
            ("Estimates of Fixed Effects", "Intercept", "df", 58.7443484958),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.83713387987),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.661896638408),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 32.0205943245),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.6379517257),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.598037342926),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 33.81424427),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.83199628319),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.633470673788),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 47.5964955218),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.49590598449),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.539606388475),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 42.9432323443),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.18764314866),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.125467730802),
            ("Estimates of Fixed Effects", "dose", "df", 37.0550562737),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 63.100171238),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1815.5049523),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 32.0205943245),
            ("Type III Tests of Fixed Effects", "arm", "F", 7.70373370082),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 29.7892346134),
            ("Type III Tests of Fixed Effects", "wave", "F", 29.2265765373),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 37.0550562737),
            ("Type III Tests of Fixed Effects", "dose", "F", 89.5999667525),
        ],
    },
    Case {
        name: "repeated UNR, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(UNR) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 450.628608857),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 496.779814025),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Estimate", 3.84288938832),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Std. Error", 1.05078205115),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Estimate", 5.46010706074),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Std. Error", 1.53458719718),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Estimate", 9.73162217153),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Std. Error", 2.66970661201),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Estimate", 10.2292624125),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Std. Error", 2.79071658226),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(2,1)", "Estimate", 0.815345727684),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(2,1)", "Std. Error", 0.0652560855434),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(3,1)", "Estimate", 0.410126389585),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(3,1)", "Std. Error", 0.162080071709),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(3,2)", "Estimate", 0.473584110462),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(3,2)", "Std. Error", 0.166164047472),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,1)", "Estimate", 0.251808537424),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,1)", "Std. Error", 0.176780727613),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,2)", "Estimate", 0.087230150592),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,2)", "Std. Error", 0.192270292891),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,3)", "Estimate", 0.660045760982),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,3)", "Std. Error", 0.131231652361),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.1757818582),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.79873471972),
            ("Estimates of Fixed Effects", "Intercept", "df", 53.6240906437),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.77781802058),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.680012727008),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 28.4247759143),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.62481665827),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.618830749937),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 28.7371767288),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.87474537208),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.708722264286),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 30.4209813406),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.5553386284),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.515952529808),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 20.1905773195),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.20498445061),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.112669199162),
            ("Estimates of Fixed Effects", "dose", "df", 35.7899026001),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 57.7348565706),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 2117.91499628),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 28.4247759143),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.83503159058),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 25.693566013),
            ("Type III Tests of Fixed Effects", "wave", "F", 41.7238083456),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 35.7899026001),
            ("Type III Tests of Fixed Effects", "dose", "F", 114.380626393),
        ],
    },
    Case {
        name: "repeated AD1, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(AD1) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 456.965594693),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 489.27143831),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Estimate", 3.79994901288),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Std. Error", 1.03745112727),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Estimate", 5.30335766146),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Std. Error", 1.47995896399),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Estimate", 10.1579400106),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Std. Error", 2.85211508326),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Estimate", 10.9891768865),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Std. Error", 3.06710998842),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(2,1)", "Estimate", 0.812263719701),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(2,1)", "Std. Error", 0.0668433788457),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(3,2)", "Estimate", 0.424239201197),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(3,2)", "Std. Error", 0.180017676121),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,3)", "Estimate", 0.706769136189),
            ("Estimates of Covariance Parameters", "Repeated Measures / Corr(4,3)", "Std. Error", 0.115831876508),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.5438314411),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.855573090386),
            ("Estimates of Fixed Effects", "Intercept", "df", 59.1021617701),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.91470336952),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.690051842639),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 31.1361210564),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.59709208378),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.640789730215),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 41.4953276461),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.86829028676),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.646516998938),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 41.8786283739),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.53981688445),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.505335841589),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 20.1040508431),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.1002129741),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.128199480001),
            ("Estimates of Fixed Effects", "dose", "df", 43.2599267855),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 68.0728192432),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1845.52235709),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 31.1361210564),
            ("Type III Tests of Fixed Effects", "arm", "F", 7.69909295859),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 29.0820482662),
            ("Type III Tests of Fixed Effects", "wave", "F", 26.4325278131),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 43.2599267855),
            ("Type III Tests of Fixed Effects", "dose", "F", 73.6513983455),
        ],
    },
    Case {
        name: "repeated HF, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(HF) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 487.663014477),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 510.738617062),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Estimate", 4.93310584993),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(1)", "Std. Error", 2.57122691218),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Estimate", 7.44834224656),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(2)", "Std. Error", 3.80298167638),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Estimate", 10.4509691159),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(3)", "Std. Error", 2.53421994875),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Estimate", 7.32469494297),
            ("Estimates of Covariance Parameters", "Repeated Measures / Var(4)", "Std. Error", 2.04887853259),
            ("Estimates of Covariance Parameters", "Repeated Measures / HF lambda", "Estimate", 4.48242018738),
            ("Estimates of Covariance Parameters", "Repeated Measures / HF lambda", "Std. Error", 0.735033586242),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.0159760707),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.950282761592),
            ("Estimates of Fixed Effects", "Intercept", "df", 68.8972060902),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.71088568956),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.690472550401),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 28.8211243686),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.53061146304),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.574714703235),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 76.188388232),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.81486939173),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.582874401236),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 77.5612117923),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.43253573407),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.596610990087),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 75.7398410976),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.22534748352),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.200518186536),
            ("Estimates of Fixed Effects", "dose", "df", 84.0835915869),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 98.5312701879),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1015.42250646),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 28.8211243686),
            ("Type III Tests of Fixed Effects", "arm", "F", 6.13972828159),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 76.3478273405),
            ("Type III Tests of Fixed Effects", "wave", "F", 22.4663684415),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 84.0835915869),
            ("Type III Tests of Fixed Effects", "dose", "F", 37.3431535086),
        ],
    },
    Case {
        name: "repeated FA1, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(FA1) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 483.692449838),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 506.768052422),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(1)", "Estimate", 0.86726644247),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(1)", "Std. Error", 0.443235209644),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(2)", "Estimate", 0.739755285493),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(2)", "Std. Error", 0.524232453598),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(3)", "Estimate", 2.68457809854),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(3)", "Std. Error", 0.566858950727),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(4)", "Estimate", 2.5782892383),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 loading(4)", "Std. Error", 0.583627385103),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 diagonal offset", "Estimate", 3.7538685139),
            ("Estimates of Covariance Parameters", "Repeated Measures / FA1 diagonal offset", "Std. Error", 0.626819945576),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.168869896),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.974263032771),
            ("Estimates of Fixed Effects", "Intercept", "df", 77.0127515368),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.65561424819),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.605886727365),
            ("Estimates of Fixed Effects", "[arm=1]", "df", 26.373282218),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.54560274049),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.616680040208),
            ("Estimates of Fixed Effects", "[wave=1]", "df", 35.1839438768),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.91916479194),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.63134551346),
            ("Estimates of Fixed Effects", "[wave=2]", "df", 29.1291466225),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.53622524837),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.556571246761),
            ("Estimates of Fixed Effects", "[wave=3]", "df", 74.5442414849),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.17044690087),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.19215051199),
            ("Estimates of Fixed Effects", "dose", "df", 83.6125076279),
            ("Type III Tests of Fixed Effects", "Intercept", "Denominator df", 93.3495877678),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1110.2711279),
            ("Type III Tests of Fixed Effects", "arm", "Denominator df", 26.373282218),
            ("Type III Tests of Fixed Effects", "arm", "F", 7.46681571391),
            ("Type III Tests of Fixed Effects", "wave", "Denominator df", 42.7720377472),
            ("Type III Tests of Fixed Effects", "wave", "F", 18.127115574),
            ("Type III Tests of Fixed Effects", "dose", "Denominator df", 83.6125076279),
            ("Type III Tests of Fixed Effects", "dose", "F", 37.103966578),
        ],
    },
    Case {
        name: "repeated FAH1, REML",
        syntax: "MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(FAH1) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 468.206697508),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 505.127661643),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 loading(1)", "Estimate", 1.61991554452),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 loading(2)", "Estimate", 2.2588493467),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 loading(3)", "Estimate", 1.80890795728),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 loading(4)", "Estimate", 0.319117836579),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 diagonal offset(1)", "Estimate", 1.16750867263),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 diagonal offset(2)", "Estimate", 0.262396255613),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 diagonal offset(3)", "Estimate", 7.16130919805),
            ("Estimates of Covariance Parameters", "Repeated Measures / FAH1 diagonal offset(4)", "Estimate", 10.2981829594),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 27.4516572353),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 0.843707791044),
            ("Estimates of Fixed Effects", "[arm=1]", "Estimate", -1.83294808759),
            ("Estimates of Fixed Effects", "[arm=1]", "Std. Error", 0.647276926593),
            ("Estimates of Fixed Effects", "[wave=1]", "Estimate", -4.61637294669),
            ("Estimates of Fixed Effects", "[wave=1]", "Std. Error", 0.687163959786),
            ("Estimates of Fixed Effects", "[wave=2]", "Estimate", -2.88095909408),
            ("Estimates of Fixed Effects", "[wave=2]", "Std. Error", 0.718292559269),
            ("Estimates of Fixed Effects", "[wave=3]", "Estimate", -1.25667886778),
            ("Estimates of Fixed Effects", "[wave=3]", "Std. Error", 0.856233454351),
            ("Estimates of Fixed Effects", "dose", "Estimate", 1.1232558584),
            ("Estimates of Fixed Effects", "dose", "Std. Error", 0.131762970979),
            ("Type III Tests of Fixed Effects", "Intercept", "F", 1968.9619671),
            ("Type III Tests of Fixed Effects", "arm", "F", 8.01899750153),
            ("Type III Tests of Fixed Effects", "wave", "F", 31.3135970789),
            ("Type III Tests of Fixed Effects", "dose", "F", 72.6725881409),
        ],
    },
    Case {
        name: "random slope CSH, REML",
        syntax: "MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(CSH) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 1130.79173042),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 1151.71616489),
            ("Estimates of Covariance Parameters", "Residual", "Estimate", 18.4131768555),
            ("Estimates of Covariance Parameters", "Residual", "Std. Error", 1.99471445683),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / Var(1)", "Estimate", 36.203082033),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / Var(1)", "Std. Error", 21.6618702157),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / Var(2)", "Estimate", 0.617476222446),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / Var(2)", "Std. Error", 0.384190074733),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / CSH rho", "Estimate", -0.688255511909),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / CSH rho", "Std. Error", 0.221363425729),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 50.2180598688),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 2.13816595008),
            ("Estimates of Fixed Effects", "Intercept", "df", 7.19280549744),
            ("Estimates of Fixed Effects", "[girl=0]", "Estimate", -1.6876448698),
            ("Estimates of Fixed Effects", "[girl=0]", "Std. Error", 0.648788888982),
            ("Estimates of Fixed Effects", "[girl=0]", "df", 174.97564453),
            ("Estimates of Fixed Effects", "hours", "Estimate", 1.95032669057),
            ("Estimates of Fixed Effects", "hours", "Std. Error", 0.289091540561),
            ("Estimates of Fixed Effects", "hours", "df", 7.54333418928),
            ("Type III Tests of Fixed Effects", "girl", "Denominator df", 174.97564453),
            ("Type III Tests of Fixed Effects", "girl", "F", 6.76636341827),
            ("Type III Tests of Fixed Effects", "hours", "Denominator df", 7.54333418928),
            ("Type III Tests of Fixed Effects", "hours", "F", 45.5138906187),
        ],
    },
    Case {
        name: "random slope FA1, REML",
        syntax: "MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(FA1) /PRINT=SOLUTION TESTCOV.",
        expected: &[
            ("Information Criteria", "-2 Restricted Log Likelihood", "Value", 1130.79173042),
            ("Information Criteria", "Schwarz's Bayesian Criterion (BIC)", "Value", 1151.71616489),
            ("Estimates of Covariance Parameters", "Residual", "Estimate", 18.4131768614),
            ("Estimates of Covariance Parameters", "Residual", "Std. Error", 1.99471788393),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / FA1 loading(1)", "Estimate", 5.99005250847),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / FA1 loading(1)", "Std. Error", 1.80834615655),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / FA1 loading(2)", "Estimate", -0.543252773763),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / FA1 loading(2)", "Std. Error", 0.303944915003),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / FA1 diagonal offset", "Estimate", 0.322352632837),
            ("Estimates of Covariance Parameters", "Intercept + hours [subject = school] / FA1 diagonal offset", "Std. Error", 0.179327707643),
            ("Estimates of Fixed Effects", "Intercept", "Estimate", 50.2180598724),
            ("Estimates of Fixed Effects", "Intercept", "Std. Error", 2.13816594108),
            ("Estimates of Fixed Effects", "Intercept", "df", 7.19098573791),
            ("Estimates of Fixed Effects", "[girl=0]", "Estimate", -1.68764486809),
            ("Estimates of Fixed Effects", "[girl=0]", "Std. Error", 0.648788889),
            ("Estimates of Fixed Effects", "[girl=0]", "df", 174.975095459),
            ("Estimates of Fixed Effects", "hours", "Estimate", 1.95032668983),
            ("Estimates of Fixed Effects", "hours", "Std. Error", 0.289091537969),
            ("Estimates of Fixed Effects", "hours", "df", 7.54120463491),
            ("Type III Tests of Fixed Effects", "girl", "Denominator df", 174.975095459),
            ("Type III Tests of Fixed Effects", "girl", "F", 6.76636340413),
            ("Type III Tests of Fixed Effects", "hours", "Denominator df", 7.54120463491),
            ("Type III Tests of Fixed Effects", "hours", "F", 45.5138914009),
        ],
    },
];

/// Estimates and likelihoods agree to 1e-6; standard errors, degrees of freedom, F, t and Wald Z
/// to 1e-3, the precision of the numerical second derivatives on the R side.
#[test]
fn numbers_agree_with_the_black_box_fits() {
    for case in CASES {
        let data = if case.syntax.contains("level") {
            waves()
        } else {
            pupils()
        };
        let (_, tables) = run(&format!("{data}{}", case.syntax));
        for (title, row, column, want) in case.expected {
            let got = cell(&tables, title, row, column)
                .unwrap_or_else(|| panic!("{}: no {title} / {row} / {column}", case.name));
            let tolerance = if matches!(*column, "Estimate" | "Value") {
                1e-6
            } else {
                1e-3
            };
            assert!(
                (got - want).abs() <= tolerance * want.abs().max(1.0),
                "{}: {title} / {row} / {column}: {got} vs {want}",
                case.name
            );
        }
    }
}

const BLOCKS: &str = "DATA LIST FREE /block treat y.
BEGIN DATA
1 1 20.0 1 2 20.4 1 3 21.8 1 4 16.8 2 1 21.0 2 2 21.5 2 3 23.9 2 4 18.7 3 1 21.5 3 2 20.2
3 3 25.5 3 4 20.9 4 1 19.0 4 2 20.4 4 3 23.2 4 4 19.8 5 1 20.8 5 2 23.2 5 3 23.0 5 4 20.4
END DATA.
";

/// Treatments in complete blocks, the blocks random. With balanced data and both variances
/// positive, REML gives the two-way analysis of variance by hand: the residual variance is the
/// error mean square, the block variance (MS blocks - MS error) / 4, the treatment F is MS treat
/// / MS error on 3 and 12 df, and the intercept (the grand mean) has the block mean square as
/// its error, so its Satterthwaite df is the 4 df of blocks. The sums of squares were worked out
/// separately from the 20 values.
#[test]
fn satterthwaite_df_of_a_balanced_block_design_by_hand() {
    let (_, t) = run(&format!(
        "{BLOCKS}MIXED y BY treat block /FIXED=treat /RANDOM=block /PRINT=TESTCOV."
    ));
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-8 * b.abs().max(1.0);
    let get = |title, row, column| cell(&t, title, row, column).unwrap();
    let tests = "Type III Tests of Fixed Effects";
    let cov = "Estimates of Covariance Parameters";
    assert!(close(get(cov, "Residual", "Estimate"), 1.17625));
    assert!(close(get(cov, "block / Variance", "Estimate"), 0.58625));
    assert!(close(get(tests, "treat", "F"), 13.09812256464754));
    assert!(close(get(tests, "treat", "Numerator df"), 3.0));
    assert!(close(get(tests, "treat", "Denominator df"), 12.0));
    assert!(close(get(tests, "Intercept", "F"), 2528.704295349665));
    assert!(close(get(tests, "Intercept", "Denominator df"), 4.0));
    // the residual method: cases less the rank of X
    let (_, t) = run(&format!(
        "{BLOCKS}MIXED y BY treat block /FIXED=treat /RANDOM=block /CRITERIA=DFMETHOD(RESIDUAL)."
    ));
    assert_eq!(cell(&t, tests, "treat", "Denominator df"), Some(16.0));
    assert_eq!(cell(&t, tests, "Intercept", "Denominator df"), Some(16.0));
}

#[test]
fn model_dimension_counts_levels_parameters_and_subjects() {
    let (_, t) = run(&format!(
        "{}MIXED mark BY track WITH hours /FIXED=track hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(UN).",
        pupils()
    ));
    let md = "Model Dimension";
    let get = |row: &str, column: &str| cell(&t, md, row, column);
    assert_eq!(
        get("Fixed Effects / Intercept", "Number of Parameters"),
        Some(1.0)
    );
    assert_eq!(get("Fixed Effects / track", "Number of Levels"), Some(3.0));
    assert_eq!(
        get("Fixed Effects / track", "Number of Parameters"),
        Some(2.0)
    );
    assert_eq!(
        get("Random Effects / Intercept", "Number of Parameters"),
        Some(3.0)
    );
    assert_eq!(get("Residual", "Number of Parameters"), Some(1.0));
    assert_eq!(get("Total", "Number of Levels"), Some(7.0));
    assert_eq!(get("Total", "Number of Parameters"), Some(8.0));
    let (engine, t) = run(&format!(
        "{}MIXED level BY wave WITH dose /FIXED=wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(VC).",
        waves()
    ));
    assert_eq!(
        cell(&t, md, "Repeated Effects / wave", "Number of Subjects"),
        Some(30.0)
    );
    assert_eq!(
        cell(&t, md, "Repeated Effects / wave", "Number of Parameters"),
        Some(4.0)
    );
    assert!(engine.warnings().iter().any(|w| w.contains("DIAG")));
}

/// FIXPRED, PRED and RESID agree with nlme's fitted values at levels 0 and 1 and its level-1
/// residuals; SEFIXP and DFFIXP with the separate R check. Cases 1, 52 and 192 are pupils 1, 52
/// and 192; the eighth case has no mark and gets nothing.
#[test]
fn save_adds_the_predictions() {
    let (engine, _) = run(&format!(
        "{}MIXED mark BY girl track WITH hours /FIXED=girl track hours /RANDOM=INTERCEPT | SUBJECT(school)\n\
         /SAVE=FIXPRED PRED RESID SEFIXP DFFIXP(dfx).",
        pupils()
    ));
    let data = engine.dataset().unwrap();
    let value = |name: &str, case: usize| -> Option<f64> {
        match data.variable(name).unwrap().values() {
            statscraft_data::Values::Numeric(v) => v.get(case),
            _ => panic!("{name} is a string"),
        }
    };
    let check = |name: &str, case: usize, want: f64, tolerance: f64| {
        let got = value(name, case).unwrap();
        assert!(
            (got - want).abs() <= tolerance * want.abs(),
            "{name} {case}: {got} vs {want}"
        );
    };
    for (case, fixpred, pred, resid) in [
        (0, 57.5493170361, 58.1306440635, -4.73064406352),
        (51, 55.3026093566, 57.2944725039, 8.30552749614),
        (191, 60.1778016173, 56.8846992194, 1.81530078059),
    ] {
        check("FXPRED_1", case, fixpred, 1e-8);
        check("PRED_1", case, pred, 1e-8);
        check("RESID_1", case, resid, 1e-7);
    }
    check("SEFIXP_1", 0, 1.52946200566802, 1e-6);
    check("SEFIXP_1", 51, 1.5792049243136, 1e-6);
    check("dfx", 0, 10.575243329, 1e-3);
    check("dfx", 51, 12.0115149387, 1e-3);
    assert_eq!(value("PRED_1", 7), None);
}

/// A frequency weight of 2 is the same as the case written twice.
#[test]
fn a_frequency_weight_counts_the_case_that_many_times() {
    let mut doubled = String::from(PUPILS);
    for line in PUPILS.lines().skip(1).take(12) {
        doubled.push_str(line);
        doubled.push('\n');
    }
    let model = "MIXED mark BY track WITH hours /FIXED=track hours /RANDOM=INTERCEPT | SUBJECT(school) /PRINT=SOLUTION.";
    let list = "school room pupil girl (F4.0) track (A4) hours mark (F8.1)";
    let (_, by_rows) = run(&format!("{}{model}", inline(&doubled, list)));
    let (_, by_weight) = run(&format!(
        "{}COMPUTE wt = 1 + (pupil <= 12).\nWEIGHT BY wt.\n{model}",
        pupils()
    ));
    for (title, row, column) in [
        (
            "Information Criteria",
            "-2 Restricted Log Likelihood",
            "Value",
        ),
        (
            "Estimates of Covariance Parameters",
            "Intercept [subject = school] / Variance",
            "Estimate",
        ),
        ("Estimates of Fixed Effects", "hours", "df"),
    ] {
        let a = cell(&by_rows, title, row, column).unwrap();
        let b = cell(&by_weight, title, row, column).unwrap();
        assert!((a - b).abs() <= 1e-9 * a.abs(), "{row}: {a} vs {b}");
    }
}

#[test]
fn errors_explain_what_is_wrong() {
    let error = |source: String| {
        let (commands, errors) = parse(&source);
        assert!(errors.is_empty(), "{errors:?}");
        let mut engine = Engine::default();
        for command in &commands {
            if let Err(e) = engine.execute(&command.command) {
                return e.0;
            }
        }
        panic!("no error");
    };
    let w = waves();
    assert!(error(format!("{w}MIXED level BY wave /RANDOM=INTERCEPT | SUBJECT(arm) /REPEATED=wave | SUBJECT(person).")).contains("REPEATED"));
    assert!(
        error(format!(
            "{w}WEIGHT BY arm.\nMIXED level BY wave /REPEATED=wave | SUBJECT(person)."
        ))
        .contains("weight")
    );
    assert!(error(format!("{w}MIXED level /REPEATED=arm | SUBJECT(person).")).contains("twice"));
    assert!(error(format!("{w}MIXED level BY wave /FIXED=dose.")).contains("dose"));
}

/// The value at (row, column) of the `n`-th table titled `title`.
fn nth(tables: &[Table], title: &str, n: usize, row: &str, column: &str) -> f64 {
    let found: Vec<Table> = tables
        .iter()
        .filter(|t| t.title == title)
        .cloned()
        .collect();
    let table = found
        .get(n)
        .unwrap_or_else(|| panic!("no table {n} {title}"));
    cell(std::slice::from_ref(table), title, row, column)
        .unwrap_or_else(|| panic!("{title} {n}: no {row} / {column}"))
}

/// EMMEANS, TEST, COVB and CORB of a random intercept model with an interaction. The expected
/// values come from the separate R check: the same likelihood, maximized and differentiated
/// numerically, the means built from the cell predictions averaged over the other factor, and
/// the Satterthwaite df of each combination from its own gradient.
#[test]
fn emmeans_and_test_agree_with_the_black_box_check() {
    let (engine, t) = run(&format!(
        "{}MIXED mark BY girl track WITH hours /FIXED=girl track girl*track hours\n\
         /RANDOM=INTERCEPT | SUBJECT(school) /PRINT=COVB CORB G\n\
         /EMMEANS=TABLES(track) COMPARE ADJ(BONFERRONI)\n\
         /EMMEANS=TABLES(girl*track) WITH(hours=5) COMPARE(girl) ADJ(SIDAK)\n\
         /EMMEANS=TABLES(track) COMPARE REFCAT('sci')\n\
         /EMMEANS=TABLES(OVERALL)\n\
         /TEST(0 1.5)='two rows' track 1 -1 0 girl*track 1/2 -1/2 0 1/2 -1/2 0; hours 1\n\
         /TEST=track 2 -2 0 girl*track 1 -1 0 1 -1 0 DIVISOR=2\n\
         /TEST=track 1 -1 0.",
        pupils()
    ));
    let check = |got: f64, want: f64, what: &str| {
        assert!(
            (got - want).abs() <= 1e-3 * want.abs().max(1.0),
            "{what}: {got} vs {want}"
        );
    };
    let ci = "95% Confidence Interval / Lower Bound";
    let cid = "95% Confidence Interval for Difference / Lower Bound";
    let em = "Estimated Marginal Means: track";
    for (row, mean, se, df, lo) in [
        (
            "arts",
            58.9117253007,
            1.49327882081,
            9.55440906283,
            55.5633388613,
        ),
        (
            "sci",
            61.2859741198,
            1.49834445138,
            9.68481913878,
            57.9326687733,
        ),
        (
            "tech",
            57.6129371841,
            1.49159594353,
            9.52142810124,
            54.2666715695,
        ),
    ] {
        check(nth(&t, em, 0, row, "Mean"), mean, row);
        check(nth(&t, em, 0, row, "Std. Error"), se, row);
        check(nth(&t, em, 0, row, "df"), df, row);
        check(nth(&t, em, 0, row, ci), lo, row);
    }
    let grand = "Estimated Marginal Means: Grand Mean";
    let only = |column: &str| {
        let table = t.iter().find(|x| x.title == grand).unwrap();
        let at = table
            .column_titles()
            .iter()
            .position(|c| c == column)
            .unwrap();
        match table.rows[0][at] {
            Cell::Number(v) => v,
            _ => panic!("{column}"),
        }
    };
    check(only("Mean"), 59.2702122016, "grand mean");
    check(only("df"), 7.71365020864, "grand df");
    let pc = "Pairwise Comparisons";
    // Bonferroni over three pairs
    for (row, diff, df, p, lo) in [
        (
            "arts / sci",
            -2.37424881908,
            175.403692979,
            0.0144159815967,
            -4.38353409341,
        ),
        (
            "sci / tech",
            3.67303693568,
            175.875073949,
            6.11753375803e-05,
            1.64579234993,
        ),
        (
            "tech / arts",
            -1.2987881166,
            175.857983045,
            0.354017974163,
            -3.29720992692,
        ),
    ] {
        check(nth(&t, pc, 0, row, "Mean Difference (I-J)"), diff, row);
        check(nth(&t, pc, 0, row, "df"), df, row);
        check(nth(&t, pc, 0, row, "Sig."), p, row);
        check(nth(&t, pc, 0, row, cid), lo, row);
    }
    // Sidak within each track, girl compared
    check(
        nth(&t, pc, 1, "sci / 0 / 1", "Mean Difference (I-J)"),
        -1.5719120839,
        "sci",
    );
    check(nth(&t, pc, 1, "sci / 0 / 1", "Sig."), 0.18878652495, "sci");
    check(nth(&t, pc, 1, "tech / 0 / 1", cid), -3.25046037308, "tech");
    check(
        nth(
            &t,
            "Estimated Marginal Means: girl * track",
            0,
            "1 / sci",
            "df",
        ),
        11.6260990511,
        "cell",
    );
    // each track against sci, no adjustment
    check(
        nth(&t, pc, 2, "tech / sci", "Sig."),
        2.03917791934e-05,
        "refcat",
    );
    check(nth(&t, pc, 2, "arts / sci", cid), -4.01480609582, "refcat");
    let ut = "Univariate Tests";
    check(nth(&t, ut, 0, "track", "F"), 9.85221107684, "univariate");
    check(
        nth(&t, ut, 0, "track", "Denominator df"),
        175.709792999,
        "univariate",
    );
    check(
        nth(&t, ut, 1, "track = arts", "F"),
        (0.0618917055795_f64 / 1.16684150975).powi(2),
        "simple",
    );
    // TEST with constants
    let ce = "Contrast Estimates";
    check(nth(&t, ce, 0, "L1", "Estimate"), -2.37424881908, "L1");
    check(nth(&t, ce, 0, "L1", "df"), 175.403692979, "L1");
    check(nth(&t, ce, 0, "L2", "t"), 2.14226982309, "L2");
    check(nth(&t, ce, 0, "L2", "Sig."), 0.0335503260666, "L2");
    check(nth(&t, ce, 0, "L2", ci), 1.5196768809, "L2");
    let tr = "Test Results";
    check(nth(&t, tr, 0, "two rows", "F"), 6.72492765487, "F");
    check(
        nth(&t, tr, 0, "two rows", "Denominator df"),
        175.449527698,
        "F",
    );
    check(nth(&t, tr, 0, "two rows", "Sig."), 0.00153438927206, "F");
    // DIVISOR halves the row back to the first one
    check(nth(&t, ce, 1, "L1", "Estimate"), -2.37424881908, "divisor");
    assert!(
        engine
            .warnings()
            .iter()
            .any(|w| w.contains("Custom Hypothesis Test 3: L1 is not estimable"))
    );
    let covb = "Covariance Matrix for Estimates of Fixed Effects";
    check(
        nth(&t, covb, 0, "[track=arts]", "[girl=0] * [track=arts]"),
        -1.584687689186124,
        "covb",
    );
    check(
        nth(&t, covb, 0, "hours", "hours"),
        0.013605233285273,
        "covb",
    );
    assert_eq!(nth(&t, covb, 0, "[girl=1]", "Intercept"), 0.0);
    let corb = "Correlation Matrix for Estimates of Fixed Effects";
    check(
        nth(&t, corb, 0, "Intercept", "hours"),
        -0.318612722115,
        "corb",
    );
    check(
        nth(&t, corb, 0, "[track=sci]", "[girl=0] * [track=sci]"),
        -0.6958634308126,
        "corb",
    );
    let g = nth(
        &t,
        "Random Effect Covariance Structure (G)",
        0,
        "Intercept",
        "Intercept",
    );
    let variance = cell(
        &t,
        "Estimates of Covariance Parameters",
        "Intercept [subject = school] / Variance",
        "Estimate",
    )
    .unwrap();
    assert_eq!(g, variance);
}

/// In the balanced block design the means and differences have closed forms: a treatment
/// mean is the raw mean with variance (block variance + residual variance) / 5, and the
/// difference of two treatments has variance 2 x residual / 5 on the 12 error df.
#[test]
fn emmeans_of_a_balanced_block_design_by_hand() {
    let (_, t) = run(&format!(
        "{BLOCKS}MIXED y BY treat block /FIXED=treat /RANDOM=block /EMMEANS=TABLES(treat) COMPARE."
    ));
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-7 * b.abs().max(1.0);
    let em = "Estimated Marginal Means: treat";
    // treatment 3: 21.8, 23.9, 25.5, 23.2, 23.0
    assert!(close(nth(&t, em, 0, "3", "Mean"), 23.48));
    let se = ((0.58625 + 1.17625) / 5.0_f64).sqrt();
    assert!(close(nth(&t, em, 0, "3", "Std. Error"), se));
    let pc = "Pairwise Comparisons";
    let se_diff = (2.0 * 1.17625 / 5.0_f64).sqrt();
    assert!(close(nth(&t, pc, 0, "1 / 2", "Std. Error"), se_diff));
    assert!(close(nth(&t, pc, 0, "1 / 2", "df"), 12.0));
    assert!(close(
        nth(&t, "Univariate Tests", 0, "treat", "F"),
        13.09812256464754
    ));
    assert!(close(
        nth(&t, "Univariate Tests", 0, "treat", "Denominator df"),
        12.0
    ));
    // the residual method gives every mean n - rank X
    let (_, t) = run(&format!(
        "{BLOCKS}MIXED y BY treat block /FIXED=treat /RANDOM=block /EMMEANS=TABLES(treat) /CRITERIA=DFMETHOD(RESIDUAL)."
    ));
    assert_eq!(nth(&t, em, 0, "1", "df"), 16.0);
}

/// PRINT G shows the unstructured block of one school; PRINT R the first-order
/// autoregressive block of one person, sigma^2 rho^|i - j|. Both from the covariance
/// parameters of the same run. G and R without RANDOM or REPEATED are ignored with a warning.
#[test]
fn print_g_and_r_show_the_blocks_of_one_subject() {
    let (_, t) = run(&format!(
        "{}MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(UN) /PRINT=G.",
        pupils()
    ));
    let cov = |row: &str| cell(&t, "Estimates of Covariance Parameters", row, "Estimate").unwrap();
    let g = "Random Effect Covariance Structure (G)";
    let prefix = "Intercept + hours [subject = school]";
    assert_eq!(
        nth(&t, g, 0, "Intercept", "Intercept"),
        cov(&format!("{prefix} / UN (1,1)"))
    );
    assert_eq!(
        nth(&t, g, 0, "hours", "Intercept"),
        cov(&format!("{prefix} / UN (2,1)"))
    );
    assert_eq!(
        nth(&t, g, 0, "Intercept", "hours"),
        cov(&format!("{prefix} / UN (2,1)"))
    );
    assert_eq!(
        nth(&t, g, 0, "hours", "hours"),
        cov(&format!("{prefix} / UN (2,2)"))
    );
    let (engine, t) = run(&format!(
        "{}MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(AR1) /PRINT=R G.",
        waves()
    ));
    let cov = |row: &str| cell(&t, "Estimates of Covariance Parameters", row, "Estimate").unwrap();
    let (s2, rho) = (
        cov("Repeated Measures / AR1 diagonal"),
        cov("Repeated Measures / AR1 rho"),
    );
    let r = "Residual Covariance (R) Matrix";
    let got = nth(&t, r, 0, "[wave = 1]", "[wave = 4]");
    assert!((got - s2 * rho.powi(3)).abs() <= 1e-12 * s2);
    assert_eq!(nth(&t, r, 0, "[wave = 2]", "[wave = 2]"), s2);
    assert!(engine.warnings().iter().any(|w| w.contains("PRINT G")));
}

/// Four plots, each with every cell of a by b except a = 3 with b = 1, which has no case. At
/// b = 1 the mean of a = 3 is not estimable, so its differences are left blank, while the
/// difference of a = 1 and a = 2 is the difference of their raw means (each plot has both)
/// and the F test of a at b = 1 rests on that one difference.
#[test]
fn emmeans_with_an_empty_cell_tests_what_is_estimable() {
    let (_, t) = run("DATA LIST FREE /plot a b y.
BEGIN DATA
1 1 1 12.3 1 1 2 13.5 1 2 1 13.4 1 2 2 15.5 1 3 2 14.6 2 1 1 13.2 2 1 2 14.7 2 2 1 14.7
2 2 2 17.2 2 3 2 16.3 3 1 1 14.4 3 1 2 14.8 3 2 1 14.0 3 2 2 17.9 3 3 2 17.3 4 1 1 15.2
4 1 2 14.3 4 2 1 14.8 4 2 2 17.5 4 3 2 17.4
END DATA.
MIXED y BY a b plot /FIXED=a b a*b /RANDOM=plot /EMMEANS=TABLES(a*b) COMPARE(a).");
    let pc = "Pairwise Comparisons";
    let diff = nth(&t, pc, 0, "1 / 1 / 2", "Mean Difference (I-J)");
    assert!((diff + 0.45).abs() < 1e-9, "{diff}");
    let table = t.iter().find(|x| x.title == pc).unwrap();
    let at = row_keys(table)
        .iter()
        .position(|k| k == "1 / 1 / 3")
        .unwrap();
    assert!(matches!(table.rows[at][3], Cell::Empty));
    assert!(table.caption.as_deref().unwrap().contains("not estimable"));
    let ut = "Univariate Tests";
    assert_eq!(nth(&t, ut, 0, "b = 1", "Numerator df"), 1.0);
    assert_eq!(nth(&t, ut, 0, "b = 2", "Numerator df"), 2.0);
    let univariate = t.iter().find(|x| x.title == ut).unwrap();
    assert!(univariate.caption.as_deref().unwrap().contains("left out"));
}

/// Without /FIXED the fixed part is the intercept alone: the same fit as /FIXED=INTERCEPT, and
/// in the balanced block design the intercept is the grand mean.
#[test]
fn without_fixed_the_fixed_part_is_the_intercept() {
    let (_, without) = run(&format!(
        "{BLOCKS}MIXED y BY treat block /RANDOM=block /PRINT=SOLUTION."
    ));
    let (_, with) = run(&format!(
        "{BLOCKS}MIXED y BY treat block /FIXED=INTERCEPT /RANDOM=block /PRINT=SOLUTION."
    ));
    let fe = "Estimates of Fixed Effects";
    let a = cell(&without, fe, "Intercept", "Estimate").unwrap();
    assert_eq!(Some(a), cell(&with, fe, "Intercept", "Estimate"));
    // the 20 values sum to 422.0
    assert!((a - 21.1).abs() < 1e-9, "{a}");
    assert_eq!(
        cell(
            &without,
            "Model Dimension",
            "Fixed Effects / Intercept",
            "Number of Parameters"
        ),
        Some(1.0)
    );
}

/// DFMETHOD(KENWARDROGER) against R's pbkrtest (lme4 fits) for random effects, and against the
/// 1997 formulas written separately in R for a REPEATED AR1 structure, which lme4 cannot fit
/// (that R code reproduces pbkrtest exactly on the lme4 models). Both use the inverse of the
/// expected information for the covariance parameters, as the method was published.
#[test]
fn kenward_roger_agrees_with_the_black_box_check() {
    let check = |t: &[Table], expected: &[(&str, &str, &str, f64)], what: &str| {
        for (title, row, column, want) in expected {
            let got = cell(t, title, row, column)
                .unwrap_or_else(|| panic!("{what}: no {title} / {row} / {column}"));
            // lme4 stops its optimizer about 1e-5 from the REML maximum
            assert!(
                (got - want).abs() <= 2e-4 * want.abs().max(1.0),
                "{what}: {title} / {row} / {column}: {got} vs {want}"
            );
        }
    };
    let tests = "Type III Tests of Fixed Effects";
    let fixed = "Estimates of Fixed Effects";
    let (_, t) = run(&format!(
        "{}MIXED mark BY girl track WITH hours /FIXED=girl track hours /RANDOM=INTERCEPT | SUBJECT(school) /CRITERIA=DFMETHOD(KENWARDROGER) /PRINT=SOLUTION /EMMEANS=TABLES(track) COMPARE REFCAT(LAST) /TEST='two rows' track 1 -1 0; track 1 0 -1.",
        pupils()
    ));
    let means = "Estimated Marginal Means: track";
    check(
        &t,
        &[
            (tests, "Intercept", "F", 1096.37838349),
            (tests, "Intercept", "Denominator df", 10.8861815744),
            (tests, "girl", "Denominator df", 177.629802339),
            (tests, "track", "F", 10.2847746669),
            (tests, "track", "Denominator df", 177.943285534),
            (tests, "hours", "F", 226.94652466),
            (fixed, "Intercept", "Std. Error", 1.62624933759),
            (fixed, "Intercept", "df", 13.9351119612),
            (fixed, "[track=arts]", "Std. Error", 0.81884380246),
            (fixed, "[track=sci]", "df", 178.221181399),
            (means, "arts", "Std. Error", 1.49027775633),
            (means, "arts", "df", 9.86785846123),
            (means, "tech", "Mean", 57.618106387),
            (means, "tech", "df", 9.82503524749),
            ("Contrast Estimates", "L1", "Std. Error", 0.824397595848),
            ("Contrast Estimates", "L1", "df", 177.637405485),
            ("Test Results", "two rows", "F", 10.2847746669),
            ("Test Results", "two rows", "Denominator df", 177.943285534),
            ("Univariate Tests", "track", "Denominator df", 177.943285534),
        ],
        "random intercept",
    );
    let (_, t) = run(&format!(
        "{}MIXED mark BY girl WITH hours /FIXED=girl hours /RANDOM=INTERCEPT hours | SUBJECT(school) COVTYPE(UN) /CRITERIA=DFMETHOD(KENWARDROGER) /PRINT=SOLUTION.",
        pupils()
    ));
    check(
        &t,
        &[
            (tests, "Intercept", "F", 538.260394426),
            (tests, "Intercept", "Denominator df", 7.97028837506),
            (tests, "girl", "F", 6.69324848),
            (tests, "girl", "Denominator df", 175.539585617),
            (tests, "hours", "F", 45.2020670833),
            (tests, "hours", "Denominator df", 7.90239945366),
            (fixed, "hours", "Std. Error", 0.290086740814),
        ],
        "random slope UN",
    );
    let (_, t) = run(&format!(
        "{}MIXED level BY arm wave WITH dose /FIXED=arm wave dose /REPEATED=wave | SUBJECT(person) COVTYPE(AR1) /CRITERIA=DFMETHOD(KENWARDROGER) /PRINT=SOLUTION.",
        waves()
    ));
    check(
        &t,
        &[
            (tests, "Intercept", "F", 1220.69667443),
            (tests, "Intercept", "Denominator df", 99.7965225308),
            (tests, "arm", "Denominator df", 31.0768984191),
            (tests, "wave", "F", 16.395189945),
            (tests, "wave", "Denominator df", 72.4085714441),
            (tests, "dose", "F", 43.7518183015),
            (fixed, "Intercept", "Std. Error", 0.906824324786),
            (fixed, "Intercept", "df", 94.7487515238),
            (fixed, "[wave=3]", "Std. Error", 0.482248856008),
            (fixed, "[wave=3]", "df", 70.0226283602),
        ],
        "repeated AR1",
    );
    // balanced complete blocks: the method gives the analysis of variance exactly
    let (_, t) = run(&format!(
        "{BLOCKS}MIXED y BY treat block /FIXED=treat /RANDOM=block /CRITERIA=DFMETHOD(KENWARDROGER)."
    ));
    let close = |a: f64, b: f64| (a - b).abs() <= 1e-8 * b.abs().max(1.0);
    let get = |row, column| cell(&t, tests, row, column).unwrap();
    assert!(close(get("treat", "F"), 13.09812256464754));
    assert!(close(get("treat", "Denominator df"), 12.0));
    assert!(close(get("Intercept", "F"), 2528.704295349665));
    assert!(close(get("Intercept", "Denominator df"), 4.0));
}
