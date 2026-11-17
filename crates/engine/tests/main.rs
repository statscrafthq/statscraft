//! The engine's integration tests, built as one test binary: a binary per file cost minutes of
//! linking after every engine edit and gigabytes of disk. A new test file needs a line here.

mod agreement;
mod alscal;
mod anova;
mod autorecode;
mod bayes;
mod bayes_loglinear;
mod bayes_models;
mod catpca;
mod cell_stats;
mod chart_input;
mod charts;
mod cluster;
mod complex_samples;
mod correspondence;
mod coxreg;
mod cs_cox;
mod cs_pps;
mod cs_regression;
mod cs_sampling;
mod ctables;
mod ctables_categories;
mod ctables_mrsets;
mod ctables_sigtest;
mod data_checks;
mod data_commands;
mod date_formats;
mod dictionary;
mod discriminant;
mod engine;
mod examine;
mod factor;
mod file_handle;
mod frequencies;
mod genlin;
mod get_data;
mod glm;
mod glm_multivariate;
mod graph;
mod inline_data;
mod input_program;
mod km_survival;
mod logistic;
mod loglinear;
mod loops;
mod macros;
mod matrix;
mod matrix_data;
mod mixed;
mod mult_response;
mod multiple_correspondence;
mod multiple_imputation;
mod mva;
mod nomreg;
mod npar;
mod nptests;
mod office;
mod olap_cubes;
mod output;
mod output_docs;
mod output_file;
mod output_file_checks;
mod output_modify;
mod pdf;
mod plots;
mod plugin;
mod plum;
mod portable;
mod power;
mod pplot_roc;
mod procedures;
mod proportions;
mod proximities;
mod proxscal;
mod quantile_regression;
mod random;
mod rank;
mod regression;
mod regression_more;
mod reliability;
mod report;
mod restructure;
mod robustness;
mod save_translate;
mod scratch;
mod settings;
mod split_filter;
mod star_join;
mod statistics;
mod summarize_codebook;
mod table_labels;
mod temporary_names;
mod timeseries;
mod transform;
mod tree;
mod ts_models;
mod ts_more;
mod twostep;
mod vega_lite;
mod workbook;

/// A test file left out of this list or of reference_main.rs would never be built.
#[test]
fn every_test_file_is_built() {
    let lists = [include_str!("main.rs"), include_str!("reference_main.rs")];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    for entry in std::fs::read_dir(dir).unwrap() {
        let name = entry.unwrap().file_name().into_string().unwrap();
        let Some(module) = name.strip_suffix(".rs") else {
            continue;
        };
        if module != "main" && module != "reference_main" {
            let line = format!("\nmod {module};\n");
            assert!(
                lists.iter().any(|l| l.contains(&line)),
                "add `mod {module};` to tests/main.rs"
            );
        }
    }
}
