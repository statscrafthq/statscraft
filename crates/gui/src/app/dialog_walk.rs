//! Every dialog of the window, with the least it needs and with every option on, writes syntax
//! that the parser takes and that runs on the survey file.

use super::{CategoryDraft, Dialog, RuleDraft};
use crate::advanced_dialogs::{
    Advanced, ContrastTest, LinearMixed, MeansTable, Multinomial, Ordinal, SampleDescriptives,
    SampleOptions, SamplePlan, SampleTables, Stage, Terms,
};
use crate::agreement_dialog::{FleissKappa, Ratio, WeightedKappa};
use crate::analyze_dialogs::{
    Correlations, Explore, ExploreMissing, Means, MrGroup, MultResponse, Oneway, TTest, TTestKind,
    Tree,
};
use crate::bootstrap_dialog::Bootstrap;
use crate::catpca_dialog::{Catpca, CatpcaVariable};
use crate::catreg_dialog::{CategoricalRegression, CatregVariable};
use crate::conjoint_dialog::{
    ConjointAnalysis, Design, FactorModel, Level, OrthogonalPlan, PlanCards, PlanFactor,
};
use crate::correspondence_dialog::{Correspondence, Role, TableVariable};
use crate::cox_dialog::Cox;
use crate::cs_model_dialog::{CsCox, CsOrdinal, Held, OddsRatio, SampleModel};
use crate::cs_sample_dialog::{SampleSelection, SamplingPlan, SamplingStage};
use crate::data_dialogs::{
    Aggregate, AggregateTo, Aggregated, ExportData, ImportData, Make, MergeFiles, Mrsets,
    Restructure, RestructureKind, SelectCases, SelectMode, SortCases, SortVariables, SplitFile,
    SplitMode, WeightCases,
};
use crate::data_tools_dialog::{DataValidation, DatasetComparison, DictionaryCopy};
use crate::dialogs::testing::{fixture, open, v};
use crate::dialogs::{
    Adjust, Autorecode, Categories, Compute, Count, Crosstabs, CrosstabsMethod, CustomTables,
    Descriptives, Frequencies, Letters, NewValue, OldValue, Rank, Recode, RecodeRule, Subtotal,
};
use crate::forecast_more_dialog::{Areg, Spectra, TwoSls};
use crate::genlin_dialog::GeneralizedLinear;
use crate::glmm_dialog::{EmmeansBlock, GenlinMixed, RandomBlock};
use crate::graph_dialogs::{ChartBuilder, ChartKind, ProbabilityPlot, RocCurve};
use crate::imputation_dialog::{Imputation, Limits};
use crate::legacy_scaling_dialog::{Anacor, LegacyScaling, LegacyVariable, ScalingKind};
use crate::loglinear_dialog::{GeneralLoglinear, ModelSelection};
use crate::mca_dialog::{Mca, McaVariable};
use crate::mds_dialog::Prefscal;
use crate::mining_dialog::{Aim, FeatureSelection, PrepareData};
use crate::model_dialogs::{
    Categorical, ClusterMeasure, Discriminant, FactorAnalysis, FactorContrast, Hierarchical,
    KMeans, Logistic, MColumn, RepeatedMeasures, Solutions, Univariate, WithinFactor,
};
use crate::model_more_dialog::{OptimalBinning, QuantileRegression, VarianceComponents};
use crate::multivar_dialog::{Knn, Manova, Pls};
use crate::mva_dialog::{Estimation, MissingValueAnalysis};
use crate::naivebayes_dialog::NaiveBayes;
use crate::neural_dialog::{Common, Mlp, Rbf};
use crate::oms_dialog::{Oms, Omsend};
use crate::proportions_dialog::{Proportions, ProportionsKind};
use crate::proximities_dialog::Distances;
use crate::reports_dialog::{Codebook, OlapCubes, OlapDifference, Summarize};
use crate::response_rate_dialog::ResponseRate;
use crate::roc_analysis_dialog::RocAnalysis;
use crate::simulation_dialog::{
    CorrelationRow, FixedRow, SaveCases, SimInputRow, SimPlan, SimRun, Simulation,
};
use crate::spchart_dialog::{CAPABILITY, ControlCharts};
use crate::stats_dialogs::{Block, NparKind, NparTests, PartialCorr, Regression, Reliability};
use crate::survival_dialog::{KaplanMeier, LifeTables};
use crate::timeseries_dialog::{
    ArimaModel, Correlogram, CreateRow, CreateSeries, Dates, ExpSmoothing, IFFT, Parameter, Season,
    SeriesChart, SeriesOptions, TimeRange, TimeSeries,
};
use crate::tsmodel_dialog::{TsOutlier, TsOutputs, TsTransfer, Tsapply, Tsmodel};
use crate::twostep_dialog::{Anomaly, TwoStep};
use std::path::Path;

fn s(text: &str) -> String {
    text.to_string()
}

fn table_variable(name: &str, min: &str, max: &str, roles: &[(i64, Role)]) -> TableVariable {
    TableVariable {
        name: s(name),
        min: s(min),
        max: s(max),
        roles: roles.iter().copied().collect(),
    }
}

/// A plan factor with the levels 1, 2, ... named by `labels`.
fn plan_factor(name: &str, label: &str, labels: &[&str]) -> PlanFactor {
    PlanFactor {
        name: s(name),
        label: s(label),
        levels: labels
            .iter()
            .enumerate()
            .map(|(i, l)| Level {
                value: (i + 1).to_string(),
                label: s(l),
            })
            .collect(),
    }
}

/// A plan file made by ORTHOPLAN from `factors` and a data file with invented scores of five
/// subjects for its rated profiles, in `out`: the two paths and the number of rated profiles.
fn conjoint_files(out: &Path, tag: &str, factors: &str, holdout: usize) -> (String, String, usize) {
    let path = |what: &str| {
        out.join(format!("{tag}_{what}.sav"))
            .to_string_lossy()
            .into_owned()
    };
    let (plan, scores) = (path("plan"), path("scores"));
    let mut session = statscraft_engine::session::Session::default();
    let holdout = if holdout > 0 {
        format!(" /HOLDOUT={holdout}")
    } else {
        String::new()
    };
    session
        .run(
            &format!(
                "SET SEED=7.\nORTHOPLAN FACTORS={factors}{holdout} /OUTFILE={}.",
                crate::syntax_text::quote(&plan)
            ),
            None,
        )
        .unwrap();
    let n = statscraft_data::Dataset::read_sav(&plan)
        .unwrap()
        .case_count();
    let rows: Vec<String> = (1..=5)
        .map(|subject| {
            let scores: Vec<String> = (1..=n)
                .map(|k| ((k * (subject + 1) + subject * 3) % 9 + 1).to_string())
                .collect();
            format!("{subject} {}", scores.join(" "))
        })
        .collect();
    let columns: Vec<String> = (1..=n).map(|k| format!("s{k}")).collect();
    let mut data = statscraft_engine::session::Session::default();
    data.run(
        &format!(
            "DATA LIST LIST /id {}.\nBEGIN DATA\n{}\nEND DATA.\nSAVE OUTFILE={}.",
            columns.join(" "),
            rows.join("\n"),
            crate::syntax_text::quote(&scores)
        ),
        None,
    )
    .unwrap();
    (plan, scores, n)
}

/// Each dialog with the fewest choices that make its syntax.
/// The least a HOMALS, PRINCALS or OVERALS dialog needs: two variables, a set each for OVERALS.
fn legacy_scaling(kind: ScalingKind) -> LegacyScaling {
    let mut second = LegacyVariable::new("region", "5");
    second.set = 2;
    LegacyScaling {
        variables: v(&["gender", "region"]),
        settings: vec![LegacyVariable::new("gender", "2"), second],
        ..LegacyScaling::new(kind)
    }
}

/// Four variables at mixed levels, two in each of two sets, for the walk with every option on.
fn legacy_four(kind: ScalingKind) -> LegacyScaling {
    let setting = |name: &str, max: &str, level: usize, set: usize| LegacyVariable {
        level,
        set,
        ..LegacyVariable::new(name, max)
    };
    LegacyScaling {
        variables: v(&["gender", "region", "q6_mention1", "q6_mention2"]),
        settings: vec![
            setting("gender", "2", 3, 1),
            setting("region", "5", 1, 2),
            setting("q6_mention1", "6", 2, 1),
            setting("q6_mention2", "6", 0, 2),
        ],
        ..LegacyScaling::new(kind)
    }
}

fn minimal(out: &Path) -> Vec<Dialog> {
    let plan = out.join("min_plan.json").to_string_lossy().into_owned();
    let mut list = vec![
        Dialog::Frequencies(Frequencies {
            variables: v(&["gender"]),
            ..Default::default()
        }),
        Dialog::Descriptives(Descriptives {
            variables: v(&["age"]),
            ..Default::default()
        }),
        Dialog::Explore(Explore {
            dependents: v(&["age"]),
            ..Default::default()
        }),
        Dialog::Crosstabs(Crosstabs {
            rows: v(&["gender"]),
            columns: v(&["region"]),
            ..Default::default()
        }),
        Dialog::CustomTables(
            CustomTables {
                rows: v(&["gender"]),
                ..Default::default()
            },
            CategoryDraft::default(),
        ),
        Dialog::Compute(
            Compute {
                target: s("decade"),
                expression: s("age / 10"),
                ..Default::default()
            },
            String::new(),
        ),
        Dialog::Recode(
            Recode {
                variables: v(&["gender"]),
                rules: vec![RecodeRule {
                    old: OldValue::Value(s("1")),
                    new: NewValue::Value(s("0")),
                }],
                ..Default::default()
            },
            RuleDraft::default(),
        ),
        Dialog::Count(
            Count {
                target: s("ones"),
                variables: v(&["q5_1", "q5_2"]),
                values: vec![OldValue::Value(s("1"))],
            },
            String::new(),
            RuleDraft::default(),
        ),
        Dialog::Rank(Rank {
            variables: v(&["age"]),
            ..Default::default()
        }),
        Dialog::Autorecode(Autorecode {
            variables: v(&["plan"]),
            into: s("plan_n"),
            ..Default::default()
        }),
        Dialog::Means(Means {
            dependents: v(&["age"]),
            ..Default::default()
        }),
        Dialog::Oneway(Oneway {
            dependents: v(&["age"]),
            factor: s("region"),
            ..Default::default()
        }),
        Dialog::Correlations(Correlations {
            variables: v(&["age", "monthly_spend"]),
            ..Default::default()
        }),
        Dialog::Mrsets(Mrsets {
            name: s("$q5"),
            variables: v(&["q5_1", "q5_2"]),
            ..Default::default()
        }),
        Dialog::SortCases(SortCases {
            keys: vec![(s("age"), false)],
        }),
        Dialog::SplitFile(SplitFile {
            variables: v(&["region"]),
            ..Default::default()
        }),
        Dialog::SelectCases(SelectCases {
            mode: SelectMode::All,
            ..Default::default()
        }),
        Dialog::WeightCases(WeightCases::default()),
        Dialog::Aggregate(
            Aggregate {
                breaks: v(&["region"]),
                variables: vec![Aggregated::new(0, "age")],
                ..Default::default()
            },
            0,
        ),
        Dialog::MergeFiles(MergeFiles {
            path: fixture("survey_basic.sav"),
            ..Default::default()
        }),
        Dialog::Advanced(Advanced::Validate(Box::new(DataValidation {
            variables: v(&["age"]),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Compare(Box::new(DatasetComparison {
            file: fixture("survey_basic.sav"),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::CopyDictionary(Box::new(DictionaryCopy {
            file: fixture("survey_basic.sav"),
            ..Default::default()
        }))),
        Dialog::MultResponse(MultResponse {
            groups: vec![MrGroup {
                name: s("q5"),
                variables: v(&["q5_1", "q5_2"]),
                ..Default::default()
            }],
            ..Default::default()
        }),
        Dialog::Tree(Tree {
            dependent: s("gender"),
            predictors: vec![(s("region"), 0)],
            ..Default::default()
        }),
        Dialog::Regression(Regression {
            dependent: s("monthly_spend"),
            blocks: vec![Block {
                variables: v(&["age"]),
                method: 0,
            }],
            ..Default::default()
        }),
        Dialog::Reliability(Reliability {
            items: v(&["q5_1", "q5_2", "q5_3"]),
            ..Default::default()
        }),
        // omega, and with the item totals omega if an item is deleted
        Dialog::Reliability(Reliability {
            items: v(&["q5_1", "q5_2", "q5_3", "q5_4"]),
            model: 5,
            descriptives: [false, false, true],
            ..Default::default()
        }),
        Dialog::PartialCorr(PartialCorr {
            variables: v(&["age", "monthly_spend"]),
            controls: v(&["q2_recommend"]),
            ..Default::default()
        }),
        Dialog::FactorAnalysis(FactorAnalysis {
            variables: v(&["q5_1", "q5_2", "q5_3", "q5_4", "q5_5"]),
            ..Default::default()
        }),
        Dialog::KMeans(KMeans {
            variables: v(&["age", "monthly_spend"]),
            ..Default::default()
        }),
        Dialog::TwoStep(TwoStep {
            continuous: v(&["age", "monthly_spend"]),
            ..Default::default()
        }),
        Dialog::Anomaly(Anomaly {
            scale: v(&["age", "monthly_spend"]),
            ..Default::default()
        }),
        Dialog::Mlp(Mlp {
            common: Common {
                dependents: v(&["monthly_spend"]),
                covariates: v(&["age"]),
                ..Default::default()
            },
            ..Default::default()
        }),
        Dialog::Rbf(Rbf {
            common: Common {
                dependents: v(&["monthly_spend"]),
                covariates: v(&["age"]),
                ..Default::default()
            },
            ..Default::default()
        }),
        Dialog::Hierarchical(Hierarchical {
            variables: v(&["age", "monthly_spend"]),
            ..Default::default()
        }),
        Dialog::Discriminant(Discriminant {
            group: s("gender"),
            min: s("1"),
            max: s("2"),
            variables: v(&["age", "monthly_spend"]),
            ..Default::default()
        }),
        Dialog::Logistic(Logistic {
            dependent: s("q5_1"),
            blocks: vec![Block {
                variables: v(&["age"]),
                method: 0,
            }],
            ..Default::default()
        }),
        Dialog::Univariate(Univariate {
            dependent: s("monthly_spend"),
            factors: v(&["region"]),
            ..Default::default()
        }),
        Dialog::Univariate(Univariate {
            dependents: v(&["monthly_spend", "q1_satisfaction"]),
            factors: v(&["region"]),
            ..Univariate::multivariate()
        }),
        Dialog::RepeatedMeasures(RepeatedMeasures {
            within: vec![WithinFactor {
                name: s("mention"),
                levels: s("3"),
            }],
            dependents: v(&["q6_mention1", "q6_mention2", "q6_mention3"]),
            ..Default::default()
        }),
        Dialog::Advanced(Advanced::Ordinal(Ordinal {
            dependent: s("q1_satisfaction"),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Multinomial(Multinomial {
            dependent: s("plan"),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Mixed(LinearMixed {
            dependent: s("monthly_spend"),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Genlin(Box::new(GeneralizedLinear {
            dependent: s("monthly_spend"),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Glmm(Box::new(GenlinMixed {
            subjects: v(&["region"]),
            target: s("monthly_spend"),
            inputs: v(&["age"]),
            random: vec![RandomBlock {
                subjects: v(&["region"]),
                ..Default::default()
            }],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Glmm(Box::new(GenlinMixed {
            subjects: v(&["region"]),
            target: s("q1_satisfaction"),
            distribution: 7,
            inputs: v(&["age"]),
            random: vec![RandomBlock {
                subjects: v(&["region"]),
                ..Default::default()
            }],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Correspondence(Correspondence {
            row: table_variable("region", "1", "5", &[]),
            column: table_variable("q6_mention1", "1", "6", &[]),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Mca(Box::new(Mca {
            variables: v(&["gender", "region"]),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Catpca(Box::new(Catpca {
            variables: v(&["gender", "region"]),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Homals(Box::new(legacy_scaling(
            ScalingKind::Homals,
        )))),
        Dialog::Advanced(Advanced::Princals(Box::new(legacy_scaling(
            ScalingKind::Princals,
        )))),
        Dialog::Advanced(Advanced::Overals(Box::new(legacy_scaling(
            ScalingKind::Overals,
        )))),
        Dialog::Advanced(Advanced::Anacor(Box::new(Anacor {
            row: table_variable("region", "1", "5", &[]),
            column: table_variable("q6_mention1", "1", "6", &[]),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Distances(Distances {
            variables: v(&["age", "monthly_spend"]),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::LifeTables(LifeTables {
            times: v(&["age"]),
            intervals: vec![(s("60"), s("10"))],
            status: s("q5_1"),
            event_from: s("1"),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Km(KaplanMeier {
            time: s("age"),
            status: s("q5_1"),
            event_values: s("1"),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Cox(Box::new(Cox {
            time: s("age"),
            status: s("q5_1"),
            event_values: s("1"),
            blocks: vec![Block {
                variables: v(&["monthly_spend"]),
                method: 0,
            }],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Mva(Box::new(MissingValueAnalysis {
            quantitative: v(&["age", "q2_recommend"]),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Imputation(Box::new(Imputation {
            variables: v(&["age", "monthly_spend", "q2_recommend"]),
            dataset: s("imputed"),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Summarize(Box::new(Summarize {
            means: Means {
                dependents: v(&["age"]),
                ..Default::default()
            },
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Codebook(Codebook::default())),
        Dialog::Advanced(Advanced::Olap(Box::new(OlapCubes {
            summary: v(&["monthly_spend"]),
            layers: vec![v(&["gender"])],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Proportions(Box::new(Proportions {
            variables: v(&["gender"]),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::WeightedKappa(WeightedKappa {
            raters: v(&["q5_1", "q5_2"]),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Fleiss(FleissKappa {
            raters: v(&["q5_1", "q5_2", "q5_3"]),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Ratio(Box::new(Ratio {
            numerator: s("monthly_spend"),
            denominator: s("age"),
            statistics: [true; 12],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Hiloglinear(Box::new(ModelSelection {
            factors: v(&["gender", "region"]),
            ranges: vec![
                table_variable("gender", "1", "2", &[]),
                table_variable("region", "1", "4", &[]),
            ],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Genlog(Box::new(GeneralLoglinear {
            factors: v(&["gender", "region"]),
            ..GeneralLoglinear::new(false)
        }))),
        Dialog::Advanced(Advanced::Genlog(Box::new(GeneralLoglinear {
            dependents: v(&["gender"]),
            factors: v(&["region"]),
            ..GeneralLoglinear::new(true)
        }))),
        // the sampling plan before the selection that reads it
        Dialog::Advanced(Advanced::Sampling(SamplingPlan {
            path: out.join("min_draw.json").to_string_lossy().into_owned(),
            stages: vec![SamplingStage {
                strata: v(&["region"]),
                amount: s("2"),
                ..Default::default()
            }],
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Selection(SampleSelection {
            plan: out.join("min_draw.json").to_string_lossy().into_owned(),
            ..Default::default()
        })),
        // the plan before the analyses that read it
        Dialog::Advanced(Advanced::Plan(SamplePlan {
            path: plan.clone(),
            weight: s("weight"),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Descriptives(SampleDescriptives {
            plan: plan.clone(),
            summary: v(&["age"]),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Tables(SampleTables {
            plan: plan.clone(),
            variables: v(&["gender"]),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("monthly_spend"),
            ..SampleModel::new(false)
        }))),
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("q5_1"),
            ..SampleModel::new(true)
        }))),
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("q5_3"),
            ..SampleModel::new_ordinal()
        }))),
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("age"),
            covariates: v(&["monthly_spend"]),
            cox: Some(CsCox {
                status: s("q5_1"),
                event_values: s("1"),
                ..Default::default()
            }),
            ..SampleModel::new_cox()
        }))),
        Dialog::Advanced(Advanced::Tables(SampleTables {
            crosstabs: true,
            plan,
            variables: v(&["gender"]),
            column: s("q5_1"),
            ..Default::default()
        })),
        Dialog::Restructure(Restructure {
            makes: vec![Make {
                name: s("mention"),
                from: v(&["q6_mention1", "q6_mention2", "q6_mention3"]),
            }],
            ..Default::default()
        }),
        Dialog::SortVariables(SortVariables::default()),
        Dialog::ImportData(ImportData::new(&out.join("in.csv"))),
        Dialog::ExportData(ExportData {
            path: out.join("min.csv").to_string_lossy().into_owned(),
            ..Default::default()
        }),
    ];
    for kind in [
        TTestKind::OneSample,
        TTestKind::Independent,
        TTestKind::Paired,
    ] {
        let mut d = TTest::new(kind);
        d.variables = v(&["age"]);
        d.grouping = s("gender");
        d.group1 = s("1");
        d.group2 = s("2");
        d.pairs = vec![(s("age"), s("monthly_spend"))];
        list.push(Dialog::TTest(d));
    }
    for kind in ChartKind::ALL {
        // the fewest roles each chart needs
        list.push(Dialog::ChartBuilder(ChartBuilder {
            kind,
            function: if kind == ChartKind::HighLow { 4 } else { 0 },
            variables: match kind {
                ChartKind::HighLow | ChartKind::OverlayScatter | ChartKind::MatrixScatter => {
                    v(&["age", "monthly_spend"])
                }
                _ => v(&["age"]),
            },
            y_variables: v(&["q2_recommend"]),
            category: s("region"),
            group: if kind.group().is_some() && kind != ChartKind::ErrorBar {
                s("gender")
            } else {
                String::new()
            },
            x: s("age"),
            y: s("monthly_spend"),
            ..Default::default()
        }));
    }
    for pp in [false, true] {
        list.push(Dialog::ProbabilityPlot(ProbabilityPlot {
            variables: v(&["age"]),
            ..ProbabilityPlot::new(pp)
        }));
    }
    list.push(Dialog::Advanced(Advanced::TimeSeries(Box::new(
        TimeSeries::TwoSls(TwoSls {
            dependent: s("monthly_spend"),
            explanatory: v(&["age"]),
            instruments: v(&["age", "q1_satisfaction"]),
            ..Default::default()
        }),
    ))));
    list.push(Dialog::QuantileRegression(QuantileRegression {
        dependent: s("monthly_spend"),
        covariates: v(&["age"]),
        ..Default::default()
    }));
    list.push(Dialog::VarianceComponents(VarianceComponents {
        dependent: s("monthly_spend"),
        random: v(&["region"]),
        ..Default::default()
    }));
    list.push(Dialog::OptimalBinning(OptimalBinning {
        variables: v(&["age"]),
        ..Default::default()
    }));
    list.push(Dialog::Aim(Box::new(Aim {
        group: s("region"),
        continuous: v(&["age"]),
        ..Default::default()
    })));
    list.push(Dialog::FeatureSelection(Box::new(FeatureSelection {
        target: s("plan"),
        covariates: v(&["age", "monthly_spend"]),
        ..Default::default()
    })));
    list.push(Dialog::PrepareData(Box::new(PrepareData {
        inputs: v(&["age", "monthly_spend"]),
        outfile: out.join("min_prep.sps").to_string_lossy().into_owned(),
        ..Default::default()
    })));
    let (cj_plan, _, _) = conjoint_files(out, "min", "a (1 2) b (1 2) c (1 2)", 0);
    list.push(Dialog::Advanced(Advanced::Conjoint(Box::new(
        Design::Generate(OrthogonalPlan {
            factors: vec![plan_factor("a", "", &["", ""])],
            ..Default::default()
        }),
    ))));
    list.push(Dialog::Advanced(Advanced::Conjoint(Box::new(
        Design::Display(PlanCards {
            factors: v(&["gender", "region"]),
            ..Default::default()
        }),
    ))));
    // four survey columns without missing values stand in for the responses to four profiles
    list.push(Dialog::Advanced(Advanced::Conjoint(Box::new(
        Design::Analysis(Box::new(ConjointAnalysis {
            plan_from_file: true,
            plan_file: cj_plan,
            responses: s("age gender region resp_id"),
            subject: s("resp_id"),
            ..Default::default()
        })),
    ))));
    list.extend(simulation_dialogs(out, "min", false));
    list.push(Dialog::RocCurve(RocCurve {
        tests: v(&["age"]),
        state: s("q5_1"),
        value: s("1"),
        ..Default::default()
    }));
    list.push(Dialog::RocAnalysis(Box::new(RocAnalysis {
        tests: v(&["age"]),
        state: s("q5_1"),
        value: s("1"),
        ..Default::default()
    })));
    list.push(Dialog::ResponseRate(ResponseRate {
        numerator: s("q1_satisfaction"),
        denominator: s("age"),
        group: s("region"),
        ..Default::default()
    }));
    list.push(Dialog::ControlCharts(Box::new(ControlCharts {
        process: s("age"),
        group: s("region"),
        ..Default::default()
    })));
    list.push(Dialog::Catreg(Box::new(CategoricalRegression {
        dependent: s("q1_satisfaction"),
        independents: v(&["age", "q2_recommend"]),
        ..Default::default()
    })));
    for kind in NparKind::ALL {
        let mut d = NparTests::new(kind);
        d.variables = v(&["q1_satisfaction", "q2_recommend"]);
        d.pairs = vec![(s("q1_satisfaction"), s("q2_recommend"))];
        d.grouping = s("gender");
        d.low = s("1");
        d.high = s("2");
        list.push(Dialog::NparTests(d));
    }
    list.push(Dialog::Advanced(Advanced::Manova(Box::new(Manova {
        dependents: v(&["monthly_spend"]),
        factors: v(&["region"]),
        ranges: [(s("region"), (s("1"), s("5")))].into_iter().collect(),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Pls(Box::new(Pls {
        scale: v(&["monthly_spend"]),
        covariates: v(&["age", "q2_recommend"]),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Knn(Box::new(Knn {
        target: s("gender"),
        covariates: v(&["age", "monthly_spend"]),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::NaiveBayes(Box::new(
        NaiveBayes {
            target: s("gender"),
            factors: v(&["region"]),
            covariates: v(&["age", "monthly_spend"]),
            ..Default::default()
        },
    ))));
    list.push(Dialog::Advanced(Advanced::Prefscal(Box::new(Prefscal {
        variables: v(&["age", "monthly_spend", "q1_satisfaction", "q2_recommend"]),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Oms(Oms {
        file: out.join("min_oms.sav").to_string_lossy().into_owned(),
        ..Default::default()
    })));
    list.push(Dialog::Advanced(Advanced::Omsend(Default::default())));
    list
}

/// A bootstrap section with every option set but BCa, few samples to keep the walk quick. BCa
/// reruns the procedure once per case, which made the walk of One-Way ANOVA alone take fifteen
/// minutes; Descriptives walks it instead, see `boot_bca`.
fn boot() -> Bootstrap {
    Bootstrap {
        on: true,
        samples: s("20"),
        level: s("90"),
        bca: false,
        stratified: true,
        strata: v(&["gender"]),
    }
}

/// The bootstrap with BCa intervals; every dialog writes it with the same code.
fn boot_bca() -> Bootstrap {
    Bootstrap {
        bca: true,
        ..boot()
    }
}

/// Each dialog with every option on, or the last of a choice.
fn maximal(out: &Path) -> Vec<Dialog> {
    let plan = out.join("max_plan.json").to_string_lossy().into_owned();
    let terms = |list: &[&[&str]]| Terms {
        terms: list.iter().map(|t| v(t)).collect(),
        building: Vec::new(),
    };
    let sample_options = || SampleOptions {
        statistics: [true; 5],
        ci: true,
        ci_level: s("90"),
        subpopulations: v(&["gender"]),
        separate: true,
        listwise: true,
        include: true,
    };
    let mut list = vec![
        Dialog::Frequencies(Frequencies {
            variables: v(&["gender", "age"]),
            statistics: [true; 9],
            quartiles: true,
            ntiles: s("10"),
            percentiles: s("2.5, 97.5"),
            tables: true,
            order: 3,
            limit: s("20"),
            bootstrap: boot(),
        }),
        Dialog::Descriptives(Descriptives {
            variables: v(&["age", "monthly_spend"]),
            statistics: [true; 10],
            order: 3,
            save_z_scores: true,
            listwise: true,
            bootstrap: boot_bca(),
        }),
        Dialog::Explore(Explore {
            bootstrap: boot(),
            dependents: v(&["age", "monthly_spend"]),
            factors: v(&["gender"]),
            id: s("resp_id"),
            descriptives: true,
            confidence: s("90"),
            mestimators: true,
            outliers: true,
            percentiles: true,
            normality: true,
            missing: ExploreMissing::Report,
        }),
        Dialog::Crosstabs(Crosstabs {
            bootstrap: boot(),
            rows: v(&["gender"]),
            columns: v(&["region"]),
            layers: v(&["q5_1"]),
            cells: [true; 8],
            statistics: [true; 15],
            cmh_odds: s("2"),
            method: CrosstabsMethod::MonteCarlo,
            mc_level: s("95"),
            mc_samples: s("200"),
            ..Default::default()
        }),
        Dialog::Crosstabs(Crosstabs {
            rows: v(&["gender"]),
            columns: v(&["plan"]),
            statistics: [true; 15],
            method: CrosstabsMethod::Exact,
            exact_minutes: s("1"),
            ..Default::default()
        }),
        Dialog::CustomTables(
            CustomTables {
                rows: v(&["gender", "$mentions"]),
                columns: v(&["region"]),
                layers: v(&["q5_1"]),
                scale_rows: v(&["age"]),
                count: true,
                effective_count: true,
                effective_weight: s("weight"),
                categories: vec![Categories {
                    descending: true,
                    total: true,
                    total_label: s("All"),
                    before: true,
                    subtotals: vec![Subtotal {
                        label: s("Male or female"),
                        values: v(&["1", "2"]),
                    }],
                    hide_sources: true,
                    missing: true,
                    other: true,
                    ..Categories::new("gender", true)
                }],
                column_percent: true,
                row_percent: true,
                mean: true,
                totals: true,
                chi_square: true,
                compare_proportions: true,
                compare_means: true,
                alpha: s("0.01"),
                adjust: Adjust::BenjaminiHochberg,
                letters: Letters::ALL[2],
                hide_small_counts: true,
                small_count: s("5"),
            },
            CategoryDraft::default(),
        ),
        Dialog::Compute(
            Compute {
                target: s("decade"),
                expression: s("age / 10"),
                condition: s("gender = 1"),
            },
            s("Age in decades"),
        ),
        Dialog::Recode(
            Recode {
                variables: v(&["age", "monthly_spend"]),
                into: s("age_r spend_r"),
                rules: vec![
                    RecodeRule {
                        old: OldValue::Missing,
                        new: NewValue::SystemMissing,
                    },
                    RecodeRule {
                        old: OldValue::SystemMissing,
                        new: NewValue::Value(s("-1")),
                    },
                    RecodeRule {
                        old: OldValue::Value(s("1")),
                        new: NewValue::Value(s("0")),
                    },
                    RecodeRule {
                        old: OldValue::Range(s("2"), s("9")),
                        new: NewValue::Value(s("1")),
                    },
                    RecodeRule {
                        old: OldValue::LowestThru(s("0")),
                        new: NewValue::Value(s("2")),
                    },
                    RecodeRule {
                        old: OldValue::ThruHighest(s("100")),
                        new: NewValue::Value(s("3")),
                    },
                    RecodeRule {
                        old: OldValue::Else,
                        new: NewValue::Copy,
                    },
                ],
            },
            RuleDraft::default(),
        ),
        Dialog::Count(
            Count {
                target: s("ones"),
                variables: v(&["q5_1", "q5_2", "q5_3"]),
                values: vec![
                    OldValue::Value(s("1")),
                    OldValue::Range(s("2"), s("3")),
                    OldValue::LowestThru(s("-1")),
                    OldValue::ThruHighest(s("9")),
                    OldValue::Missing,
                    OldValue::SystemMissing,
                ],
            },
            s("Ones"),
            RuleDraft::default(),
        ),
        Dialog::Rank(Rank {
            variables: v(&["age"]),
            descending: true,
            by: v(&["gender"]),
            functions: [true; 7],
            ntiles: true,
            groups: s("4"),
            into: ["r1", "r2", "r3", "r4", "r5", "r6", "r7", "r8"].map(String::from),
            ties: 3,
            fraction: 3,
            print: true,
            include_missing: true,
        }),
        Dialog::Autorecode(Autorecode {
            variables: v(&["plan", "q3_comment"]),
            into: s("plan_n comment_n"),
            descending: true,
            group: true,
            blank_missing: true,
            print: true,
        }),
        Dialog::Means(Means {
            dependents: v(&["age", "monthly_spend"]),
            layers: vec![v(&["gender"]), v(&["region"])],
            layer: 1,
            cells: [true; 21],
            within: s("region"),
            npct_within: true,
            spct_within: true,
            anova: true,
            linearity: true,
            bootstrap: boot(),
        }),
        // no bootstrap: Games-Howell, Dunnett's C and the other range tests take seconds a run in a
        // debug build, times every bootstrap sample; the bootstrap section writes its syntax with
        // the same code for every dialog
        Dialog::Oneway(Oneway {
            bootstrap: Bootstrap::default(),
            dependents: v(&["age", "monthly_spend"]),
            factor: s("region"),
            statistics: [true; 5],
            polynomial: 3,
            post_hoc: [true; 16],
            dunnett: true,
            dunnett_first: true,
            dunnett_side: 2,
            alpha: s("0.01"),
            listwise: true,
            confidence: s("90"),
        }),
        Dialog::Correlations(Correlations {
            variables: v(&["age", "monthly_spend", "q2_recommend"]),
            pearson: true,
            kendall: true,
            spearman: true,
            one_tailed: true,
            flag: true,
            descriptives: true,
            cross_products: true,
            listwise: true,
            ci: true,
            ci_level: s("90"),
            ci_bias: true,
            spearman_ci: 1,
            bootstrap: boot(),
        }),
        Dialog::Mrsets(Mrsets {
            delete: false,
            dichotomy: true,
            name: s("$q5"),
            label: String::new(),
            variables: v(&["q5_1", "q5_2", "q5_3"]),
            value: s("1"),
            counted_value_labels: true,
            label_from_variables: true,
            sets: Vec::new(),
        }),
        Dialog::Mrsets(Mrsets {
            delete: false,
            dichotomy: false,
            name: s("$q6"),
            label: s("Mentions"),
            variables: v(&["q6_mention1", "q6_mention2"]),
            ..Default::default()
        }),
        Dialog::Mrsets(Mrsets {
            delete: true,
            sets: v(&["$mentions"]),
            ..Default::default()
        }),
        Dialog::SortCases(SortCases {
            keys: vec![(s("region"), true), (s("age"), false)],
        }),
        Dialog::SplitFile(SplitFile {
            mode: SplitMode::Separate,
            variables: v(&["region", "gender"]),
            sort: true,
        }),
        Dialog::SplitFile(SplitFile {
            mode: SplitMode::Off,
            ..Default::default()
        }),
        Dialog::SelectCases(SelectCases {
            mode: SelectMode::Filter,
            condition: s("age >= 18 AND gender = 2"),
        }),
        Dialog::SelectCases(SelectCases {
            mode: SelectMode::Delete,
            condition: s("age >= 18"),
        }),
        Dialog::WeightCases(WeightCases {
            on: true,
            variable: s("weight"),
        }),
        Dialog::Aggregate(
            Aggregate {
                breaks: v(&["region", "gender"]),
                variables: (0..crate::data_dialogs::AGGREGATE_FUNCTIONS.len())
                    .map(|f| Aggregated::new(f, "age"))
                    .collect(),
                count: true,
                count_name: s("cases"),
                to: AggregateTo::File,
                path: out.join("agg.sav").to_string_lossy().into_owned(),
            },
            0,
        ),
        Dialog::MergeFiles(MergeFiles {
            add_variables: true,
            path: fixture("survey_basic.sav"),
            table: true,
            by: v(&["resp_id"]),
        }),
        Dialog::Advanced(Advanced::Validate(Box::new(DataValidation {
            variables: v(&["age", "monthly_spend", "gender"]),
            ids: v(&["resp_id"]),
            rules: s("ALL"),
            pct_missing: s("40"),
            pct_equal: s("80"),
            pct_unequal: s("50"),
            cv: s("0.01"),
            std_dev: s("0.5"),
            incomplete_ids: false,
            report_empty: false,
            all_variables: true,
            by_variable: true,
            by_rule: true,
            min_violations: s("2"),
            case_limit: s("20"),
            save: [true; 4],
            save_names: [s("emptyflag"), s("incflag"), s("dupgroup"), s("nviol")],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Compare(Box::new(DatasetComparison {
            file: fixture("survey_basic.sav"),
            variables: v(&["age", "monthly_spend"]),
            ids: v(&["resp_id"]),
            flag_name: s("changed"),
            matches: 1,
            match_file: out.join("matched.sav").to_string_lossy().into_owned(),
            mismatches: 2,
            mismatch_dataset: s("review"),
            properties: [true; 9],
            table_limit: s("50"),
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Compare(Box::new(DatasetComparison {
            file: fixture("survey_basic.sav"),
            flag: false,
            case_table: false,
            properties: [true; 9],
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::CopyDictionary(Box::new(DictionaryCopy {
            file: fixture("survey_basic.sav"),
            varinfo: [true; 9],
            file_info: [true; 6],
            var_merge: [true; 9],
            file_merge: [true; 6],
            new_variables: true,
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::CopyDictionary(Box::new(DictionaryCopy {
            file: fixture("survey_basic.sav"),
            varinfo: [true, true, false, false, false, false, false, false, false],
            source_variable: s("age"),
            targets: v(&["monthly_spend"]),
            ..Default::default()
        }))),
        Dialog::MultResponse(MultResponse {
            crosstabs: true,
            groups: vec![
                MrGroup {
                    name: s("q5"),
                    label: s("Channels"),
                    variables: v(&["q5_1", "q5_2", "q5_3"]),
                    dichotomy: true,
                    value: s("1"),
                    max: String::new(),
                },
                MrGroup {
                    name: s("q6"),
                    label: s("Mentions"),
                    variables: v(&["q6_mention1", "q6_mention2", "q6_mention3"]),
                    dichotomy: false,
                    value: s("1"),
                    max: s("4"),
                },
            ],
            rows: v(&["q5", "gender"]),
            columns: v(&["q6"]),
            layers: v(&["region"]),
            ranges: vec![(s("gender"), s("1"), s("3")), (s("region"), s("1"), s("4"))],
            cells: [true; 3],
            base_responses: true,
            missing_dichotomies: true,
            missing_categories: true,
        }),
        Dialog::Tree(Tree {
            dependent: s("gender"),
            dependent_level: 1,
            predictors: vec![(s("region"), 0), (s("age"), 2), (s("q1_satisfaction"), 1)],
            exhaustive: true,
            max_depth: s("3"),
            min_parent: s("20"),
            min_child: s("10"),
            targets: s("1 2"),
            save: [true; 3],
        }),
        Dialog::Regression(Regression {
            dependent: s("monthly_spend"),
            // a string selection variable, its value quoted
            select: s("plan"),
            select_relation: 1,
            select_value: s("NONE"),
            blocks: vec![
                Block {
                    variables: v(&["age"]),
                    method: 0,
                },
                Block {
                    variables: v(&["q2_recommend"]),
                    method: 4,
                },
            ],
            block: 1,
            statistics: [true; 5],
            ci: true,
            ci_level: s("90"),
            descriptives: true,
            save: [true; 17],
            constant: true,
            missing: 2,
            bootstrap: boot(),
            ..Default::default()
        }),
        Dialog::Reliability(Reliability {
            items: v(&["q5_1", "q5_2", "q5_3", "q5_4"]),
            model: 4,
            label: s("Channels"),
            descriptives: [true; 3],
            inter_item: [true; 2],
            summaries: [true; 4],
            anova: 3,
            hotelling: true,
            tukey: true,
            icc: true,
            icc_model: 2,
            icc_absolute: true,
            icc_level: s("90"),
            icc_test_value: s("0.5"),
        }),
        Dialog::PartialCorr(PartialCorr {
            bootstrap: boot(),
            variables: v(&["age", "monthly_spend"]),
            controls: v(&["q2_recommend"]),
            one_tailed: true,
            condensed: true,
            descriptives: true,
            zero_order: true,
            pairwise: true,
        }),
        Dialog::FactorAnalysis(FactorAnalysis {
            variables: v(&["q5_1", "q5_2", "q5_3", "q5_4", "q5_5", "age"]),
            descriptives: [true; 9],
            extraction: 6,
            covariance: true,
            fixed: true,
            factors: s("2"),
            iterations: s("50"),
            rotation: 5,
            save: true,
            score_method: 2,
            score_coefficients: true,
            missing: 2,
            sort: true,
            blank: true,
            blank_value: s("0.3"),
            ..Default::default()
        }),
        Dialog::KMeans(KMeans {
            variables: v(&["age", "monthly_spend"]),
            clusters: s("3"),
            classify_only: false,
            iterations: s("20"),
            converge: s("0.001"),
            save_cluster: true,
            save_distance: true,
            initial: true,
            anova: true,
            membership: true,
            label: s("resp_id"),
            pairwise: true,
        }),
        Dialog::TwoStep(TwoStep {
            categorical: v(&["region", "gender"]),
            continuous: v(&["age", "monthly_spend"]),
            auto: true,
            max: s("6"),
            aic: true,
            unstandardized: v(&["age"]),
            noise: s("20"),
            include: true,
            print: [true; 3],
            model_summary: false,
            save: true,
            save_name: s("segment"),
            ..Default::default()
        }),
        Dialog::TwoStep(TwoStep {
            continuous: v(&["age", "monthly_spend"]),
            euclidean: true,
            auto: false,
            fixed: s("2"),
            ..Default::default()
        }),
        Dialog::Mlp(Mlp {
            common: Common {
                dependents: v(&["gender"]),
                level: 3,
                factors: v(&["region"]),
                covariates: v(&["age", "monthly_spend"]),
                rescale_covariates: 2,
                testing: s("20"),
                holdout: s("10"),
                include_missing: true,
                print: [true; 6],
                plot: [true; 6],
                save_predicted: true,
                predicted_names: s("mlp_pred"),
                save_probabilities: true,
                probability_roots: s("mlp_prob"),
                ..Default::default()
            },
            automatic: false,
            layers: 2,
            auto_units: false,
            units: [s("4"), s("3")],
            hidden: 1,
            training: 1,
            optimization: 1,
            error_steps: s("2"),
            max_minutes: s("1"),
            max_epochs: s("30"),
            ..Default::default()
        }),
        Dialog::Mlp(Mlp {
            common: Common {
                dependents: v(&["monthly_spend"]),
                level: 1,
                covariates: v(&["age"]),
                rescale_dependents: 2,
                ..Default::default()
            },
            min_units: s("1"),
            max_units: s("3"),
            ..Default::default()
        }),
        Dialog::Rbf(Rbf {
            common: Common {
                dependents: v(&["monthly_spend"]),
                level: 1,
                factors: v(&["region"]),
                covariates: v(&["age"]),
                print: [true; 6],
                plot: [true, true, true, false, false, false],
                save_predicted: true,
                predicted_names: s("rbf_pred"),
                ..Default::default()
            },
            min_units: s("2"),
            max_units: s("4"),
            hidden: 1,
            overlap: s("1.2"),
            ..Default::default()
        }),
        Dialog::Rbf(Rbf {
            common: Common {
                dependents: v(&["gender"]),
                level: 3,
                covariates: v(&["age", "monthly_spend"]),
                save_probabilities: true,
                ..Default::default()
            },
            automatic: false,
            units: s("3"),
            ..Default::default()
        }),
        Dialog::Anomaly(Anomaly {
            categorical: v(&["region"]),
            scale: v(&["age", "monthly_spend"]),
            id: s("resp_id"),
            by_number: true,
            number: s("8"),
            use_cutoff: false,
            min_peers: s("2"),
            max_peers: s("6"),
            reasons: s("2"),
            apply_missing: true,
            missing_share: true,
            save: [true; 8],
            print: [true; 5],
            ..Default::default()
        }),
        Dialog::Hierarchical(Hierarchical {
            variables: v(&["age", "monthly_spend"]),
            label: s("plan"),
            method: 6,
            measure: ClusterMeasure {
                interval: 7,
                ..Default::default()
            },
            schedule: true,
            proximity: true,
            membership: Solutions {
                kind: 2,
                clusters: String::new(),
                min: s("2"),
                max: s("4"),
            },
            save: Solutions {
                kind: 1,
                clusters: s("3"),
                min: String::new(),
                max: String::new(),
            },
        }),
        Dialog::Hierarchical(Hierarchical {
            variables: v(&["q5_1", "q5_2", "q5_3"]),
            measure: ClusterMeasure {
                data: 2,
                // Hamann: the ratios after it are undefined for a pair of cases with no 1 at all
                binary: 7,
                ..Default::default()
            },
            ..Default::default()
        }),
        Dialog::Hierarchical(Hierarchical {
            variables: v(&["q6_mention1", "q6_mention2"]),
            measure: ClusterMeasure {
                data: 1,
                counts: 1,
                ..Default::default()
            },
            ..Default::default()
        }),
        Dialog::Discriminant(Discriminant {
            group: s("gender"),
            min: s("1"),
            max: s("2"),
            variables: v(&["age", "monthly_spend", "q2_recommend"]),
            stepwise: true,
            method: 4,
            statistics: [true; 9],
            priors_size: true,
            separate: true,
            table: true,
            leave_one_out: true,
            save: [true; 3],
            select: s("region"),
            select_value: s("1"),
            ..Default::default()
        }),
        Dialog::Logistic(Logistic {
            bootstrap: boot(),
            dependent: s("q5_1"),
            blocks: vec![
                Block {
                    variables: v(&["age", "region"]),
                    method: 0,
                },
                Block {
                    variables: v(&["monthly_spend"]),
                    method: 6,
                },
            ],
            block: 1,
            select: s("gender"),
            select_relation: 1,
            select_value: s("9"),
            categorical: vec![Categorical {
                name: s("region"),
                contrast: 6,
                first: true,
            }],
            save: [true; 10],
            goodfit: true,
            ci: true,
            ci_level: s("90"),
            iteration_history: true,
            cut: s("0.4"),
            iterations: s("30"),
            constant: true,
            ..Default::default()
        }),
        // no bootstrap, as for One-Way ANOVA: the range tests times every bootstrap sample
        Dialog::Univariate(Univariate {
            bootstrap: Bootstrap::default(),
            dependent: s("monthly_spend"),
            factors: v(&["region", "gender"]),
            random: v(&["q1_satisfaction"]),
            covariates: v(&["age"]),
            custom: true,
            terms: vec![
                v(&["region"]),
                v(&["gender"]),
                v(&["region", "gender"]),
                v(&["q1_satisfaction"]),
            ],
            building: Vec::new(),
            sstype: 3,
            intercept: true,
            post_hoc_factors: v(&["region"]),
            post_hoc: [true; 16],
            dunnett: true,
            dunnett_first: true,
            dunnett_side: 2,
            means_overall: true,
            means_factors: v(&["region"]),
            compare: true,
            adjustment: 2,
            contrasts: vec![FactorContrast {
                name: s("region"),
                contrast: 2,
                first: true,
            }],
            spread_level: true,
            residual_plots: true,
            profiles: vec![v(&["region", "gender"]), v(&["gender"])],
            profile_draft: Default::default(),
            profile_bars: true,
            profile_mean_line: true,
            profile_errors: 2,
            save: [true; 8],
            print: [true; 11],
            het: [true; 4],
            het_model: [1, 0, 1],
            robust: true,
            robust_estimator: 4,
            alpha: s("0.01"),
            ..Default::default()
        }),
        // no bootstrap, as for One-Way ANOVA: the range tests times every bootstrap sample
        Dialog::Univariate(Univariate {
            bootstrap: Bootstrap::default(),
            multivariate: true,
            dependents: v(&["monthly_spend", "q1_satisfaction", "q2_recommend"]),
            factors: v(&["region", "gender"]),
            covariates: v(&["age"]),
            custom: true,
            terms: vec![v(&["region"]), v(&["gender"]), v(&["age"])],
            sstype: 0,
            post_hoc_factors: v(&["region"]),
            post_hoc: [true; 16],
            dunnett: true,
            means_overall: true,
            means_factors: v(&["region", "gender"]),
            compare: true,
            adjustment: 1,
            contrasts: vec![FactorContrast {
                name: s("gender"),
                contrast: 6,
                first: false,
            }],
            spread_level: true,
            residual_plots: true,
            profiles: vec![v(&["gender", "region"])],
            profile_errors: 1,
            save: [true; 8],
            print: [true; 11],
            het: [true; 4],
            het_model: [0, 1, 0],
            alpha: s("0.1"),
            m_matrix: vec![MColumn {
                label: s("Spend minus satisfaction"),
                coefficients: [
                    (s("monthly_spend"), s("1")),
                    (s("q1_satisfaction"), s("-1")),
                ]
                .into(),
            }],
            ..Default::default()
        }),
        Dialog::RepeatedMeasures(RepeatedMeasures {
            bootstrap: boot(),
            within: vec![WithinFactor {
                name: s("mention"),
                levels: s("3"),
            }],
            dependents: v(&["q6_mention1", "q6_mention2", "q6_mention3"]),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            sstype: 3,
            print: [true; 3],
            alpha: s("0.01"),
        }),
        Dialog::Advanced(Advanced::Ordinal(Ordinal {
            bootstrap: boot(),
            dependent: s("q1_satisfaction"),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            link: 4,
            custom_location: true,
            location: terms(&[&["gender"], &["age"], &["gender", "age"]]),
            scale: true,
            scale_terms: terms(&[&["age"]]),
            print: [true; 9],
            save: [true; 4],
            ci_level: s("90"),
            iterations: s("150"),
            halvings: s("10"),
            delta: s("0.5"),
            include: true,
            tests: vec![ContrastTest {
                label: s("Age"),
                // user-missing values valid: seven categories, six thresholds
                rows: s("gender*age 1 -1 0\nINTERCEPT 1 0 0 0 0 0"),
            }],
        })),
        Dialog::Advanced(Advanced::Multinomial(Multinomial {
            bootstrap: boot(),
            dependent: s("plan"),
            base: 2,
            base_value: s("BASIC"),
            descending: true,
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            model: 2,
            terms: terms(&[&["age"]]),
            step_terms: terms(&[&["gender"], &["gender", "age"]]),
            step_method: 2,
            step_rule: 1,
            min_effects: s("1"),
            max_effects: s("2"),
            entry: s("0.1"),
            removal: s("0.2"),
            score_entry: true,
            wald_removal: true,
            intercept: true,
            print: [true; 15],
            history: s("2"),
            save: [true; 4],
            ci_level: s("90"),
            iterations: s("150"),
            halvings: s("10"),
            include: true,
        })),
        Dialog::Advanced(Advanced::Mixed(LinearMixed {
            subjects: v(&["region"]),
            repeated: v(&["resp_id"]),
            repeated_covariance: 1,
            dependent: s("monthly_spend"),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            custom_fixed: true,
            fixed: terms(&[&["gender"], &["age"]]),
            intercept: true,
            type_one: true,
            random: true,
            random_intercept: true,
            random_terms: terms(&[&["age"]]),
            random_subjects: v(&["region"]),
            // heterogeneous compound symmetry
            random_covariance: 9,
            random_solution: true,
            // Kenward-Roger below needs REML
            ml: false,
            print: [true; 10],
            history: s("3"),
            means: vec![
                MeansTable::default(),
                MeansTable {
                    factors: v(&["gender"]),
                    compare: s("gender"),
                    contrast: 2,
                    adjustment: 2,
                },
            ],
            means_at: vec![(s("age"), s("40"))],
            tests: vec![ContrastTest {
                label: s("Men and women"),
                rows: s("gender 1 -1 0 | INTERCEPT 1 age 40"),
            }],
            save: [true; 7],
            ci_level: s("90"),
            iterations: s("150"),
            halvings: s("5"),
            df_method: 2,
            include: true,
        })),
        Dialog::Advanced(Advanced::Genlin(Box::new(GeneralizedLinear {
            dependent: s("q5_1"),
            reference: 2,
            reference_value: s("1"),
            factors: v(&["gender", "region"]),
            covariates: v(&["age"]),
            distribution: 1,
            link: 3,
            model: 2,
            terms: terms(&[&["gender"], &["region"], &["age"], &["gender", "age"]]),
            intercept: true,
            offset: s("q5_2"),
            scale_weight: s("weight"),
            method: 0,
            fisher_iterations: s("3"),
            scale: 2,
            robust: true,
            ci_level: s("90"),
            profile: true,
            profile_tolerance: s("0.001"),
            iterations: s("150"),
            halvings: s("10"),
            convergence: s("0.0001"),
            kernel: true,
            analysis: 2,
            likelihood_ratio: true,
            print: [true; 12],
            exponentiated: true,
            means: vec![
                MeansTable::default(),
                MeansTable {
                    factors: v(&["gender", "region"]),
                    compare: s("region"),
                    contrast: 6,
                    adjustment: 4,
                },
            ],
            means_at: vec![(s("age"), s("40"))],
            transformed: true,
            save: [true; 14],
            include: true,
            ..Default::default()
        }))),
        // repeated measures with every setting; they leave out likelihood ratio tests and the
        // saved variables of influence, so they have an entry of their own
        Dialog::Advanced(Advanced::Genlin(Box::new(GeneralizedLinear {
            dependent: s("monthly_spend"),
            factors: v(&["gender"]),
            covariates: v(&["age"]),
            analysis: 2,
            print: [true; 12],
            exponentiated: true,
            means: vec![MeansTable::default()],
            save: std::array::from_fn(|i| matches!(i, 0..=4 | 7 | 8)),
            repeated: true,
            // one case per respondent, placed by gender
            subjects: v(&["resp_id"]),
            within: v(&["gender"]),
            correlation: 1,
            adjust: false,
            gee_robust: false,
            gee_iterations: s("50"),
            update: s("2"),
            gee_convergence: s("1E-5"),
            gee_relative: true,
            hessian: s("0.001"),
            working_corr: true,
            ..Default::default()
        }))),
        Dialog::Advanced(Advanced::Correspondence(Correspondence {
            row: table_variable(
                "region",
                "1",
                "5",
                &[(4, Role::Equal(1)), (5, Role::Equal(1))],
            ),
            column: table_variable(
                "q6_mention1",
                "1",
                "6",
                &[(1, Role::Supplementary), (6, Role::Supplementary)],
            ),
            dimensions: s("3"),
            distance: 5,
            normalization: 4,
            q: s("-0.5"),
            print: [true; 7],
            permutation: true,
            permutation_dims: s("3"),
            plots: [true; 5],
            label_length: s("12"),
            ndim: true,
            ndim_first: s("1"),
            ndim_last: s("3"),
        })),
        Dialog::Advanced(Advanced::Homals(Box::new(LegacyScaling {
            dimensions: s("3"),
            max_iterations: s("50"),
            convergence: s("0.0001"),
            print: vec![true; 5],
            plots: vec![true; 3],
            plot_variables: vec![v(&["region"]), v(&["gender", "region"]), Vec::new()],
            label_length: s("8"),
            ndim: true,
            ndim_last: s("3"),
            save: true,
            save_root: s("hs"),
            save_dims: s("2"),
            ..legacy_four(ScalingKind::Homals)
        }))),
        Dialog::Advanced(Advanced::Princals(Box::new(LegacyScaling {
            dimensions: s("3"),
            print: vec![true; 6],
            plots: vec![true; 3],
            plot_variables: vec![v(&["q6_mention1"]), v(&["region"]), Vec::new()],
            label_length: s("5"),
            save: true,
            save_dims: s("3"),
            ..legacy_four(ScalingKind::Princals)
        }))),
        Dialog::Advanced(Advanced::Overals(Box::new(LegacyScaling {
            random_start: true,
            print: vec![true; 7],
            plots: vec![true; 5],
            plot_variables: vec![
                v(&["region"]),
                v(&["region"]),
                v(&["region"]),
                v(&["region"]),
                v(&["region"]),
            ],
            label_length: s("5"),
            save: true,
            save_root: s("ov"),
            ..legacy_four(ScalingKind::Overals)
        }))),
        Dialog::Advanced(Advanced::Anacor(Box::new(Anacor {
            row: table_variable("region", "1", "5", &[]),
            column: table_variable("q6_mention1", "1", "6", &[]),
            dimensions: s("3"),
            normalization: 4,
            q: s("0.5"),
            row_variances: true,
            column_variances: true,
            print: [true; 5],
            plots: [true; 5],
            label_length: s("6"),
            ndim: true,
            ndim_first: s("1"),
            ndim_last: s("3"),
        }))),
        // between variables, standardized within each case, every change of the results
        Dialog::Advanced(Advanced::Distances(Distances {
            variables: v(&["age", "monthly_spend", "q2_recommend"]),
            id: s("resp_id"),
            between_variables: true,
            measure: ClusterMeasure {
                interval: 3,
                ..Default::default()
            },
            standardize: 1,
            by_case: true,
            transforms: [true; 3],
            print: false,
            include: true,
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Distances(Distances {
            variables: v(&["age", "monthly_spend"]),
            id: s("resp_id"),
            measure: ClusterMeasure {
                interval: 6,
                power: s("3"),
                ..Default::default()
            },
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Distances(Distances {
            variables: v(&["age", "monthly_spend"]),
            measure: ClusterMeasure {
                interval: 7,
                power: s("3"),
                root: s("1.5"),
                ..Default::default()
            },
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Distances(Distances {
            variables: v(&["q6_mention1", "q6_mention2"]),
            between_variables: true,
            measure: ClusterMeasure {
                data: 1,
                counts: 0,
                ..Default::default()
            },
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Distances(Distances {
            variables: v(&["q5_1", "q5_2", "q5_3"]),
            measure: ClusterMeasure {
                data: 2,
                binary: 2,
                present: s("0"),
                absent: s("1"),
                ..Default::default()
            },
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Distances(Distances {
            variables: v(&["age", "monthly_spend"]),
            matrix: 2,
            dataset: s("gaps"),
            ..Default::default()
        })),
        // two time variables sharing one status, two factors and two interval periods
        Dialog::Advanced(Advanced::LifeTables(LifeTables {
            times: v(&["age", "monthly_spend"]),
            intervals: vec![(s("40"), s("10")), (s("100"), s("20"))],
            status: s("q1_satisfaction"),
            event_from: s("4"),
            event_to: s("5"),
            factor: table_variable("gender", "1", "2", &[]),
            second: table_variable("region", "1", "5", &[]),
            table: false,
            plots: [true; 5],
            compare: 2,
            approximate: true,
            listwise: true,
            include: true,
        })),
        Dialog::Advanced(Advanced::Km(KaplanMeier {
            time: s("age"),
            status: s("q1_satisfaction"),
            event_range: true,
            event_from: s("4"),
            event_to: s("5"),
            lost: s("1, 98"),
            factor: s("gender"),
            strata: s("region"),
            id: s("resp_id"),
            tests: [true; 3],
            compare: 1,
            trend: true,
            trend_weights: s("1 2 4"),
            save: [true; 4],
            save_names: [s("surv"), s("surv_se"), s("cum_haz"), s("events")],
            table: false,
            quartiles: true,
            percentiles: s("10 90"),
            plots: [true; 4],
            ..Default::default()
        })),
        // an entered block, then a forward LR block with an interaction, within strata
        Dialog::Advanced(Advanced::Cox(Box::new(Cox {
            time: s("age"),
            status: s("q5_1"),
            event_values: s("1"),
            blocks: vec![
                Block {
                    variables: v(&["monthly_spend"]),
                    method: 0,
                },
                Block {
                    variables: v(&["q2_recommend", "monthly_spend BY q2_recommend"]),
                    method: 2,
                },
            ],
            strata: s("gender"),
            ..Default::default()
        }))),
        // a categorical covariate with simple contrasts against the first category, every plot,
        // a line per region and spending held at a value
        Dialog::Advanced(Advanced::Cox(Box::new(Cox {
            time: s("age"),
            status: s("q1_satisfaction"),
            event_range: true,
            event_from: s("4"),
            event_to: s("5"),
            blocks: vec![Block {
                variables: v(&["monthly_spend", "region"]),
                method: 0,
            }],
            categorical: vec![Categorical {
                name: s("region"),
                contrast: 1,
                first: true,
            }],
            plots: [true; 4],
            pattern_by: s("region"),
            pattern: vec![(s("monthly_spend"), s("50"))],
            ..Default::default()
        }))),
        // every saved variable, some under names of their own, and every option
        Dialog::Advanced(Advanced::Cox(Box::new(Cox {
            time: s("age"),
            status: s("q5_1"),
            event_values: s("1"),
            blocks: vec![Block {
                variables: v(&["monthly_spend", "q2_recommend"]),
                method: 0,
            }],
            save: [true; 7],
            save_names: [
                s("surv"),
                s("surv_se"),
                String::new(),
                String::new(),
                s("part"),
                String::new(),
                s("risk"),
            ],
            ci: true,
            ci_level: s("90"),
            correlations: true,
            baseline: true,
            last_step: true,
            entry: s("0.1"),
            removal: s("0.2"),
            iterations: s("30"),
            include: true,
            ..Default::default()
        }))),
        // a covariate that grows with time
        Dialog::Advanced(Advanced::Cox(Box::new(Cox {
            time: s("age"),
            status: s("q5_1"),
            event_values: s("1"),
            blocks: vec![Block {
                variables: v(&["q2_recommend"]),
                method: 0,
            }],
            save: [false, false, false, false, false, true, false],
            save_names: Default::default(),
            time_expression: s("monthly_spend * LN(T_ + 1)"),
            ..Default::default()
        }))),
        // the sampling plans before the selections that read them
        Dialog::Advanced(Advanced::Sampling(SamplingPlan {
            path: out.join("max_draw.json").to_string_lossy().into_owned(),
            weight_name: s("draw_weight"),
            stages: vec![
                SamplingStage {
                    label: s("Areas"),
                    strata: v(&["region"]),
                    clusters: v(&["resp_id"]),
                    method: 4,
                    amount: s("3"),
                    mos_variable: s("age"),
                    mos_min: s("1"),
                    mos_max: s("100"),
                    inclusion_name: s("p_first"),
                    ..Default::default()
                },
                SamplingStage {
                    method: 2,
                    by_rate: true,
                    amount: s("0.5"),
                    min_size: s("1"),
                    max_size: s("5"),
                    ..Default::default()
                },
            ],
        })),
        Dialog::Advanced(Advanced::Selection(SampleSelection {
            plan: out.join("max_draw.json").to_string_lossy().into_owned(),
            seed_fixed: true,
            seed: s("4711"),
            to_file: true,
            outfile: out.join("max_draw.sav").to_string_lossy().into_owned(),
            rename: true,
            joint_file: out.join("max_joint.sav").to_string_lossy().into_owned(),
            counts: true,
        })),
        Dialog::Advanced(Advanced::Sampling(SamplingPlan {
            path: out.join("max_draw2.json").to_string_lossy().into_owned(),
            stages: vec![SamplingStage {
                strata: v(&["region"]),
                clusters: v(&["gender"]),
                method: 5,
                amount: s("2"),
                mos_from_data: true,
                ..Default::default()
            }],
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Selection(SampleSelection {
            plan: out.join("max_draw2.json").to_string_lossy().into_owned(),
            // a plan that draws with replacement needs a file
            to_file: true,
            outfile: out.join("max_draw2.sav").to_string_lossy().into_owned(),
            ..Default::default()
        })),
        Dialog::Advanced(Advanced::Plan(SamplePlan {
            path: plan.clone(),
            weight: s("weight"),
            stages: vec![
                Stage {
                    label: s("Regions"),
                    strata: v(&["region"]),
                    clusters: v(&["gender"]),
                    estimator: 1,
                    rate: 1,
                    rate_value: s("0.5"),
                    ..Default::default()
                },
                Stage {
                    strata: v(&["q5_1"]),
                    clusters: v(&["resp_id"]),
                    estimator: 1,
                    rate: 1,
                    rate_value: s("0.5"),
                    ..Default::default()
                },
                Stage::default(),
            ],
            srs_with_replacement: true,
        })),
        Dialog::Advanced(Advanced::Descriptives(SampleDescriptives {
            plan: plan.clone(),
            summary: v(&["age", "monthly_spend"]),
            mean: true,
            sum: true,
            numerators: v(&["monthly_spend"]),
            denominators: v(&["age"]),
            population_size: true,
            options: sample_options(),
        })),
        Dialog::Advanced(Advanced::Tables(SampleTables {
            plan: plan.clone(),
            variables: v(&["q5_5", "q6_mention1"]),
            cells: [true; 4],
            cumulative: true,
            homogeneity: true,
            options: sample_options(),
            ..Default::default()
        })),
        // region, gender, q5_1 and resp_id are in the plan, so out of the model
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("monthly_spend"),
            factors: v(&["plan", "q5_2"]),
            covariates: v(&["age"]),
            model: 1,
            show_intercept: false,
            statistics: [true; 7],
            test_type: 3,
            adjustment: 4,
            tests: vec![ContrastTest {
                label: s("Second channel"),
                rows: s("age 1 = 2"),
            }],
            means: vec![
                MeansTable::default(),
                MeansTable {
                    factors: v(&["plan", "q5_2"]),
                    compare: s("plan"),
                    contrast: 1,
                    adjustment: 0,
                },
            ],
            means_at: vec![(s("age"), s("40"))],
            ci_level: s("90"),
            df: s("100"),
            domain: s("region"),
            domain_value: s("1"),
            include: true,
            print: [true; 8],
            save: [true; 2],
            save_names: [s("fitted"), s("residual")],
            ..SampleModel::new(false)
        }))),
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("q5_3"),
            reference: 2,
            reference_value: s("0"),
            factors: v(&["plan", "q5_2"]),
            covariates: v(&["age"]),
            model: 2,
            terms: Terms {
                terms: vec![
                    v(&["plan"]),
                    v(&["q5_2"]),
                    v(&["age"]),
                    v(&["plan", "q5_2"]),
                ],
                building: Vec::new(),
            },
            show_intercept: false,
            statistics: [true; 7],
            test_type: 2,
            adjustment: 1,
            tests: vec![ContrastTest {
                label: s("Age"),
                rows: s("age 1 = 0.1"),
            }],
            odds: vec![
                OddsRatio {
                    variable: s("plan"),
                    level: 2,
                    reference: s("BASIC"),
                    control: vec![Held {
                        name: s("q5_2"),
                        level: 1,
                        value: String::new(),
                        covariate: false,
                    }],
                    covariate: false,
                },
                OddsRatio {
                    variable: s("age"),
                    level: 0,
                    reference: s("1 10"),
                    control: vec![Held {
                        name: s("q5_2"),
                        level: 2,
                        value: s("1"),
                        covariate: false,
                    }],
                    covariate: true,
                },
            ],
            ci_level: s("99"),
            df: s("100"),
            iterations: s("50"),
            halvings: s("10"),
            pconverge: s("0.0000001"),
            lconverge: s("0.00001"),
            separation: s("5"),
            domain: s("region"),
            domain_value: s("1"),
            include: true,
            print: [true; 8],
            history: s("2"),
            save: [true; 2],
            save_names: [s("prob"), s("guess")],
            probabilities: s("2"),
            ..SampleModel::new(true)
        }))),
        // the logit link, with the statistics and odds ratios that need it
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("q5_3"),
            factors: v(&["plan", "q5_2"]),
            covariates: v(&["age"]),
            model: 1,
            statistics: [true; 7],
            test_type: 2,
            adjustment: 1,
            odds: vec![
                OddsRatio {
                    variable: s("plan"),
                    level: 2,
                    reference: s("BASIC"),
                    control: vec![Held {
                        name: s("q5_2"),
                        level: 2,
                        value: s("1"),
                        covariate: false,
                    }],
                    covariate: false,
                },
                OddsRatio {
                    variable: s("age"),
                    reference: s("1 10"),
                    covariate: true,
                    ..Default::default()
                },
            ],
            ci_level: s("99"),
            df: s("100"),
            iterations: s("50"),
            halvings: s("10"),
            pconverge: s("0.0000001"),
            lconverge: s("0.00001"),
            separation: s("5"),
            domain: s("region"),
            domain_value: s("1"),
            include: true,
            print: [true; 8],
            history: s("2"),
            ordinal: Some(CsOrdinal {
                descending: true,
                parallel: [true; 3],
                save: [true; 5],
                save_names: [s("cum"), s("cat"), s("guess"), s("guessp"), s("obsp")],
                cumulative_count: s("2"),
                category_count: s("2"),
                ..Default::default()
            }),
            ..SampleModel::new(true)
        }))),
        // another link function, with the default tables
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("q5_3"),
            factors: v(&["plan"]),
            covariates: v(&["age"]),
            ordinal: Some(CsOrdinal {
                link: 2,
                parallel: [true, false, false],
                ..Default::default()
            }),
            ..SampleModel::new(true)
        }))),
        Dialog::Advanced(Advanced::Model(Box::new(SampleModel {
            plan: plan.clone(),
            dependent: s("age"),
            factors: v(&["plan"]),
            covariates: v(&["monthly_spend"]),
            statistics: [true, true, true, true, true, true, true],
            test_type: 1,
            adjustment: 1,
            ci_level: s("90"),
            df: s("100"),
            iterations: s("50"),
            halvings: s("10"),
            domain: s("q5_4"),
            domain_value: s("1"),
            include: true,
            cox: Some(CsCox {
                status: s("q5_1"),
                event_range: true,
                event_from: s("1"),
                event_to: s("2"),
                strata: s("q5_2"),
                ties: 1,
                baseline: 3,
                interval: 2,
                print: [true; 8],
                history: s("2"),
                save: [true; 13],
                save_names: [
                    s("sch"),
                    s("sco"),
                    s("dfb"),
                    s("mart"),
                    s("dev"),
                    s("snell"),
                    s("xb"),
                    s("surv"),
                    s("lcls"),
                    s("ucls"),
                    s("cum"),
                    s("lclc"),
                    s("uclc"),
                ],
                plots: [true; 4],
                plot_ci: true,
                pattern: s("monthly_spend 100"),
                pattern_by: s("plan"),
                ..Default::default()
            }),
            ..SampleModel::new_cox()
        }))),
        Dialog::Advanced(Advanced::Tables(SampleTables {
            crosstabs: true,
            plan,
            variables: v(&["q5_2", "q5_3"]),
            column: s("q5_4"),
            cells: [true; 4],
            residuals: [true; 3],
            tests: [true; 4],
            options: sample_options(),
            ..Default::default()
        })),
        Dialog::Restructure(Restructure {
            makes: vec![
                Make {
                    name: s("mention"),
                    from: v(&["q6_mention1", "q6_mention2", "q6_mention3"]),
                },
                Make {
                    name: s("channel"),
                    from: v(&["q5_1", "q5_2", "q5_3"]),
                },
            ],
            index: s("which"),
            id: s("case"),
            keep: v(&["resp_id", "gender"]),
            ..Default::default()
        }),
        Dialog::Restructure(Restructure {
            kind: RestructureKind::CasesToVariables,
            ids: v(&["resp_id"]),
            indexes: v(&["gender"]),
            sort: true,
            by_index: true,
            ..Default::default()
        }),
        Dialog::Restructure(Restructure {
            kind: RestructureKind::Transpose,
            variables: v(&["age", "monthly_spend"]),
            names: s("resp_id"),
            ..Default::default()
        }),
        Dialog::SortVariables(SortVariables {
            by: crate::data_dialogs::SORT_VARIABLES_KEYS.len() - 1,
            descending: true,
        }),
        Dialog::ExportData(ExportData {
            path: out.join("max.xlsx").to_string_lossy().into_owned(),
            names: 2,
            labels: true,
            recode_missing: true,
            variables: v(&["gender", "age"]),
            ..Default::default()
        }),
        Dialog::ExportData(ExportData {
            path: out.join("max.dta").to_string_lossy().into_owned(),
            recode_missing: true,
            variables: v(&["gender", "age"]),
            stata_version: "13".into(),
            ..Default::default()
        }),
    ];
    list.push(Dialog::Advanced(Advanced::Olap(Box::new(OlapCubes {
        summary: v(&["monthly_spend", "age"]),
        layers: vec![v(&["region"]), v(&["gender"])],
        cells: [true; 21],
        difference: OlapDifference::Groups,
        group: s("gender"),
        pairs: vec![[s("1"), s("2")]],
        percent: true,
        hide_small: true,
        hide_below: s("3"),
        title: s("Spend by region"),
        footnote: s("Invented survey"),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Olap(Box::new(OlapCubes {
        summary: v(&["monthly_spend", "age"]),
        layers: vec![v(&["gender"])],
        difference: OlapDifference::Variables,
        pairs: vec![[s("monthly_spend"), s("age")]],
        percent: true,
        ..Default::default()
    }))));
    let mut one = Proportions::new(ProportionsKind::OneSample);
    one.variables = v(&["gender", "q5_1"]);
    one.test_value = s("0.4");
    one.tests[0] = [true; 6];
    one.intervals[0] = [true; 9];
    one.success = 3;
    one.level = s("90");
    list.push(Dialog::Advanced(Advanced::Proportions(Box::new(one))));
    let mut paired = Proportions::new(ProportionsKind::Paired);
    paired.variables = v(&["q5_1", "q5_2"]);
    paired.with = v(&["q5_2", "q5_3"]);
    paired.in_order = true;
    paired.tests[1] = [true; 6];
    paired.intervals[1] = [true; 9];
    paired.success = 2;
    paired.success_values = s("1, 2");
    paired.listwise = true;
    paired.include_missing = true;
    list.push(Dialog::Advanced(Advanced::Proportions(Box::new(paired))));
    let mut independent = Proportions::new(ProportionsKind::Independent);
    independent.variables = v(&["q2_recommend"]);
    independent.group = v(&["age"]);
    independent.groups = 3;
    independent.cutpoint = s("40");
    independent.tests[2] = [true; 6];
    independent.intervals[2] = [true; 9];
    independent.success = 4;
    independent.success_cutpoint = s("4");
    list.push(Dialog::Advanced(Advanced::Proportions(Box::new(
        independent,
    ))));
    for kind in [
        TTestKind::OneSample,
        TTestKind::Independent,
        TTestKind::Paired,
    ] {
        let mut d = TTest::new(kind);
        d.variables = v(&["age", "monthly_spend"]);
        d.test_value = s("40");
        d.grouping = s("gender");
        d.cut_point = true;
        d.group1 = s("2");
        d.pairs = vec![
            (s("age"), s("monthly_spend")),
            (s("q1_satisfaction"), s("q2_recommend")),
        ];
        d.confidence = s("90");
        d.listwise = true;
        d.effect_sizes = true;
        d.standardizer = 2;
        d.bootstrap = boot();
        list.push(Dialog::TTest(d));
    }
    for kind in ChartKind::ALL {
        // the last function a chart takes, titles, panels and every missing-value option
        let mut d = ChartBuilder {
            kind,
            function: *kind.functions().last().unwrap(),
            low: s("20"),
            high: s("60"),
            variables: v(&["age", "monthly_spend"]),
            y_variables: v(&["q2_recommend", "q1_satisfaction"]),
            pair: true,
            category: s("region"),
            group: s("gender"),
            x: s("age"),
            y: s("monthly_spend"),
            label: s("resp_id"),
            normal: true,
            error_bar: 2,
            level: s("90"),
            multiplier: s("1"),
            deviations: s("1"),
            cumulative: false,
            title: s("A title"),
            subtitle: s("A subtitle"),
            footnote: s("A footnote"),
            by_variable: true,
            report: true,
            include: true,
            panel_columns: v(&["q5_1"]),
            panel_rows: v(&["q5_2"]),
        };
        // a group variable and a pie take one summarized variable
        d.variables = match kind {
            ChartKind::OverlayScatter | ChartKind::MatrixScatter => d.variables,
            ChartKind::HighLow => v(&["age", "monthly_spend", "q2_recommend"]),
            _ => v(&["age"]),
        };
        list.push(Dialog::ChartBuilder(d));
    }
    for (pp, distribution) in [(false, 0), (true, 12)] {
        list.push(Dialog::ProbabilityPlot(ProbabilityPlot {
            variables: v(&["age", "monthly_spend"]),
            distribution,
            estimate: false,
            parameters: [s("40"), s("12")],
            fraction: 3,
            ties: 3,
            plot: 2,
            ln: true,
            standardize: true,
            diff: true,
            diff_degree: s("1"),
            sdiff: true,
            sdiff_degree: s("1"),
            period: s("4"),
            ..ProbabilityPlot::new(pp)
        }));
    }
    list.push(Dialog::Advanced(Advanced::TimeSeries(Box::new(
        TimeSeries::TwoSls(TwoSls {
            dependent: s("monthly_spend"),
            explanatory: v(&["age", "q2_recommend"]),
            instruments: v(&["age", "q2_recommend", "q1_satisfaction"]),
            constant: 2,
            covariances: true,
            save_predicted: true,
            save_residuals: true,
        }),
    ))));
    list.push(Dialog::QuantileRegression(QuantileRegression {
        dependent: s("monthly_spend"),
        factors: v(&["region"]),
        covariates: v(&["age"]),
        quantiles: s("0.25 0.5 0.75"),
        intercept: true,
        iid: false,
        bandwidth: 1,
        level: s("90"),
        method: 1,
        include: true,
        print: [true, true, true],
        saves: [
            (true, s("qfit")),
            (true, String::new()),
            (true, s("qlo")),
            (true, s("qhi")),
        ],
    }));
    for method in [1, 2, 3, 4, 5] {
        list.push(Dialog::VarianceComponents(VarianceComponents {
            dependent: s("monthly_spend"),
            fixed: v(&["gender"]),
            random: v(&["region"]),
            covariates: v(&["age"]),
            weight: s("q1_satisfaction"),
            method,
            intercept: method != 3,
            include: true,
            converge: s("0.000001"),
            eps: s("0.000001"),
            iterate: s("30"),
            sums: true,
            expected: true,
            history: true,
            every: s("2"),
        }));
    }
    list.push(Dialog::OptimalBinning(OptimalBinning {
        variables: v(&["age", "monthly_spend"]),
        guide: s("plan"),
        equal: false,
        bins: s("5"),
        preprocess: true,
        classes: s("50"),
        merge: s("0.05"),
        lower_observed: true,
        upper_observed: true,
        lower_exclusive: true,
        listwise: true,
        save: true,
        into: s("age_cut spend_cut"),
        rules: out.join("max_bins.sps").to_string_lossy().into_owned(),
        print: [true, true, true],
    }));
    list.push(Dialog::OptimalBinning(OptimalBinning {
        variables: v(&["age"]),
        guide: s("plan"),
        equal: true,
        bins: s("4"),
        print: [true, true, true],
        ..Default::default()
    }));
    list.push(Dialog::Aim(Box::new(Aim {
        group: s("region"),
        categorical: v(&["gender", "plan"]),
        continuous: v(&["age", "monthly_spend"]),
        bonferroni: false,
        ci: s("90"),
        hide_not_significant: true,
        reference_lines: false,
        include: true,
        category_plot: true,
        cluster_plot: 2,
        error_bar: true,
        importance: true,
        by_variable: true,
        pvalue: true,
    })));
    list.push(Dialog::FeatureSelection(Box::new(FeatureSelection {
        target: s("plan"),
        level: 3,
        factors: v(&["region", "gender"]),
        covariates: v(&["age", "monthly_spend"]),
        except: v(&["gender"]),
        screening: true,
        pct_missing: s("60"),
        pct_equal: s("90"),
        pct_unequal: s("80"),
        min_sd: s("0.5"),
        min_cv: s("0.01"),
        auto_size: false,
        size: s("2"),
        pcutoff: s("0.1"),
        ranking: 2,
        show_unselected: s("1"),
        timer: s("1"),
        include: true,
        print: [true; 4],
        plot: true,
    })));
    list.push(Dialog::PrepareData(Box::new(PrepareData {
        target: s("monthly_spend"),
        inputs: v(&["age", "region", "plan"]),
        date_reference: s("2025-01-01"),
        date_unit: 2,
        time_reference: s("12:00:00"),
        time_unit: 1,
        extract: [true; 6],
        max_missing: s("40"),
        max_categories: s("50"),
        screen_single: true,
        max_single: s("90"),
        max_ordinal: s("8"),
        min_continuous: s("4"),
        cutoff: s("2.5"),
        outliers_to_missing: true,
        replace_target: true,
        reorder_input: true,
        rescale_input: 1,
        min: s("0"),
        max: s("1"),
        merge_unsupervised: true,
        min_pct: s("20"),
        pvalue: s("0.1"),
        suffix_target: s("_t"),
        suffix_input: s("_p"),
        outfile: out.join("max_prep.sps").to_string_lossy().into_owned(),
        ..Default::default()
    })));
    list.push(Dialog::RocCurve(RocCurve {
        tests: v(&["age", "monthly_spend"]),
        state: s("plan"),
        value: s("BASIC"),
        curve: true,
        reference: true,
        se: true,
        model_quality: true,
        coordinates: true,
        small_positive: true,
        exclude_cutoff: true,
        // unequal groups fall back to the nonparametric error with a note
        negexpo: true,
        level: s("90"),
        include: true,
    }));
    for (pair, group) in [(true, ""), (false, "gender")] {
        list.push(Dialog::RocAnalysis(Box::new(RocAnalysis {
            tests: v(&["age", "monthly_spend"]),
            state: s("plan"),
            value: s("BASIC"),
            pair,
            group: s(group),
            first: s("1"),
            second: s("2"),
            curve: 2,
            reference: true,
            pr_reference: true,
            interpolate_fp: true,
            model_quality: true,
            se: true,
            classifier: true,
            coordinates_roc: true,
            coordinates_pr: true,
            small_positive: true,
            exclude_cutoff: true,
            negexpo: true,
            level: s("90"),
            include: true,
            ..Default::default()
        })));
    }
    for (min_rate, max_count) in [(1, 1), (2, 2)] {
        list.push(Dialog::ResponseRate(ResponseRate {
            numerator: s("q1_satisfaction"),
            denominator: s("age"),
            group: s("region"),
            min_rate,
            rate: s("5"),
            cost: s("0.6"),
            revenue: s("30"),
            max_count,
            count: s("600"),
            percent: s("50"),
        }));
    }
    list.push(Dialog::ControlCharts(Box::new(ControlCharts {
        process: s("age"),
        group: s("region"),
        xbar_only: true,
        sigmas: s("2.5"),
        minsample: s("3"),
        rules: [true; 11],
        id: s("resp_id"),
        statistics: {
            let mut on = [false; 17];
            [0, 3, 6, 9, 12, 13, 14, 15, 16]
                .iter()
                .for_each(|i| on[*i] = true);
            on
        },
        capsigma: 4,
        lsl: s("10"),
        usl: s("90"),
        target: s("50"),
        report: true,
        include: true,
        title: s("Ages"),
        subtitle: s("By region"),
        footnote: s("Invented survey"),
        ..Default::default()
    })));
    list.push(Dialog::Catreg(Box::new(CategoricalRegression {
        dependent: s("q1_satisfaction"),
        independents: v(&["age", "q2_recommend", "region"]),
        settings: {
            let setting = |name: &str, change: &dyn Fn(&mut CatregVariable)| {
                let mut x = CatregVariable::new(name);
                change(&mut x);
                x
            };
            vec![
                setting("q1_satisfaction", &|x| x.level.level = 2),
                setting("age", &|x| {
                    x.level.level = 1;
                    x.level.degree = s("3");
                    x.level.knots = s("1");
                    x.discretize = 1;
                    x.groups = s("5");
                    x.uniform = true;
                }),
                setting("q2_recommend", &|x| {
                    x.level.level = 4;
                    x.level.missing = 1;
                }),
                setting("region", &|x| x.level.level = 3),
            ]
        },
        max_iterations: s("50"),
        convergence: s("0.001"),
        initial: 1,
        penalty: 3,
        resample: 1,
        resample_count: s("4"),
        print: [true; 9],
        quant: v(&["region"]),
        plot_trans: true,
        plot_trans_variables: v(&["region"]),
        plot_resid: true,
        plot_regu: true,
        label_length: s("8"),
        save: [true; 3],
        save_roots: [s("tr"), s("pr"), s("re")],
        ..Default::default()
    })));
    for kind in NparKind::ALL {
        let mut d = NparTests::new(kind);
        d.variables = v(&["q1_satisfaction", "q2_recommend", "age"]);
        d.pairs = vec![
            (s("q1_satisfaction"), s("q2_recommend")),
            (s("age"), s("monthly_spend")),
        ];
        d.grouping = s("region");
        d.low = s("1");
        d.high = s("4");
        d.tests = [true; 4];
        d.range = true;
        d.expected_given = true;
        d.expected = s("1 2 3 4");
        d.proportion = s("0.4");
        d.cut = true;
        d.cut_value = s("3");
        d.descriptives = true;
        d.quartiles = true;
        d.listwise = true;
        d.exact = true;
        if kind == NparKind::TwoIndependent {
            d.high = s("2");
        }
        list.push(Dialog::NparTests(d));
    }
    list.push(Dialog::Advanced(Advanced::Glmm(Box::new(GenlinMixed {
        subjects: v(&["region", "gender"]),
        target: s("q2_recommend"),
        distribution: 5,
        link: 2,
        trials_form: 0,
        weight: s("q1_satisfaction"),
        offset_form: 1,
        offset: s("0.1"),
        inputs: v(&["age", "monthly_spend"]),
        custom_fixed: true,
        fixed: terms(&[&["age"], &["monthly_spend"]]),
        intercept: true,
        random: vec![
            RandomBlock {
                intercept: true,
                terms: terms(&[&["age"]]),
                subjects: v(&["region"]),
                covariance: 3,
                solution: true,
            },
            RandomBlock {
                subjects: v(&["region", "gender"]),
                covariance: 0,
                ..Default::default()
            },
        ],
        order: 1,
        iterations: s("50"),
        ci_level: s("90"),
        df_method: 1,
        pconverge: s("0.00001"),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Glmm(Box::new(GenlinMixed {
        subjects: v(&["region"]),
        target: s("q5_2"),
        distribution: 2,
        link: 4,
        reference: s("0"),
        inputs: v(&["age"]),
        random: vec![RandomBlock {
            subjects: v(&["region"]),
            ..Default::default()
        }],
        ..Default::default()
    }))));
    // an ordinal target with a cumulative link, and a nominal one with its reference
    for (target, subjects, link, reference) in [
        ("q1_satisfaction", "region", 9, ""),
        ("plan", "region", 0, "BASIC"),
    ] {
        list.push(Dialog::Advanced(Advanced::Glmm(Box::new(GenlinMixed {
            subjects: v(&[subjects]),
            target: s(target),
            distribution: 7,
            link,
            reference: s(reference),
            inputs: v(&["age"]),
            random: vec![RandomBlock {
                subjects: v(&[subjects]),
                ..Default::default()
            }],
            ..Default::default()
        }))));
    }
    list.push(Dialog::Advanced(Advanced::Glmm(Box::new(GenlinMixed {
        subjects: v(&["region"]),
        repeated: v(&["resp_id"]),
        residual: 4,
        target: s("q2_recommend"),
        distribution: 6,
        inputs: v(&["gender", "age"]),
        random: Vec::new(),
        robust: true,
        emmeans: vec![
            EmmeansBlock::default(),
            EmmeansBlock {
                tables: v(&["gender"]),
                compare: s("gender"),
                contrast: 1,
            },
        ],
        control: s("age=40"),
        transformed: true,
        adjust: 2,
        save: [true, false, false, true, true],
        save_names: [s("glmm_fit"), s(""), s(""), s("glmm_ci"), s("glmm_res")],
        outfile: out.join("glmm_model.json").to_string_lossy().into_owned(),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Manova(Box::new(Manova {
        dependents: v(&["monthly_spend", "q1_satisfaction"]),
        factors: v(&["region", "gender"]),
        ranges: [
            (s("region"), (s("1"), s("5"))),
            (s("gender"), (s("1"), s("2"))),
        ]
        .into_iter()
        .collect(),
        covariates: v(&["age"]),
        error: 3,
        sequential: true,
        main_effects: true,
        print: [true; 8],
        ws_factors: vec![(s("visit"), s("2"))],
        measure: s("spend"),
        ws_main_effects: true,
        transform: 0,
        orthonorm: false,
        basis: false,
    }))));
    list.push(Dialog::Advanced(Advanced::Manova(Box::new(Manova {
        dependents: v(&["monthly_spend", "q1_satisfaction"]),
        factors: v(&["region"]),
        ranges: [(s("region"), (s("1"), s("5")))].into_iter().collect(),
        transform: 4,
        orthonorm: true,
        basis: true,
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Pls(Box::new(Pls {
        scale: v(&["monthly_spend", "q2_recommend"]),
        categorical: v(&["gender"]),
        first_reference: true,
        factors: v(&["region"]),
        covariates: v(&["age", "q1_satisfaction"]),
        id: s("resp_id"),
        latent: s("2"),
        cases: true,
        loadings: true,
        predictors: true,
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Knn(Box::new(Knn {
        target: s("gender"),
        level: 3,
        factors: v(&["region"]),
        covariates: v(&["age", "monthly_spend"]),
        training: s("8"),
        holdout: s("2"),
        auto: true,
        kmin: s("2"),
        kmax: s("4"),
        folds: s("5"),
        metric: 1,
        scale_covariates: false,
        weight_features: true,
        include_missing: true,
        show_model: false,
        save: [true; 4],
        save_names: [s("knn_pred"), s("knn_prob"), s("knn_part"), s("knn_fold")],
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Knn(Box::new(Knn {
        target: s("monthly_spend"),
        level: 1,
        covariates: v(&["age", "q2_recommend"]),
        by_variable: true,
        partition_variable: s("gender"),
        k: s("5"),
        select_features: true,
        median: true,
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::NaiveBayes(Box::new(
        NaiveBayes {
            target: s("gender"),
            factors: v(&["region"]),
            covariates: v(&["age", "monthly_spend"]),
            selection: 1,
            size: s("2"),
            best: 2,
            bins: s("4"),
            partition: 1,
            percent: s("70"),
            include_missing: true,
            print: [true, false, true, false, true],
            save_value: true,
            value_name: s("nb_pred"),
            save_probability: true,
            probability_root: s("nb_prob"),
            probability_count: s("2"),
            model_file: out.join("max_nb.json").to_string_lossy().into_owned(),
            ..Default::default()
        },
    ))));
    list.push(Dialog::Advanced(Advanced::NaiveBayes(Box::new(
        NaiveBayes {
            target: s("gender"),
            covariates: v(&["age", "monthly_spend"]),
            selection: 2,
            size: s("1"),
            partition: 2,
            partition_variable: s("q2_recommend"),
            print: [false; 5],
            ..Default::default()
        },
    ))));
    // twenty iterations: the unfolding runs for minutes in a debug build otherwise, and the
    // walk checks the syntax, not the solution
    list.push(Dialog::Advanced(Advanced::Prefscal(Box::new(Prefscal {
        variables: v(&["age", "monthly_spend", "q1_satisfaction", "q2_recommend"]),
        layout: 2,
        layout_value: s("2"),
        similarities: true,
        transformation: 3,
        intercept: true,
        degree: s("3"),
        knots: s("0"),
        condition: 1,
        model: 1,
        start: 4,
        start_count: s("2"),
        lambda: s("0.8"),
        omega: s("0.5"),
        min_dimensions: s("1"),
        max_dimensions: s("2"),
        max_iterations: s("20"),
        diff_stress: s("0.00001"),
        min_stress: s("0.001"),
        print: [true; 11],
        plot: [true; 10],
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Prefscal(Box::new(Prefscal {
        variables: v(&["age", "monthly_spend", "q1_satisfaction", "q2_recommend"]),
        layout: 3,
        layout_value: s("region"),
        transformation: 1,
        intercept: true,
        condition: 2,
        start: 3,
        max_iterations: s("20"),
        ..Default::default()
    }))));
    // the four columns fixed from a file of their coordinates: external unfolding
    let columns = out
        .join("max_prefscal_columns.sav")
        .to_string_lossy()
        .into_owned();
    statscraft_engine::session::Session::default()
        .run(
            &format!(
                "DATA LIST FREE / x y.\nBEGIN DATA\n1 0\n0 1\n-1 0\n0 -1\nEND DATA.\n\
                 SAVE OUTFILE={}.",
                crate::syntax_text::quote(&columns)
            ),
            None,
        )
        .unwrap();
    list.push(Dialog::Advanced(Advanced::Prefscal(Box::new(Prefscal {
        variables: v(&["age", "monthly_spend", "q1_satisfaction", "q2_recommend"]),
        fixed: [(String::new(), String::new()), (columns, s("x y"))],
        max_iterations: s("20"),
        ..Default::default()
    }))));
    list.push(Dialog::Advanced(Advanced::Oms(Oms {
        kinds: [true; 4],
        commands: s("Frequencies, Descriptives"),
        subtypes: s("Statistics"),
        format: 4,
        file: out.join("max_oms.xlsx").to_string_lossy().into_owned(),
        viewer: false,
        tag: s("walk"),
    })));
    list.push(Dialog::Advanced(Advanced::Omsend(Omsend {
        tags: s("walk, other"),
        files: s("max_oms.xlsx"),
    })));
    let (cj_plan, cj_scores, n) =
        conjoint_files(out, "max", "brand (1 2 3) price (1 2 3) organic (1 2)", 2);
    list.push(Dialog::Advanced(Advanced::Conjoint(Box::new(
        Design::Generate(OrthogonalPlan {
            factors: vec![
                plan_factor("brand", "Brand", &["Acme", "Brio", "Cora"]),
                plan_factor("price", "Price", &["", "", ""]),
                plan_factor("organic", "", &["no", "yes"]),
            ],
            at: 1,
            minimum: s("9"),
            holdout: s("2"),
            mixed: true,
            seed: s("5"),
            output: 1,
            file: out.join("max_made.sav").to_string_lossy().into_owned(),
        }),
    ))));
    list.push(Dialog::Advanced(Advanced::Conjoint(Box::new(
        Design::Display(PlanCards {
            factors: v(&["region", "gender"]),
            format: 2,
            title: s("Profile )CARD\nOf the survey"),
            footer: s("Score it from 0 to 10"),
            file: out.join("max_cards.txt").to_string_lossy().into_owned(),
        }),
    ))));
    list.push(Dialog::Advanced(Advanced::Conjoint(Box::new(
        Design::Analysis(Box::new(ConjointAnalysis {
            plan_from_file: true,
            plan_file: cj_plan,
            data_from_file: true,
            data_file: cj_scores,
            record: 0,
            responses: format!("s1 TO s{n}"),
            subject: s("id"),
            factors: vec![
                FactorModel {
                    name: s("brand"),
                    model: 0,
                    direction: 1,
                },
                FactorModel {
                    name: s("price"),
                    model: 2,
                    direction: 0,
                },
                FactorModel {
                    name: s("organic"),
                    model: 1,
                    direction: 2,
                },
            ],
            print: 1,
            summary_only: true,
            utility: out.join("max_utility.sav").to_string_lossy().into_owned(),
            plot: 3,
            ..Default::default()
        })),
    ))));
    list.extend(simulation_dialogs(out, "max", true));
    list
}

/// Draws the dialog as its window does (variable list, body, footer) for two frames, with no
/// input: a panic in the view code, or two widgets sharing an id, fails the test. The id clash
/// warning only paints a fire sign, so the shapes are searched for it.
fn draw_once(dialog: &mut Dialog, data: &statscraft_data::Dataset) {
    fn burnt(shape: &eframe::egui::Shape) -> bool {
        match shape {
            eframe::egui::Shape::Text(t) => t.galley.text().contains('\u{1F525}'),
            eframe::egui::Shape::Vec(v) => v.iter().any(burnt),
            _ => false,
        }
    }
    let ctx = eframe::egui::Context::default();
    ctx.options_mut(|o| o.warn_on_id_clash = true);
    let (mut filter, mut picked, mut folder) = (String::new(), Vec::new(), None);
    let sets = matches!(
        dialog,
        Dialog::CustomTables(..) | Dialog::Advanced(Advanced::Codebook(_))
    );
    for _ in 0..2 {
        let input = eframe::egui::RawInput {
            screen_rect: Some(eframe::egui::Rect::from_min_size(
                eframe::egui::Pos2::ZERO,
                eframe::egui::vec2(1280.0, 900.0),
            )),
            ..Default::default()
        };
        let mut out = ctx.run_ui(input, |ui| {
            eframe::egui::CentralPanel::default().show(ui, |ui| {
                super::variable_picker(ui, data, sets, &mut filter, &mut picked, 200.0);
                super::dialog_body(ui, data, dialog, &mut picked, &mut folder);
                super::dialog_footer(ui, &dialog.syntax(data), true);
            });
        });
        out.textures_delta.clear();
        assert!(
            !out.shapes.iter().any(|s| burnt(&s.shape)),
            "{}: two widgets share an id",
            dialog.title()
        );
    }
}

/// The syntax of a dialog, after drawing it once.
fn drawn(mut dialog: Dialog, data: &statscraft_data::Dataset) -> Result<String, String> {
    draw_once(&mut dialog, data);
    dialog.syntax(data)
}

/// Build Simulation Plan then Run Simulation: the plan is written to the walk's folder and the
/// run reads it, so the list keeps them in this order.
fn simulation_dialogs(out: &Path, tag: &str, full: bool) -> [Dialog; 2] {
    let file = |name: &str| out.join(name).to_string_lossy().into_owned();
    let plan_file = file(&format!("{tag}_sim.splan"));
    let input = |name: &str, dist: usize, params: [&str; 3]| SimInputRow {
        name: s(name),
        dist,
        params: params.map(s),
        ..Default::default()
    };
    let mut plan = SimPlan {
        model: s("margin = units * (price - cost)"),
        model_file: file(&format!("{tag}_sim_model.json")),
        inputs: vec![
            input("units", 3, ["800", "1400", "1000"]),
            input("cost", 1, ["7", "1", ""]),
        ],
        fixed: vec![FixedRow {
            name: s("price"),
            value: s("12"),
        }],
        max_cases: s("2000"),
        plan_file: plan_file.clone(),
        ..Default::default()
    };
    let mut run = SimRun {
        plan_file,
        ..Default::default()
    };
    if full {
        plan.model
            .push_str(" - rework * 50\nCOMPUTE big = margin > 4000.");
        plan.inputs[1].min = s("4");
        plan.inputs[1].max = s("10");
        plan.inputs
            .push(input("rework", 12, ["0 1 2", "0.5 0.3 0.2", ""]));
        plan.correlations = vec![CorrelationRow {
            first: s("units"),
            second: s("cost"),
            value: s("-0.3"),
        }];
        plan.targets = s("margin big");
        plan.stop_target = s("margin");
        plan.stop_width = s("100");
        plan.quiet = true;
        run = SimRun {
            seed: s("4711"),
            pdf: true,
            cdf: true,
            scatter: true,
            boxplot: true,
            tornado_delta: true,
            delta: s("1.5"),
            tornado_variance: true,
            level: s("90"),
            percentiles: true,
            ptiles: s("5 50 95"),
            associations: true,
            save: SaveCases::File,
            out_file: file("max_sim_cases.sav"),
            ..run
        };
    }
    [
        Dialog::Advanced(Advanced::Simulation(Box::new(Simulation::Plan(Box::new(
            plan,
        ))))),
        Dialog::Advanced(Advanced::Simulation(Box::new(Simulation::Run(Box::new(
            run,
        ))))),
    ]
}

/// The syntax of each dialog is Ok, parses without an error and runs on the survey file.
fn walk(dialogs: Vec<Dialog>) {
    let mut failures = Vec::new();
    for dialog in dialogs {
        let mut session = open("survey_basic.sav");
        // a set for Custom Tables and the deletion of sets, and a label of its own for the
        // counted value of each dichotomy, which labelling categories by that value needs
        session
            .run(
                "MRSETS /MCGROUP NAME=$mentions VARIABLES=q6_mention1 q6_mention2 q6_mention3.
                 ADD VALUE LABELS q5_2 1 'Selected 2' /q5_3 1 'Selected 3'.",
                None,
            )
            .unwrap();
        let data = session.dataset().unwrap().clone();
        let mut dialog = dialog;
        draw_once(&mut dialog, &data);
        let syntax = match dialog.syntax(&data) {
            Ok(text) => text,
            Err(e) => {
                failures.push(format!("{}: no syntax: {e}", dialog.title()));
                continue;
            }
        };
        let (_, errors) = statscraft_syntax::parse(&syntax);
        if let Some(e) = errors.first() {
            failures.push(format!("{}: parse: {e:?}\n{syntax}", dialog.title()));
            continue;
        }
        if let Err(e) = session.run(&syntax, None) {
            failures.push(format!("{}: run: {e:?}\n{syntax}", dialog.title()));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

/// Invented ratings of how unlike five fruits taste, one lower triangle per judge.
const FRUIT_MATRICES: &str = "DATA LIST LIST /judge apple pear plum cherry grape.
BEGIN DATA
1 0 . . . .
1 2 0 . . .
1 5 4 0 . .
1 6 6 3 0 .
1 7 8 4 2 0
2 0 . . . .
2 3 0 . . .
2 4 5 0 . .
2 7 5 2 0 .
2 8 7 5 3 0
3 0 . . . .
3 1 0 . . .
3 6 3 0 . .
3 5 7 4 0 .
3 9 6 3 1 0
END DATA.";

/// The ratings of the first two judges, one pair of fruits per case, with a weight.
const FRUIT_PAIRS: &str = "DATA LIST LIST /first second judge score trust.
BEGIN DATA
2 1 1 2 1
3 1 1 5 2
3 2 1 4 1
4 1 1 6 1
4 2 1 6 2
4 3 1 3 1
5 1 1 7 1
5 2 1 8 1
5 3 1 4 2
5 4 1 2 1
2 1 2 3 2
3 1 2 4 1
3 2 2 5 1
4 1 2 7 1
4 2 2 5 1
4 3 2 2 2
5 1 2 8 1
5 2 2 7 1
5 3 2 5 1
5 4 2 3 2
END DATA.
VALUE LABELS judge 1 'ann' 2 'ben'.";

/// Invented scores of six shoppers on five fruits, to make distances of.
const FRUIT_SCORES: &str = "DATA LIST LIST /apple pear plum cherry grape.
BEGIN DATA
4 3 2 5 1
5 4 1 4 2
2 2 4 3 5
3 4 3 2 4
1 2 5 3 4
4 5 2 4 2
END DATA.";

/// The scaling dialogs on invented proximities: the syntax is as expected, parses and runs.
#[test]
fn the_scaling_dialogs_write_syntax_that_runs() {
    use crate::mds_dialog::{Alscal, Proxscal};
    let out = folder("walk-scaling");
    let common = out.join("common.sav").to_string_lossy().into_owned();
    let fruits = v(&["apple", "pear", "plum", "cherry", "grape"]);
    let cases: Vec<(&str, Advanced, String)> = vec![
        (
            FRUIT_MATRICES,
            Advanced::Proxscal(Box::new(Proxscal {
                variables: fruits.clone(),
                source: s("judge"),
                ..Default::default()
            })),
            s("PROXSCAL apple pear plum cherry grape\n  /TABLE=judge."),
        ),
        (
            FRUIT_PAIRS,
            Advanced::Proxscal(Box::new(Proxscal {
                format: 1,
                variables: v(&["score"]),
                row: s("first"),
                column: s("second"),
                source: s("judge"),
                weights: v(&["trust"]),
                model: 1,
                transformation: 3,
                degree: s("3"),
                knots: s("2"),
                plot: [false, true, true, true, false, false],
                plot_sources: [v(&["ben"]), Vec::new(), Vec::new()],
                ..Default::default()
            })),
            s(
                "PROXSCAL score\n  /TABLE=first BY second BY judge\n  /WEIGHTS=trust\n  \
               /TRANSFORMATION=SPLINE DEGREE=3 INKNOT=2\n  /MODEL=WEIGHTED\n  \
               /PLOT=COMMON WEIGHTS INDIVIDUAL(ben).",
            ),
        ),
        (
            FRUIT_MATRICES,
            Advanced::Proxscal(Box::new(Proxscal {
                variables: fruits.clone(),
                source: s("judge"),
                shape: 2,
                start: 1,
                unconditional: true,
                transformation: 2,
                untie: true,
                model: 3,
                rank: s("2"),
                max_dimensions: s("3"),
                max_iterations: s("200"),
                diff_stress: s("0.00001"),
                acceleration: false,
                print: [true, false, true, true, true, true, true, false, true, true],
                plot: [true, true, false, false, true, true],
                plot_sources: [Vec::new(), Vec::new(), v(&["SRC_2"])],
                outfiles: [common.clone(), String::new(), String::new(), String::new()],
                ..Default::default()
            })),
            format!(
                "PROXSCAL apple pear plum cherry grape\n  /TABLE=judge\n  /SHAPE=BOTH\n  \
                 /INITIAL=TORGERSON\n  /CONDITION=UNCONDITIONAL\n  \
                 /TRANSFORMATION=ORDINAL(UNTIE)\n  /MODEL=REDUCED(2)\n  /ACCELERATION=NONE\n  \
                 /CRITERIA=DIMENSIONS(2,3) MAXITER(200) DIFFSTRESS(0.00001)\n  \
                 /PRINT=INPUT HISTORY STRESS DECOMPOSITION COMMON DISTANCES INDIVIDUAL \
                 TRANSFORMATIONS\n  /PLOT=STRESS COMMON TRANSFORMATIONS(ALL) RESIDUALS(SRC_2)\n  \
                 /OUTFILE=COMMON('{common}')."
            ),
        ),
        (
            FRUIT_MATRICES,
            Advanced::Alscal(Alscal {
                variables: fruits.clone(),
                ..Default::default()
            }),
            s("ALSCAL VARIABLES=apple pear plum cherry grape."),
        ),
        (
            FRUIT_SCORES,
            Advanced::Alscal(Alscal {
                from_data: true,
                variables: fruits.clone(),
                standardize: 1,
                measure: 1,
                plot: 1,
                ..Default::default()
            }),
            s(
                "DATASET DECLARE alscal_distances.\nPROXIMITIES apple pear plum cherry grape\n  \
               /VIEW=VARIABLE\n  /STANDARDIZE=VARIABLE Z\n  /MEASURE=SEUCLID\n  /PRINT=NONE\n  \
               /MATRIX=OUT(alscal_distances).\nALSCAL\n  /PLOT=DEFAULT\n  \
               /MATRIX=IN(alscal_distances).\nDATASET CLOSE alscal_distances.",
            ),
        ),
        // the matrix names the shoppers, and ALSCAL without VARIABLES takes them all
        (
            FRUIT_SCORES,
            Advanced::Alscal(Alscal {
                from_data: true,
                between_cases: true,
                variables: fruits.clone(),
                ..Default::default()
            }),
            s(
                "DATASET DECLARE alscal_distances.\nPROXIMITIES apple pear plum cherry grape\n  \
               /PRINT=NONE\n  /MATRIX=OUT(alscal_distances).\nALSCAL\n  \
               /MATRIX=IN(alscal_distances).\nDATASET CLOSE alscal_distances.",
            ),
        ),
        (
            FRUIT_MATRICES,
            Advanced::Alscal(Alscal {
                variables: fruits,
                level: 1,
                indscal: true,
                negative: true,
                max_dimensions: s("3"),
                print_header: true,
                plot: 2,
                ..Default::default()
            }),
            s(
                "ALSCAL VARIABLES=apple pear plum cherry grape\n  /LEVEL=INTERVAL(1)\n  \
               /MODEL=INDSCAL\n  /CRITERIA=NEGATIVE DIMENS(2,3)\n  /PRINT=HEADER\n  /PLOT=ALL.",
            ),
        ),
    ];
    for (program, dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(dialog), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        // nothing asked for was dropped, such as a source to plot or a model
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
        // the distances made from the data leave the open data active
        let after = session.dataset().unwrap();
        assert_eq!(after.variable_count(), data.variable_count(), "{syntax}");
    }
    std::fs::remove_dir_all(out).unwrap();
}

/// Missing Value Analysis on the survey file, whose ratings have user-missing codes and whose
/// later mentions are often empty: the syntax is as expected, parses and runs without a warning.
#[test]
fn the_missing_value_dialog_writes_syntax_that_runs() {
    let out = folder("walk-mva");
    let file = out
        .join("regression filled.sav")
        .to_string_lossy()
        .into_owned();
    let base = MissingValueAnalysis::default();
    let cases: Vec<(MissingValueAnalysis, String)> = vec![
        (
            MissingValueAnalysis {
                quantitative: v(&["age", "q2_recommend"]),
                ..base.clone()
            },
            s("MVA VARIABLES=age q2_recommend."),
        ),
        // a string category needs no CATEGORICAL
        (
            MissingValueAnalysis {
                quantitative: v(&["age", "monthly_spend", "q2_recommend"]),
                categorical: v(&["region", "plan"]),
                maxcat: s("10"),
                id: s("resp_id"),
                listwise: true,
                pairwise: true,
                ..base.clone()
            },
            s(
                "MVA VARIABLES=age monthly_spend q2_recommend region plan\n  /CATEGORICAL=region\n  \
                 /MAXCAT=10\n  /ID=resp_id\n  /LISTWISE\n  /PAIRWISE.",
            ),
        ),
        (
            MissingValueAnalysis {
                quantitative: v(&["age", "q1_satisfaction", "q2_recommend"]),
                categorical: v(&["gender"]),
                univariate: false,
                tpattern: true,
                tpattern_percent: s("0"),
                tpattern_nosort: true,
                mpattern: true,
                mpattern_nosort: true,
                dpattern: true,
                sort: v(&["region", "age"]),
                descending: v(&["region"]),
                describe: v(&["region", "monthly_spend"]),
                ..base.clone()
            },
            s(
                "MVA VARIABLES=age q1_satisfaction q2_recommend gender\n  /CATEGORICAL=gender\n  \
                 /NOUNIVARIATE\n  /DPATTERN SORT=region(DESCENDING) age DESCRIBE=region \
                 monthly_spend\n  /MPATTERN NOSORT DESCRIBE=region monthly_spend\n  \
                 /TPATTERN NOSORT PERCENT=0 DESCRIBE=region monthly_spend.",
            ),
        ),
        (
            MissingValueAnalysis {
                quantitative: v(&["age", "q2_recommend", "q6_mention2"]),
                categorical: v(&["region"]),
                ttest: true,
                ttest_percent: s("0"),
                prob: true,
                crosstab: true,
                crosstab_percent: s("0"),
                mismatch: true,
                mismatch_percent: s("2.5"),
                mismatch_nosort: true,
                ..base.clone()
            },
            s(
                "MVA VARIABLES=age q2_recommend q6_mention2 region\n  /CATEGORICAL=region\n  \
                 /TTEST PERCENT=0 PROB\n  /CROSSTAB PERCENT=0\n  /MISMATCH PERCENT=2.5 NOSORT.",
            ),
        ),
        (
            MissingValueAnalysis {
                quantitative: v(&["age", "monthly_spend", "q1_satisfaction", "q2_recommend"]),
                em: Estimation {
                    on: true,
                    predicted: v(&["q2_recommend", "q1_satisfaction"]),
                    predictors: v(&["age", "monthly_spend"]),
                    distribution: 1,
                    df: s("4"),
                    iterations: s("50"),
                    output: 2,
                    dataset: s("em_filled"),
                    ..Default::default()
                },
                ..base.clone()
            },
            s(
                "DATASET DECLARE em_filled.\nMVA VARIABLES=age monthly_spend q1_satisfaction \
                 q2_recommend\n  /EM q2_recommend q1_satisfaction WITH age monthly_spend \
                 (ITERATIONS=50 TDF=4 OUTFILE=em_filled).",
            ),
        ),
        (
            MissingValueAnalysis {
                quantitative: v(&["age", "monthly_spend", "q2_recommend"]),
                regression: Estimation {
                    on: true,
                    predicted: v(&["q2_recommend"]),
                    addtype: 3,
                    npredictors: s("1"),
                    flimit: s("2"),
                    output: 1,
                    file: file.clone(),
                    ..Default::default()
                },
                ..base.clone()
            },
            format!(
                "MVA VARIABLES=age monthly_spend q2_recommend\n  /REGRESSION q2_recommend \
                 (FLIMIT=2 NPREDICTORS=1 ADDTYPE=T(5) OUTFILE='{file}')."
            ),
        ),
    ];
    for (dialog, expected) in cases {
        let mut session = open("survey_basic.sav");
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(Advanced::Mva(Box::new(dialog))), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
    assert!(Path::new(&file).exists());
    std::fs::remove_dir_all(out).unwrap();
}

/// Invented bakery shifts: hours worked, loaves baked, kilograms thrown away, the oven used and
/// a weight. Loaves are missing in every sixth shift, waste also in every tenth, so hours,
/// loaves and waste form a monotone pattern; the oven is missing in every eighth shift.
fn bakery_shifts() -> String {
    let mut program =
        String::from("DATA LIST LIST /shift hours loaves waste_kg oven share.\nBEGIN DATA\n");
    for i in 1..=60u32 {
        let hours = 4 + (i * 7) % 9;
        let loaves = 10 * hours + (i * 13) % 17;
        let waste = f64::from(loaves) / 10.0 + f64::from((i * 5) % 7) * 0.3;
        let oven = i % 3 + 1;
        let share = 0.5 + f64::from(i % 4) * 0.25;
        let gap = |missing: bool, text: String| if missing { ".".to_string() } else { text };
        program.push_str(&format!(
            "{i} {hours} {} {} {} {share}\n",
            gap(i % 6 == 0, loaves.to_string()),
            gap(i % 6 == 0 || i % 10 == 0, format!("{waste:.1}")),
            gap(i % 8 == 0, oven.to_string()),
        ));
    }
    program.push_str(
        "END DATA.\nVARIABLE LEVEL oven (NOMINAL) /hours loaves waste_kg share (SCALE).\n",
    );
    program
}

/// Multiple Imputation on invented bakery shifts: the syntax is as expected, parses and runs
/// without a warning, and the files asked for are written.
#[test]
fn the_imputation_dialogs_write_syntax_that_runs() {
    let out = folder("walk-imputation");
    let path = |name: &str| out.join(name).to_string_lossy().into_owned();
    let (imputed, history) = (path("shifts imputed.sav"), path("history.sav"));
    let base = Imputation {
        variables: v(&["hours", "loaves", "waste_kg"]),
        ..Default::default()
    };
    let cases: Vec<(Imputation, String)> = vec![
        (
            Imputation {
                dataset: s("filled"),
                ..base.clone()
            },
            s(
                "DATASET DECLARE filled.\nMULTIPLE IMPUTATION hours loaves waste_kg\n  \
               /OUTFILE IMPUTATIONS=filled.",
            ),
        ),
        // iterative, with limits and rounding, and the oven only predicting
        (
            Imputation {
                variables: v(&["hours", "loaves", "waste_kg", "oven"]),
                nimputations: s("3"),
                dataset: s("filled"),
                method: 1,
                maxiter: s("4"),
                maxcasedraws: s("80"),
                limits: vec![
                    Limits {
                        name: s("loaves"),
                        min: s("30"),
                        max: s("140"),
                        rnd: s("1"),
                        ..Default::default()
                    },
                    Limits {
                        name: s("oven"),
                        role: 2,
                        ..Default::default()
                    },
                ],
                ..base.clone()
            },
            s(
                "DATASET DECLARE filled.\nMULTIPLE IMPUTATION hours loaves waste_kg oven\n  \
               /IMPUTE METHOD=FCS NIMPUTATIONS=3 MAXITER=4 MAXCASEDRAWS=80\n  \
               /CONSTRAINTS loaves (MIN=30 MAX=140 RND=1)\n  /CONSTRAINTS oven (ROLE=IND)\n  \
               /OUTFILE IMPUTATIONS=filled.",
            ),
        ),
        // monotone in list order, matching on the three nearest predictions
        (
            Imputation {
                dataset: s("filled"),
                method: 2,
                pmm: true,
                pmm_k: s("3"),
                singular: s("1E-10"),
                limits: vec![Limits {
                    name: s("waste_kg"),
                    rnd: s("0.5"),
                    role: 1,
                    ..Default::default()
                }],
                ..base.clone()
            },
            s(
                "DATASET DECLARE filled.\nMULTIPLE IMPUTATION hours loaves waste_kg\n  \
               /IMPUTE METHOD=MONOTONE SCALEMODEL=PMM(3) SINGULAR=0.0000000001\n  \
               /CONSTRAINTS waste_kg (RND=0.5 ROLE=DEP)\n  /OUTFILE IMPUTATIONS=filled.",
            ),
        ),
        // tables of their own, a weight, and both outputs to files
        (
            Imputation {
                variables: v(&["hours", "loaves", "waste_kg", "oven"]),
                method: 1,
                twoway: true,
                maxpctmissing: s("40"),
                overall: false,
                variable_summary: true,
                minpct: s("5"),
                maxvars: s("3"),
                patterns: true,
                models: false,
                descriptives: true,
                described: v(&["loaves", "oven"]),
                weight: s("share"),
                output: 1,
                file: imputed.clone(),
                history: 1,
                history_file: history.clone(),
                ..base.clone()
            },
            format!(
                "MULTIPLE IMPUTATION hours loaves waste_kg oven\n  /IMPUTE METHOD=FCS \
                 INTERACTIONS=TWOWAY MAXPCTMISSING=40\n  /MISSINGSUMMARIES \
                 VARIABLES(MAXVARS=3 MINPCTMISSING=5) PATTERNS\n  /IMPUTATIONSUMMARIES \
                 DESCRIPTIVES(loaves oven)\n  /ANALYSISWEIGHT share\n  \
                 /OUTFILE IMPUTATIONS='{imputed}' FCSITERATIONS='{history}'."
            ),
        ),
        (
            Imputation {
                patterns_only: true,
                variables: v(&["hours", "loaves", "waste_kg", "oven"]),
                variable_summary: true,
                minpct: s("0"),
                patterns: true,
                ..base.clone()
            },
            s(
                "MULTIPLE IMPUTATION hours loaves waste_kg oven\n  /IMPUTE METHOD=NONE\n  \
               /MISSINGSUMMARIES OVERALL VARIABLES(MINPCTMISSING=0) PATTERNS.",
            ),
        ),
    ];
    let program = bakery_shifts();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(
            Dialog::Advanced(Advanced::Imputation(Box::new(dialog))),
            &data,
        )
        .unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
    assert!(Path::new(&imputed).exists());
    assert!(Path::new(&history).exists());
    std::fs::remove_dir_all(out).unwrap();
}

/// Case Summaries and Codebook on the survey file with a set of mentions: the syntax is as
/// expected, parses and runs without a warning.
#[test]
fn the_report_dialogs_write_syntax_that_runs() {
    let spend = Means {
        dependents: v(&["monthly_spend", "q2_recommend"]),
        ..Default::default()
    };
    let mut cells = [false; 21];
    // count, mean, median and maximum
    for i in [0, 1, 3, 8] {
        cells[i] = true;
    }
    let mut varinfo = [true; 10];
    varinfo[0] = false;
    varinfo[5] = false;
    let mut fileinfo = [false; 8];
    fileinfo[2] = true;
    fileinfo[6] = true;
    let cases: Vec<(Advanced, &str)> = vec![
        (
            Advanced::Summarize(Box::new(Summarize {
                means: spend.clone(),
                ..Default::default()
            })),
            "SUMMARIZE TABLES=monthly_spend q2_recommend.",
        ),
        // grouped, the cases listed, and every option changed
        (
            Advanced::Summarize(Box::new(Summarize {
                means: Means {
                    layers: vec![v(&["region"]), v(&["gender"])],
                    cells,
                    anova: true,
                    linearity: true,
                    ..spend.clone()
                },
                list: true,
                limit: s("60"),
                case_numbers: false,
                total_label: false,
                missing_text: s("n/a"),
                title: s("Spend by region"),
                footnote: s("Invented survey\\nsecond line"),
                ..Default::default()
            })),
            "SUMMARIZE TABLES=monthly_spend q2_recommend BY region BY gender\n  \
             /CELLS=MEAN COUNT MEDIAN MAX\n  /STATISTICS=LINEARITY\n  \
             /FORMAT=LIST LIMIT=60 NOCASENUM MISSING='n/a' NOTOTAL\n  \
             /TITLE='Spend by region'\n  /FOOTNOTE='Invented survey\\nsecond line'.",
        ),
        // listed with case numbers, only valid cases, ANOVA alone
        (
            Advanced::Summarize(Box::new(Summarize {
                means: Means {
                    layers: vec![v(&["plan"])],
                    anova: true,
                    ..spend.clone()
                },
                list: true,
                valid_only: true,
                tablewise: true,
                ..Default::default()
            })),
            "SUMMARIZE TABLES=monthly_spend q2_recommend BY plan\n  /STATISTICS=ANOVA\n  \
             /FORMAT=VALIDLIST\n  /MISSING=TABLE.",
        ),
        (Advanced::Codebook(Codebook::default()), "CODEBOOK."),
        (
            Advanced::Codebook(Codebook {
                variables: v(&["region", "$mentions", "age", "plan"]),
                varinfo,
                fileinfo,
                statistics: [true, false, true, false, true],
                maxcats: s("4"),
                order: 3,
                descending: true,
                ..Default::default()
            }),
            "CODEBOOK region $mentions age plan\n  \
             /VARINFO=LABEL TYPE FORMAT MEASURE ATTRIBUTES VALUELABELS MISSING RESERVEDATTRIBUTES\n  \
             /FILEINFO=CASECOUNT WEIGHT\n  /STATISTICS=COUNT MEAN QUARTILES\n  \
             /OPTIONS MAXCATS=4 VARORDER=MEASURE SORT=DESCENDING.",
        ),
        (
            Advanced::Codebook(Codebook {
                variables: v(&["$mentions", "gender"]),
                statistics: [false; 5],
                order: 2,
                ..Default::default()
            }),
            "CODEBOOK $mentions gender\n  /STATISTICS=NONE\n  /OPTIONS VARORDER=ALPHA.",
        ),
    ];
    for (dialog, expected) in cases {
        let mut session = open("survey_basic.sav");
        session
            .run(
                "MRSETS /MCGROUP NAME=$mentions VARIABLES=q6_mention1 q6_mention2 q6_mention3.",
                None,
            )
            .unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(dialog), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
}

/// Invented bike rentals: the station, the kind of bike, whether it came back late, the minutes
/// ridden, whether the dock was open (a cell weight), a coefficient for the downtown station and
/// a contrast of late against on time.
fn bike_rentals() -> String {
    let mut program = String::from(
        "DATA LIST LIST /station bike late minutes dock_open downtown contrast.\nBEGIN DATA\n",
    );
    for i in 1..=120u32 {
        let station = i % 3 + 1;
        let bike = (i / 3) % 2 + 1;
        let late = u32::from((i * 7 + station * 3) % 5 < 2);
        let minutes = 10 + (i * 11) % 37;
        let downtown = u32::from(station == 1);
        let contrast = if late == 1 { 1 } else { -1 };
        program.push_str(&format!(
            "{station} {bike} {late} {minutes} 1 {downtown} {contrast}\n"
        ));
    }
    program.push_str("END DATA.\n");
    program
}

/// The Loglinear dialogs on invented bike rentals: the syntax is as expected, parses and runs,
/// with no warning but the one a case expects.
#[test]
fn the_loglinear_dialogs_write_syntax_that_runs() {
    let factors = v(&["station", "bike", "late"]);
    let ranges = vec![
        table_variable("station", "1", "3", &[]),
        table_variable("bike", "1", "2", &[]),
        table_variable("late", "0", "1", &[]),
    ];
    let selection = ModelSelection {
        factors: factors.clone(),
        ranges,
        ..Default::default()
    };
    let terms = |list: &[&[&str]]| Terms {
        terms: list.iter().map(|t| v(t)).collect(),
        building: Vec::new(),
    };
    let cases: Vec<(Advanced, &str, Option<&str>)> = vec![
        (
            Advanced::Hiloglinear(Box::new(selection.clone())),
            "HILOGLINEAR station(1,3) bike(1,2) late(0,1)\n  /DESIGN.",
            None,
        ),
        // backward from the two-way terms, with every table and plot; estimates and
        // associations exist only for a saturated model, so the command says so
        (
            Advanced::Hiloglinear(Box::new(ModelSelection {
                backward: true,
                maxorder: s("2"),
                maxsteps: s("5"),
                probability: s("0.1"),
                iterations: s("30"),
                converge: s("0.01"),
                delta: s("0.25"),
                cweight: s("dock_open"),
                print_chosen: true,
                print: [true; 4],
                plots: [true; 2],
                include: true,
                custom: true,
                terms: terms(&[
                    &["station", "bike"],
                    &["bike", "late"],
                    &["station", "late"],
                ]),
                ..selection.clone()
            })),
            "HILOGLINEAR station(1,3) bike(1,2) late(0,1)\n  /METHOD=BACKWARD\n  /MAXORDER=2\n  \
             /CRITERIA=MAXSTEPS(5) P(0.1) ITERATE(30) CONVERGE(0.01) DELTA(0.25)\n  \
             /CWEIGHT=dock_open\n  /PRINT=FREQ RESID ESTIM ASSOCIATION\n  /PLOT=RESID NORMPROB\n  \
             /MISSING=INCLUDE\n  /DESIGN=station*bike bike*late station*late.",
            Some("need the saturated model"),
        ),
        (
            Advanced::Genlog(Box::new(GeneralLoglinear {
                factors: factors.clone(),
                ..GeneralLoglinear::new(false)
            })),
            "GENLOG station bike late.",
            None,
        ),
        (
            Advanced::Genlog(Box::new(GeneralLoglinear {
                factors: factors.clone(),
                covariates: v(&["minutes"]),
                cstructure: s("dock_open"),
                gresid: v(&["downtown"]),
                multinomial: true,
                iterations: s("50"),
                ci_level: s("90"),
                print: [true; 10],
                normal_plots: false,
                plotted: 1,
                save: [true, false, false, true, false],
                save_names: [s(""), s(""), s(""), s("adj_fit"), s("")],
                custom: true,
                terms: terms(&[
                    &["station"],
                    &["bike"],
                    &["late"],
                    &["station", "bike"],
                    &["minutes"],
                ]),
                ..GeneralLoglinear::new(false)
            })),
            "GENLOG station bike late WITH minutes\n  /CSTRUCTURE=dock_open\n  /GRESID=downtown\n  \
             /MODEL=MULTINOMIAL\n  /CRITERIA=ITERATE(50) CIN(90)\n  \
             /PRINT=FREQ RESID ZRESID ADJRESID DEV ITERATION COV DESIGN ESTIM CORR\n  \
             /PLOT=RESID(DEV)\n  /SAVE=PRED ADJRESID(adj_fit)\n  \
             /DESIGN=station bike late station*bike minutes.",
            None,
        ),
        (
            Advanced::Genlog(Box::new(GeneralLoglinear {
                dependents: v(&["late"]),
                factors: v(&["station"]),
                ..GeneralLoglinear::new(true)
            })),
            "GENLOG late BY station\n  /MODEL=MULTINOMIAL.",
            None,
        ),
        (
            Advanced::Genlog(Box::new(GeneralLoglinear {
                dependents: v(&["late"]),
                factors: v(&["station", "bike"]),
                glor: v(&["contrast"]),
                custom: true,
                terms: terms(&[&["late"], &["late", "station"], &["late", "bike"]]),
                ..GeneralLoglinear::new(true)
            })),
            "GENLOG late BY station bike\n  /GLOR=contrast\n  /MODEL=MULTINOMIAL\n  \
             /DESIGN=late late*station late*bike.",
            None,
        ),
    ];
    let program = bike_rentals();
    for (dialog, expected, warning) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(dialog), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        match warning {
            None => assert!(warnings.is_empty(), "{syntax}\n{warnings:?}"),
            Some(w) => assert!(
                warnings.iter().all(|x| x.contains(w)) && !warnings.is_empty(),
                "{syntax}\n{warnings:?}"
            ),
        }
    }
}

/// Invented grades of thirty essays by three tutors on a scale of 1 to 4, mostly agreeing, and
/// the fuel and distance of the vans of three depots.
fn essays_and_vans() -> String {
    let mut program =
        String::from("DATA LIST LIST /tutor_a tutor_b tutor_c depot km litres.\nBEGIN DATA\n");
    for i in 1..=30u32 {
        let grade = i % 4 + 1;
        let b = if i % 5 == 0 {
            (grade + 1).min(4)
        } else {
            grade
        };
        let c = if i % 7 == 0 { grade.max(2) - 1 } else { grade };
        let km = 100 + (i * 37) % 200;
        let litres = f64::from(km) * (0.07 + f64::from((i * 13) % 10) / 200.0);
        program.push_str(&format!("{grade} {b} {c} {} {km} {litres:.1}\n", i % 3 + 1));
    }
    program.push_str("END DATA.\n");
    program
}

/// The kappa and Ratio Statistics dialogs on invented essays and vans: the syntax is as
/// expected, parses and runs without a warning.
#[test]
fn the_agreement_dialogs_write_syntax_that_runs() {
    let tutors = v(&["tutor_a", "tutor_b", "tutor_c"]);
    let cases: Vec<(Advanced, &str)> = vec![
        (
            Advanced::WeightedKappa(WeightedKappa {
                raters: v(&["tutor_a", "tutor_b"]),
                ..Default::default()
            }),
            "WEIGHTED KAPPA tutor_a tutor_b.",
        ),
        (
            Advanced::WeightedKappa(WeightedKappa {
                raters: v(&["tutor_a"]),
                by: true,
                columns: v(&["tutor_b", "tutor_c"]),
                quadratic: true,
                include: true,
                tabulate: true,
                descending: true,
                ci_level: s("90"),
            }),
            "WEIGHTED KAPPA tutor_a BY tutor_b tutor_c\n  /MISSING CLASSMISSING=INCLUDE\n  \
             /CRITERIA ASYMPTOTICCILEVEL=90 WEIGHTING=QUADRATIC\n  \
             /PRINT TABULATION(ALL) ORDER(DESCENDING).",
        ),
        (
            Advanced::Fleiss(FleissKappa {
                raters: tutors.clone(),
                ..Default::default()
            }),
            "FLEISS MULTIRATER KAPPA tutor_a tutor_b tutor_c.",
        ),
        (
            Advanced::Fleiss(FleissKappa {
                raters: tutors.clone(),
                include: true,
                category: true,
                ci_level: s("99"),
            }),
            "FLEISS MULTIRATER KAPPA tutor_a tutor_b tutor_c\n  /MISSING CLASSMISSING=INCLUDE\n  \
             /CRITERIA ASYMPTOTICCILEVEL=99\n  /PRINT CATEGORY.",
        ),
        (
            Advanced::Ratio(Box::new(Ratio {
                numerator: s("litres"),
                denominator: s("km"),
                statistics: [
                    true, false, false, false, false, false, false, false, false, false, false,
                    false,
                ],
                ..Default::default()
            })),
            "RATIO STATISTICS litres WITH km\n  /PRINT=MEDIAN.",
        ),
        (
            Advanced::Ratio(Box::new(Ratio {
                numerator: s("litres"),
                denominator: s("km"),
                group: s("depot"),
                order: 1,
                statistics: [true; 12],
                cin: true,
                cin_level: s("90"),
                bcoc: true,
                intervals: s("0.05 0.08; 0.08, 0.12"),
                wcoc: true,
                percents: s("10, 25"),
                include: true,
                save: 2,
                dataset: s("van_ratios"),
                ..Default::default()
            })),
            "DATASET DECLARE van_ratios.\n\
             RATIO STATISTICS litres WITH km BY depot (DESCENDING)\n  /MISSING=INCLUDE\n  \
             /PRINT=MEDIAN MEAN WGTMEAN MIN MAX RANGE STDDEV AAD COD PRD MNCOV MDCOV CIN(90) \
             BCOC((0.05,0.08) (0.08,0.12)) WCOC(10 25)\n  \
             /OUTFILE('van_ratios')=MEDIAN MEAN WGTMEAN MIN MAX RANGE STDDEV AAD COD PRD MNCOV \
             MDCOV CIN(90) BCOC((0.05,0.08) (0.08,0.12)) WCOC(10 25).",
        ),
    ];
    let program = essays_and_vans();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(dialog), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
        // the saved statistics fill the declared dataset: the overall record and one per depot
        if syntax.starts_with("DATASET DECLARE") {
            let result = session
                .run("DATASET ACTIVATE van_ratios.\nLIST.", None)
                .unwrap();
            assert!(result.commands.iter().any(|c| !c.tables.is_empty()));
            assert_eq!(session.dataset().unwrap().case_count(), 4);
        }
    }
}

/// Invented rides of a cycling club: membership, bike type, route and lane, minutes on three
/// climbs, whether each of three days was ridden in the rain, and the effort felt on two rides.
fn cycling_club() -> String {
    let mut program = String::from(
        "DATA LIST LIST /member bike route lane climb_a climb_b climb_c rain_mon rain_tue \
         rain_wed effort1 effort2.\nBEGIN DATA\n",
    );
    for i in 1..=30u32 {
        let route = i % 3 + 1;
        let climb_a = 20.0 + f64::from((i * 37) % 100) / 10.0 + f64::from(route);
        let climb_b = climb_a + 1.2 + f64::from((i * 13) % 7) / 10.0 - 0.3;
        let climb_c = climb_a + 2.5 + f64::from((i * 11) % 9) / 10.0;
        let effort1 = i % 5 + 1;
        let effort2 = (effort1 + u32::from(i % 3 == 0)).min(5);
        program.push_str(&format!(
            "{} {} {route} {} {climb_a:.1} {climb_b:.1} {climb_c:.1} {} {} {} {effort1} \
             {effort2}\n",
            if i % 3 == 0 { 2 } else { 1 },
            i % 4 + 1,
            if i <= 15 { 1 } else { 2 },
            i % 2,
            u32::from(i % 3 == 0),
            u32::from(i % 4 < 2),
        ));
    }
    program.push_str(
        "END DATA.\nVARIABLE LEVEL member bike route lane rain_mon rain_tue rain_wed (NOMINAL) \
         /effort1 effort2 (ORDINAL) /climb_a climb_b climb_c (SCALE).\n",
    );
    program
}

/// The Nonparametric Tests dialogs on invented rides: left to the data, customized, and with
/// changed criteria and missing values. The syntax is as expected, parses and runs.
#[test]
fn the_nonparametric_tests_dialogs_write_syntax_that_runs() {
    use crate::nptests_dialog::{NpKind, Nptests};
    let new = |kind: NpKind, fields: &[&str]| Nptests {
        fields: v(fields),
        ..Nptests::new(kind)
    };
    let cases: Vec<(Nptests, &str)> = vec![
        (
            new(NpKind::OneSample, &["member", "bike", "climb_a"]),
            "NPTESTS /ONESAMPLE TEST(member bike climb_a).",
        ),
        (
            Nptests {
                customize: true,
                binomial: true,
                proportion: s("0.4"),
                cut_value: true,
                cut: s("25"),
                intervals: [true, true, false],
                chisquare: true,
                expected_custom: true,
                categories: s("1 2 3 4"),
                frequencies: s("2 1 1 1"),
                ks: true,
                distributions: [true, true, true, false],
                normal_custom: true,
                normal_mean: s("26"),
                normal_sd: s("3"),
                exponential_custom: true,
                exponential_mean: s("25"),
                samples: s("500"),
                runs: true,
                runs_cut: 1,
                wilcoxon: true,
                test_median: s("25"),
                ..new(NpKind::OneSample, &["member", "bike", "climb_a"])
            },
            "NPTESTS /ONESAMPLE TEST(member bike climb_a)\n    \
             BINOMIAL(TESTVALUE=0.4 SUCCESSCONTINUOUS=CUTPOINT(25) CLOPPERPEARSON JEFFREYS)\n    \
             CHISQUARE(EXPECTED=CUSTOM(FREQUENCIES=2 1 1 1 CATEGORIES=1 2 3 4))\n    \
             KOLMOGOROV_SMIRNOV(NSAMPLES=500 NORMAL=CUSTOM(MEAN=26 SD=3) UNIFORM=SAMPLE \
             EXPONENTIAL=CUSTOM(MEAN=25))\n    \
             RUNS(GROUPCONTINUOUS=CUTPOINT(SAMPLEMEAN))\n    \
             WILCOXON(TESTVALUE=25).",
        ),
        (
            Nptests {
                customize: true,
                binomial: true,
                success_list: true,
                success: s("2"),
                runs: true,
                runs_list: true,
                runs_values: s("1"),
                runs_cut: 2,
                runs_cut_value: s("24.5"),
                ks: true,
                distributions: [false, false, false, true],
                poisson_mean: s("2"),
                ..new(NpKind::OneSample, &["member", "bike"])
            },
            "NPTESTS /ONESAMPLE TEST(member bike)\n    \
             BINOMIAL(SUCCESSCATEGORICAL=LIST(2))\n    \
             KOLMOGOROV_SMIRNOV(POISSON=CUSTOM(MEAN=2))\n    \
             RUNS(GROUPCATEGORICAL=LIST(1) GROUPCONTINUOUS=CUTPOINT(24.5)).",
        ),
        (
            Nptests {
                listwise: true,
                include: true,
                alpha: s("0.01"),
                ci_level: s("90"),
                ..new(NpKind::OneSample, &["member", "climb_a"])
            },
            "NPTESTS /ONESAMPLE TEST(member climb_a)\n  \
             /MISSING SCOPE=LISTWISE USERMISSING=INCLUDE\n  /CRITERIA ALPHA=0.01 CILEVEL=90.",
        ),
        (
            Nptests {
                group: s("route"),
                ..new(NpKind::Independent, &["climb_a", "climb_b"])
            },
            "NPTESTS /INDEPENDENT TEST(climb_a climb_b) GROUP(route).",
        ),
        (
            Nptests {
                group: s("lane"),
                customize: true,
                two_tests: [true; 4],
                moses: true,
                trim: s("1"),
                ..new(NpKind::Independent, &["climb_a", "effort1"])
            },
            "NPTESTS /INDEPENDENT TEST(climb_a effort1) GROUP(lane)\n    \
             MANN_WHITNEY\n    WALD_WOLFOWITZ\n    KOLMOGOROV_SMIRNOV\n    HODGES_LEHMAN\n    \
             MOSES(TRIMOUTLIERS=1).",
        ),
        (
            Nptests {
                group: s("route"),
                customize: true,
                k_tests: [true; 3],
                compare: [1, 2, 0],
                median_value: s("25"),
                descending: true,
                ..new(NpKind::Independent, &["climb_a"])
            },
            "NPTESTS /INDEPENDENT TEST(climb_a) GROUP(route)\n    \
             KRUSKAL_WALLIS(COMPARE=STEPWISE)\n    MEDIAN(TESTVALUE=25 COMPARE=NONE)\n    \
             JONCKHEERE_TERPSTRA(ORDER=DESCENDING).",
        ),
        (
            Nptests {
                group: s("lane"),
                listwise: true,
                alpha: s("0.1"),
                ..new(NpKind::Independent, &["climb_a"])
            },
            "NPTESTS /INDEPENDENT TEST(climb_a) GROUP(lane)\n  /MISSING SCOPE=LISTWISE\n  \
             /CRITERIA ALPHA=0.1.",
        ),
        (
            new(NpKind::Related, &["climb_a", "climb_b", "climb_c"]),
            "NPTESTS /RELATED TEST(climb_a climb_b climb_c).",
        ),
        (
            Nptests {
                customize: true,
                mcnemar: true,
                success_list: true,
                success: s("4, 5"),
                two_tests: [true; 4],
                ..new(NpKind::Related, &["effort1", "effort2"])
            },
            "NPTESTS /RELATED TEST(effort1 effort2)\n    MCNEMAR(SUCCESS=LIST(4 5))\n    \
             WILCOXON\n    SIGN\n    MARGINAL_HOMOGENEITY\n    HODGES_LEHMAN.",
        ),
        (
            Nptests {
                customize: true,
                k_tests: [true, false, false],
                compare: [1, 0, 0],
                success_list: true,
                success: s("1"),
                ..new(NpKind::Related, &["rain_mon", "rain_tue", "rain_wed"])
            },
            "NPTESTS /RELATED TEST(rain_mon rain_tue rain_wed)\n    \
             COCHRAN(SUCCESS=LIST(1) COMPARE=STEPWISE).",
        ),
        (
            Nptests {
                customize: true,
                k_tests: [false, true, true],
                compare: [0, 2, 0],
                ..new(NpKind::Related, &["climb_a", "climb_b", "climb_c"])
            },
            "NPTESTS /RELATED TEST(climb_a climb_b climb_c)\n    FRIEDMAN(COMPARE=NONE)\n    \
             KENDALL.",
        ),
        (
            Nptests {
                include: true,
                ci_level: s("99"),
                ..new(NpKind::Related, &["climb_a", "climb_b"])
            },
            "NPTESTS /RELATED TEST(climb_a climb_b)\n  /MISSING USERMISSING=INCLUDE\n  \
             /CRITERIA CILEVEL=99.",
        ),
    ];
    let program = cycling_club();
    for (dialog, expected) in cases {
        // the customized one-sample tests name fields some tests do not take
        let skips = dialog.kind == NpKind::OneSample && dialog.customize;
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(Advanced::Nptests(Box::new(dialog))), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(
            warnings
                .iter()
                .all(|w| skips && (w.contains("does not test") || w.contains("needs two"))),
            "{syntax}\n{warnings:?}"
        );
    }
}

/// Four invented years of a village bakery by month: loaves sold, with a summer peak and slow
/// growth, and the rainfall in millimetres.
fn bakery_months() -> String {
    let peak = [3, 5, 9, 14, 22, 30, 34, 31, 20, 11, 6, 2];
    let wet = [61, 48, 52, 40, 35, 28, 22, 30, 45, 66, 72, 69];
    let mut program = String::from("DATA LIST LIST /loaves rainfall.\nBEGIN DATA\n");
    for t in 0..48usize {
        let loaves = 180 + 2 * t + peak[t % 12] * 3 + (t * 7) % 5;
        let rainfall = wet[t % 12] + (t * t) % 7;
        program.push_str(&format!("{loaves} {rainfall}\n"));
    }
    program.push_str("END DATA.\n");
    program
}

/// The time series dialogs on the invented bakery months: the syntax is as expected, parses
/// and runs without a warning.
#[test]
fn the_time_series_dialogs_write_syntax_that_runs() {
    let both = v(&["loaves", "rainfall"]);
    let series = |d: TimeSeries| Advanced::TimeSeries(Box::new(d));
    let acf = |d: Correlogram| series(TimeSeries::Correlogram(d));
    let row = |function: usize, sources: &[&str], first: &str, second: &str| CreateRow {
        first: s(first),
        second: s(second),
        ..CreateRow::new(function, v(sources))
    };
    let cases: Vec<(Advanced, &str)> = vec![
        (
            acf(Correlogram {
                variables: v(&["loaves"]),
                ..Default::default()
            }),
            "ACF VARIABLES=loaves.",
        ),
        (
            acf(Correlogram {
                variables: both.clone(),
                ln: true,
                diff: true,
                sdiff: true,
                period: s("12"),
                max_lag: s("10"),
                display: 2,
                ma_errors: true,
                ..Default::default()
            }),
            "ACF VARIABLES=loaves rainfall\n  /LN\n  /DIFF=1\n  /SDIFF=1\n  /PERIOD=12\n  \
             /MXAUTO=10\n  /SERROR=MA\n  /PACF.",
        ),
        (
            acf(Correlogram {
                variables: v(&["loaves"]),
                diff: true,
                diff_order: s("2"),
                display: 1,
                ..Default::default()
            }),
            "PACF VARIABLES=loaves\n  /DIFF=2.",
        ),
        (
            acf(Correlogram {
                variables: both.clone(),
                ..Correlogram::new(true)
            }),
            "CCF VARIABLES=loaves rainfall.",
        ),
        (
            acf(Correlogram {
                variables: both.clone(),
                ln: true,
                diff: true,
                sdiff: true,
                period: s("12"),
                max_lag: s("5"),
                ..Correlogram::new(true)
            }),
            "CCF VARIABLES=loaves rainfall\n  /LN\n  /DIFF=1\n  /SDIFF=1\n  /PERIOD=12\n  \
             /MXCROSS=5.",
        ),
        (
            series(TimeSeries::Season(Season {
                variables: v(&["loaves"]),
                period: s("12"),
                ..Default::default()
            })),
            "SEASON VARIABLES=loaves\n  /PERIOD=12.",
        ),
        (
            series(TimeSeries::Season(Season {
                variables: both.clone(),
                additive: true,
                equal: true,
                period: s("12"),
            })),
            "SEASON VARIABLES=loaves rainfall\n  /MODEL=ADDITIVE\n  /MA=EQUAL\n  /PERIOD=12.",
        ),
        (
            series(TimeSeries::Create(CreateSeries {
                rows: vec![
                    row(0, &["loaves"], "", ""),
                    row(1, &["loaves"], "1", ""),
                    row(2, &["loaves"], "1", "12"),
                    CreateRow {
                        names: s("rain_l1 rain_l2"),
                        ..row(3, &["rainfall"], "1", "2")
                    },
                    row(4, &["rainfall"], "1", ""),
                    row(5, &["loaves"], "5", "3"),
                    row(6, &["loaves"], "3", ""),
                    row(7, &["loaves"], "4", ""),
                    row(8, &["loaves"], "", ""),
                    row(9, &["rainfall"], "", ""),
                    // the transform back of the parts the row before made
                    row(IFFT, &["rainfall_re", "rainfall_im"], "", ""),
                ],
                function: 0,
            })),
            "CREATE loaves_csum = CSUM(loaves)\n  /loaves_diff = DIFF(loaves, 1)\n  \
             /loaves_sdiff = SDIFF(loaves, 1, 12)\n  /rain_l1 rain_l2 = LAG(rainfall, 1, 2)\n  \
             /rainfall_lead = LEAD(rainfall, 1)\n  /loaves_ma = MA(loaves, 5, 3)\n  \
             /loaves_rmed = RMED(loaves, 3)\n  /loaves_pma = PMA(loaves, 4)\n  \
             /loaves_t4253h = T4253H(loaves)\n  /rainfall_re rainfall_im = FFT(rainfall)\n  \
             /rainfall_re_ifft = IFFT(rainfall_re, rainfall_im).",
        ),
        (
            series(TimeSeries::Options(SeriesOptions {
                reset: true,
                newvar: 1,
                mxnewvars: s("30"),
                mxauto: s("20"),
                mxcross: s("9"),
                period: s("12"),
                cin: s("90"),
                include: true,
                acfse_ma: true,
                print: 1,
            })),
            "TSET DEFAULT NEWVAR=NONE MXNEWVARS=30 MXAUTO=20 MXCROSS=9 PERIOD=12 CIN=90 \
             MISSING=INCLUDE ACFSE=MA PRINT=BRIEF.",
        ),
        (
            series(TimeSeries::Dates(Dates {
                case: 2,
                starts: [s("2021"), s("3"), s("")],
                ..Default::default()
            })),
            "DATE YEAR 2021 MONTH 3.",
        ),
        (
            series(TimeSeries::Dates(Dates {
                case: 19,
                starts: [s(""), s("2"), s("")],
                period: s("6"),
            })),
            "DATE CYCLE OBS 2 6.",
        ),
    ];
    let program = bakery_months();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(dialog), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let mut result = session.run(&syntax, None).unwrap();
        // TSET makes no table: the procedures after it take its period
        if syntax.starts_with("TSET") {
            result = session
                .run("ACF loaves /SDIFF.\nSEASON loaves.", None)
                .unwrap();
        }
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
}

/// The forecasting and chart dialogs on the invented bakery months. Each case runs a program
/// first (dates, a model to APPLY, a variable to mark), then the dialog's syntax and a command
/// that shows what USE and PREDICT did; the syntax is as expected, parses and runs without a
/// warning.
#[test]
fn the_forecasting_dialogs_write_syntax_that_runs() {
    let series = |d: TimeSeries| Advanced::TimeSeries(Box::new(d));
    let smooth = |d: ExpSmoothing| series(TimeSeries::Smoothing(d));
    let arima = |d: ArimaModel| series(TimeSeries::Arima(d));
    let range = |d: TimeRange| series(TimeSeries::Range(d));
    let chart = |d: SeriesChart| series(TimeSeries::Chart(d));
    let value = |v: &str| Parameter {
        value: s(v),
        ..Default::default()
    };
    let grid = |from: &str, to: &str, step: &str| Parameter {
        grid: true,
        from: s(from),
        to: s(to),
        step: s(step),
        ..Default::default()
    };
    let dated = "DATE YEAR 2019 MONTH 1.";
    let cases: Vec<(&str, Advanced, &str, &str)> = vec![
        (
            "",
            smooth(ExpSmoothing {
                variables: v(&["loaves"]),
                ..Default::default()
            }),
            "EXSMOOTH VARIABLES=loaves.",
            "",
        ),
        (
            "EXSMOOTH VARIABLES=rainfall /MODEL=HOLT.",
            smooth(ExpSmoothing {
                variables: v(&["loaves"]),
                model: 5,
                period: s("12"),
                parameters: [
                    grid("", "", ""),
                    grid("0.1", "0.5", "0.2"),
                    value("0.3"),
                    value("0.5"),
                ],
                apply: true,
                ..Default::default()
            }),
            "EXSMOOTH VARIABLES=loaves\n  /MODEL=LM\n  /PERIOD=12\n  /ALPHA=GRID\n  \
             /GAMMA=GRID(0.1 0.5 0.2)\n  /DELTA=0.3\n  /APPLY.",
            "",
        ),
        (
            dated,
            smooth(ExpSmoothing {
                variables: v(&["loaves", "rainfall"]),
                model: 10,
                factors: s("-30 -24 -12 3 27 51 63 54 21 -6 -21 -36"),
                parameters: [
                    value("0.2"),
                    value(""),
                    value(""),
                    grid("0.2", "0.8", "0.3"),
                ],
                initial: true,
                start: s("190"),
                trend: s("2"),
                ..Default::default()
            }),
            "EXSMOOTH VARIABLES=loaves rainfall\n  /MODEL=DA\n  \
             /SEASFACT=(-30 -24 -12 3 27 51 63 54 21 -6 -21 -36)\n  /ALPHA=0.2\n  \
             /PHI=GRID(0.2 0.8 0.3)\n  /INITIAL=(190, 2).",
            "",
        ),
        (
            "",
            arima(ArimaModel {
                dependent: s("loaves"),
                ..Default::default()
            }),
            "ARIMA VARIABLES=loaves.",
            "",
        ),
        (
            "",
            arima(ArimaModel {
                dependent: s("loaves"),
                regressors: v(&["rainfall"]),
                orders: [s("1"), s("0"), s("0")],
                seasonal: [s("0"), s("1"), s("1")],
                period: s("12"),
                constant: 2,
                log: 2,
                lags: [s("1 2"), s(""), s(""), s("")],
                mxiter: s("20"),
                ssqpct: s("0.01"),
                pareps: s("0.0001"),
                cinpct: s("90"),
                forecast: 1,
                ..Default::default()
            }),
            "ARIMA VARIABLES=loaves WITH rainfall\n  /MODEL=(1,0,0) (0,1,1) 12 NOCONSTANT LN\n  \
             /P=(1, 2)\n  /MXITER=20\n  /SSQPCT=0.01\n  /PAREPS=0.0001\n  /CINPCT=90\n  \
             /FORECAST=CLS.",
            "",
        ),
        (
            "ARIMA VARIABLES=loaves /MODEL=(1,0,0).",
            arima(ArimaModel {
                dependent: s("rainfall"),
                mxlamb: s("100000"),
                forecast: 2,
                apply: 2,
                ..Default::default()
            }),
            "ARIMA VARIABLES=rainfall\n  /MXLAMB=100000\n  /FORECAST=AUTOINIT\n  \
             /APPLY INITIAL.",
            "",
        ),
        (
            "",
            range(TimeRange {
                start: s("13"),
                end: s("40"),
                ..TimeRange::new(false)
            }),
            "USE 13 THRU 40.",
            "ACF VARIABLES=loaves.",
        ),
        (
            dated,
            range(TimeRange {
                by_date: true,
                start_date: [s("2020"), s(""), s(""), s("")],
                end_date: [s("2022"), s("6"), s(""), s("")],
                permanent: true,
                ..TimeRange::new(false)
            }),
            "USE YEAR 2020 THRU YEAR 2022 MONTH 6\n  /PERMANENT.",
            "ACF VARIABLES=loaves.",
        ),
        (
            "USE FIRST THRU 40.",
            range(TimeRange {
                start: s("41"),
                end: s("54"),
                ..TimeRange::new(true)
            }),
            "PREDICT 41 THRU 54.",
            "EXSMOOTH VARIABLES=loaves /MODEL=HOLT.",
        ),
        (
            dated,
            range(TimeRange {
                by_date: true,
                end_date: [s("2023"), s("6"), s(""), s("")],
                ..TimeRange::new(true)
            }),
            "PREDICT THRU YEAR 2023 MONTH 6.",
            "ARIMA VARIABLES=loaves /MODEL=(1,0,0).",
        ),
        (
            "",
            chart(SeriesChart {
                variables: v(&["loaves"]),
                ..SeriesChart::new(false)
            }),
            "TSPLOT VARIABLES=loaves.",
            "",
        ),
        (
            "COMPUTE late = $CASENUM > 24.",
            chart(SeriesChart {
                variables: v(&["loaves"]),
                ln: true,
                diff: true,
                sdiff: true,
                period: s("12"),
                mark: s("late"),
                reference: 2,
                reference_value: s("0"),
                ..SeriesChart::new(false)
            }),
            "TSPLOT VARIABLES=loaves\n  /LN\n  /DIFF=1\n  /SDIFF=1\n  /PERIOD=12\n  \
             /MARK=late\n  /FORMAT=REFERENCE(0).",
            "",
        ),
        (
            "",
            chart(SeriesChart {
                variables: v(&["rainfall"]),
                ..SeriesChart::new(true)
            }),
            "CASEPLOT VARIABLES=rainfall.",
            "",
        ),
        (
            dated,
            chart(SeriesChart {
                variables: v(&["loaves", "rainfall"]),
                id: s("DATE_"),
                diff: true,
                join: 2,
                ..SeriesChart::new(true)
            }),
            "CASEPLOT VARIABLES=loaves rainfall\n  /DIFF=1\n  /ID=DATE_\n  /FORMAT=HILO.",
            "",
        ),
        (
            "CASEPLOT VARIABLES=rainfall /LN.",
            chart(SeriesChart {
                variables: v(&["rainfall"]),
                fill: true,
                reference: 1,
                apply: true,
                ..SeriesChart::new(true)
            }),
            "CASEPLOT VARIABLES=rainfall\n  /FORMAT=LEFT REFERENCE\n  /APPLY.",
            "",
        ),
        (
            "",
            series(TimeSeries::Spectra(Spectra {
                variables: v(&["loaves"]),
                ..Default::default()
            })),
            "SPECTRA VARIABLES=loaves\n  /CENTER.",
            "",
        ),
        (
            "",
            series(TimeSeries::Spectra(Spectra {
                variables: v(&["loaves", "rainfall"]),
                cross: true,
                window: 2,
                span: s("7"),
                plots: [false, true, false, false, false, false, false, true],
                axis: 1,
                saves: {
                    let mut saves = Spectra::default().saves;
                    saves[4] = (true, s("pg"));
                    saves[13] = (true, s("co"));
                    saves
                },
                ..Default::default()
            })),
            "SPECTRA VARIABLES=loaves rainfall\n  /CENTER\n  /CROSS\n  /WINDOW=PARZEN(7)\n  \
             /PLOT=S K BY FREQ\n  /SAVE=P(pg) K(co).",
            "",
        ),
        (
            "",
            series(TimeSeries::Areg(Areg {
                dependent: s("loaves"),
                independents: v(&["rainfall"]),
                ..Default::default()
            })),
            "AREG VARIABLES=loaves WITH rainfall.",
            "",
        ),
        (
            "",
            series(TimeSeries::Areg(Areg {
                dependent: s("loaves"),
                independents: v(&["rainfall"]),
                method: 1,
                constant: 1,
                rho: s("0.3"),
                mxiter: s("20"),
            })),
            "AREG VARIABLES=loaves WITH rainfall\n  /METHOD=CO\n  /CONSTANT\n  /RHO=0.3\n  \
             /MXITER=20.",
            "",
        ),
        (
            "",
            series(TimeSeries::Areg(Areg {
                dependent: s("loaves"),
                independents: v(&["rainfall"]),
                method: 2,
                ..Default::default()
            })),
            "AREG VARIABLES=loaves WITH rainfall\n  /METHOD=ML.",
            "",
        ),
        (
            "",
            series(TimeSeries::Tsmodel(Tsmodel {
                dependents: v(&["loaves"]),
                independents: v(&["rainfall"]),
                method: 2,
                ar: s("1"),
                transfers: vec![TsTransfer {
                    variable: s("rainfall"),
                    num: s("1"),
                    delay: s("1"),
                    ..Default::default()
                }],
                outliers: vec![TsOutlier {
                    location: s("20"),
                    kind: 0,
                }],
                ..Default::default()
            })),
            "TSMODEL /MODEL DEPENDENT=loaves INDEPENDENT=rainfall\n  /ARIMA AR=1 MA=0\n  \
             /TRANSFERFUNCTION VARIABLES=rainfall NUM=1 DELAY=1\n  \
             /OUTLIER LOCATION=[20] TYPE=ADDITIVE.",
            "",
        ),
        (
            "",
            series(TimeSeries::Tsmodel(Tsmodel {
                dependents: v(&["loaves"]),
                auto_outliers: true,
                ..Default::default()
            })),
            "TSMODEL /MODEL DEPENDENT=loaves\n  /AUTOOUTLIER DETECT=ON.",
            "",
        ),
    ];
    let program = bakery_months();
    for (before, dialog, expected, after) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&format!("{program}{before}"), None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(dialog), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&format!("{syntax}\n{after}"), None).unwrap();
        assert!(
            result
                .commands
                .iter()
                .any(|c| !c.tables.is_empty() || !c.charts.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
    tsmodel_then_tsapply(&program);
}

/// Create Models writes a model file and Apply Models reads it, with and without estimating
/// again; both forecast past the 48 of the series (the saved variables add cases, so Apply goes further) and save the predicted values.
fn tsmodel_then_tsapply(program: &str) {
    let folder = std::env::temp_dir().join(format!("statscraft_tsmodel_{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let file = folder
        .join("it's models.json")
        .to_string_lossy()
        .into_owned();
    let outputs = |last: &str, root: &str| TsOutputs {
        last: s(last),
        saves: [
            (true, s(root)),
            Default::default(),
            Default::default(),
            Default::default(),
        ],
        // every statistic and chart that compares models or shows each one's residuals
        summary: [true; 3],
        summary_plots: [true; 10],
        detail_plots: [true; 2],
        ..Default::default()
    };
    let create = |method, ar: &str, ma: &str| {
        series(TimeSeries::Tsmodel(Tsmodel {
            dependents: v(&["loaves"]),
            method,
            ar: s(ar),
            ma: s(ma),
            smoothing: 2,
            file: file.clone(),
            outputs: outputs("54", "fc"),
            ..Default::default()
        }))
    };
    fn series(d: TimeSeries) -> Advanced {
        Advanced::TimeSeries(Box::new(d))
    }
    let apply = |reestimate| {
        series(TimeSeries::Tsapply(Tsapply {
            file: file.clone(),
            reestimate,
            outputs: outputs("60", "ap"),
            ..Default::default()
        }))
    };
    for (first, expect) in [
        (
            create(0, "", ""),
            "PREDICT THRU 54.\nTSMODEL /MODELSUMMARY PRINT=[MODELFIT RESIDACF RESIDPACF] PLOT=[SRSQUARE \
             RSQUARE RMSE MAPE MAE MAXAPE MAXAE NORMBIC RESIDACF RESIDPACF]\n  \
             /MODELDETAILS PLOT=[RESIDACF RESIDPACF]\n  /SAVE PREDICTED(fc)",
        ),
        (create(1, "", ""), "/EXSMOOTH TYPE=HOLT."),
        (create(2, "1", "1"), "/ARIMA AR=1 MA=1."),
    ] {
        for reestimate in [false, true] {
            // forecasting adds cases, so each run starts from the series itself
            let mut session = statscraft_engine::session::Session::default();
            session.run(program, None).unwrap();
            let data = session.dataset().unwrap().clone();
            let create = Dialog::Advanced(first.clone()).syntax(&data).unwrap();
            assert!(create.contains(expect), "{create}");
            let apply = Dialog::Advanced(apply(reestimate)).syntax(&data).unwrap();
            for syntax in [&create, &apply] {
                let (_, errors) = statscraft_syntax::parse(syntax);
                assert!(errors.is_empty(), "{syntax}\n{errors:?}");
            }
            let result = session.run(&format!("{create}\n{apply}"), None).unwrap();
            let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
            assert!(warnings.is_empty(), "{create}\n{apply}\n{warnings:?}");
            let data = session.dataset().unwrap();
            assert!(data.variable("ap_loaves_Model_1").is_ok(), "{apply}");
            assert!(data.variable("fc_loaves_Model_1").is_ok(), "{create}");
        }
    }
    std::fs::remove_dir_all(&folder).unwrap();
}

/// Invented records: trainees of three schools passing a test after hours of practice (with
/// untrained groups at 0 hours), weekly sales and visits of a shop, and the repair cost of
/// machines by their running hours and age.
fn practice_shop_machines() -> String {
    let mut program = String::from(
        "DATA LIST LIST /school hours trainees passed week store sales visits machine_hours \
         age cost.\nBEGIN DATA\n",
    );
    let mut i = 0u32;
    for school in 1..=3u32 {
        for (k, hours) in [0u32, 1, 2, 4, 8, 16].into_iter().enumerate() {
            i += 1;
            let k = k as u32;
            let passed = 3 + school * 2 + k * 5 + (k * school) % 3;
            let week = i;
            let sales = 20 + 3 * week + (week * 7) % 5;
            let visits = (50.0 * 1.08f64.powi(week as i32)).round() + f64::from((week * 3) % 4);
            let machine_hours = 10 + (i * 17) % 90;
            let age = 1 + i % 8;
            let noise = f64::from((i * 13) % 7) - 3.0;
            let cost = 5.0
                + 0.8 * f64::from(machine_hours)
                + 2.0 * f64::from(age)
                + noise * f64::from(machine_hours) / 30.0;
            program.push_str(&format!(
                "{school} {hours} 40 {passed} {week} {} {sales} {visits} {machine_hours} {age} \
                 {cost:.2}\n",
                i % 4 + 1
            ));
        }
    }
    program.push_str("END DATA.\n");
    program
}

/// The Curve Estimation, Probit and Weight Estimation dialogs on invented records: the syntax is
/// as expected, parses and runs, with no warning but the one a case expects.
#[test]
fn the_regression_more_dialogs_write_syntax_that_runs() {
    use crate::regression_more_dialog::{
        CURVE_LOGISTIC, CurveEstimation, PROBIT_LOGS, Probit, WeightEstimation,
    };
    let probit = Probit {
        response: s("passed"),
        total: s("trainees"),
        covariates: v(&["hours"]),
        ..Default::default()
    };
    let curve = CurveEstimation {
        dependents: v(&["sales"]),
        independent: s("week"),
        ..Default::default()
    };
    let wls = WeightEstimation {
        dependent: s("cost"),
        independents: v(&["machine_hours", "age"]),
        ..Default::default()
    };
    let mut models = [false; 11];
    for i in [0, 3, 8, CURVE_LOGISTIC] {
        models[i] = true;
    }
    let cases: Vec<(Advanced, &str, Option<&str>)> = vec![
        (
            Advanced::Probit(Box::new(probit.clone())),
            "PROBIT passed OF trainees WITH hours.",
            None,
        ),
        (
            Advanced::Probit(Box::new(Probit {
                factor: table_variable("school", "1", "3", &[]),
                model: 2,
                log: PROBIT_LOGS.len() - 1,
                natres: 2,
                natres_value: s("0.05"),
                converge: s("0.000001"),
                iterations: s("100"),
                heterogeneity: s("0.05"),
                print: [true; 4],
                include: true,
                ..probit.clone()
            })),
            "PROBIT passed OF trainees WITH hours BY school(1,3)\n  /MODEL=BOTH\n  /LOG=NONE\n  \
             /CRITERIA=CONVERGE(0.000001) ITERATE(100) P(0.05)\n  /NATRES=0.05\n  \
             /PRINT=FREQ CI RMP PARALL\n  /MISSING=INCLUDE.",
            None,
        ),
        (
            Advanced::Curve(curve.clone()),
            "CURVEFIT VARIABLES=sales WITH week.",
            None,
        ),
        (
            Advanced::Curve(CurveEstimation {
                dependents: v(&["sales", "visits"]),
                id: s("store"),
                models,
                upper_bound: s("500"),
                constant: false,
                anova: true,
                plot: false,
                save: [true; 3],
                ci_level: s("90"),
                ..curve.clone()
            }),
            "CURVEFIT VARIABLES=sales visits WITH week\n  /MODEL=LINEAR QUADRATIC GROWTH LGSTIC\n  \
             /UPPERBOUND=500\n  /NOCONSTANT\n  /PRINT=ANOVA\n  /PLOT=NONE\n  /CIN=90\n  \
             /ID=store\n  /SAVE=PRED RESID CIN.",
            None,
        ),
        (
            Advanced::Curve(CurveEstimation {
                dependents: v(&["visits"]),
                sequence: true,
                ..curve.clone()
            }),
            "CURVEFIT VARIABLES=visits.",
            None,
        ),
        (
            Advanced::Wls(wls.clone()),
            "WLS VARIABLES=cost WITH machine_hours age.",
            Some("neither SOURCE"),
        ),
        (
            Advanced::Wls(WeightEstimation {
                source: s("machine_hours"),
                power_list: true,
                powers: s("0, 0.5 1 1.5 2"),
                constant: false,
                print_all: true,
                save_weight: true,
                ..wls.clone()
            }),
            "WLS VARIABLES=cost WITH machine_hours age\n  /SOURCE=machine_hours\n  \
             /POWER=0 0.5 1 1.5 2\n  /NOCONSTANT\n  /PRINT=ALL\n  /SAVE=WEIGHT.",
            None,
        ),
        (
            Advanced::Wls(WeightEstimation {
                from: s("-1"),
                to: s("2"),
                by: s("0.5"),
                ..wls.clone()
            }),
            "WLS VARIABLES=cost WITH machine_hours age\n  /POWER=-1 TO 2 BY 0.5.",
            None,
        ),
    ];
    let program = practice_shop_machines();
    for (dialog, expected, warning) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(dialog), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        match warning {
            None => assert!(warnings.is_empty(), "{syntax}\n{warnings:?}"),
            Some(w) => assert!(
                warnings.iter().all(|x| x.contains(w)) && !warnings.is_empty(),
                "{syntax}\n{warnings:?}"
            ),
        }
    }
}

/// The Power Analysis dialogs on invented planning values, with no data open: for each command
/// the power of given sizes and the sizes for given powers, then charts and changed options. The
/// syntax is as expected, parses and runs without a warning.
#[test]
fn the_power_dialogs_write_syntax_that_runs() {
    use crate::power_dialog::{PowerAnalysis, PowerKind::*};
    let new = PowerAnalysis::new;
    let cases: Vec<(PowerAnalysis, &str)> = vec![
        (
            PowerAnalysis {
                n1: s("25"),
                mean1: s("3.5"),
                null: s("2"),
                sd1: s("4"),
                ..new(MeansOneSample)
            },
            "N=25 SD=4 MEAN=3.5 NULL=2",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.75, 0.85"),
                mean1: s("3.5"),
                null: s("2"),
                sd1: s("4"),
                ..new(MeansOneSample)
            },
            "POWER=0.75 0.85 SD=4 MEAN=3.5 NULL=2",
        ),
        (
            PowerAnalysis {
                n1: s("30"),
                n2: s("45"),
                sd1: s("6"),
                mean1: s("2.5"),
                ..new(MeansIndependent)
            },
            "N=30 45 SD=6 MEAN=2.5",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.9"),
                ratio: s("1.5"),
                two_sds: true,
                sd1: s("5"),
                sd2: s("7"),
                two_means: true,
                mean1: s("18"),
                mean2: s("14.5"),
                ..new(MeansIndependent)
            },
            "POWER=0.9 NRATIO=1.5 SD=5 7 MEAN=18 14.5",
        ),
        (
            PowerAnalysis {
                n1: s("22"),
                sd1: s("3.2"),
                mean1: s("1.1"),
                ..new(MeansRelated)
            },
            "NPAIRS=22 SD=3.2 MEAN=1.1",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.8"),
                two_sds: true,
                sd1: s("4"),
                sd2: s("4.5"),
                rho: s("0.6"),
                two_means: true,
                mean1: s("12"),
                mean2: s("10.8"),
                ..new(MeansRelated)
            },
            "POWER=0.8 SD=4 4.5 RHO=0.6 MEAN=12 10.8",
        ),
        (
            PowerAnalysis {
                n1: s("60"),
                alternative: s("0.65"),
                ..new(ProportionsOneSample)
            },
            "N=60 ALTERNATIVE=0.65",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.85"),
                null: s("0.2"),
                alternative: s("0.32"),
                binomial: true,
                timer: s("2"),
                ..new(ProportionsOneSample)
            },
            "POWER=0.85 NULL=0.2 ALTERNATIVE=0.32 ESTIMATE=BINOMIAL TIMER=2",
        ),
        (
            PowerAnalysis {
                n1: s("75"),
                n2: s("80"),
                p1: s("0.15"),
                p2: s("0.3"),
                ..new(ProportionsIndependent)
            },
            "N=75 80 PROPORTIONS=0.15 0.3",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.8 0.9"),
                ratio: s("2"),
                p1: s("0.4"),
                p2: s("0.25"),
                method: 2,
                ..new(ProportionsIndependent)
            },
            "POWER=0.8 0.9 NRATIO=2 PROPORTIONS=0.4 0.25 METHOD=LRT",
        ),
        (
            PowerAnalysis {
                n1: s("120"),
                counts: true,
                p1: s("14"),
                p2: s("6"),
                ..new(ProportionsRelated)
            },
            "NPAIRS=120 VALUES=COUNTS(14 6)",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.8"),
                marginal: true,
                p1: s("0.55"),
                p2: s("0.4"),
                correlation: s("0.2"),
                ..new(ProportionsRelated)
            },
            "POWER=0.8 VALUES=PROPORTIONS(0.55 0.4) MARGINAL=TRUE CORRELATION=0.2",
        ),
        // charts with ranges and a surface, a one-sided test at another level
        (
            PowerAnalysis {
                n1: s("40"),
                n2: s("40"),
                directional: true,
                significance: s("0.01"),
                sd1: s("2"),
                mean1: s("1"),
                charts: [true, true, true, false, false, false, false],
                from: [s("0.5"), s("-1"), s(""), s(""), s(""), s(""), s("")],
                to: [s("3"), s("2"), s(""), s(""), s(""), s(""), s("")],
                ..new(MeansIndependent)
            },
            "TEST=DIRECTIONAL SIGNIFICANCE=0.01 N=40 40 SD=2 MEAN=1\n  \
             /PLOT NRATIO(0.5 3) ES(-1 2) NRATIO_XAXIS ES_YAXIS",
        ),
        // the other test options, and charts of risk ratio and the two group sizes
        (
            PowerAnalysis {
                n1: s("50"),
                n2: s("60"),
                p1: s("0.3"),
                p2: s("0.45"),
                continuity: true,
                pooled: false,
                charts: [false, false, true, false, true, false, false],
                from: [s(""), s(""), s("0.3"), s(""), s(""), s(""), s("")],
                to: [s(""), s(""), s("1.2"), s(""), s(""), s(""), s("")],
                ..new(ProportionsIndependent)
            },
            "N=50 60 PROPORTIONS=0.3 0.45 CONTINUITY=TRUE POOLED=FALSE\n  \
             /PLOT RISK_RATIO(0.3 1.2) N1_XAXIS N2_YAXIS",
        ),
        // success rates with the charts that need them
        (
            PowerAnalysis {
                n1: s("80"),
                directional: true,
                marginal: true,
                p1: s("0.5"),
                p2: s("0.35"),
                correlation: s("0.25"),
                charts: [true, false, false, false, true, true, false],
                from: [s("20"), s(""), s(""), s(""), s(""), s(""), s("")],
                to: [s("200"), s(""), s(""), s(""), s(""), s(""), s("")],
                ..new(ProportionsRelated)
            },
            "TEST=DIRECTIONAL NPAIRS=80 VALUES=PROPORTIONS(0.5 0.35) MARGINAL=TRUE \
             CORRELATION=0.25\n  /PLOT NPAIRS(20 200) CORRELATION DISCORDANT",
        ),
        (
            PowerAnalysis {
                n1: s("14 18 14"),
                mean1: s("31 34.5 38"),
                sd1: s("7.5"),
                ..new(OnewayAnova)
            },
            "GROUP_SIZES=14 18 14 POOLED_SD=7.5 GROUP_MEANS=31 34.5 38",
        ),
        // weighted groups, a one-sided contrast and pairs compared with another adjustment
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.8"),
                mean1: s("12 15 19 21"),
                sd1: s("6"),
                weights: s("1 1 2 2"),
                coefficients: s("-1 -1 1 1"),
                directional: true,
                pairwise: true,
                adjust: 1,
                ..new(OnewayAnova)
            },
            "POWER=0.8 POOLED_SD=6 GROUP_MEANS=12 15 19 21 GROUP_WEIGHTS=1 1 2 2\n  \
             /CONTRAST COEFFICIENTS=-1 -1 1 1 TEST=DIRECTIONAL PAIRWISE=TRUE ADJUST=SIDAK",
        ),
        (
            PowerAnalysis {
                n1: s("10 10 10"),
                mean1: s("5 6.2 7"),
                sd1: s("1.6"),
                significance: s("0.1"),
                charts: [true, true, true, false, false, false, false],
                from: [s("9"), s("0.8"), s(""), s(""), s(""), s(""), s("")],
                to: [s("90"), s("3"), s(""), s(""), s(""), s(""), s("")],
                ..new(OnewayAnova)
            },
            "SIGNIFICANCE=0.1 GROUP_SIZES=10 10 10 POOLED_SD=1.6 GROUP_MEANS=5 6.2 7\n  \
             /PLOT TOTAL_N(9 90) POOLED_SD(0.8 3) TOTAL_N_XAXIS POOLED_SD_YAXIS",
        ),
        (
            PowerAnalysis {
                n1: s("55"),
                alternative: s("0.32"),
                ..new(PearsonOneSample)
            },
            "N=55 ALTERNATIVE=0.32",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.8 0.9"),
                null: s("0.1"),
                alternative: s("0.45"),
                adjust_bias: false,
                ..new(PearsonOneSample)
            },
            "POWER=0.8 0.9 NULL=0.1 ALTERNATIVE=0.45 ADJUST_BIAS=FALSE",
        ),
        (
            PowerAnalysis {
                n1: s("70"),
                directional: true,
                significance: s("0.025"),
                null: s("0.2"),
                alternative: s("0.42"),
                charts: [true, true, false, true, true, true, false],
                from: [s("20"), s(""), s(""), s(""), s(""), s(""), s("")],
                to: [s("150"), s(""), s(""), s(""), s(""), s(""), s("")],
                ..new(PearsonOneSample)
            },
            "TEST=DIRECTIONAL SIGNIFICANCE=0.025 N=70 NULL=0.2 ALTERNATIVE=0.42\n  \
             /PLOT N(20 150) NULL DIFF N_XAXIS DIFF_YAXIS NULL_XAXIS ALTERNATIVE_YAXIS",
        ),
        (
            PowerAnalysis {
                n1: s("40"),
                alternative: s("-0.3"),
                ..new(SpearmanOneSample)
            },
            "N=40 ALTERNATIVE=-0.3",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.85"),
                alternative: s("0.5"),
                variance: 1,
                ..new(SpearmanOneSample)
            },
            "POWER=0.85 ALTERNATIVE=0.5 VARIANCE=FHP",
        ),
        (
            PowerAnalysis {
                n1: s("35"),
                null: s("0.1"),
                alternative: s("0.55"),
                variance: 2,
                charts: [true, false, true, false, false, false, false],
                from: [s("10"), s(""), s(""), s(""), s(""), s(""), s("")],
                to: [s("80"), s(""), s(""), s(""), s(""), s(""), s("")],
                ..new(SpearmanOneSample)
            },
            "N=35 NULL=0.1 ALTERNATIVE=0.55 VARIANCE=CC\n  /PLOT N(10 80) ALTERNATIVE",
        ),
        (
            PowerAnalysis {
                n1: s("90"),
                partialout: s("3"),
                alternative: s("0.27"),
                ..new(PartialCorr)
            },
            "N=90 PARTIALOUT=3 ALTERNATIVE=0.27",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.8"),
                partialout: s("2"),
                null: s("-0.1"),
                alternative: s("0.2"),
                ..new(PartialCorr)
            },
            "POWER=0.8 PARTIALOUT=2 NULL=-0.1 ALTERNATIVE=0.2",
        ),
        (
            PowerAnalysis {
                n1: s("60"),
                directional: true,
                partialout: s("5"),
                alternative: s("0.3"),
                charts: [true, true, false, false, true, false, false],
                ..new(PartialCorr)
            },
            "TEST=DIRECTIONAL N=60 PARTIALOUT=5 ALTERNATIVE=0.3\n  /PLOT N PARTIALOUT DIFF",
        ),
        (
            PowerAnalysis {
                n1: s("80"),
                total_predictors: s("5"),
                test_predictors: s("2"),
                partial: s("0.3"),
                ..new(UnivariateLinear)
            },
            "N=80 TOTAL_PREDICTORS=5 TEST_PREDICTORS=2 PARTIAL_CORR=0.3",
        ),
        (
            PowerAnalysis {
                find_size: true,
                powers: s("0.9"),
                random: true,
                total_predictors: s("6"),
                test_predictors: s("2"),
                models: true,
                p1: s("0.4"),
                p2: s("0.3"),
                ..new(UnivariateLinear)
            },
            "POWER=0.9 MODEL=RANDOM TOTAL_PREDICTORS=6 TEST_PREDICTORS=2 FULL_MODEL=0.4 \
             NESTED_MODEL=0.3",
        ),
        (
            PowerAnalysis {
                n1: s("60"),
                significance: s("0.01"),
                total_predictors: s("4"),
                test_predictors: s("1"),
                partial: s("0.35"),
                intercept: false,
                charts: [true, true, true, false, false, false, false],
                from: [s("10"), s(""), s(""), s(""), s(""), s(""), s("")],
                to: [s("120"), s(""), s(""), s(""), s(""), s(""), s("")],
                ..new(UnivariateLinear)
            },
            "SIGNIFICANCE=0.01 N=60 TOTAL_PREDICTORS=4 TEST_PREDICTORS=1 PARTIAL_CORR=0.35 \
             INTERCEPT=FALSE\n  /PLOT N(10 120) PARTIAL_CORR N_XAXIS PARTIAL_CORR_YAXIS",
        ),
    ];
    let data = statscraft_data::Dataset::new(0);
    for (dialog, expected) in cases {
        let name = dialog.kind.name();
        let syntax = drawn(Dialog::Advanced(Advanced::Power(Box::new(dialog))), &data).unwrap();
        assert_eq!(syntax, format!("{name}\n  /PARAMETERS {expected}."));
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let mut session = statscraft_engine::session::Session::default();
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
}

/// Invented records of thirty students in a tutoring program: their club, weekly minutes, test
/// scores before and after, two quizzes, a pass flag, help desk visits and a team color, with two
/// answers missing.
fn tutoring_students() -> String {
    let mut program = String::from(
        "DATA LIST LIST /student club minutes before after quiz1 quiz2 passed visits (9F8.2) \
         team (A4).\nBEGIN DATA\n",
    );
    for i in 1..=30u32 {
        let club = i % 2 + 1;
        let minutes = match i {
            9 => ".".to_string(),
            _ => (30 + (i * 7) % 25 + club * 3).to_string(),
        };
        let before = 50 + (i * 11) % 30;
        let after = match i {
            17 => ".".to_string(),
            _ => (before + (i * 5) % 9 - 2).to_string(),
        };
        let quiz1 = 5 + (i * 3) % 7;
        let quiz2 = quiz1 + i % 4;
        let passed = u32::from((i * 7) % 10 >= 4);
        let visits = (i * 5) % 6;
        let team = if (i * 13) % 5 < 3 { "red" } else { "blue" };
        program.push_str(&format!(
            "{i} {club} {minutes} {before} {after} {quiz1} {quiz2} {passed} {visits} {team}\n"
        ));
    }
    program.push_str("END DATA.\n");
    program
}

/// The Bayesian Statistics dialogs on invented records: for each command the least it needs, then
/// a changed analysis, priors and Bayes factor settings. The syntax is as expected, parses and
/// runs without a warning.
#[test]
fn the_bayes_dialogs_write_syntax_that_runs() {
    use crate::bayes_dialog::{BayesKind::*, Bayesian};
    let with = |kind, names: &[&str]| Bayesian {
        variables: v(names),
        ..Bayesian::new(kind)
    };
    let pair = (s("before"), s("after"));
    let cases: Vec<(Bayesian, &str)> = vec![
        (
            with(Normal, &["minutes"]),
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=minutes.",
        ),
        (
            Bayesian {
                analysis: 2,
                null: s("40"),
                variance_prior: 2,
                prior_a: s("3"),
                prior_b: s("200"),
                normal_mean: true,
                prior_mean: s("38"),
                kappa: s("4"),
                ci_level: s("90"),
                tolerance: s("0.00001"),
                monte_carlo: true,
                seed: s("7"),
                mc_samples: s("20000"),
                listwise: true,
                ..with(Normal, &["minutes", "before"])
            },
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=minutes before \
             ANALYSIS=BOTH\n  /DATA VARIABLES=minutes before NULLVALUE=40\n  \
             /PRIOR VARDIST=INVGAMMA(3 200) MEANDIST=NORMAL(38) KAPPA=4\n  \
             /CRITERIA CILEVEL=90 TOL=0.00001 METHOD=MCA SEED=7 MCSAMPLES=20000\n  \
             /MISSING SCOPE=LISTWISE.",
        ),
        (
            with(Binomial, &["passed"]),
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=passed\n  \
             /PRIOR TYPE=CONJUGATE(1 1).",
        ),
        (
            Bayesian {
                analysis: 2,
                success: 4,
                success_value: s("red"),
                null_prior: s("3 3"),
                alter_prior: s("1, 2"),
                conjugate: s("0.5 0.5"),
                ci_level: s("99"),
                ..with(Binomial, &["team"])
            },
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=BINOMIAL VARIABLES=team ANALYSIS=BOTH\n  \
             /DATA VARIABLES=team SUCCESS=LEVEL('red') NULLPRIORPARAM=3 3 ALTERPRIORPARAM=1 2\n  \
             /PRIOR TYPE=CONJUGATE(0.5 0.5)\n  /CRITERIA CILEVEL=99.",
        ),
        (
            with(Poisson, &["visits"]),
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=POISSON VARIABLES=visits\n  \
             /PRIOR TYPE=CONJUGATE(1 0).",
        ),
        (
            Bayesian {
                analysis: 1,
                null_prior: s("1 1"),
                alter_prior: s("0.5 2"),
                listwise: true,
                ..with(Poisson, &["visits", "quiz1"])
            },
            "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=POISSON VARIABLES=visits quiz1 \
             ANALYSIS=BAYESFACTOR\n  /DATA VARIABLES=visits quiz1 NULLPRIORPARAM=1 1 \
             ALTERPRIORPARAM=0.5 2\n  /MISSING SCOPE=LISTWISE.",
        ),
        (
            Bayesian {
                pairs: vec![pair.clone()],
                ..Bayesian::new(Related)
            },
            "BAYES RELATED /PAIR VARIABLES=before after.",
        ),
        (
            Bayesian {
                pairs: vec![pair.clone(), (s("quiz1"), s("quiz2"))],
                analysis: 2,
                known_variance: s("30"),
                null: s("-1"),
                g: s("2"),
                normal_mean: true,
                prior_mean: s("1"),
                prior_mean_variance: s("4"),
                ci_level: s("92.5"),
                max_iterations: s("500"),
                ..Bayesian::new(Related)
            },
            "BAYES RELATED /PAIR VARIABLES=before after VARIANCE=30 NULLVALUE=-1 ALTERGVALUE=2\n  \
             /PAIR VARIABLES=quiz1 quiz2 VARIANCE=30 NULLVALUE=-1 ALTERGVALUE=2\n  \
             /INFERENCE ANALYSIS=BOTH\n  /PRIOR MEANDIST=NORMAL(1 4)\n  \
             /CRITERIA CILEVEL=92.5 MAXITER=500.",
        ),
        (
            Bayesian {
                group: s("club"),
                level1: s("1"),
                level2: s("2"),
                ..with(Independent, &["minutes"])
            },
            "BAYES INDEPENDENT /INFERENCE VARIABLES=minutes GROUP=club SELECT=LEVEL(1 2).",
        ),
        (
            Bayesian {
                group: s("team"),
                level1: s("red"),
                level2: s("blue"),
                analysis: 2,
                equal_variances: true,
                variance_prior: 2,
                prior_a: s("2"),
                prior_b: s("50"),
                factor: 1,
                gonen_mean: s("0.2"),
                gonen_variance: s("0.5"),
                listwise: true,
                ..with(Independent, &["minutes", "after"])
            },
            "BAYES INDEPENDENT /INFERENCE VARIABLES=minutes after GROUP=team \
             SELECT=LEVEL('red' 'blue') ANALYSIS=BOTH\n  /ESTBF COMPUTATION=GONEN(0.2 0.5)\n  \
             /PRIOR EQUALDATAVAR=TRUE VARDIST=INVCHISQ(2 50)\n  /MISSING SCOPE=LISTWISE.",
        ),
        (
            Bayesian {
                group: s("club"),
                select: 2,
                cutpoint: s("2"),
                analysis: 2,
                known_variances: true,
                variance1: s("60"),
                variance2: s("80"),
                normal_mean: true,
                group_means: [s("40"), s("45"), s("100"), s("100")],
                factor: 2,
                hyper: s("-0.6"),
                ..with(Independent, &["minutes"])
            },
            "BAYES INDEPENDENT /INFERENCE VARIABLES=minutes GROUP=club SELECT=CUTPOINT(2) \
             ANALYSIS=BOTH DATAVAR=60 80\n  /ESTBF COMPUTATION=HYPER(-0.6)\n  \
             /PRIOR MEANDIST=NORMAL(40 45 100 100).",
        ),
        (
            with(Correlation, &["minutes", "before"]),
            "BAYES CORRELATION /INFERENCE VARIABLES=minutes before.",
        ),
        (
            Bayesian {
                analysis: 2,
                cvalue: s("2"),
                fraction: true,
                fraction_value: s("0.4"),
                fraction_null: s("0.1"),
                tolerance: s("0.00001"),
                listwise: true,
                ..with(Correlation, &["minutes", "before", "after"])
            },
            "BAYES CORRELATION /INFERENCE VARIABLES=minutes before after ANALYSIS=BOTH CVALUE=2\n  \
             /ESTBF TYPE=FRACTION(0.4) NULLVALUE=0.1\n  /CRITERIA TOL=0.00001\n  \
             /MISSING SCOPE=LISTWISE.",
        ),
    ];
    let program = tutoring_students();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(Advanced::Bayes(Box::new(dialog))), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
}

/// The Bayesian one-way ANOVA, linear regression, repeated measures and log-linear dialogs on the
/// tutoring records: the least each needs, then changed analyses, priors, plots, a prediction with
/// saved values, two F tests and each model of a table. The syntax is as expected, parses and runs
/// without a warning.
#[test]
fn the_bayes_model_dialogs_write_syntax_that_runs() {
    use crate::bayes_dialog::{BayesKind::*, Bayesian, FTestRow};
    let anova = Bayesian {
        dependent: s("minutes"),
        group: s("club"),
        ..Bayesian::new(Anova)
    };
    let regression = Bayesian {
        dependent: s("after"),
        covariates: v(&["before"]),
        ..Bayesian::new(Regression)
    };
    let table = Bayesian {
        row: s("club"),
        column: s("team"),
        ..Bayesian::new(Loglinear)
    };
    let cases: Vec<(Bayesian, &str)> = vec![
        (anova.clone(), "BAYES ANOVA minutes BY club."),
        (
            Bayesian {
                dependent: s("after"),
                group: s("team"),
                weight: s("quiz1"),
                analysis: 2,
                factor: 3,
                rouder: s("0.5"),
                variance_prior: 1,
                prior_a: s("2"),
                prior_b: s("40"),
                prior_mean: s("60 65"),
                vmatrix: s("2; 0.5 3"),
                plot_values: s("red"),
                plot_error: true,
                ci_level: s("90"),
                ..anova.clone()
            },
            "BAYES ANOVA after BY team\n  /DESIGN REGWGT=quiz1\n  /INFERENCE ANALYSIS=BOTH\n  \
             /ESTBF COMPUTATION=ROUDER(0.5)\n  /PRIOR TYPE=CONJUGATE SHAPEPARAM=2 SCALEPARAM=40 \
             MEANVECTOR=60 65 VMATRIX=2; 0.5 3;\n  /PLOT MEANS='red' ERRORVAR=TRUE\n  \
             /CRITERIA CILEVEL=90.",
        ),
        (regression.clone(), "BAYES REGRESSION after WITH before."),
        (
            Bayesian {
                factors: v(&["team"]),
                covariates: v(&["before", "minutes"]),
                analysis: 2,
                factor: 2,
                hyper: s("4"),
                compare_full: true,
                predict: [("team", "blue"), ("before", "62"), ("minutes", "45")]
                    .into_iter()
                    .map(|(a, b)| (s(a), s(b)))
                    .collect(),
                save: [true, false, false, true, true],
                save_names: [s(""), s(""), s(""), s("low_after"), s("")],
                plotted: v(&["team", "before"]),
                plot_intercept: true,
                plot_error: true,
                plot_prediction: true,
                ftests: vec![
                    FTestRow {
                        label: s("team effect"),
                        variables: v(&["team"]),
                        ..FTestRow::default()
                    },
                    FTestRow {
                        variables: v(&["before", "minutes"]),
                        intercept: true,
                        values: s("5 1 0"),
                        ..FTestRow::default()
                    },
                ],
                max_iterations: s("800"),
                ..regression.clone()
            },
            "BAYES REGRESSION after BY team WITH before minutes\n  /INFERENCE ANALYSIS=BOTH\n  \
             /ESTBF COMPUTATION=HYPER(4) COMPARE=FULL\n  \
             /PREDICT REGRESSORS='blue' 62 45\n  /SAVE MEANPRED CIMEANPREDL(low_after) CIMEANPREDU\n  \
             /PLOT COVARIATES=before FACTORS=team INTERCEPT=TRUE ERRORVAR=TRUE BAYESPRED=TRUE\n  \
             /FTEST LABEL='team effect' VARIABLES=team\n  \
             /FTEST VARIABLES=before minutes INTERCEPT=TRUE TESTVALUES=5 1 0\n  \
             /CRITERIA MAXITER=800.",
        ),
        (
            Bayesian {
                variables: v(&["quiz1", "quiz2"]),
                ..Bayesian::new(Repeated)
            },
            "BAYES REPEATED quiz1 quiz2.",
        ),
        (
            Bayesian {
                variables: v(&["quiz1", "quiz2", "before"]),
                weight: s("minutes"),
                analysis: 2,
                factor: 1,
                seed: s("4242"),
                mc_samples: s("5000"),
                plotted: v(&["quiz2", "quiz1"]),
                ci_level: s("99"),
                ..Bayesian::new(Repeated)
            },
            "BAYES REPEATED quiz1 quiz2 before\n  /DESIGN REGWGT=minutes\n  \
             /INFERENCE ANALYSIS=BOTH\n  /ESTBF COMPUTATION=ROUDER\n  \
             /PLOT MEANS=quiz2 quiz1\n  /CRITERIA CILEVEL=99 SEED=4242 MCSAMPLES=5000.",
        ),
        (table.clone(), "BAYES LOGLINEAR /TABLE club BY team."),
        (
            Bayesian {
                analysis: 2,
                count_model: 1,
                shapes: s("1 2; 2 1;"),
                scale: s("0.5"),
                cells: [true, true, true, false, false],
                lrt: true,
                ci_level: s("90"),
                seed: s("99"),
                mc_samples: s("2000"),
                descending: true,
                ..table.clone()
            },
            "BAYES LOGLINEAR /TABLE club BY team\n  /INFERENCE ANALYSIS=BOTH\n  \
             /ESTBF MODEL=POISSON SHAPEPARAMS=1 2; 2 1 SCALEPARAM=0.5\n  \
             /PRINT CELLS=OBSERVED EXPECTED ROW STATISTICS=CHISQ LRT\n  \
             /CRITERIA CILEVEL=90 SEED=99 POSTSAMPLES=2000 ORDER=DESCENDING.",
        ),
        (
            Bayesian {
                row: s("passed"),
                column: s("visits"),
                analysis: 1,
                margin: 1,
                factor: 1,
                suppress_table: true,
                chisq: false,
                lrt: true,
                tolerance: s("0.00001"),
                ..table.clone()
            },
            "BAYES LOGLINEAR /TABLE passed BY visits\n  /INFERENCE ANALYSIS=BAYESFACTOR\n  \
             /ESTBF FIX_MARGIN=ROW PRIOR_TYPE=MIXTURE_DIRICHLET\n  \
             /PRINT SUPPRESS_TABLE=TRUE STATISTICS=LRT\n  /CRITERIA TOL=0.00001.",
        ),
        (
            Bayesian {
                row: s("team"),
                column: s("club"),
                analysis: 1,
                count_model: 2,
                margin: 2,
                lambdas: s("2"),
                ..table.clone()
            },
            "BAYES LOGLINEAR /TABLE team BY club\n  /INFERENCE ANALYSIS=BAYESFACTOR\n  \
             /ESTBF MODEL=NONPARAMETRIC FIX_MARGIN=COLUMN LAMBDAS=2.",
        ),
    ];
    let program = tutoring_students();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(Advanced::Bayes(Box::new(dialog))), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
}

/// Invented records of forty members of a community garden: their site, main crop, watering
/// habit, weekly hours, shift and member number, with a few answers missing.
fn garden_members() -> String {
    let mut program = String::from(
        "DATA LIST LIST /member site crop water hours (5F4.1) shift (A5).\nBEGIN DATA\n",
    );
    for i in 1..=40u32 {
        let site = i % 3 + 1;
        let crop = match i % 11 {
            0 => ".".to_string(),
            k => ((k + site) % 4 + 1).to_string(),
        };
        let water = match i % 13 {
            5 => ".".to_string(),
            k => ((k * 2 + site) % 3 + 1).to_string(),
        };
        let hours = f64::from((i * 7) % 23) / 2.0 + 1.0;
        let shift = ["dawn", "noon", "dusk"][((i / 2 + site) % 3) as usize];
        program.push_str(&format!("{i} {site} {crop} {water} {hours} {shift}\n"));
    }
    program.push_str("END DATA.\n");
    program
}

/// The Multiple Correspondence dialog on invented garden members: the syntax is as expected,
/// parses and runs without a warning.
#[test]
fn the_multiple_correspondence_dialog_writes_syntax_that_runs() {
    let out = folder("walk-mca");
    let recoded = out.join("recoded.sav").to_string_lossy().into_owned();
    let base = Mca {
        variables: v(&["site", "crop", "water", "hours", "shift"]),
        ..Default::default()
    };
    let setting = |name: &str, change: &dyn Fn(&mut McaVariable)| {
        let mut s = McaVariable::new(name);
        change(&mut s);
        s
    };
    let cases: Vec<(Mca, String)> = vec![
        (
            base.clone(),
            "MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift."
                .into(),
        ),
        // weights, recoding and missing values per variable
        (
            Mca {
                settings: vec![
                    setting("site", &|x| x.missing = 1),
                    setting("crop", &|x| {
                        x.weight = s("2");
                        x.missing = 2;
                    }),
                    setting("water", &|x| {
                        x.weight = s("2");
                        x.missing = 3;
                    }),
                    setting("hours", &|x| {
                        x.discretize = 1;
                        x.groups = s("4");
                        x.uniform = true;
                    }),
                    setting("shift", &|x| x.discretize = 3),
                ],
                ..base.clone()
            },
            "MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site(WEIGHT=1) crop water(WEIGHT=2) hours shift\n  \
             /DISCRETIZATION=hours(GROUPING NCAT=4 DISTR=UNIFORM) shift(RANKING)\n  \
             /MISSING=site(PASSIVE EXTRACAT) crop(ACTIVE MODEIMPU) water(ACTIVE EXTRACAT)."
                .into(),
        ),
        (
            Mca {
                settings: vec![
                    setting("crop", &|x| x.missing = 4),
                    setting("water", &|x| x.missing = 5),
                    setting("hours", &|x| {
                        x.discretize = 2;
                        x.width = s("2.5");
                    }),
                    setting("shift", &|x| x.discretize = 4),
                ],
                ..base.clone()
            },
            "MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  \
             /DISCRETIZATION=hours(GROUPING EQINTV=2.5) shift(MULTIPLYING)\n  \
             /MISSING=crop(ACTIVE RANDIMPU) water(LISTWISE)."
                .into(),
        ),
        // supplementary cases and a variable, the model and its criteria
        (
            Mca {
                settings: vec![setting("water", &|x| x.supplementary = true)],
                objects: s("2, 5-7"),
                dimensions: s("3"),
                normalization: 2,
                max_iterations: s("50"),
                convergence: s("0.0001"),
                ..base.clone()
            },
            "MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  \
             /SUPPLEMENTARY=OBJECT(2 5 TO 7) VARIABLE(water)\n  /DIMENSION=3\n  \
             /NORMALIZATION=SYMMETRICAL\n  /MAXITER=50\n  /CRITITER=0.0001."
                .into(),
        ),
        (
            Mca {
                normalization: 4,
                q: s("0.5"),
                ..base.clone()
            },
            "MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  /NORMALIZATION=0.5."
                .into(),
        ),
        // every table and plot, with lists
        (
            Mca {
                labels: v(&["member"]),
                dimensions: s("3"),
                print: [true; 7],
                descrip: v(&["member", "site"]),
                quant: v(&["crop", "shift"]),
                object_label: s("member"),
                object_shown: v(&["site"]),
                plots: [true; 7],
                plot_variables: [
                    v(&["site"]),
                    v(&["crop"]),
                    v(&["site", "water"]),
                    v(&["hours"]),
                    v(&["shift"]),
                    v(&["site", "crop"]),
                    v(&["water"]),
                ],
                biplot_labels: v(&["member"]),
                plot_dimensions: s("2"),
                label_length: s("10"),
                ndim: true,
                ndim_first: s("1"),
                ndim_last: s("3"),
                ..base.clone()
            },
            "MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift member\n  \
             /ANALYSIS=site crop water hours shift\n  /DIMENSION=3\n  \
             /PRINT=CORR DESCRIP(site member) DISCRIM HISTORY OBJECT((member) site) OCORR \
             QUANT(crop shift)\n  \
             /PLOT=OBJECT(site)(10) CATEGORY(crop)(10) JOINTCAT(site water)(10) \
             DISCRIM(hours)(10) BIPLOT((shift))(member)(10) TRANS(site(2) crop(2)) \
             RESID(water(2))(10) NDIM(1,3)."
                .into(),
        ),
        (
            Mca {
                print: [false; 7],
                plots: [false; 7],
                ..base.clone()
            },
            "MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  /PRINT=NONE\n  /PLOT=NONE."
                .into(),
        ),
        // saved variables with roots, and the three outputs to a file and new datasets
        (
            Mca {
                save: [true; 2],
                save_roots: [s("gq"), s("")],
                outfile: [1, 2, 2],
                outfile_files: [recoded.clone(), s(""), s("")],
                outfile_datasets: [s(""), s("quantified"), s("scores")],
                ..base.clone()
            },
            format!(
                "DATASET DECLARE quantified.\nDATASET DECLARE scores.\n\
                 MULTIPLE CORRESPONDENCE VARIABLES=site crop water hours shift\n  \
                 /ANALYSIS=site crop water hours shift\n  /SAVE=TRDATA(gq) OBJECT\n  \
                 /OUTFILE=DISCRDATA('{recoded}') TRDATA(quantified) OBJECT(scores)."
            ),
        ),
    ];
    let program = garden_members();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(Advanced::Mca(Box::new(dialog))), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
    assert!(Path::new(&recoded).exists());
    std::fs::remove_dir_all(out).unwrap();
}

/// The Categorical Principal Components dialog on the invented garden members: the syntax is
/// as expected, parses and runs without a warning.
#[test]
fn the_categorical_principal_components_dialog_writes_syntax_that_runs() {
    let out = folder("walk-catpca");
    let recoded = out.join("recoded.sav").to_string_lossy().into_owned();
    let scores = out.join("scores.sav").to_string_lossy().into_owned();
    let base = Catpca {
        variables: v(&["site", "crop", "water", "hours", "shift"]),
        ..Default::default()
    };
    let setting = |name: &str, change: &dyn Fn(&mut CatpcaVariable)| {
        let mut s = CatpcaVariable::new(name);
        change(&mut s);
        s
    };
    let mixed = vec![
        setting("site", &|x| {
            x.weight = s("2");
            x.level.degree = s("3");
            x.level.knots = s("1");
        }),
        setting("crop", &|x| {
            x.weight = s("2");
            x.level.level = 3;
            x.missing = 2;
        }),
        setting("water", &|x| {
            x.level.level = 4;
            x.missing = 1;
        }),
        setting("hours", &|x| {
            x.level.level = 5;
            x.discretize = 1;
            x.groups = s("4");
            x.uniform = true;
        }),
        setting("shift", &|x| {
            x.level.level = 1;
            x.discretize = 3;
        }),
    ];
    let cases: Vec<(Catpca, String)> = vec![
        (
            base.clone(),
            "CATPCA VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift."
                .into(),
        ),
        // every scaling level, weights, recoding and missing values per variable
        (
            Catpca {
                settings: mixed.clone(),
                ..base.clone()
            },
            "CATPCA VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site(WEIGHT=2 LEVEL=SPORD DEGREE=3 INKNOT=1) crop(WEIGHT=2 LEVEL=NOMI) \
             water(LEVEL=MNOM) hours(LEVEL=NUME) shift(LEVEL=SPNOM)\n  \
             /DISCRETIZATION=hours(GROUPING NCAT=4 DISTR=UNIFORM) shift(RANKING)\n  \
             /MISSING=crop(ACTIVE MODEIMPU) water(PASSIVE EXTRACAT)."
                .into(),
        ),
        // supplementary cases and a variable, the model and its criteria
        (
            Catpca {
                settings: vec![setting("water", &|x| x.supplementary = true)],
                objects: s("2, 5-7"),
                dimensions: s("3"),
                normalization: 2,
                max_iterations: s("50"),
                convergence: s("0.0001"),
                ..base.clone()
            },
            "CATPCA VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  \
             /SUPPLEMENTARY=OBJECT(2 5 TO 7) VARIABLE(water)\n  /DIMENSION=3\n  \
             /NORMALIZATION=SYMMETRICAL\n  /MAXITER=50\n  /CRITITER=0.0001."
                .into(),
        ),
        // an orthogonal and an oblique rotation
        (
            Catpca {
                rotation: 1,
                kaiser: false,
                ..base.clone()
            },
            "CATPCA VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  /ROTATION=VARIMAX NOKAISER."
                .into(),
        ),
        (
            Catpca {
                dimensions: s("3"),
                rotation: 4,
                kappa: s("3"),
                ..base.clone()
            },
            "CATPCA VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  /DIMENSION=3\n  /ROTATION=PROMAX(3)."
                .into(),
        ),
        // every table and plot, with lists
        (
            Catpca {
                settings: mixed.clone(),
                labels: v(&["member"]),
                dimensions: s("3"),
                // three dimensions of these levels settle slowly
                max_iterations: s("1000"),
                print: [true; 8],
                sort_loadings: true,
                descrip: v(&["member", "site"]),
                quant: v(&["crop", "water"]),
                object_label: s("member"),
                object_shown: v(&["site"]),
                plots: [true; 10],
                plot_variables: [
                    v(&["shift"]),
                    v(&["site", "hours"]),
                    v(&["crop"]),
                    v(&["site", "water"]),
                    v(&["water", "hours"]),
                    v(&["water"]),
                    v(&["site", "crop"]),
                    v(&["crop", "water"]),
                    v(&["site", "hours"]),
                    Vec::new(),
                ],
                loading_centroids: true,
                centroids: v(&["water"]),
                biplot_labels: v(&["member"]),
                triplot_labels: v(&["shift"]),
                projected: s("water"),
                plot_dimensions: s("2"),
                label_length: s("10"),
                ndim: true,
                ndim_first: s("1"),
                ndim_last: s("3"),
                ..base.clone()
            },
            "CATPCA VARIABLES=site crop water hours shift member\n  \
             /ANALYSIS=site(WEIGHT=2 LEVEL=SPORD DEGREE=3 INKNOT=1) crop(WEIGHT=2 LEVEL=NOMI) \
             water(LEVEL=MNOM) hours(LEVEL=NUME) shift(LEVEL=SPNOM)\n  \
             /DISCRETIZATION=hours(GROUPING NCAT=4 DISTR=UNIFORM) shift(RANKING)\n  \
             /MISSING=crop(ACTIVE MODEIMPU) water(PASSIVE EXTRACAT)\n  /DIMENSION=3\n  \
             /MAXITER=1000\n  /PRINT=CORR DESCRIP(site member) HISTORY LOADING(SORT) OBJECT((member) site) OCORR \
             QUANT(crop water) VAF\n  \
             /PLOT=OBJECT(shift)(10) LOADING(site hours (CENTR(water)))(10) CATEGORY(crop)(10) \
             JOINTCAT(site water)(10) TRANS(water(2) hours)(10) RESID(water(2))(10) \
             BIPLOT(LOADING(site crop))(member)(10) TRIPLOT(crop water(shift))(10) \
             PROJCENTR(water, site hours)(10) VAF NDIM(1,3)."
                .into(),
        ),
        (
            Catpca {
                print: [false; 8],
                plots: [false; 10],
                ..base.clone()
            },
            "CATPCA VARIABLES=site crop water hours shift\n  \
             /ANALYSIS=site crop water hours shift\n  /PRINT=NONE\n  /PLOT=NONE."
                .into(),
        ),
        // saved variables with roots, and the four outputs to files and new datasets
        (
            Catpca {
                save: [true; 3],
                save_roots: [s("gq"), s(""), s("ap")],
                outfile: [1, 2, 1, 2],
                outfile_files: [recoded.clone(), s(""), scores.clone(), s("")],
                outfile_datasets: [s(""), s("quantified"), s(""), s("approximated")],
                ..base.clone()
            },
            format!(
                "DATASET DECLARE quantified.\nDATASET DECLARE approximated.\n\
                 CATPCA VARIABLES=site crop water hours shift\n  \
                 /ANALYSIS=site crop water hours shift\n  /SAVE=TRDATA(gq) OBJECT APPROX(ap)\n  \
                 /OUTFILE=DISCRDATA('{recoded}') TRDATA(quantified) OBJECT('{scores}') \
                 APPROX(approximated)."
            ),
        ),
        // the scores the case before wrote, as a start and fixed
        (
            Catpca {
                configuration: 1,
                configuration_file: scores.clone(),
                ..base.clone()
            },
            format!(
                "CATPCA VARIABLES=site crop water hours shift\n  \
                 /ANALYSIS=site crop water hours shift\n  /CONFIGURATION=INITIAL('{scores}')."
            ),
        ),
        (
            Catpca {
                configuration: 2,
                configuration_file: scores.clone(),
                ..base.clone()
            },
            format!(
                "CATPCA VARIABLES=site crop water hours shift\n  \
                 /ANALYSIS=site crop water hours shift\n  /CONFIGURATION=FIXED('{scores}')."
            ),
        ),
    ];
    let program = garden_members();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::Advanced(Advanced::Catpca(Box::new(dialog))), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        assert!(
            result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
    assert!(Path::new(&recoded).exists());
    std::fs::remove_dir_all(out).unwrap();
}

/// The conjoint dialogs in a chain: the plan is generated, shown as cards and then analysed
/// from invented ranks, and the syntax is as expected, parses and runs.
#[test]
fn the_conjoint_dialogs_write_syntax_that_runs() {
    let out = folder("walk-conjoint");
    let cards = out.join("cards.txt").to_string_lossy().into_owned();
    let ranks = out.join("ranks.sav").to_string_lossy().into_owned();
    let mut session = statscraft_engine::session::Session::default();
    let step = |session: &mut statscraft_engine::session::Session, design: Design| {
        let data = session
            .dataset()
            .cloned()
            .unwrap_or_else(|| statscraft_data::Dataset::new(0));
        let syntax = drawn(
            Dialog::Advanced(Advanced::Conjoint(Box::new(design))),
            &data,
        )
        .unwrap();
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session.run(&syntax, None).unwrap();
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
        // cards written to a file leave nothing to show
        assert!(
            syntax.contains("OUTFILE") || result.commands.iter().any(|c| !c.tables.is_empty()),
            "{syntax}"
        );
        syntax
    };
    let generate = step(
        &mut session,
        Design::Generate(OrthogonalPlan {
            factors: vec![
                plan_factor("brand", "Brand", &["Acme", "Brio", "Cora"]),
                plan_factor("price", "Price", &["", "", ""]),
                plan_factor("organic", "", &["no", "yes"]),
            ],
            holdout: s("2"),
            seed: s("11"),
            ..Default::default()
        }),
    );
    assert_eq!(
        generate,
        "SET SEED=11.\nORTHOPLAN\n  /FACTORS=brand 'Brand' (1 'Acme' 2 'Brio' 3 'Cora')\n    \
         price 'Price' (1 2 3)\n    organic (1 'no' 2 'yes')\n  /HOLDOUT=2\n  /REPLACE."
    );
    let rated = session.dataset().unwrap().case_count();
    let show = step(
        &mut session,
        Design::Display(PlanCards {
            factors: v(&["brand", "price", "organic"]),
            format: 2,
            title: s("Profile )CARD"),
            footer: s("Rank the profiles"),
            file: cards.clone(),
        }),
    );
    assert_eq!(
        show,
        format!(
            "PLANCARDS\n  /FACTORS=brand price organic\n  /FORMAT=BOTH\n  \
             /TITLE='Profile )CARD'\n  /FOOTER='Rank the profiles'\n  /OUTFILE='{cards}'."
        )
    );
    assert!(Path::new(&cards).exists());
    // five subjects rank the rated profiles, each starting the order at another place
    let rows: Vec<String> = (0..5)
        .map(|subject| {
            let ranks: Vec<String> = (0..rated)
                .map(|k| ((k + subject * 3) % rated + 1).to_string())
                .collect();
            format!("{} {}", subject + 1, ranks.join(" "))
        })
        .collect();
    let columns: Vec<String> = (1..=rated).map(|k| format!("r{k}")).collect();
    let mut data = statscraft_engine::session::Session::default();
    data.run(
        &format!(
            "DATA LIST LIST /id {}.\nBEGIN DATA\n{}\nEND DATA.\nSAVE OUTFILE='{ranks}'.",
            columns.join(" "),
            rows.join("\n")
        ),
        None,
    )
    .unwrap();
    let conjoint = step(
        &mut session,
        Design::Analysis(Box::new(ConjointAnalysis {
            data_from_file: true,
            data_file: ranks.clone(),
            record: 1,
            responses: format!("r1 TO r{rated}"),
            subject: s("id"),
            factors: vec![
                FactorModel {
                    name: s("price"),
                    model: 1,
                    direction: 2,
                },
                FactorModel {
                    name: s("brand"),
                    ..Default::default()
                },
            ],
            ..Default::default()
        })),
    );
    assert_eq!(
        conjoint,
        format!(
            "CONJOINT PLAN=*\n  /DATA='{ranks}'\n  /RANK=r1 TO r{rated}\n  /SUBJECT=id\n  \
             /FACTORS=price (LINEAR LESS) brand (DISCRETE)."
        )
    );
    std::fs::remove_dir_all(out).unwrap();
}

/// A folder of its own for the files a test writes.
fn folder(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-gui-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn every_dialog_with_the_least_it_needs_writes_syntax_that_runs() {
    let out = folder("walk-min");
    std::fs::write(out.join("in.csv"), "a,b\n1,x\n2,y\n").unwrap();
    walk(minimal(&out));
    std::fs::remove_dir_all(out).unwrap();
}

/// The family of a dialog in the walk with every option on, which is split into a test per
/// family so that they run in parallel. A dialog not named here is in the last family.
fn family(dialog: &Dialog) -> usize {
    match dialog {
        Dialog::Frequencies(_)
        | Dialog::Descriptives(_)
        | Dialog::Explore(_)
        | Dialog::Crosstabs(_)
        | Dialog::CustomTables(..)
        | Dialog::Means(_)
        | Dialog::MultResponse(_)
        | Dialog::TTest(_)
        | Dialog::Oneway(_)
        | Dialog::Correlations(_)
        | Dialog::NparTests(_) => 0,
        Dialog::Compute(..)
        | Dialog::Recode(..)
        | Dialog::Count(..)
        | Dialog::Rank(_)
        | Dialog::Autorecode(_)
        | Dialog::Mrsets(_)
        | Dialog::SortCases(_)
        | Dialog::SplitFile(_)
        | Dialog::SelectCases(_)
        | Dialog::WeightCases(_)
        | Dialog::Aggregate(..)
        | Dialog::MergeFiles(_)
        | Dialog::Restructure(_)
        | Dialog::SortVariables(_)
        | Dialog::ImportData(_)
        | Dialog::ExportData(_) => 1,
        Dialog::ChartBuilder(_)
        | Dialog::ProbabilityPlot(_)
        | Dialog::RocCurve(_)
        | Dialog::RocAnalysis(_)
        | Dialog::ResponseRate(_)
        | Dialog::ControlCharts(_) => 2,
        Dialog::Regression(_)
        | Dialog::Reliability(_)
        | Dialog::PartialCorr(_)
        | Dialog::FactorAnalysis(_)
        | Dialog::KMeans(_)
        | Dialog::Hierarchical(_)
        | Dialog::TwoStep(_)
        | Dialog::Anomaly(_)
        | Dialog::Mlp(_)
        | Dialog::Rbf(_)
        | Dialog::Discriminant(_)
        | Dialog::Logistic(_)
        | Dialog::Tree(_)
        | Dialog::Catreg(_) => 3,
        Dialog::Univariate(_)
        | Dialog::RepeatedMeasures(_)
        | Dialog::Advanced(
            Advanced::Ordinal(_)
            | Advanced::Multinomial(_)
            | Advanced::Mixed(_)
            | Advanced::Genlin(_)
            | Advanced::Glmm(_),
        ) => 4,
        _ => 5,
    }
}

/// Walks the dialogs of one family with every option on.
fn walk_family(number: usize) {
    let out = folder(&format!("walk-max-{number}"));
    walk(
        maximal(&out)
            .into_iter()
            .filter(|d| family(d) == number)
            .collect(),
    );
    std::fs::remove_dir_all(out).unwrap();
}

#[test]
fn the_descriptive_dialogs_with_every_option_on_write_syntax_that_runs() {
    walk_family(0);
}

#[test]
fn the_data_dialogs_with_every_option_on_write_syntax_that_runs() {
    walk_family(1);
}

#[test]
fn the_chart_dialogs_with_every_option_on_write_syntax_that_runs() {
    walk_family(2);
}

#[test]
fn the_regression_and_classification_dialogs_with_every_option_on_write_syntax_that_runs() {
    walk_family(3);
}

#[test]
fn the_linear_model_dialogs_with_every_option_on_write_syntax_that_runs() {
    walk_family(4);
}

/// Survival, complex samples, correspondence, distances and the other advanced dialogs.
#[test]
fn the_other_dialogs_with_every_option_on_write_syntax_that_runs() {
    walk_family(5);
}

#[test]
fn categories_labelled_by_the_counted_value_need_a_label_each() {
    let session = open("survey_basic.sav");
    let data = session.dataset().unwrap();
    let set = |variables: &[&str]| {
        Dialog::Mrsets(Mrsets {
            name: s("$q5"),
            variables: v(variables),
            counted_value_labels: true,
            ..Default::default()
        })
        .syntax(data)
    };
    // every variable labels 1 'Selected'
    assert_eq!(
        set(&["q5_1", "q5_2"]).unwrap_err(),
        "q5_2 has the same label for the counted value as another variable"
    );
    assert_eq!(
        set(&["q5_1", "age"]).unwrap_err(),
        "age has no value label for the counted value"
    );
}

/// Sixty invented fill measurements in twelve batches of five, one case per measurement, each
/// case also a subgroup of five values with a count of rejects out of fifty.
fn filling_line() -> String {
    let mut program = String::from(
        "DATA LIST LIST /resp batch fill status defects x1 x2 x3 x4 x5 bad size.\nBEGIN DATA\n",
    );
    for i in 0..60usize {
        let fill = 500 + (i * 7) % 11;
        let status = usize::from(i % 7 == 0);
        let defects = (i * 3) % 5;
        let x: Vec<usize> = (0..5).map(|j| 498 + (i * (j + 3)) % 9).collect();
        program.push_str(&format!(
            "{} {} {fill} {status} {defects} {} {} {} {} {} {} 50\n",
            i + 1,
            i / 5 + 1,
            x[0],
            x[1],
            x[2],
            x[3],
            x[4],
            (i * 5) % 6
        ));
    }
    program.push_str("END DATA.\n");
    program
}

/// The control chart dialog on the invented filling line: every chart and data layout writes
/// the expected syntax, which parses and runs without a warning.
#[test]
fn the_control_chart_dialog_writes_syntax_that_runs() {
    use crate::spchart_dialog::{C, I, IR, NP, P, U, XS};
    let base = ControlCharts::default;
    let limits = |d: ControlCharts| ControlCharts {
        lsl: s("490"),
        usl: s("515"),
        ..d
    };
    let on = |picked: &[usize]| {
        let mut flags = [false; 17];
        picked.iter().for_each(|i| flags[*i] = true);
        flags
    };
    let wide = || v(&["x1", "x2", "x3", "x4", "x5"]);
    let cases: Vec<(ControlCharts, &str)> = vec![
        (
            ControlCharts {
                process: s("fill"),
                group: s("batch"),
                rules: [true; 11],
                id: s("resp"),
                statistics: on(&[0, 3, 6, 9]),
                ..limits(base())
            },
            "SPCHART /XR=fill BY batch\n  /RULES=ALL\n  /ID=resp\n  /LSL=490\n  /USL=515\n  \
             /STATISTICS=CP CPK PP PPK.",
        ),
        (
            ControlCharts {
                chart: XS,
                layout: 1,
                variables: wide(),
                label: s("resp"),
                xbar_only: true,
                ..base()
            },
            "SPCHART /XS=x1 x2 x3 x4 x5 BY resp (XBARONLY).",
        ),
        (
            ControlCharts {
                chart: IR,
                process: s("fill"),
                span: s("3"),
                capsigma: 3,
                ..base()
            },
            "SPCHART /IR=fill\n  /SPAN=3\n  /CAPSIGMA=MRBAR.",
        ),
        (
            ControlCharts {
                chart: I,
                process: s("fill"),
                label: s("resp"),
                ..base()
            },
            "SPCHART /I=fill BY resp.",
        ),
        (
            ControlCharts {
                chart: P,
                process: s("status"),
                group: s("batch"),
                status_value: s("1"),
                ..base()
            },
            "SPCHART /P=status BY batch\n  /NONCONFORM=1.",
        ),
        (
            ControlCharts {
                chart: NP,
                layout: 1,
                count: s("bad"),
                size_name: s("size"),
                label: s("resp"),
                ..base()
            },
            "SPCHART /NP=COUNT(bad) N(size) BY resp.",
        ),
        (
            ControlCharts {
                chart: C,
                process: s("defects"),
                group: s("batch"),
                ..base()
            },
            "SPCHART /C=defects BY batch.",
        ),
        (
            ControlCharts {
                chart: U,
                layout: 1,
                count: s("bad"),
                size_variable: false,
                size: s("50"),
                ..base()
            },
            "SPCHART /U=COUNT(bad) N(50).",
        ),
        (
            ControlCharts {
                chart: CAPABILITY,
                layout: 1,
                variables: wide(),
                normal: 2,
                reference: 1,
                statistics: on(&[0, 3, 13]),
                ..limits(base())
            },
            "SPCHART /CPCHART=x1 x2 x3 x4 x5\n  /LSL=490\n  /USL=515\n  \
             /STATISTICS=CP CPK MEAN\n  /NORMAL=BOTH\n  /REFERENCE=SL SIGMAS.",
        ),
        (
            ControlCharts {
                chart: CAPABILITY,
                layout: 2,
                process: s("fill"),
                ..limits(base())
            },
            "SPCHART /CPCHART=fill\n  /LSL=490\n  /USL=515.",
        ),
    ];
    let program = filling_line();
    for (dialog, expected) in cases {
        let mut session = statscraft_engine::session::Session::default();
        session.run(&program, None).unwrap();
        let data = session.dataset().unwrap().clone();
        let syntax = drawn(Dialog::ControlCharts(Box::new(dialog)), &data).unwrap();
        assert_eq!(syntax, expected);
        let (_, errors) = statscraft_syntax::parse(&syntax);
        assert!(errors.is_empty(), "{syntax}\n{errors:?}");
        let result = session
            .run(&syntax, None)
            .unwrap_or_else(|e| panic!("{syntax}\n{e:?}"));
        assert!(
            result
                .commands
                .iter()
                .any(|c| !c.tables.is_empty() || !c.charts.is_empty()),
            "{syntax}"
        );
        let warnings: Vec<&String> = result.commands.iter().flat_map(|c| &c.warnings).collect();
        assert!(warnings.is_empty(), "{syntax}\n{warnings:?}");
    }
}
