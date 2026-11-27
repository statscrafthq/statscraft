//! GLM with three within-subjects factors (a 2, b 2, c 3) and a between factor on 16 invented
//! subjects, against R: a WSDESIGN that names only some of the within effects. Each effect is
//! tested on its own contrasts of the cells (the other factors averaged), so the tests of the
//! effects a design keeps are the same as in the full factorial, and the ones it leaves out are
//! not shown. The script writes the orthonormal contrasts of each effect and takes the
//! hypothesis and error SSCP matrices from `lm` on a matrix response (Type III for the unbalanced
//! between factor). Scripts in lab/oracles/glm-wsd.

use statscraft_engine::{Cell, Engine, Table};
use statscraft_syntax::parse;

const DATA: &str = "DATA LIST FREE /g y1 y2 y3 y4 y5 y6 y7 y8 y9 y10 y11 y12 .
BEGIN DATA.
1 9.17 11.32 15.06 11.06 13.57 12.51 15.07 13.08 15.25 13.67 12.42 14.04 
1 9.81 10.6 12.88 10.59 11.51 13.97 10.75 12.67 13.41 10.48 13.56 14.2 
1 11.23 11.44 13.35 9.86 10.72 11.08 10.74 12.25 12.62 12.66 13.38 12.32 
1 9.59 10.89 12.52 12.04 11.29 11.24 11.31 11.25 13.83 12.65 12.05 13.9 
1 10.82 11.28 9.02 10.72 11.17 10.69 9.21 11.12 13.21 7.99 10.64 13.39 
1 11.48 12.7 12.89 13.05 14.2 16 12.46 14.92 13.96 15.13 16.49 16.31 
1 8.97 10.72 11.46 9.76 10.99 12.15 10.42 11.79 14.41 10.64 12.38 13.39 
2 8 7.06 9.52 8.62 11 12.18 9.61 10.39 12.49 7.16 11.21 12.64 
2 8.95 10.2 13.31 10.77 11.37 14.18 10.84 12.72 15.11 14.43 14.37 15 
2 6.94 11.59 12.19 8.8 11.11 13.1 9.16 13.15 13.26 12.23 10.86 13.48 
2 10.46 10.71 11.11 10.81 10.84 11.66 12.35 12.04 14.71 11.9 12.25 13.3 
2 10.61 10.29 12.71 9.94 11.3 13.07 10.87 12.57 12.4 11.28 13.32 14.35 
2 14.34 14.15 15.39 13.06 13.96 15.13 12.58 16.18 16.83 14.67 16.33 18.13 
2 7.24 10.42 10.23 8.98 11.88 13.86 8.55 10.97 14.21 9.93 12.49 14.79 
2 8.04 5.01 11.56 8.34 8.01 11.11 7.37 6.92 10.14 7.58 10.4 11.2 
2 8.53 13.04 12.35 13.12 15.74 13.69 12.91 13.16 13.32 13.63 12.56 16.01 
END DATA.
";

fn run(syntax: &str) -> Vec<Table> {
    let mut engine = Engine::default();
    let (commands, errors) = parse(&format!("{DATA}{syntax}"));
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

/// The row headers joined with " / ", an empty header taking the one above it.
fn row_keys(table: &Table) -> Vec<String> {
    let mut carried = vec![String::new(); table.row_headers];
    table
        .rows
        .iter()
        .map(|row| {
            for (level, cell) in row.iter().take(table.row_headers).enumerate() {
                let shown = match cell {
                    Cell::Text(t) if !t.is_empty() => Some(t.clone()),
                    Cell::Number(n) => Some(format!("{n}")),
                    _ => None,
                };
                if let Some(t) = shown {
                    carried[level] = t;
                    for deeper in carried.iter_mut().skip(level + 1) {
                        deeper.clear();
                    }
                }
            }
            carried
                .iter()
                .filter(|s| !s.is_empty())
                .cloned()
                .collect::<Vec<_>>()
                .join(" / ")
        })
        .collect()
}

fn value(tables: &[Table], title: &str, row: &str, column: &str) -> f64 {
    let table = tables
        .iter()
        .find(|t| t.title == title)
        .unwrap_or_else(|| panic!("no table {title}"));
    let r = row_keys(table)
        .iter()
        .position(|k| k == row)
        .unwrap_or_else(|| panic!("{title}: no row {row} in {:?}", row_keys(table)));
    let c = table
        .column_titles()
        .iter()
        .position(|t| t == column)
        .unwrap_or_else(|| panic!("{title}: no column {column} in {:?}", table.column_titles()));
    match &table.rows[r][c] {
        Cell::Number(n) => *n,
        other => panic!("{title} / {row} / {column}: {other:?}"),
    }
}

fn near(actual: f64, expected: f64, what: &str) {
    assert!(
        (actual - expected).abs() <= 1e-8 + 1e-7 * expected.abs(),
        "{what}: {actual} is not {expected}"
    );
}

/// R's values: table, row, column, value.
const R: &[(&str, &str, &str, f64)] = &[
    ("Mauchly's Test of Sphericity", "a", "Mauchly's W", 1.0),
    (
        "Mauchly's Test of Sphericity",
        "a",
        "Epsilon / Greenhouse-Geisser",
        1.0,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a",
        "Epsilon / Huynh-Feldt",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Sphericity Assumed",
        "Type III Sum of Squares",
        73.9156509259,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Sphericity Assumed",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Sphericity Assumed",
        "F",
        85.7205498671,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Sphericity Assumed",
        "Sig.",
        2.40459578127e-07,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Greenhouse-Geisser",
        "Type III Sum of Squares",
        73.9156509259,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Greenhouse-Geisser",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Greenhouse-Geisser",
        "F",
        85.7205498671,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a / Greenhouse-Geisser",
        "Sig.",
        2.40459578127e-07,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a) / Sphericity Assumed",
        "Type III Sum of Squares",
        12.0720074074,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a) / Sphericity Assumed",
        "df",
        14.0,
    ),
    (
        "Multivariate Tests",
        "a / Pillai's Trace",
        "Value",
        0.859607673457,
    ),
    (
        "Multivariate Tests",
        "a / Pillai's Trace",
        "F",
        85.7205498671,
    ),
    (
        "Multivariate Tests",
        "a / Pillai's Trace",
        "Hypothesis df",
        1.0,
    ),
    ("Multivariate Tests", "a / Pillai's Trace", "Error df", 14.0),
    (
        "Multivariate Tests",
        "a / Pillai's Trace",
        "Sig.",
        2.40459578127e-07,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Sphericity Assumed",
        "Type III Sum of Squares",
        0.164890509259,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Sphericity Assumed",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Sphericity Assumed",
        "F",
        0.191224793998,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Sphericity Assumed",
        "Sig.",
        0.668569269463,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.164890509259,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Greenhouse-Geisser",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Greenhouse-Geisser",
        "F",
        0.191224793998,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * g / Greenhouse-Geisser",
        "Sig.",
        0.668569269463,
    ),
    (
        "Multivariate Tests",
        "a * g / Pillai's Trace",
        "Value",
        0.0134748618794,
    ),
    (
        "Multivariate Tests",
        "a * g / Pillai's Trace",
        "F",
        0.191224793998,
    ),
    (
        "Multivariate Tests",
        "a * g / Pillai's Trace",
        "Hypothesis df",
        1.0,
    ),
    (
        "Multivariate Tests",
        "a * g / Pillai's Trace",
        "Error df",
        14.0,
    ),
    (
        "Multivariate Tests",
        "a * g / Pillai's Trace",
        "Sig.",
        0.668569269463,
    ),
    ("Mauchly's Test of Sphericity", "b", "Mauchly's W", 1.0),
    (
        "Mauchly's Test of Sphericity",
        "b",
        "Epsilon / Greenhouse-Geisser",
        1.0,
    ),
    (
        "Mauchly's Test of Sphericity",
        "b",
        "Epsilon / Huynh-Feldt",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Sphericity Assumed",
        "Type III Sum of Squares",
        23.8364783069,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Sphericity Assumed",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Sphericity Assumed",
        "F",
        14.0767662942,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Sphericity Assumed",
        "Sig.",
        0.00214530538587,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Greenhouse-Geisser",
        "Type III Sum of Squares",
        23.8364783069,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Greenhouse-Geisser",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Greenhouse-Geisser",
        "F",
        14.0767662942,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b / Greenhouse-Geisser",
        "Sig.",
        0.00214530538587,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(b) / Sphericity Assumed",
        "Type III Sum of Squares",
        23.7064883598,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(b) / Sphericity Assumed",
        "df",
        14.0,
    ),
    (
        "Multivariate Tests",
        "b / Pillai's Trace",
        "Value",
        0.501367078626,
    ),
    (
        "Multivariate Tests",
        "b / Pillai's Trace",
        "F",
        14.0767662942,
    ),
    (
        "Multivariate Tests",
        "b / Pillai's Trace",
        "Hypothesis df",
        1.0,
    ),
    ("Multivariate Tests", "b / Pillai's Trace", "Error df", 14.0),
    (
        "Multivariate Tests",
        "b / Pillai's Trace",
        "Sig.",
        0.00214530538587,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Sphericity Assumed",
        "Type III Sum of Squares",
        3.18305955688,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Sphericity Assumed",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Sphericity Assumed",
        "F",
        1.87977371933,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Sphericity Assumed",
        "Sig.",
        0.191938880182,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Greenhouse-Geisser",
        "Type III Sum of Squares",
        3.18305955688,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Greenhouse-Geisser",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Greenhouse-Geisser",
        "F",
        1.87977371933,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * g / Greenhouse-Geisser",
        "Sig.",
        0.191938880182,
    ),
    (
        "Multivariate Tests",
        "b * g / Pillai's Trace",
        "Value",
        0.118375346686,
    ),
    (
        "Multivariate Tests",
        "b * g / Pillai's Trace",
        "F",
        1.87977371933,
    ),
    (
        "Multivariate Tests",
        "b * g / Pillai's Trace",
        "Hypothesis df",
        1.0,
    ),
    (
        "Multivariate Tests",
        "b * g / Pillai's Trace",
        "Error df",
        14.0,
    ),
    (
        "Multivariate Tests",
        "b * g / Pillai's Trace",
        "Sig.",
        0.191938880182,
    ),
    (
        "Mauchly's Test of Sphericity",
        "c",
        "Mauchly's W",
        0.975504140161,
    ),
    ("Mauchly's Test of Sphericity", "c", "df", 2.0),
    ("Mauchly's Test of Sphericity", "c", "Sig.", 0.851116990164),
    (
        "Mauchly's Test of Sphericity",
        "c",
        "Epsilon / Greenhouse-Geisser",
        0.976089840087,
    ),
    (
        "Mauchly's Test of Sphericity",
        "c",
        "Epsilon / Huynh-Feldt",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Sphericity Assumed",
        "Type III Sum of Squares",
        195.673093965,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Sphericity Assumed",
        "F",
        69.0578371408,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Sphericity Assumed",
        "Sig.",
        1.4943896514e-11,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Greenhouse-Geisser",
        "Type III Sum of Squares",
        195.673093965,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Greenhouse-Geisser",
        "df",
        1.95217968017,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Greenhouse-Geisser",
        "F",
        69.0578371408,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c / Greenhouse-Geisser",
        "Sig.",
        2.51976157938e-11,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(c) / Sphericity Assumed",
        "Type III Sum of Squares",
        39.6685362434,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(c) / Sphericity Assumed",
        "df",
        28.0,
    ),
    (
        "Multivariate Tests",
        "c / Pillai's Trace",
        "Value",
        0.900049647237,
    ),
    (
        "Multivariate Tests",
        "c / Pillai's Trace",
        "F",
        58.5322867334,
    ),
    (
        "Multivariate Tests",
        "c / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    ("Multivariate Tests", "c / Pillai's Trace", "Error df", 13.0),
    (
        "Multivariate Tests",
        "c / Pillai's Trace",
        "Sig.",
        3.1520866899e-07,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Sphericity Assumed",
        "Type III Sum of Squares",
        6.07779188161,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Sphericity Assumed",
        "F",
        2.14500191841,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Sphericity Assumed",
        "Sig.",
        0.135912401436,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Greenhouse-Geisser",
        "Type III Sum of Squares",
        6.07779188161,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Greenhouse-Geisser",
        "df",
        1.95217968017,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Greenhouse-Geisser",
        "F",
        2.14500191841,
    ),
    (
        "Tests of Within-Subjects Effects",
        "c * g / Greenhouse-Geisser",
        "Sig.",
        0.137331093804,
    ),
    (
        "Multivariate Tests",
        "c * g / Pillai's Trace",
        "Value",
        0.227426045668,
    ),
    (
        "Multivariate Tests",
        "c * g / Pillai's Trace",
        "F",
        1.9134340325,
    ),
    (
        "Multivariate Tests",
        "c * g / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    (
        "Multivariate Tests",
        "c * g / Pillai's Trace",
        "Error df",
        13.0,
    ),
    (
        "Multivariate Tests",
        "c * g / Pillai's Trace",
        "Sig.",
        0.186900476191,
    ),
    ("Mauchly's Test of Sphericity", "a * b", "Mauchly's W", 1.0),
    (
        "Mauchly's Test of Sphericity",
        "a * b",
        "Epsilon / Greenhouse-Geisser",
        1.0,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a * b",
        "Epsilon / Huynh-Feldt",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Sphericity Assumed",
        "Type III Sum of Squares",
        0.894523842593,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Sphericity Assumed",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Sphericity Assumed",
        "F",
        0.618259946547,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Sphericity Assumed",
        "Sig.",
        0.444800615872,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.894523842593,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Greenhouse-Geisser",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Greenhouse-Geisser",
        "F",
        0.618259946547,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b / Greenhouse-Geisser",
        "Sig.",
        0.444800615872,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a * b) / Sphericity Assumed",
        "Type III Sum of Squares",
        20.2557740741,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a * b) / Sphericity Assumed",
        "df",
        14.0,
    ),
    (
        "Multivariate Tests",
        "a * b / Pillai's Trace",
        "Value",
        0.0422936757731,
    ),
    (
        "Multivariate Tests",
        "a * b / Pillai's Trace",
        "F",
        0.618259946547,
    ),
    (
        "Multivariate Tests",
        "a * b / Pillai's Trace",
        "Hypothesis df",
        1.0,
    ),
    (
        "Multivariate Tests",
        "a * b / Pillai's Trace",
        "Error df",
        14.0,
    ),
    (
        "Multivariate Tests",
        "a * b / Pillai's Trace",
        "Sig.",
        0.444800615872,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Sphericity Assumed",
        "Type III Sum of Squares",
        0.205359259259,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Sphericity Assumed",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Sphericity Assumed",
        "F",
        0.141936300194,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Sphericity Assumed",
        "Sig.",
        0.712012265103,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.205359259259,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Greenhouse-Geisser",
        "df",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Greenhouse-Geisser",
        "F",
        0.141936300194,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * g / Greenhouse-Geisser",
        "Sig.",
        0.712012265103,
    ),
    (
        "Multivariate Tests",
        "a * b * g / Pillai's Trace",
        "Value",
        0.0100365534946,
    ),
    (
        "Multivariate Tests",
        "a * b * g / Pillai's Trace",
        "F",
        0.141936300194,
    ),
    (
        "Multivariate Tests",
        "a * b * g / Pillai's Trace",
        "Hypothesis df",
        1.0,
    ),
    (
        "Multivariate Tests",
        "a * b * g / Pillai's Trace",
        "Error df",
        14.0,
    ),
    (
        "Multivariate Tests",
        "a * b * g / Pillai's Trace",
        "Sig.",
        0.712012265103,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a * c",
        "Mauchly's W",
        0.986646102892,
    ),
    ("Mauchly's Test of Sphericity", "a * c", "df", 2.0),
    (
        "Mauchly's Test of Sphericity",
        "a * c",
        "Sig.",
        0.916324147233,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a * c",
        "Epsilon / Greenhouse-Geisser",
        0.986822079487,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a * c",
        "Epsilon / Huynh-Feldt",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Sphericity Assumed",
        "Type III Sum of Squares",
        0.520836226852,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Sphericity Assumed",
        "F",
        0.123403852511,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Sphericity Assumed",
        "Sig.",
        0.884384680414,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.520836226852,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Greenhouse-Geisser",
        "df",
        1.97364415897,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Greenhouse-Geisser",
        "F",
        0.123403852511,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c / Greenhouse-Geisser",
        "Sig.",
        0.881952896348,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a * c) / Sphericity Assumed",
        "Type III Sum of Squares",
        59.0881648148,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a * c) / Sphericity Assumed",
        "df",
        28.0,
    ),
    (
        "Multivariate Tests",
        "a * c / Pillai's Trace",
        "Value",
        0.016793334466,
    ),
    (
        "Multivariate Tests",
        "a * c / Pillai's Trace",
        "F",
        0.111021088297,
    ),
    (
        "Multivariate Tests",
        "a * c / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    (
        "Multivariate Tests",
        "a * c / Pillai's Trace",
        "Error df",
        13.0,
    ),
    (
        "Multivariate Tests",
        "a * c / Pillai's Trace",
        "Sig.",
        0.895759230022,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Sphericity Assumed",
        "Type III Sum of Squares",
        0.102259143519,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Sphericity Assumed",
        "F",
        0.0242286761443,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Sphericity Assumed",
        "Sig.",
        0.97608292212,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.102259143519,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Greenhouse-Geisser",
        "df",
        1.97364415897,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Greenhouse-Geisser",
        "F",
        0.0242286761443,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * c * g / Greenhouse-Geisser",
        "Sig.",
        0.975075975697,
    ),
    (
        "Multivariate Tests",
        "a * c * g / Pillai's Trace",
        "Value",
        0.00389065183796,
    ),
    (
        "Multivariate Tests",
        "a * c * g / Pillai's Trace",
        "F",
        0.0253880128656,
    ),
    (
        "Multivariate Tests",
        "a * c * g / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    (
        "Multivariate Tests",
        "a * c * g / Pillai's Trace",
        "Error df",
        13.0,
    ),
    (
        "Multivariate Tests",
        "a * c * g / Pillai's Trace",
        "Sig.",
        0.974979766282,
    ),
    (
        "Mauchly's Test of Sphericity",
        "b * c",
        "Mauchly's W",
        0.935672186407,
    ),
    ("Mauchly's Test of Sphericity", "b * c", "df", 2.0),
    (
        "Mauchly's Test of Sphericity",
        "b * c",
        "Sig.",
        0.649088895706,
    ),
    (
        "Mauchly's Test of Sphericity",
        "b * c",
        "Epsilon / Greenhouse-Geisser",
        0.939560149822,
    ),
    (
        "Mauchly's Test of Sphericity",
        "b * c",
        "Epsilon / Huynh-Feldt",
        1.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Sphericity Assumed",
        "Type III Sum of Squares",
        1.12098285384,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Sphericity Assumed",
        "F",
        0.456434907047,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Sphericity Assumed",
        "Sig.",
        0.638168889585,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Greenhouse-Geisser",
        "Type III Sum of Squares",
        1.12098285384,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Greenhouse-Geisser",
        "df",
        1.87912029964,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Greenhouse-Geisser",
        "F",
        0.456434907047,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c / Greenhouse-Geisser",
        "Sig.",
        0.626494820177,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(b * c) / Sphericity Assumed",
        "Type III Sum of Squares",
        34.3833473545,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(b * c) / Sphericity Assumed",
        "df",
        28.0,
    ),
    (
        "Multivariate Tests",
        "b * c / Pillai's Trace",
        "Value",
        0.0797083376876,
    ),
    (
        "Multivariate Tests",
        "b * c / Pillai's Trace",
        "F",
        0.562978255901,
    ),
    (
        "Multivariate Tests",
        "b * c / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    (
        "Multivariate Tests",
        "b * c / Pillai's Trace",
        "Error df",
        13.0,
    ),
    (
        "Multivariate Tests",
        "b * c / Pillai's Trace",
        "Sig.",
        0.582794800066,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Sphericity Assumed",
        "Type III Sum of Squares",
        0.473651603836,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Sphericity Assumed",
        "F",
        0.192858548219,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Sphericity Assumed",
        "Sig.",
        0.82568473849,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.473651603836,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Greenhouse-Geisser",
        "df",
        1.87912029964,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Greenhouse-Geisser",
        "F",
        0.192858548219,
    ),
    (
        "Tests of Within-Subjects Effects",
        "b * c * g / Greenhouse-Geisser",
        "Sig.",
        0.812639692695,
    ),
    (
        "Multivariate Tests",
        "b * c * g / Pillai's Trace",
        "Value",
        0.0344319506698,
    ),
    (
        "Multivariate Tests",
        "b * c * g / Pillai's Trace",
        "F",
        0.231788613458,
    ),
    (
        "Multivariate Tests",
        "b * c * g / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    (
        "Multivariate Tests",
        "b * c * g / Pillai's Trace",
        "Error df",
        13.0,
    ),
    (
        "Multivariate Tests",
        "b * c * g / Pillai's Trace",
        "Sig.",
        0.796322088338,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a * b * c",
        "Mauchly's W",
        0.654341699969,
    ),
    ("Mauchly's Test of Sphericity", "a * b * c", "df", 2.0),
    (
        "Mauchly's Test of Sphericity",
        "a * b * c",
        "Sig.",
        0.0634935904998,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a * b * c",
        "Epsilon / Greenhouse-Geisser",
        0.743130704115,
    ),
    (
        "Mauchly's Test of Sphericity",
        "a * b * c",
        "Epsilon / Huynh-Feldt",
        0.870250819607,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Sphericity Assumed",
        "Type III Sum of Squares",
        0.174271941138,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Sphericity Assumed",
        "F",
        0.161902092034,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Sphericity Assumed",
        "Sig.",
        0.851314976045,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.174271941138,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Greenhouse-Geisser",
        "df",
        1.48626140823,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Greenhouse-Geisser",
        "F",
        0.161902092034,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c / Greenhouse-Geisser",
        "Sig.",
        0.787906953975,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a * b * c) / Sphericity Assumed",
        "Type III Sum of Squares",
        15.0696457672,
    ),
    (
        "Tests of Within-Subjects Effects",
        "Error(a * b * c) / Sphericity Assumed",
        "df",
        28.0,
    ),
    (
        "Multivariate Tests",
        "a * b * c / Pillai's Trace",
        "Value",
        0.0378821895456,
    ),
    (
        "Multivariate Tests",
        "a * b * c / Pillai's Trace",
        "F",
        0.255929398012,
    ),
    (
        "Multivariate Tests",
        "a * b * c / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    (
        "Multivariate Tests",
        "a * b * c / Pillai's Trace",
        "Error df",
        13.0,
    ),
    (
        "Multivariate Tests",
        "a * b * c / Pillai's Trace",
        "Sig.",
        0.778007265459,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Sphericity Assumed",
        "Type III Sum of Squares",
        0.521499024471,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Sphericity Assumed",
        "df",
        2.0,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Sphericity Assumed",
        "F",
        0.484482943752,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Sphericity Assumed",
        "Sig.",
        0.6210842923,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Greenhouse-Geisser",
        "Type III Sum of Squares",
        0.521499024471,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Greenhouse-Geisser",
        "df",
        1.48626140823,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Greenhouse-Geisser",
        "F",
        0.484482943752,
    ),
    (
        "Tests of Within-Subjects Effects",
        "a * b * c * g / Greenhouse-Geisser",
        "Sig.",
        0.567635026721,
    ),
    (
        "Multivariate Tests",
        "a * b * c * g / Pillai's Trace",
        "Value",
        0.130120565406,
    ),
    (
        "Multivariate Tests",
        "a * b * c * g / Pillai's Trace",
        "F",
        0.972299886061,
    ),
    (
        "Multivariate Tests",
        "a * b * c * g / Pillai's Trace",
        "Hypothesis df",
        2.0,
    ),
    (
        "Multivariate Tests",
        "a * b * c * g / Pillai's Trace",
        "Error df",
        13.0,
    ),
    (
        "Multivariate Tests",
        "a * b * c * g / Pillai's Trace",
        "Sig.",
        0.404095405726,
    ),
    (
        "Tests of Between-Subjects Effects",
        "Intercept",
        "Type III Sum of Squares",
        27013.3984619,
    ),
    (
        "Tests of Between-Subjects Effects",
        "g",
        "Type III Sum of Squares",
        7.33568902116,
    ),
    (
        "Tests of Between-Subjects Effects",
        "Error",
        "Type III Sum of Squares",
        400.785235979,
    ),
];

/// The effect a row of the within-subjects tables belongs to: its error term and its interaction
/// with the between factor count for the effect.
fn effect_of(row: &str) -> String {
    let head = row.split(" / ").next().unwrap();
    let head = head
        .strip_prefix("Error(")
        .and_then(|h| h.strip_suffix(')'))
        .unwrap_or(head);
    head.strip_suffix(" * g").unwrap_or(head).to_string()
}

fn check_design(wsdesign: &str, kept: &[&str]) {
    let t = run(&format!(
        "GLM y1 y2 y3 y4 y5 y6 y7 y8 y9 y10 y11 y12 BY g /WSFACTOR=a 2 b 2 c 3 {wsdesign} /DESIGN=g."
    ));
    let mut seen = 0;
    for (title, row, column, expected) in R {
        let within = *title != "Tests of Between-Subjects Effects";
        if within && !kept.contains(&effect_of(row).as_str()) {
            continue;
        }
        seen += 1;
        near(
            value(&t, title, row, column),
            *expected,
            &format!("{wsdesign}: {title} / {row} / {column}"),
        );
    }
    assert!(seen > 0);
    // no row of an effect the design leaves out
    for title in [
        "Tests of Within-Subjects Effects",
        "Multivariate Tests",
        "Mauchly's Test of Sphericity",
    ] {
        let table = t.iter().find(|x| x.title == title).unwrap();
        for key in row_keys(table) {
            let effect = effect_of(&key);
            assert!(
                kept.contains(&effect.as_str()),
                "{wsdesign}: {title} shows {key}"
            );
        }
    }
}

#[test]
fn main_effects_only_leave_out_every_interaction() {
    check_design("/WSDESIGN=a b c", &["a", "b", "c"]);
}

#[test]
fn one_interaction_alone_is_tested_without_its_main_effects() {
    check_design("/WSDESIGN=a*c", &["a * c"]);
}

#[test]
fn a_mixed_design_in_any_order_keeps_what_it_names() {
    check_design("/WSDESIGN=c, b*c, a", &["a", "c", "b * c"]);
    check_design("/WSDESIGN=a*b*c", &["a * b * c"]);
}

#[test]
fn an_empty_wsdesign_and_a_missing_one_are_the_full_factorial() {
    let all = ["a * b * c", "b * c", "a * c", "a * b", "c", "b", "a"];
    check_design("/WSDESIGN", &all);
    check_design("", &all);
}

#[test]
fn a_repeated_term_is_one_effect() {
    check_design("/WSDESIGN=a, a, c", &["a", "c"]);
}
