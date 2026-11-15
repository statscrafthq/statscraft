//! Every dialog of the window, with the least it needs and with every option on, writes syntax
//! that the parser takes and that runs on the survey file.

use super::{CategoryDraft, Dialog, RuleDraft};
use crate::advanced_dialogs::{
    Advanced, ContrastTest, LinearMixed, MeansTable, Multinomial, Ordinal, SampleDescriptives,
    SampleOptions, SamplePlan, SampleTables, Stage, Terms,
};
use crate::analyze_dialogs::{
    Correlations, Explore, ExploreMissing, Means, MrGroup, MultResponse, Oneway, TTest, TTestKind,
    Tree,
};
use crate::correspondence_dialog::{Correspondence, Role, TableVariable};
use crate::cox_dialog::Cox;
use crate::cs_model_dialog::{Held, OddsRatio, SampleModel};
use crate::data_dialogs::{
    Aggregate, AggregateTo, Aggregated, ExportData, ImportData, Make, MergeFiles, Mrsets,
    Restructure, RestructureKind, SelectCases, SelectMode, SortCases, SortVariables, SplitFile,
    SplitMode, WeightCases,
};
use crate::dialogs::testing::{fixture, open, v};
use crate::dialogs::{
    Adjust, Autorecode, Categories, Compute, Count, Crosstabs, CustomTables, Descriptives,
    Frequencies, Letters, NewValue, OldValue, Rank, Recode, RecodeRule, Subtotal,
};
use crate::genlin_dialog::GeneralizedLinear;
use crate::graph_dialogs::{ChartBuilder, ChartKind, ProbabilityPlot, RocCurve};
use crate::imputation_dialog::{Imputation, Limits};
use crate::loglinear_dialog::{GeneralLoglinear, ModelSelection};
use crate::model_dialogs::{
    Categorical, ClusterMeasure, Discriminant, FactorAnalysis, FactorContrast, Hierarchical,
    KMeans, Logistic, MColumn, RepeatedMeasures, Solutions, Univariate, WithinFactor,
};
use crate::mva_dialog::{Estimation, MissingValueAnalysis};
use crate::proximities_dialog::Distances;
use crate::reports_dialog::{Codebook, Summarize};
use crate::stats_dialogs::{Block, NparKind, NparTests, PartialCorr, Regression, Reliability};
use crate::survival_dialog::{KaplanMeier, LifeTables};
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

/// Each dialog with the fewest choices that make its syntax.
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
        Dialog::Advanced(Advanced::Correspondence(Correspondence {
            row: table_variable("region", "1", "5", &[]),
            column: table_variable("q6_mention1", "1", "6", &[]),
            ..Default::default()
        })),
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
    list.push(Dialog::RocCurve(RocCurve {
        tests: v(&["age"]),
        state: s("q5_1"),
        value: s("1"),
        ..Default::default()
    }));
    for kind in NparKind::ALL {
        let mut d = NparTests::new(kind);
        d.variables = v(&["q1_satisfaction", "q2_recommend"]);
        d.pairs = vec![(s("q1_satisfaction"), s("q2_recommend"))];
        d.grouping = s("gender");
        d.low = s("1");
        d.high = s("2");
        list.push(Dialog::NparTests(d));
    }
    list
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
        }),
        Dialog::Descriptives(Descriptives {
            variables: v(&["age", "monthly_spend"]),
            statistics: [true; 10],
            save_z_scores: true,
            listwise: true,
        }),
        Dialog::Explore(Explore {
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
            rows: v(&["gender"]),
            columns: v(&["region"]),
            layers: v(&["q5_1"]),
            cells: [true; 8],
            chi_square: true,
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
        }),
        Dialog::Oneway(Oneway {
            dependents: v(&["age", "monthly_spend"]),
            factor: s("region"),
            statistics: [true; 4],
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
        Dialog::Univariate(Univariate {
            dependent: s("monthly_spend"),
            factors: v(&["region", "gender"]),
            covariates: v(&["age"]),
            custom: true,
            terms: vec![v(&["region"]), v(&["gender"]), v(&["region", "gender"])],
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
            print: [true; 9],
            alpha: s("0.01"),
            ..Default::default()
        }),
        Dialog::Univariate(Univariate {
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
            print: [true; 9],
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
            print: [true; 12],
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
            iterations: s("150"),
            halvings: s("10"),
            convergence: s("0.0001"),
            kernel: true,
            analysis: 2,
            likelihood_ratio: true,
            print: [true; 8],
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
            print: [true; 8],
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
        }),
    ];
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
        if kind == NparKind::TwoIndependent {
            d.high = s("2");
        }
        list.push(Dialog::NparTests(d));
    }
    list
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
        let syntax = Dialog::Advanced(dialog).syntax(&data).unwrap();
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
        let syntax = Dialog::Advanced(Advanced::Mva(Box::new(dialog)))
            .syntax(&data)
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
        let syntax = Dialog::Advanced(Advanced::Imputation(Box::new(dialog)))
            .syntax(&data)
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
        let syntax = Dialog::Advanced(dialog).syntax(&data).unwrap();
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
        let syntax = Dialog::Advanced(dialog).syntax(&data).unwrap();
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

#[test]
fn every_dialog_with_every_option_on_writes_syntax_that_runs() {
    let out = folder("walk-max");
    walk(maximal(&out));
    std::fs::remove_dir_all(out).unwrap();
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
