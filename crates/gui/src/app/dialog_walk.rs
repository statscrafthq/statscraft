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
use crate::model_dialogs::{
    Categorical, Discriminant, FactorAnalysis, Hierarchical, KMeans, Logistic, RepeatedMeasures,
    Solutions, Univariate, WithinFactor,
};
use crate::stats_dialogs::{Block, NparKind, NparTests, PartialCorr, Regression, Reliability};
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
            interval: 7,
            power: s("2"),
            root: s("2"),
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
            ..Default::default()
        }),
        Dialog::Hierarchical(Hierarchical {
            variables: v(&["q5_1", "q5_2", "q5_3"]),
            data: 2,
            // Hamann: the ratios after it are undefined for a pair of cases with no 1 at all
            binary: 7,
            present: s("1"),
            absent: s("0"),
            ..Default::default()
        }),
        Dialog::Hierarchical(Hierarchical {
            variables: v(&["q6_mention1", "q6_mention2"]),
            data: 1,
            counts: 1,
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
            save: [true; 8],
            print: [true; 5],
            alpha: s("0.01"),
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
