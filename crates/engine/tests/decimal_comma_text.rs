//! SET DECIMAL=COMMA and the sentences a procedure writes with a number in them: a caption
//! ("R Squared = 0,720"), a column header ("Subset for alpha = 0,05"), a cell, a warning. Each
//! of about 190 commands runs on invented data under the setting, and no text of any table or
//! warning is left with a number written with a period. A number in the user's own texts, a
//! value label "2.5 kg", a string value "3.14", a variable label with "v1.2", stays as it is.

use statscraft_engine::session::Session;

const DATA: &str = "\
DATA LIST FREE /g h x z y b o s w.
BEGIN DATA
1 1 8.65 4.74 14.77 0 1 1 1
2 1 7.68 3.68 14.04 1 2 2 2
3 1 6.36 1.54 11.42 0 3 3 3
1 2 5.65 0.79 9.31 1 4 4 1
2 2 4.45 1.97 9.12 0 1 1 2
3 2 8.6 2.72 10.92 1 2 2 3
1 1 4.56 1.34 9.62 0 3 3 1
2 1 4.72 1.59 10.34 1 4 4 2
3 1 4.04 4.46 8.35 0 1 1 3
1 2 2.89 0.12 6.96 1 2 2 1
2 2 3.6 0.68 3.47 0 3 3 2
3 2 6.4 0.91 12.73 1 4 4 3
1 1 8.15 3.98 13.26 0 1 1 1
2 1 7.1 3.95 9.87 1 2 2 2
3 1 3.83 4.9 10.86 0 3 3 3
1 2 7.03 3.58 12.03 1 4 4 1
2 2 4.69 2.65 6.82 0 1 1 2
3 2 5.01 4.16 11.63 1 2 2 3
1 1 3.83 4.41 9.74 0 3 3 1
2 1 5.54 4.6 10.27 1 4 4 2
3 1 6.79 2.43 13.78 0 1 1 3
1 2 6.6 0.83 13.03 1 2 2 1
2 2 8.26 1.34 15.61 0 3 3 2
3 2 8.66 3.53 15.11 1 4 4 3
1 1 5.03 2.59 8.68 0 1 1 1
2 1 3.49 1.04 7.16 1 2 2 2
3 1 5.1 4.67 11.1 0 3 3 3
1 2 7.56 3.63 12.46 1 4 4 1
2 2 8.26 0.96 14.49 0 1 1 2
3 2 6.22 1.37 12.34 1 2 2 3
END DATA.
VALUE LABELS g 1 'a' 2 'b' 3 'c'.
";

const COMMANDS: &[&str] = &[
    "FREQUENCIES g h /STATISTICS=MEAN MEDIAN STDDEV SKEWNESS KURTOSIS /PERCENTILES=25 75.",
    "DESCRIPTIVES x z y /STATISTICS=MEAN STDDEV VARIANCE MIN MAX SKEWNESS KURTOSIS.",
    "MEANS y BY g h /CELLS=MEAN COUNT STDDEV MEDIAN.",
    "EXAMINE y BY g /PLOT=NONE /STATISTICS=DESCRIPTIVES.",
    "CROSSTABS g BY h /STATISTICS=CHISQ PHI LAMBDA /CELLS=COUNT EXPECTED.",
    "CORRELATIONS x z y /STATISTICS=DESCRIPTIVES.",
    "NONPAR CORR x z y.",
    "REGRESSION /DEPENDENT y /METHOD=ENTER x z.",
    "T-TEST GROUPS=h(1,2) /VARIABLES=y x.",
    "T-TEST PAIRS=x WITH z.",
    "T-TEST /TESTVAL=5 /VARIABLES=y.",
    "ONEWAY y BY g /STATISTICS=DESCRIPTIVES HOMOGENEITY /POSTHOC=TUKEY.",
    "GLM y BY g h WITH x.",
    "UNIANOVA y BY g WITH x /EMMEANS=TABLES(g).",
    "MANOVA y x BY g(1,3).",
    "LOGISTIC REGRESSION b WITH x z.",
    "NPAR TESTS /M-W=y BY h(1,2).",
    "NPAR TESTS /K-W=y BY g(1,3).",
    "NPAR TESTS /WILCOXON=x WITH z.",
    "NPAR TESTS /CHISQUARE=o.",
    "NPAR TESTS /RUNS(MEDIAN)=y.",
    "NPAR TESTS /K-S(NORMAL)=y.",
    "NPAR TESTS /BINOMIAL=b.",
    "FACTOR /VARIABLES x z y /PRINT=INITIAL EXTRACTION.",
    "RELIABILITY /VARIABLES=x z y /SUMMARY=TOTAL.",
    "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x z y.",
    "QUICK CLUSTER x z y /CRITERIA=CLUSTER(2).",
    "NOMREG o BY g WITH x.",
    "PLUM o BY g WITH x.",
    "GENLIN y BY g WITH x.",
    "GENLIN b (REFERENCE=FIRST) BY g WITH x /MODEL g x DISTRIBUTION=BINOMIAL LINK=LOGIT.",
    "CURVEFIT y WITH x.",
    "PARTIAL CORR x WITH y BY z.",
    "COXREG y /STATUS=b(1) /METHOD=ENTER x.",
    "SORT CASES BY x.",
    "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=x z y.",
    "SUMMARIZE /TABLES=x y /CELLS=COUNT MEAN.",
    "CTABLES /TABLE y BY g.",
    "OLAP CUBES y BY g /CELLS=MEAN COUNT.",
    "MODEL PROGRAM a=1 b=1.\nCOMPUTE PRED = a + b*x.\nNLR y WITH x /PRED PRED.",
    "SURVIVAL TABLE=y /INTERVALS=THRU 100 BY 10 /STATUS=b(1).",
    "CATREG VARIABLES=o x /ANALYSIS=o(LEVEL=NOMINAL) x(LEVEL=NUMERIC) /DISCRETIZATION=x(NONE).",
    "VARCOMP y BY g h /RANDOM=g.",
    "MIXED y BY g h WITH x /FIXED=g x /RANDOM=h.",
    "CORRESPONDENCE TABLE=g(1,3) BY o(1,4).",
    "ANOVA y BY g(1,3) h(1,2).",
    "HILOGLINEAR g(1,3) h(1,2) o(1,4) /MAXORDER=2.",
    "LOGLINEAR g(1,3) h(1,2) /DESIGN=g h.",
    "GENLOG g h /MODEL=POISSON /DESIGN=g h.",
    "MULT RESPONSE GROUPS=$m 'm' (g h (1,3)) /FREQUENCIES=$m.",
    "2SLS y WITH x z /INSTRUMENTS=x g.",
    "QUANTILE REGRESSION y WITH x z.",
    "PROBIT b OF o WITH x /LOG=NONE.",
    "ROC y BY b(1).",
    "KNN b BY g WITH x z.",
    "NAIVEBAYES b BY g WITH x z.",
    "TREE b [n] BY g [n] x [s] z [s].",
    "MVA VARIABLES=x z y.",
    "PLS y WITH x z.",
    "HOMALS VARIABLES=g(3) h(2) o(4).",
    "CATPCA VARIABLES=x z y /ANALYSIS=x z y.",
    "REPORT /VARIABLES=y x /BREAK=g /SUMMARY=MEAN.",
    "RATIO STATISTICS x WITH z /PRINT=COD MEAN.",
    "NPTESTS /INDEPENDENT TEST (y) GROUP (h).",
    "WEIGHTED KAPPA g h.",
    "CODEBOOK x y /STATISTICS=ALL.",
    "FREQUENCIES x /FORMAT=NOTABLE /NTILES=4 /STATISTICS=MODE.",
    "DESCRIPTIVES x /SAVE.\nLIST zx.",
    "REGRESSION /STATISTICS=COEFF R ANOVA CHANGE /DEPENDENT y /METHOD=STEPWISE x z /RESIDUALS=DURBIN /SAVE=PRED RESID.\nLIST pre_1 res_1.",
    "GRAPH /SCATTERPLOT=x WITH y.",
    "PPLOT /VARIABLES=y /DISTRIBUTION=NORMAL.",
    "FACTOR /VARIABLES x z y /EXTRACTION=PC /ROTATION=VARIMAX /PRINT=ROTATION.",
    "ALSCAL x z y.",
    "PRINCALS VARIABLES=g(3) h(2) o(4) /ANALYSIS=g h o.",
    "OVERALS VARIABLES=g(3) h(2) o(4) /ANALYSIS=g h o /SETS=3(1,1,1).",
    "ANACOR TABLE=g(1,3) BY o(1,4).",
    "PROPORTIONS b.",
    "BOOTSTRAP.\nDESCRIPTIVES y.",
    "GENLIN y BY g h WITH x /MODEL g h x /REPEATED SUBJECT=g.",
    "GLM y BY g /LMATRIX=g 1 -1 0.",
    "MANOVA y x WITH z BY g(1,3).",
    "SPCHART.",
    "DETECTANOMALY /VARIABLES CATEGORICAL=g CONTINUOUS=x z y.",
    "OPTIMAL BINNING /VARIABLES GUIDE=b BIN=x.",
    "KM y BY g /STATUS=b(1) /PRINT=MEAN.",
    "PARTIAL CORR x y BY z.",
    "GLM y BY g /POSTHOC=g(TUKEY BONFERRONI SCHEFFE LSD SNK DUNCAN GH T2 T3 C DUNNETT GT2 GABRIEL WALLER QREGW FREGW TUKEYB).",
    "UNIANOVA y BY g h /POSTHOC=g h(TUKEY GH) /EMMEANS=TABLES(g*h) COMPARE(g).",
    "ONEWAY y BY g /POSTHOC=SNK DUNCAN GH T2 T3 C DUNNETT GT2 GABRIEL WALLER QREGW FREGW TUKEYB BONFERRONI SIDAK SCHEFFE LSD /STATISTICS=BROWNFORSYTHE WELCH.",
    "ONEWAY y BY g /CONTRAST=1 -1 0 /CONTRAST=1 0 -1 /POLYNOMIAL=2.",
    "EXAMINE y BY g h /PLOT=BOXPLOT HISTOGRAM NPPLOT SPREADLEVEL /STATISTICS=DESCRIPTIVES EXTREME /PERCENTILES.",
    "MEANS y BY g BY h /STATISTICS=ANOVA LINEARITY.",
    "CROSSTABS g BY h BY o /STATISTICS=ALL /CELLS=ALL.",
    "CROSSTABS g BY h /STATISTICS=CMH(1) RISK KAPPA MCNEMAR ETA.",
    "NPAR TESTS /FRIEDMAN=x z y.",
    "NPAR TESTS /COCHRAN=b o.",
    "NPAR TESTS /MCNEMAR=b WITH b.",
    "NPAR TESTS /MEDIAN=y BY g(1,3).",
    "NPAR TESTS /JONCKHEERE=y BY g(1,3).",
    "T-TEST GROUPS=g(1,3) /VARIABLES=y.",
    "REGRESSION /DEPENDENT y /METHOD=STEPWISE x z /STATISTICS=ALL /RESIDUALS=ALL.",
    "LOGISTIC REGRESSION b WITH x z /METHOD=FSTEP(LR) /PRINT=ALL.",
    "NOMREG o BY g h WITH x z /PRINT=ALL.",
    "COXREG y /STATUS=b(1) /METHOD=FSTEP(LR) x z /PRINT=ALL.",
    "KM y BY g /STATUS=b(1) /TEST=LOGRANK BRESLOW TARONE /PLOT=SURVIVAL HAZARD.",
    "FACTOR /VARIABLES x z y /PRINT=ALL /PLOT=EIGEN ROTATION(1 2) /EXTRACTION=ML /ROTATION=PROMAX.",
    "DISCRIMINANT /GROUPS=g(1,3) /VARIABLES=x z y /METHOD=WILKS /STATISTICS=ALL /CLASSIFY=SEPARATE.",
    "CLUSTER x z y /METHOD=AVERAGE /PRINT=DISTANCE CLUSTER(2,3) /PLOT=DENDROGRAM.",
    "QUICK CLUSTER x z y /CRITERIA=CLUSTER(3) /PRINT=ALL.",
    "RELIABILITY /VARIABLES=x z y /MODEL=ALPHA /STATISTICS=ALL /ICC=MODEL(MIXED).",
    "CORRELATIONS x z y /STATISTICS=ALL.",
    "PARTIAL CORR x WITH y BY z /STATISTICS=ALL.",
    "GENLIN b BY g /MODEL g DISTRIBUTION=BINOMIAL LINK=LOGIT /PRINT=ALL.",
    "PLUM o BY g h WITH x /PRINT=ALL.",
    "MIXED y BY g h WITH x /FIXED=g h x /RANDOM=g | SUBJECT(h) /PRINT=SOLUTION TESTCOV.",
    "MANOVA y x z BY g(1,3) /PRINT=ALL.",
    "CORRESPONDENCE TABLE=g(1,3) BY o(1,3) /PRINT=ALL.",
    "PROBIT b OF s WITH x.",
    "CURVEFIT y WITH x /MODEL=ALL.",
    "SURVIVAL TABLE=y BY g(1,3) /INTERVALS=THRU 20 BY 2 /STATUS=b(1).",
    "DESCRIPTIVES x z y /SAVE.",
    "FREQUENCIES g h y /HISTOGRAM /BARCHART /PIECHART /PERCENTILES=10 90 /GROUPED=y.",
    "RANK VARIABLES=y BY g /NTILES(4) /RANK /NORMAL /PROPORTION.",
    "ROC y BY b(1) /PLOT=CURVE /PRINT=SE COORDINATES.",
    "RATIO STATISTICS x WITH z /PRINT=ALL.",
    "UNIANOVA y BY g h WITH x /EMMEANS=TABLES(g) COMPARE ADJ(SIDAK) /PRINT=ALL.",
];

fn has_period_in_a_number(text: &str) -> bool {
    text.as_bytes()
        .windows(3)
        .any(|w| w[0].is_ascii_digit() && w[1] == b'.' && w[2].is_ascii_digit())
}

fn run(source: &str) -> Vec<statscraft_engine::session::CommandResult> {
    Session::default()
        .run(source, None)
        .unwrap_or_else(|e| panic!("{e:?} in {source}"))
        .commands
}

#[test]
fn no_text_of_any_command_keeps_a_period_in_a_number() {
    let mut left: Vec<String> = Vec::new();
    for command in COMMANDS {
        let source = format!("{DATA}SET DECIMAL=COMMA.\n{command}\n");
        let Ok(result) = Session::default().run(&source, None) else {
            continue;
        };
        for done in result.commands {
            for table in &done.tables {
                let mut texts = vec![table.title.clone()];
                texts.extend(table.columns.iter().cloned());
                texts.extend(table.column_groups.iter().flatten().cloned());
                texts.extend(table.caption.clone());
                for cell in table.rows.iter().flatten() {
                    if let statscraft_engine::Cell::Text(t) = cell {
                        texts.push(t.clone());
                    }
                }
                left.extend(
                    texts
                        .into_iter()
                        .filter(|t| has_period_in_a_number(t))
                        .map(|t| format!("{command}: {t}")),
                );
            }
            left.extend(
                done.warnings
                    .iter()
                    .filter(|w| has_period_in_a_number(w))
                    .map(|w| format!("{command}: {w}")),
            );
        }
    }
    left.sort();
    left.dedup();
    assert!(left.is_empty(), "{}", left.join("\n"));
}

#[test]
fn the_users_own_texts_with_a_number_are_kept() {
    let source = "DATA LIST LIST(',') /w (F4.1) kind (A8) v1.2 (F4.1).
BEGIN DATA
1.5,3.14,2.5
2.5,3.14,3.5
3.5,2.71,4.5
4.5,2.71,5.5
END DATA.
VARIABLE LABELS w 'Weight 1.5 kg'.
VALUE LABELS v1.2 2.5 'Under 2.5 kg' 3.5 'Over 2.5 kg'.
SET DECIMAL=COMMA.
FREQUENCIES kind v1.2.
CROSSTABS kind BY v1.2.
MEANS w BY kind.
";
    let commands = run(source);
    let texts: Vec<String> = commands
        .iter()
        .flat_map(|c| &c.tables)
        .flat_map(|t| {
            let mut texts = vec![t.title.clone()];
            texts.extend(t.rows.iter().flatten().filter_map(|cell| match cell {
                statscraft_engine::Cell::Text(x) => Some(x.clone()),
                _ => None,
            }));
            texts
        })
        .collect();
    for kept in [
        "3.14",
        "2.71",
        "Under 2.5 kg",
        "Over 2.5 kg",
        "Weight 1.5 kg",
    ] {
        assert!(
            texts.iter().any(|t| t.contains(kept)),
            "{kept} in {texts:?}"
        );
    }
    assert!(
        !texts
            .iter()
            .any(|t| t.contains("3,14") || t.contains("2,5 kg")),
        "{texts:?}"
    );
}
