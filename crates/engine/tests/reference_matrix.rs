//! MATRIX programs compared with GNU PSPP, run as a black box on the same syntax: each program
//! saves its results with SAVE or MSAVE, and the data files of both programs must hold the
//! same names and the same numbers (to 1e-9 relative). Ignored by default because they need
//! the PSPP program:
//!
//! ```text
//! STATSCRAFT_PSPP=/path/to/pspp cargo test -p statscraft-engine --test reference_matrix -- --ignored
//! ```
//!
//! Left out where the two differ on purpose (see docs/differences.md): RND of halves, the
//! bivariate normal CDF, the negative binomial, the Laplace density (PSPP's is twice what its
//! own CDF gives), MAGIC of even sizes, the signs of singular vectors, the scalar that COMPUTE
//! assigns to a part of a matrix, and the free-field and fixed WRITE layout. PSPP's matrix
//! language has no half-normal, inverse Gaussian, studentized range, maximum modulus or
//! noncentral functions; the unit tests of `distributions.rs` check those.

use statscraft_data::{Dataset, Values};
use statscraft_engine::session::Session;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::{env, fs};

fn folder(case: &str, who: &str) -> PathBuf {
    let dir = env::temp_dir().join(format!(
        "statscraft-reference-matrix-{}-{case}-{who}",
        std::process::id()
    ));
    fs::create_dir_all(&dir).unwrap();
    dir
}

/// Runs `job` in both programs, each in a folder of its own holding the `inputs`, and compares
/// the data files named in `files`.
fn compare(case: &str, job: &str, inputs: &[(&str, &str)], files: &[&str]) {
    let Some(pspp) = env::var_os("STATSCRAFT_PSPP") else {
        eprintln!("{case}: skipped, STATSCRAFT_PSPP is not set");
        return;
    };
    let (theirs, ours) = (folder(case, "pspp"), folder(case, "ours"));
    for dir in [&theirs, &ours] {
        for (name, text) in inputs {
            fs::write(dir.join(name), text).unwrap();
        }
    }
    fs::write(theirs.join("job.sps"), job).unwrap();
    // PSPP is a Windows program on the development machine: relative paths only
    let status = Command::new(&pspp)
        .args(["job.sps", "-o", "pspp.txt"])
        .current_dir(&theirs)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", PathBuf::from(&pspp).display()));
    let log = fs::read_to_string(theirs.join("pspp.txt")).unwrap_or_default();
    assert!(
        status.success() && !log.contains("error:"),
        "{case}: PSPP failed\n{log}"
    );
    let mut session = Session::default();
    session
        .run(&format!("CD '{}'.\n{job}", ours.display()), None)
        .unwrap_or_else(|e| panic!("{case}: {e:?}"));
    for file in files {
        same_numbers(
            &format!("{case} {file}"),
            &theirs.join(file),
            &ours.join(file),
        );
    }
    fs::remove_dir_all(theirs).unwrap();
    fs::remove_dir_all(ours).unwrap();
}

fn same_numbers(what: &str, theirs: &Path, ours: &Path) {
    let read = |p: &Path| Dataset::read_sav(p).unwrap_or_else(|e| panic!("{what}: {e}"));
    let (a, b) = (read(theirs), read(ours));
    let names = |d: &Dataset| {
        d.variable_names()
            .map(str::to_uppercase)
            .collect::<Vec<_>>()
    };
    assert_eq!(names(&b), names(&a), "{what}: variables");
    assert_eq!(b.case_count(), a.case_count(), "{what}: cases");
    for (x, y) in a.variables().zip(b.variables()) {
        let name = x.name().to_string();
        match (x.values(), y.values()) {
            (Values::Numeric(x), Values::Numeric(y)) => {
                for (case, (p, q)) in x.iter().zip(y.iter()).enumerate() {
                    let same = match (p, q) {
                        (Some(p), Some(q)) => (p - q).abs() <= 1e-9 * p.abs().max(1.0),
                        (p, q) => p == q,
                    };
                    assert!(
                        same,
                        "{what}: {name} case {}: PSPP {p:?}, ours {q:?}",
                        case + 1
                    );
                }
            }
            (Values::String(x), Values::String(y)) => {
                let trim = |v: Vec<&str>| {
                    v.into_iter()
                        .map(str::trim_end)
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                };
                assert_eq!(
                    trim(y.iter().collect()),
                    trim(x.iter().collect()),
                    "{what}: strings"
                );
            }
            _ => panic!("{what}: {name} has another type"),
        }
    }
}

/// SAVEs each expression to its own file, r1.sav, r2.sav, ...
fn saves(expressions: &[&str]) -> (String, Vec<String>) {
    let mut body = String::from("MATRIX.\n");
    let mut files = Vec::new();
    for (k, e) in expressions.iter().enumerate() {
        let file = format!("r{}.sav", k + 1);
        body.push_str(&format!("SAVE ({e}) /OUTFILE='{file}'.\n"));
        files.push(file);
    }
    body.push_str("END MATRIX.\n");
    (body, files)
}

fn compare_saves(case: &str, setup: &str, expressions: &[&str]) {
    let (body, files) = saves(expressions);
    let files: Vec<&str> = files.iter().map(String::as_str).collect();
    compare(case, &format!("{setup}{body}"), &[], &files);
}

#[test]
#[ignore = "needs PSPP"]
fn operators_and_construction() {
    compare_saves(
        "operators",
        "",
        &[
            "{1,2,3;4,5,6} + 4",
            "{1,2,3;4,5,6} &** 2",
            "2 &** {1,2,3;4,5,6}",
            "({1,2,3;4,5,6} &** 2 &/ 2) < ({1,2,3;4,5,6} + 4)",
            "{1, 2; 3, 4} * {5, 6; 7, 8}",
            "{1, 2; 3, 4} ** 3",
            "{2, 1; 1, 3} ** -2",
            "{1:7:2; 2:11:3}",
            "{5:1:-1, 0:2}",
            "-2 ** 2",
            "{1, 2} / {4, 8} - 3 * {1, 1}",
            "NOT {1, 0, -1} AND {1, 1, 1} OR {0, 0, 1} XOR {1, 0, 0}",
            "({1, 2, 3} >= 2) + ({1, 2, 3} <> 2) + ({1, 2, 3} LT 3)",
            "KRONEKER({1, 2; 3, 4}, {1, 10})",
            "BLOCK({1}, {2, 3; 4, 5}, {6})",
            "RESHAPE({1:12}, 3, 4)",
            "{MAKE(2, 2, 7), IDENT(2, 3); MDIAG({1, 2, 3, 4, 5})}",
            "T({1, 2, 3; 4, 5, 6})",
            "{DIAG({1, 2, 3; 4, 5, 6}), DIAG({1, 2; 3, 4; 5, 6})}",
        ],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn linear_algebra() {
    let a = "{4, 2, 1; 2, 5, 3; 1, 3, 6}";
    compare_saves(
        "algebra",
        "",
        &[
            &format!("INV({a})"),
            &format!("DET({a})"),
            &format!("SOLVE({a}, {{1, 2; 3, 4; 5, 6}})"),
            &format!("CHOL({a})"),
            &format!("EVAL({a})"),
            "SVAL({1, 2; 3, 4; 5, 6})",
            "SVAL({1, 2, 3; 4, 5, 6})",
            "GINV({1, 2; 3, 4; 5, 6})",
            "GINV({1, 2; 2, 4})",
            &format!("SWEEP({a}, 2)"),
            "SWEEP({0, 2; 3, 4}, 1)",
            "GSCH({1, 2, 3; 4, 5, 6})",
            "{RANK({1, 2; 2, 4}), RANK(IDENT(3)), RANK({1, 2, 3; 4, 5, 6})}",
            &format!("{{TRACE({a}), MSUM(SSCP({a}))}}"),
            "MAGIC(5)",
            "MAGIC(7)",
        ],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn elementwise_and_summary_functions() {
    let m = "{1, -5, 2.5; 3, 2, -0.5}";
    compare_saves(
        "functions",
        "",
        &[
            &format!("ABS({m})"),
            &format!("EXP({m})"),
            "LN({1, 2, 10})",
            "LG10({1, 2, 1000})",
            "SQRT({0, 2, 9})",
            "{ARSIN({-1, 0, .5}), ARTAN({-1, 0, 2})}",
            &format!("{{COS({m}), SIN({m})}}"),
            "MOD({-7, 7, 7.5, -7.5}, 3)",
            &format!("TRUNC({m})"),
            "RND({-2.4, -1.6, 0.4, 1.6})",
            &format!("{{CMAX({m}); CMIN({m}); CSUM({m}); CSSQ({m})}}"),
            &format!("{{RMAX({m}), RMIN({m}), RSUM({m}), RSSQ({m})}}"),
            &format!("{{MMAX({m}), MMIN({m}), MSUM({m}), MSSQ({m}), NROW({m}), NCOL({m})}}"),
            "{ALL({1, 0}), ANY({1, 0}), ALL({1, -2}), ANY({0, 0})}",
            "GRADE({3, 1, 3; 2, 5, 1})",
            "RNKORDER({3, 1, 3; 2, 5, 1})",
            "DESIGN({1, 5; 2, 5; 1, 5; 3, 5})",
        ],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn distribution_functions() {
    compare_saves(
        "distributions",
        "",
        &[
            "{CDF.NORMAL({-1.96, 0, 1.96}, 0, 1), IDF.NORMAL(.975, 10, 2), PDF.NORMAL(0, 0, 1)}",
            "{CDF.T(2, 5), IDF.T(.95, 10), PDF.T(1, 3), TCDF(-1, 4)}",
            "{CDF.CHISQ(3, 2), IDF.CHISQ(.95, 3), PDF.CHISQ(2, 3), SIG.CHISQ(3.84, 1), CHICDF(5, 3)}",
            "{CDF.F(3, 2, 10), IDF.F(.95, 3, 100), PDF.F(1, 4, 6), SIG.F(3, 2, 10), FCDF(2, 3, 7)}",
            "{CDF.BETA(.3, 2, 3), IDF.BETA(.5, 2, 3), PDF.BETA(.3, 2, 3)}",
            "{CDF.GAMMA(2, 2, 1), IDF.GAMMA(.5, 2, 1), PDF.GAMMA(2, 2, 1)}",
            "{CDF.EXP(1, 2), IDF.EXP(.5, 2), PDF.EXP(1, 2)}",
            "{CDF.WEIBULL(2, 1.5, 2), IDF.WEIBULL(.5, 1.5, 2), PDF.WEIBULL(2, 1.5, 2)}",
            "{CDF.LNORMAL(2, 1, 1), IDF.LNORMAL(.5, 2, 1), PDF.LNORMAL(2, 1, 1)}",
            "{CDF.LOGISTIC(1, 0, 1), IDF.LOGISTIC(.7, 0, 1), PDF.LOGISTIC(1, 0, 2)}",
            "{CDF.LAPLACE(1, 0, 2), IDF.LAPLACE(.3, 0, 2)}",
            "{CDF.CAUCHY(1, 0, 2), IDF.CAUCHY(.3, 0, 2), PDF.CAUCHY(1, 0, 2)}",
            "{CDF.PARETO(4, 2, 3), IDF.PARETO(.5, 2, 3), PDF.PARETO(4, 2, 3)}",
            "{CDF.UNIFORM(.5, 0, 2), IDF.UNIFORM(.5, 0, 2), PDF.UNIFORM(.5, 0, 2)}",
            "{CDF.BINOM(3, 10, .3), PDF.BINOM(3, 10, .3), CDF.BERNOULLI(0, .3), PDF.BERNOULLI(1, .3)}",
            "{CDF.POISSON(2, 3), PDF.POISSON(2, 3), CDF.GEOM(3, .5), PDF.GEOM(3, .5)}",
            "{CDF.HYPER(1, 10, 3, 4), PDF.HYPER(1, 10, 3, 4)}",
            "{PDF.BVNOR(.5, .2, .3), CDFNORM(1), PROBIT(.975)}",
        ],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn control_flow_eigen_and_indexes() {
    compare(
        "control",
        "MATRIX.\n\
         COMPUTE S = 0.\nLOOP I = 1 TO 10 BY 3.\nDO IF I = 4.\nCOMPUTE S = S + 100.\n\
         ELSE IF I > 6.\nCOMPUTE S = S + 1000.\nELSE.\nCOMPUTE S = S + 1.\nEND IF.\nEND LOOP.\n\
         COMPUTE VEC = {5; 8; 13; 21}.\nLOOP L = 1 TO NROW(VEC).\nDO IF VEC(L) = 13.\nBREAK.\nEND IF.\nEND LOOP.\n\
         COMPUTE K = 0.\nLOOP IF K < 5.\nCOMPUTE K = K + 2.\nEND LOOP.\n\
         COMPUTE N = 0.\nLOOP J = 1 TO 100.\nCOMPUTE N = N + 1.\nEND LOOP.\n\
         COMPUTE M = MAKE(3, 3, 0).\nCOMPUTE M(2, :) = {1, 2, 3}.\nCOMPUTE M({1, 3}, {1, 2}) = {7, 8; 5, 6}.\n\
         CALL SETDIAG(M, {-1, -2}).\n\
         SAVE {S, I, L, K, N, J} /OUTFILE='flow.sav'.\nSAVE M /OUTFILE='index.sav'.\n\
         CALL EIGEN({4, 2, 1; 2, 5, 3; 1, 3, 6}, V, E).\nSAVE {E, ABS(V)} /OUTFILE='eigen.sav'.\n\
         END MATRIX.\n",
        &[],
        &["flow.sav", "index.sav", "eigen.sav"],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn get_save_and_read() {
    let data = "DATA LIST LIST /x y z.\nBEGIN DATA\n1 2 3\n4 5 6\n7 . 9\n10 11 12\nEND DATA.\n\
                MISSING VALUES z (12).\n";
    compare(
        "data",
        &format!(
            "{data}MATRIX.\nGET A /VARIABLES=x TO z /NAMES=NM /MISSING=ACCEPT /SYSMIS=-1.\n\
             SAVE A /OUTFILE='get.sav' /NAMES=NM.\n\
             READ R /FILE='in.txt' /FIELD=1 TO 30 BY 10 /SIZE={{2, 3}}.\n\
             READ Q /FIELD=1 TO 20 /SIZE={{2, 2}}.\n\
             READ TS /FIELD=1 TO 9 BY 3 /SIZE={{3, 3}} /MODE=SYMMETRIC.\n\
             SAVE R /OUTFILE='r.sav'.\nSAVE Q /OUTFILE='q.sav'.\nSAVE TS /OUTFILE='t.sav'.\n\
             SAVE {{1, 2; 3, 4}} /OUTFILE='twice.sav' /VARIABLES=a, b.\nSAVE {{5, 6}}.\n\
             END MATRIX.\n"
        ),
        &[(
            "in.txt",
            "      1.50      2.00      3.00\n      4.00      5.00      6.00\n1 2\n3 4\n  1\n  4  5\n  7  8  9\n",
        )],
        &["get.sav", "r.sav", "q.sav", "t.sav", "twice.sav"],
    );
}

#[test]
#[ignore = "needs PSPP"]
fn msave_and_mget() {
    compare(
        "msave",
        "MATRIX.\n\
         MSAVE {1, 2, 3} /TYPE=MEAN /OUTFILE='ms.sav' /VARIABLES=v1 TO v3.\n\
         MSAVE {1, 1, 1} /TYPE=STDDEV.\nMSAVE MAKE(1, 3, 10) /TYPE=N.\n\
         MSAVE {1, .5, .25; .5, 1, .1; .25, .1, 1} /TYPE=CORR.\nEND MATRIX.\n\
         MATRIX DATA VARIABLES=ROWTYPE_ a b c.\nBEGIN DATA\nMEAN 1 2 3\nSD 4 5 6\nN 7 7 7\n\
         CORR 1\nCORR .2 1\nCORR .3 .4 1\nEND DATA.\n\
         MATRIX.\nMGET.\nSAVE {MN; SD; NC; CR} /OUTFILE='mget.sav'.\nEND MATRIX.\n",
        &[],
        &["ms.sav", "mget.sav"],
    );
}
