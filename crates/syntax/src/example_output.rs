//! One table of output for the command examples that run on the sample file
//! docs/sample/survey.sav, shown on the command pages. A test in crates/cli runs every example
//! and checks that its output still holds this text.

/// (command, part of the output of its example).
const OUTPUTS: &[(&str, &str)] = &[
    (
        "GET",
        r#"### Variables in the active dataset

| Variable | Name in the file |
| --- | --- |
| id | id |
| region | region |
| gender | gender |
| age | age |
| age_group | age_group |
| education | education |
| weight | weight |
| satisfaction | satisfaction |
| recommend | recommend |
| contacted | contacted |
| switching | switching |
| monthly_bill | spend |
| interview_date | interview_date |"#,
    ),
    (
        "SAVE",
        r#"### Variables in the file

| Name in the file | Variable |
| --- | --- |
| id | id |
| region | region |
| sat | satisfaction |"#,
    ),
    (
        "FREQUENCIES",
        r#"### Overall satisfaction with the service

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 1 | Very dissatisfied | 20 | 6.67 | 7.22 | 7.22 |
| valid | 2 | Dissatisfied | 66 | 22 | 23.83 | 31.05 |
| valid | 3 | Neutral | 85 | 28.33 | 30.69 | 61.73 |
| valid | 4 | Satisfied | 63 | 21 | 22.74 | 84.48 |
| valid | 5 | Very satisfied | 43 | 14.33 | 15.52 | 100 |
| missing | 9 | Don't know | 23 | 7.67 |  |  |
| total |  |  | 300 | 100 |  |  |"#,
    ),
    (
        "DESCRIPTIVES",
        r#"### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| age | Age in years | 300 | 45.74 | 13.70 | 18 | 84 |
| spend | Monthly spend in euros | 284 | 38.51 | 13.21 | 13.63 | 94.52 |
| satisfaction | Overall satisfaction with the service | 277 | 3.16 | 1.16 | 1 | 5 |
| Valid N (listwise) |  | 261 |  |  |  |  |
| Missing N (listwise) |  | 39 |  |  |  |  |"#,
    ),
    (
        "CROSSTABS",
        r#"### Would recommend us to a friend by Region

| Would recommend us to a friend | Statistic | Region / North | Region / South | Region / East | Region / West | Total |
| --- | --- | --- | --- | --- | --- | --- |
| No | Count | 20 | 23 | 37 | 33 | 113 |
| Yes | Count | 55 | 39 | 44 | 49 | 187 |
| Total | Count | 75 | 62 | 81 | 82 | 300 |"#,
    ),
    (
        "CTABLES",
        r#"### Custom Tables

|  |  | Region / North / Count | Region / North / Column % | Region / South / Count | Region / South / Column % | Region / East / Count | Region / East / Column % | Region / West / Count | Region / West / Column % |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Overall satisfaction with the service | Very dissatisfied | 3 | 4.3% | 4 | 6.9% | 11 | 15.1% | 2 | 2.6% |
| Overall satisfaction with the service | Dissatisfied | 14 | 20.0% | 5 | 8.6% | 25 | 34.2% | 22 | 28.9% |
| Overall satisfaction with the service | Neutral | 11 | 15.7% | 19 | 32.8% | 26 | 35.6% | 29 | 38.2% |
| Overall satisfaction with the service | Satisfied | 26 | 37.1% | 16 | 27.6% | 9 | 12.3% | 12 | 15.8% |
| Overall satisfaction with the service | Very satisfied | 16 | 22.9% | 14 | 24.1% | 2 | 2.7% | 11 | 14.5% |
| Overall satisfaction with the service | Total | 70 | 100.0% | 58 | 100.0% | 73 | 100.0% | 76 | 100.0% |"#,
    ),
    (
        "MEANS",
        r#"### Report: Monthly spend in euros by Region

| Region | Mean | N | Std. Deviation |
| --- | --- | --- | --- |
| North | 36.14 | 71 | 11.89 |
| South | 40.50 | 59 | 16.44 |
| East | 38.47 | 76 | 13.51 |
| West | 39.19 | 78 | 11.10 |
| Total | 38.51 | 284 | 13.21 |"#,
    ),
    (
        "T-TEST",
        r#"### Group Statistics

|  |  | N | Mean | Std. Deviation | Std. Error Mean |
| --- | --- | --- | --- | --- | --- |
| Overall satisfaction with the service | No | 144 | 3.13 | 1.02 | 0.08 |
|  | Yes | 133 | 3.18 | 1.31 | 0.11 |
| Monthly spend in euros | No | 146 | 39.40 | 14.01 | 1.16 |
|  | Yes | 138 | 37.56 | 12.30 | 1.05 |"#,
    ),
    (
        "ONEWAY",
        r#"### ANOVA

|  |  | Sum of Squares | df | Mean Square | F | Sig. |
| --- | --- | --- | --- | --- | --- | --- |
| Overall satisfaction with the service | Between Groups | 47.20 | 3 | 15.73 | 13.13 | 0.00 |
|  | Within Groups | 327.12 | 273 | 1.20 |  |  |
|  | Total | 374.32 | 276 |  |  |  |"#,
    ),
    (
        "CORRELATIONS",
        r#"### Correlations

|  |  | Age in years | Monthly spend in euros | Overall satisfaction with the service |
| --- | --- | --- | --- | --- |
| Age in years | Pearson Correlation | 1 | -0.14 | -0.14 |
|  | Sig. (2-tailed) |  | 0.02 | 0.02 |
|  | N | 300 | 284 | 277 |
| Monthly spend in euros | Pearson Correlation | -0.14 | 1 | -0.16 |
|  | Sig. (2-tailed) | 0.02 |  | 0.01 |
|  | N | 284 | 284 | 261 |
| Overall satisfaction with the service | Pearson Correlation | -0.14 | -0.16 | 1 |
|  | Sig. (2-tailed) | 0.02 | 0.01 |  |
|  | N | 277 | 261 | 277 |"#,
    ),
    (
        "REGRESSION",
        r#"### Coefficients

| Model |  | Unstandardized Coefficients / B | Unstandardized Coefficients / Std. Error | Standardized Coefficients / Beta | t | Sig. |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | (Constant) | 4.34 | 0.35 |  | 12.38 | 0.00 |
|  | Age in years | -0.01 | 0.01 | -0.16 | -2.63 | 0.01 |
|  | Monthly spend in euros | -0.02 | 0.01 | -0.18 | -2.90 | 0.00 |
|  | Contacted customer support in the last year | 0.11 | 0.14 | 0.05 | 0.82 | 0.41 |

Dependent variable: Overall satisfaction with the service."#,
    ),
    (
        "COMPUTE",
        r#"### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| spend_year |  | 284 | 462.11 | 158.57 | 163.56 | 1134.24 |
| channels |  | 300 | 2.08 | 1.02 | 0 | 5 |
| days_in |  | 300 | 20.98 | 12.82 | 0 | 41 |
| Valid N (listwise) |  | 284 |  |  |  |  |
| Missing N (listwise) |  | 16 |  |  |  |  |"#,
    ),
    (
        "RECODE",
        r#"### sat3

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 1 | Dissatisfied | 86 | 28.67 | 31.05 | 31.05 |
| valid | 2 | Neutral | 85 | 28.33 | 30.69 | 61.73 |
| valid | 3 | Satisfied | 106 | 35.33 | 38.27 | 100 |
| missing |  | System | 23 | 7.67 |  |  |
| total |  |  | 300 | 100 |  |  |"#,
    ),
    (
        "IF",
        r#"### at_risk

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 0 |  | 269 | 89.67 | 89.67 | 89.67 |
| valid | 1 |  | 31 | 10.33 | 10.33 | 100 |
| total |  |  | 300 | 100 |  |  |"#,
    ),
    (
        "SELECT IF",
        r#"### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| age | Age in years | 205 | 44.76 | 7.87 | 30 | 59 |
| spend | Monthly spend in euros | 205 | 38.28 | 12.81 | 13.63 | 94.52 |
| Valid N (listwise) |  | 205 |  |  |  |  |
| Missing N (listwise) |  | 0 |  |  |  |  |"#,
    ),
    (
        "FILTER",
        r#"### Descriptive Statistics

| Variable | Label | N | Mean | Std Dev | Minimum | Maximum |
| --- | --- | --- | --- | --- | --- | --- |
| age | Age in years | 86 | 39.90 | 12.16 | 18 | 64 |
| spend | Monthly spend in euros | 84 | 39.24 | 13.65 | 15.79 | 94.52 |
| Valid N (listwise) |  | 84 |  |  |  |  |
| Missing N (listwise) |  | 2 |  |  |  |  |"#,
    ),
    (
        "WEIGHT",
        r#"### Age group

| Category | Value | Label | Frequency | Percent | Valid Percent | Cumulative Percent |
| --- | --- | --- | --- | --- | --- | --- |
| valid | 1 | 18-29 | 50.68 | 15.97 | 15.97 | 15.97 |
| valid | 2 | 30-44 | 106.51 | 33.55 | 33.55 | 49.52 |
| valid | 3 | 45-59 | 117.82 | 37.12 | 37.12 | 86.64 |
| valid | 4 | 60 and over | 42.41 | 13.36 | 13.36 | 100 |
| total |  |  | 317.42 | 100 |  |  |"#,
    ),
    (
        "SORT CASES",
        r#"### List

| id | region | spend |
| --- | --- | --- |
| 1 | 2 | 94.52 |
| 113 | 2 | 90.56 |
| 80 | 3 | 85.18 |
| 49 | 3 | 79.74 |
| 61 | 2 | 72.36 |"#,
    ),
    (
        "AGGREGATE",
        r#"### List

| region | respondents | mean_spend | share_recommend |
| --- | --- | --- | --- |
| 1 | 75 | 36.14 | .73 |
| 2 | 62 | 40.50 | .63 |
| 3 | 81 | 38.47 | .54 |
| 4 | 82 | 39.19 | .60 |"#,
    ),
    (
        "MRSETS",
        r#"### Multiple Response Sets

| Name | Label | Type | Counted value | Variables |
| --- | --- | --- | --- | --- |
| $channels | Channels used | Multiple dichotomy | 1 | ch_web ch_app ch_phone ch_store ch_email |"#,
    ),
    (
        "MULT RESPONSE",
        r#"### Channels used Frequencies

| Channels used | Responses / N | Responses / Percent | Percent of Cases |
| --- | --- | --- | --- |
| Website | 212 | 33.97 | 74.39 |
| Mobile app | 86 | 13.78 | 30.18 |
| Phone | 149 | 23.88 | 52.28 |
| Store | 65 | 10.42 | 22.81 |
| Email | 112 | 17.95 | 39.30 |
| Total | 624 | 100 | 218.95 |

channels is a dichotomy group counting the value 1."#,
    ),
];

/// Part of the output of the example of `command`, as Markdown, when the page shows one.
pub fn example_output(command: &str) -> Option<&'static str> {
    OUTPUTS
        .iter()
        .find(|(name, _)| *name == command)
        .map(|(_, text)| *text)
}
