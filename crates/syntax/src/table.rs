//! The command table: the single source for command name matching, parsing and help.

use crate::adp;
use crate::agreement;
use crate::aim;
use crate::alscal;
use crate::anova;
use crate::ast::Command;
use crate::bayes;
use crate::bootstrap;
use crate::catpca;
use crate::catreg;
use crate::cluster;
use crate::codebook;
use crate::commands;
use crate::complex_samples;
use crate::conjoint;
use crate::correspondence;
use crate::coxreg;
use crate::ctables;
use crate::data_commands;
use crate::data_list;
use crate::datacheck;
use crate::dictionary;
use crate::discriminant;
use crate::examine;
use crate::factor;
use crate::files;
use crate::frequencies;
use crate::genlin;
use crate::genlinmixed;
use crate::get_data;
use crate::ggraph;
use crate::glm;
use crate::graph;
use crate::igraph;
use crate::input_program;
use crate::knn;
use crate::legacy_scaling;
use crate::lexer::{Punct, Token, TokenKind};
use crate::logistic;
use crate::loglinear;
use crate::loops;
use crate::manova;
use crate::matrix;
use crate::matrix_data;
use crate::mixed;
use crate::mrsets;
use crate::mult_response;
use crate::multiple_correspondence;
use crate::multiple_imputation;
use crate::mva;
use crate::naivebayes;
use crate::neural;
use crate::nomreg;
use crate::npar;
use crate::nptests;
use crate::olap_cubes;
use crate::oms;
use crate::optimal_binning;
use crate::output;
use crate::output_docs;
use crate::output_modify;
use crate::parser::{PResult, Parser};
use crate::pls;
use crate::plum;
use crate::portable;
use crate::power;
use crate::pplot;
use crate::prefscal;
use crate::procedures;
use crate::proportions;
use crate::proximities;
use crate::proxscal;
use crate::quantile_regression;
use crate::rank;
use crate::ratio_statistics;
use crate::regression;
use crate::regression_more;
use crate::reliability;
use crate::report;
use crate::response_rate;
use crate::restructure;
use crate::roc_analysis;
use crate::save_translate;
use crate::selection;
use crate::session_commands;
use crate::simulation;
use crate::spchart;
use crate::stats;
use crate::summarize;
use crate::survival;
use crate::timeseries;
use crate::tree;
use crate::ts_models;
use crate::ts_more;
use crate::tsmodel;
use crate::twostep;
use crate::varcomp;
use crate::xgraph;

/// One supported command: its name, its help text and its parser.
pub struct CommandSpec {
    pub name: &'static str,
    pub summary: &'static str,
    /// The syntax diagram, then optional note lines.
    pub syntax: &'static str,
    pub example: &'static str,
    pub(crate) parse: fn(&mut Parser) -> PResult<Command>,
}

/// The help syntax of UNIANOVA and GLM, which share their parser.
macro_rules! glm_syntax {
    ($name:literal) => {
        concat!($name, " dependent [BY factors] [WITH covariates]\n\
                 \x20  [/WSFACTOR=name levels [POLYNOMIAL | contrast] ...] [/WSDESIGN=effects] (GLM)\n\
                 \x20  [/METHOD=SSTYPE(1 | 2 | 3 | 4)] [/INTERCEPT=INCLUDE | EXCLUDE] [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/CRITERIA=[EPS(1E-8)] [ALPHA(0.05)]] [/REGWGT=var]\n\
                 \x20  [/PRINT=[DESCRIPTIVE] [HOMOGENEITY] [PARAMETER] [ETASQ] [OPOWER] [LOF] [RSSCP] [GEF]\n\
                 \x20         [TEST([SSCP] [LMATRIX | ESTIMABLE] [MMATRIX | TRANSFORM])]]\n\
                 \x20  [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]\n\
                 \x20            [COMPARE[(factor)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...\n\
                 \x20  [/POSTHOC=factor ... (tests) [VS effect]] ... [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] |\n\
                 \x20  DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)] | SPECIAL(matrix)] ...\n\
                 \x20  [/LMATRIX=['label'] effect coefficients ... [; ...] | ALL coefficients] ...\n\
                 \x20  [/TEST=effect VS coefficient*effect +/- ... [DF(df)] | value DF(df)] ... (one dependent)\n\
                 \x20  [/MMATRIX=['label'] dependent value ... | ALL values [; ...]] (GLM) [/KMATRIX=values [; ...]]\n\
                 \x20  [/SAVE=tempvar[(name)] ...] [/DESIGN=effects]\n\
                 \x20  [/PLOT=[SPREADLEVEL] [RESIDUALS] [PROFILE(factor[*factor[*factor]] ...) [WITH(covariate=value | MEAN ...)]\n\
                 \x20         [TYPE=LINE | BAR] [MEANREFERENCE=NO | YES] [ERRORBAR=NO | CI | SE[(2)]] [YAXIS=AUTO | 0]]] ...\n\
                 effects: INTERCEPT, a factor or covariate, a*b or a BY b, a(b) or a WITHIN b, a(b(c)), x*x, x*a.\n\
                 tempvar: PRED WPRED RESID WRESID DRESID ZRESID SRESID SEPRED COOK LEVER. tests: as in ONEWAY\n\
                 /POSTHOC (LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2 SNK BTUKEY DUNCAN GT2 GABRIEL FREGW QREGW T3 C\n\
                 DUNNETT DUNNETTL DUNNETTR). VS names another effect of the design (not the intercept, and not a\n\
                 main effect named on the same POSTHOC) whose mean square and df are the error term of the\n\
                 pairwise and subset tests, which the tables then say; the tests of separate variances do not use\n\
                 an error term. Without DESIGN the model is the intercept, the covariates and the full\n\
                 factorial of the factors. A case missing on any variable is left out; the weight is rounded to a\n\
                 whole number. The design matrix has one column per effect level (only combinations that occur);\n\
                 sweeping it gives a generalized inverse, and a column that adds nothing (EPS) is redundant, its\n\
                 parameter 0. Tables: Between-Subjects Factors, Descriptive Statistics, Levene's Test of Equality\n\
                 of Error Variances (absolute residuals by cell; based on the mean, the median, the median with\n\
                 adjusted df and the trimmed mean for a full factorial without covariates), Tests of\n\
                 Between-Subjects Effects (Type I sequential, II, III or IV sums of squares, with partial eta\n\
                 squared, noncentrality and observed power), Parameter Estimates, Lack of Fit Tests, Contrast\n\
                 Results and Test Results for each CONTRAST and LMATRIX, Estimated Marginal Means (cells weighted\n\
                 equally, covariates at their means or the WITH values) with Pairwise Comparisons and Univariate\n\
                 Tests, and the post hoc tables of ONEWAY on the observed means with the model's error. T2, T3,\n\
                 GH and C need a model with one factor. SAVE adds PRE_1, RES_1 and so on, or the names given.\n\
                 GLM with WSFACTOR: the dependents are the levels of the within-subjects factors (the first\n\
                 factor's levels change slowest); Multivariate Tests, Mauchly's Test of Sphericity, Tests of\n\
                 Within-Subjects Effects (sphericity assumed, Greenhouse-Geisser, Huynh-Feldt, lower bound), Tests\n\
                 of Within-Subjects Contrasts and Tests of Between-Subjects Effects on the average.\n\
                 GLM with several dependents and no WSFACTOR is the multivariate model: Multivariate Tests of every\n\
                 effect (Pillai's trace, Wilks' lambda, Hotelling's trace, Roy's largest root, each with F, df and\n\
                 Sig., and with ETASQ and OPOWER partial eta squared, noncentrality and power), then the univariate\n\
                 tables with a row per dependent. HOMOGENEITY adds Box's M test of equal covariance matrices in\n\
                 the cells to Levene's test of each dependent; RSSCP gives the residual SSCP, covariance and\n\
                 correlation matrices and Bartlett's test of sphericity; TEST(SSCP) the SSCP of each effect and of\n\
                 the error. A custom test L B M = K takes L from LMATRIX (or the intercept's test when only\n\
                 MMATRIX or KMATRIX is given), M from MMATRIX (the identity by default; columns end with ';' and\n\
                 are named by their label or T1, T2, ...) and K from KMATRIX (rows end with ';', zero by default);\n\
                 CONTRAST always uses the identity; SPECIAL(matrix) takes the k x k matrix of a factor with k levels\n\
                 column by column, the first column the constant and the others one contrast each (the contrasts\n\
                 alone, k - 1 columns, also work), without fractions. Each test gives Contrast Results (K Matrix), Multivariate\n\
                 Test Results and Univariate Test Results; TEST(MMATRIX) shows M. EMMEANS, POSTHOC, PLOT and SAVE\n\
                 work on each dependent in turn (SAVE without names).\n\
                 PLOT gives chart data. PROFILE draws the estimated marginal means as a line chart (bars with\n\
                 TYPE=BAR) over the levels of the first factor, one line per level of a second factor and one chart\n\
                 per level of a third; MEANREFERENCE adds the grand mean as a flat line. ERRORBAR=CI spans each\n\
                 mean's confidence interval at 1 minus CRITERIA ALPHA, ERRORBAR=SE(n) n standard errors on each\n\
                 side (2 by default); YAXIS is left to the drawing. SPREADLEVEL plots the mean of every\n\
                 cell with two cases or more against its standard deviation, then against its variance. RESIDUALS\n\
                 gives every case's predicted value, observed value and standardized residual. With WSFACTOR, PLOT\n\
                 is ignored with a warning. GEF gives the general form of the estimable functions: a row per\n\
                 parameter, a column per free coefficient L1, L2, ... (a redundant parameter is a combination of them).\n\
                 TEST(LMATRIX) (or ESTIMABLE) gives the L' matrix of every effect of the design for the sum of\n\
                 squares type in use (a row per parameter, a column per row of L), and of every LMATRIX and\n\
                 CONTRAST. TEST tests an effect of the design against an error term, either a\n\
                 linear combination of the mean squares of other effects (coefficients may be fractions;\n\
                 coefficients of one effect add up) with Satterthwaite's degrees of freedom unless DF(df) gives\n\
                 them, or a mean square with its DF(df); one table of Hypothesis and Error rows each. It is for one\n\
                 dependent variable and without WSFACTOR. Not supported yet: RANDOM,\n\
                 ROBUST, OUTFILE, MEASURE, MBP, BP, F and\n\
                 WHITE; in the multivariate model the multivariate tests of EMMEANS COMPARE; with WSFACTOR also\n\
                 EMMEANS (left out with a warning), POSTHOC, CONTRAST, LMATRIX, MMATRIX, SAVE and PRINT\n\
                 PARAMETER, HOMOGENEITY, LOF, RSSCP and TEST(SSCP).")
    };
}

/// The help syntax of PPLOT and its other name NPPLOT.
const PPLOT_SYNTAX: &str = "PPLOT [VARIABLES=] varlist\n\
                 \x20  [/DISTRIBUTION=NORMAL[(a,b)] | EXPONENTIAL[(a)] | WEIBULL[(a,b)] | PARETO[(a,b)] |\n\
                 \x20   LNORMAL[(a,b)] | BETA[(a,b)] | GAMMA[(a,b)] | LOGISTIC[(a,b)] | LAPLACE[(a,b)] |\n\
                 \x20   UNIFORM[(a,b)] | HNORMAL[(a)] | CHI(df) | STUDENT(df)]\n\
                 \x20  [/FRACTION=BLOM | RANKIT | TUKEY | VW] [/TIES=MEAN | LOW | HIGH | BREAK]\n\
                 \x20  [/TYPE=Q-Q | P-P] [/PLOT=BOTH | NORMAL | DETRENDED] [/STANDARDIZE | /NOSTANDARDIZE]\n\
                 \x20  [/LN | /NOLOG] [/DIFF[=n]] [/SDIFF[=n]] [/PERIOD=n] [/APPLY].\n\
                 Parameters: NORMAL, LOGISTIC, LAPLACE location and scale; EXPONENTIAL rate; WEIBULL,\n\
                 LNORMAL scale and shape; PARETO threshold and shape; BETA two shapes; GAMMA shape and rate;\n\
                 UNIFORM minimum and maximum; HNORMAL scale (location 0). Without them they are estimated\n\
                 from the plotted values (moments; Weibull by least squares on the ranks; Pareto from the\n\
                 minimum and the mean log; half-normal by maximum likelihood) and listed in a table. Each\n\
                 variable is a series in case order: missing values are left out (user-missing ones kept\n\
                 under TSET MISSING=INCLUDE), LN logs it first (0 and below become missing), DIFF and SDIFF\n\
                 (over PERIOD cases, else TSET PERIOD, else the DATE period; with none SDIFF is left out)\n\
                 difference it, STANDARDIZE makes z-scores of the result. Ranks of tied values follow\n\
                 TIES (BREAK numbers the distinct values 1, 2, ...); FRACTION turns rank r of n into a\n\
                 proportion p: BLOM (r-3/8)/(n+1/4), RANKIT (r-1/2)/n, TUKEY (r-1/3)/(n+1/3), VW r/(n+1).\n\
                 Q-Q (the default) plots each value against the distribution's quantile at p, P-P plots p\n\
                 against the distribution function at the value, both with the line y = x; the detrended\n\
                 plot shows observed minus expected, with the line y = 0. BOTH draws both plots. Values the\n\
                 distribution cannot take leave the variable out with a warning. Weights are not used.\n\
                 APPLY repeats the previous PPLOT (its distribution without the parameters, and its\n\
                 variables unless new ones are given); the subcommands given with it override.\n\
                 Not supported yet: APPLY of a named model, TEMPLATE.";

const MVA_SYNTAX: &str = "MVA [VARIABLES=] varlist | ALL\n\
                 \x20  [/CATEGORICAL=varlist] [/MAXCAT=25] [/ID=var]\n\
                 \x20  [/NOUNIVARIATE] [/TTEST [PERCENT=5] [T | NOT] [DF | NODF] [PROB | NOPROB] [COUNTS | NOCOUNTS]\n\
                 \x20                         [MEANS | NOMEANS]]\n\
                 \x20  [/CROSSTAB [PERCENT=5]] [/MISMATCH [PERCENT=5] [NOSORT]]\n\
                 \x20  [/DPATTERN [SORT=var[(ASCENDING | DESCENDING)] ...] [DESCRIBE=varlist]]\n\
                 \x20  [/MPATTERN [NOSORT] [DESCRIBE=varlist]] [/TPATTERN [NOSORT] [DESCRIBE=varlist] [PERCENT=1]]\n\
                 \x20  [/LISTWISE] [/PAIRWISE]\n\
                 \x20  [/EM [varlist] [WITH varlist] [(TOLERANCE=0.001 CONVERGENCE=0.0001 ITERATIONS=25\n\
                 \x20       TDF=n | LAMBDA=a PROPORTION=b, OUTFILE='file' | dataset)]]\n\
                 \x20  [/REGRESSION [varlist] [WITH varlist] [(TOLERANCE=0.001 FLIMIT=4 NPREDICTORS=n\n\
                 \x20       ADDTYPE=RESIDUAL | NORMAL | T[(5)] | NONE, OUTFILE='file' | dataset)]].\n\
                 String variables and those on CATEGORICAL are categorical (a long string by its first 8\n\
                 bytes); MVA stops with a warning when one has more than MAXCAT values. All others are\n\
                 quantitative. System-missing and user-missing values count as missing; user-missing ones are\n\
                 coded A, B, C in the order of their definition (a range is A, its extra value B). CATEGORICAL,\n\
                 MAXCAT and ID come before the other subcommands; VARIABLES, SORT, NOSORT, DESCRIBE and WITH\n\
                 cannot name variables here. Weights are not used.\n\
                 Univariate Statistics (unless NOUNIVARIATE): valid N, missing count and percent, and for\n\
                 quantitative variables the mean, standard deviation and the count of extremes: values beyond\n\
                 1.5 interquartile ranges outside the quartiles (at (n+1)p), or, when variables times n times\n\
                 log10(n) passes 150000, beyond 2 standard deviations from the mean.\n\
                 TTEST: for each variable with at least PERCENT% missing, every quantitative variable is\n\
                 compared between the cases where it is present and missing (Welch t, its df, two-sided p with\n\
                 PROB, the counts and the means). CROSSTAB: per categorical variable and category, the count and\n\
                 percent present and the percent of each missing kind of every other variable with at least\n\
                 PERCENT% missing. MISMATCH: the percent of cases where one variable of a pair is missing and\n\
                 the other not, the percent missing on the diagonal.\n\
                 DPATTERN lists every case (in file order or by SORT), MPATTERN the cases with a missing value,\n\
                 each with S, A, B, C for missing and + or - for extremes, the number and percent missing and the\n\
                 DESCRIBE values. TPATTERN counts each pattern of missing values (X), leaving out those under\n\
                 PERCENT% of the cases; DESCRIBE adds category counts or means. ID labels the cases. In\n\
                 MPATTERN, TPATTERN and MISMATCH, unless NOSORT, variables go from fewest to most missing and\n\
                 alike patterns stand together; DPATTERN keeps the list order.\n\
                 LISTWISE: means, covariances and correlations over the cases complete on every variable.\n\
                 PAIRWISE: counts, means and standard deviations of the column variable, covariances and\n\
                 correlations over the cases valid on both variables.\n\
                 EM estimates the means, covariances and correlations of the quantitative variables (or the\n\
                 ones listed, from those after WITH) under a normal, a t (TDF) or a mixed normal (LAMBDA, the ratio\n\
                 of the standard deviations, PROPORTION, the share of the second part) distribution, starting from\n\
                 the univariate means and pairwise covariances; covariances divide by n - 1. It stops when no\n\
                 variance changes by more than CONVERGENCE relative to its value, or after ITERATIONS, and gives\n\
                 Little's MCAR test. REGRESSION fills each missing value by forward stepwise regression on the\n\
                 predictors the case has (F to enter FLIMIT, at most NPREDICTORS), from the univariate means\n\
                 and pairwise covariances, plus the residual of a random complete case, a normal or t error\n\
                 scaled by the residual standard deviation, or nothing; NORMAL is the default when fewer than\n\
                 half the cases are complete. Random terms follow SET SEED. When two or more ways to estimate are\n\
                 shown (univariate, LISTWISE, PAIRWISE, EM, REGRESSION), summary tables compare their means and\n\
                 standard deviations (pairwise ones are those of all values). OUTFILE saves the active\n\
                 data with the missing values of the estimated variables filled in and their user-missing\n\
                 definitions dropped.";

const MULTIPLE_IMPUTATION_SYNTAX: &str = "MULTIPLE IMPUTATION varlist\n\
                 \x20  [/IMPUTE [METHOD=AUTO | FCS | MONOTONE | NONE] [NIMPUTATIONS=5] [SCALEMODEL=LINEAR | PMM[(5)]]\n\
                 \x20          [INTERACTIONS=NONE | TWOWAY] [MAXPCTMISSING=NONE | pct] [MAXCASEDRAWS=50]\n\
                 \x20          [MAXPARAMDRAWS=2] [MAXITER=10] [SINGULAR=1E-12] [MAXMODELPARAM=100]]\n\
                 \x20  [/CONSTRAINTS varlist ([MIN=NONE | value] [MAX=NONE | value] [RND=NO | step]\n\
                 \x20                         [ROLE=BOTH | DEP | IND])] ...\n\
                 \x20  [/MISSINGSUMMARIES [OVERALL] [VARIABLES[(MAXVARS=25 MINPCTMISSING=10)]] [PATTERNS] | NONE]\n\
                 \x20  [/IMPUTATIONSUMMARIES [MODELS] [DESCRIPTIVES[(varlist)]] | NONE]\n\
                 \x20  [/ANALYSISWEIGHT var]\n\
                 \x20  [/OUTFILE [IMPUTATIONS='file' | dataset] [FCSITERATIONS='file' | dataset]].\n\
                 Fills in the missing values of the listed variables several times over and writes every\n\
                 completed copy, stacked under the original cases, to IMPUTATIONS (needed unless METHOD=NONE).\n\
                 The new first variable Imputation_ is 0 for the original cases and 1 to n for the copies, and\n\
                 the new data are split by it. Keywords are written in full and their equals signs are needed.\n\
                 Each variable is imputed from the others by its own model: linear regression for a scale\n\
                 variable (PMM instead takes the observed value of one of the k cases whose predictions lie\n\
                 nearest), multinomial logistic regression for a nominal, ordinal or string one; model\n\
                 parameters are drawn from their posterior first, so the copies differ. Categorical predictors\n\
                 enter as indicators, with TWOWAY also their pairwise products. MONOTONE imputes once in list\n\
                 order, each variable from the ones before it, and needs a monotone pattern of missing values;\n\
                 FCS starts from random values and cycles through the variables MAXITER times, each one from\n\
                 all the others; AUTO orders the variables from fewest to most missing, takes MONOTONE when the\n\
                 pattern allows it and FCS (10 rounds, linear, main effects) otherwise. A variable with more than\n\
                 MAXPCTMISSING percent missing is left out. CONSTRAINTS: MIN and MAX bound linear imputations\n\
                 (a value outside is drawn again, up to MAXCASEDRAWS times, then the parameters, up to\n\
                 MAXPARAMDRAWS times), RND rounds them to a multiple, ROLE=DEP imputes without predicting and\n\
                 ROLE=IND predicts and is imputed only for that. User-missing and system-missing values are\n\
                 both filled; cases with nothing valid, a weight under 0.5 or an ANALYSISWEIGHT of 0 or less\n\
                 keep theirs. WEIGHT counts cases (rounded); ANALYSISWEIGHT weights the models. Random draws\n\
                 follow SET SEED and SET RNG. FCSITERATIONS saves the mean and standard deviation of the imputed\n\
                 values of each scale variable after every round.\n\
                 Tables: the overall missing summary, VARIABLES (the most missing first, at least MINPCTMISSING\n\
                 percent), PATTERNS, the specifications, constraints and results, MODELS and DESCRIPTIVES of the\n\
                 original, imputed and completed values. A summary subcommand shows only the keywords on it.\n\
                 Pooling: with the data split by Imputation_, DESCRIPTIVES adds a table of pooled means by Rubin's\n\
                 rules (standard error, df, 95% interval, fraction of missing information, relative increase in\n\
                 variance, relative efficiency) and FREQUENCIES a table of the frequencies and percents averaged\n\
                 over the imputations. MEANS pools the mean of every row of its reports, T-TEST the mean\n\
                 differences, CORRELATIONS each correlation (on Fisher's z scale), REGRESSION, LOGISTIC\n\
                 REGRESSION, PLUM and NOMREG the coefficients; REGRESSION and LOGISTIC REGRESSION also get an F\n\
                 test that all their slopes are 0, pooled from the covariance matrices of the coefficients (Li,\n\
                 Raghunathan and Rubin 1991). SET MIOUTPUT chooses which of the original, imputed and pooled\n\
                 results appear and whether the pooling diagnostics do.";

pub(crate) static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "DATA LIST",
        summary: "Define text data, inline or in a file, and read it as the new active dataset.",
        syntax: "DATA LIST [FILE='path'] [FIXED | FREE [(delimiters)] | LIST [(delimiters)]] [RECORDS=n] [SKIP=n]\n\
                 \x20         [END=var] /[record] var [TO var] columns [(type[,decimals])] | (formats) [/ ...].\n\
                 FIXED (the default): columns such as 1-3, or formats such as (F2.0, 1X, A10) with Tn and nX.\n\
                 FREE and LIST: var [(format)], values separated by blanks and commas or by the delimiters\n\
                 given as quoted characters or TAB; LIST reads one case per line. * ends a run of F8.2 variables.\n\
                 Without FILE the data follow between BEGIN DATA and END DATA; transformations may stand\n\
                 between. Inside INPUT PROGRAM each run reads the next case's records into the case being\n\
                 built; without variables it passes over a record. END (fixed data in an input program\n\
                 only) names a variable that is 0, and 1 once the data are used up.\n\
                 Formats: F, N, E, COMMA, DOT, DOLLAR, PCT, A, DATE, ADATE, EDATE, SDATE, DATETIME,\n\
                 YMDHMS (year first), TIME (hours:minutes:seconds), MTIME (minutes:seconds), DTIME (days, then time).\n\
                 A value that cannot be read is system-missing, with a warning naming its line.",
        example: "DATA LIST LIST /id (F3) name (A12) score.",
        parse: data_list::data_list,
    },
    CommandSpec {
        name: "BEGIN DATA",
        summary: "Inline data for the DATA LIST before it, up to a line that holds END DATA.",
        syntax: "BEGIN DATA\n\
                 data lines\n\
                 END DATA.\n\
                 BEGIN DATA and END DATA each stand alone on their line. Every line between is data.",
        example: "BEGIN DATA\n1 'Ann' 12.5\nEND DATA.",
        parse: data_list::begin_data,
    },
    CommandSpec {
        name: "INPUT PROGRAM",
        summary: "Start an input program: commands that build the cases of a new active dataset.",
        syntax: "INPUT PROGRAM.\n\
                 \x20 DATA LIST, REREAD, REPEATING DATA, END CASE, END FILE and transformations\n\
                 END INPUT PROGRAM.\n\
                 The commands run again and again, each time from the top, until a DATA LIST runs out\n\
                 of data or END FILE runs. Each run builds a case at its end, or at each END CASE when\n\
                 the program has one. Before each case the variables are blank or system-missing again,\n\
                 except scratch variables and those named on LEAVE. A run that reads no record and does\n\
                 not end the file is an error, since it would repeat forever. SELECT IF, SAMPLE, FILTER,\n\
                 TEMPORARY and procedures cannot stand inside. The program runs right away, or at\n\
                 BEGIN DATA when it reads inline data.",
        example: "INPUT PROGRAM.\nDATA LIST /#n 1 head 3-4 (A).\nLEAVE head.\nLOOP #i = 1 TO #n.\nDATA LIST /item 1-2.\nEND CASE.\nEND LOOP.\nEND INPUT PROGRAM.",
        parse: input_program::input_program,
    },
    CommandSpec {
        name: "END INPUT PROGRAM",
        summary: "End an input program and run it.",
        syntax: "END INPUT PROGRAM.",
        example: "INPUT PROGRAM.\nLOOP #i = 1 TO 5.\nCOMPUTE x = #i.\nEND CASE.\nEND LOOP.\nEND FILE.\nEND INPUT PROGRAM.",
        parse: input_program::end_input_program,
    },
    CommandSpec {
        name: "END CASE",
        summary: "In an input program: add the case built so far to the dataset and go on.",
        syntax: "END CASE.\n\
                 The program goes on with the command after END CASE, with the variables blank or\n\
                 system-missing again except scratch and LEAVE variables. With END CASE in a program\n\
                 no case is built at the end of a run.",
        example: "LOOP #k = 1 TO 4.\nCOMPUTE score = item(#k).\nEND CASE.\nEND LOOP.",
        parse: input_program::end_case,
    },
    CommandSpec {
        name: "END FILE",
        summary: "In an input program or FILE TYPE: stop reading; the case being built is dropped.",
        syntax: "END FILE.\n\
                 Usually inside DO IF. Put END CASE before it to keep the case being built.",
        example: "DO IF day > 30.\nEND FILE.\nEND IF.",
        parse: input_program::end_file,
    },
    CommandSpec {
        name: "LEAVE",
        summary: "Keep the values of new variables from case to case instead of resetting them.",
        syntax: "LEAVE varlist.\n\
                 The variables must be new: made by the transformations since the data were last read,\n\
                 before LEAVE, and not scratch variables. A numeric one starts at 0 for the first case,\n\
                 a string one blank; each later case starts with the value the case before ended with.\n\
                 In an input program the variables are not reset before each case.",
        example: "COMPUTE runsum = runsum + amount.\nLEAVE runsum.",
        parse: input_program::leave,
    },
    CommandSpec {
        name: "REREAD",
        summary: "In an input program: let the next DATA LIST read the last record again.",
        syntax: "REREAD [FILE='path' | handle] [COLUMN = expression].\n\
                 COLUMN makes that column of the record column 1 for the DATA LIST (default 1). FILE\n\
                 names the data of the DATA LIST to reread (default: the last DATA LIST's); a DATA\n\
                 LIST that reads other data ignores the REREAD. Of several REREAD commands in a row\n\
                 the last counts.",
        example: "DATA LIST /kind 10-14 (A).\nDO IF kind = 'BELT'.\nREREAD.\nDATA LIST /cost 15-18 (DOLLAR,2).\nEND CASE.\nEND IF.",
        parse: input_program::reread,
    },
    CommandSpec {
        name: "REPEATING DATA",
        summary: "In an input program or FILE TYPE: one case per repeating group of a record.",
        syntax: "REPEATING DATA [FILE='path' | handle] [/ENCODING='UTF-8'] /STARTS=column[-column]\n\
                 \x20   /OCCURS=n [/LENGTH=n] [/CONTINUED[=column[-column]]] [/ID=columns=var]\n\
                 \x20   [/TABLE | /NOTABLE] /DATA=variables as on DATA LIST.\n\
                 STARTS, OCCURS, LENGTH and the end column on STARTS take a number or a variable.\n\
                 Reads OCCURS groups from the record the last DATA LIST read, starting in column\n\
                 STARTS; the columns on DATA count from the start of a group. A group is LENGTH\n\
                 columns, by default up to the last variable or skip. Each group gives a case with the\n\
                 values of the other variables spread to it. CONTINUED: groups that do not fit before\n\
                 the end column (default 80, or the longest record) go on in the next records, from\n\
                 the column given (default 1); ID checks that an identifier on those records matches.\n\
                 OCCURS missing or below 1 gives no case.",
        example: "DATA LIST /truck 1-3 nload 7-8.\nREPEATING DATA STARTS=10 /OCCURS=nload\n /DATA=cargo 1-6 (A) crates 8-9.",
        parse: input_program::repeating_data,
    },
    CommandSpec {
        name: "FILE TYPE",
        summary: "Start the definition of a file with several kinds of records.",
        syntax: "FILE TYPE MIXED | GROUPED | NESTED [FILE='path' | handle] [ENCODING='UTF-8']\n\
                 \x20   RECORD=[var] columns [(format)] [CASE=[var] columns [(format)]]\n\
                 \x20   [WILD=WARN | NOWARN] [DUPLICATE=WARN | NOWARN | CASE] [MISSING=WARN | NOWARN]\n\
                 \x20   [ORDERED=YES | NO].\n\
                 RECORD TYPE value ... / DATA LIST ... pairs follow, then END FILE TYPE.\n\
                 MIXED: each record of a listed type is a case; others are skipped. GROUPED: the\n\
                 records with one CASE value make one case (CASE required). NESTED: the RECORD TYPE\n\
                 commands go from the highest level to the lowest; each lowest-level record is a case\n\
                 with the values of the records above it. Defaults: WILD=NOWARN (WARN for grouped),\n\
                 DUPLICATE and MISSING WARN for grouped and NOWARN for nested, ORDERED=YES (grouped\n\
                 only); DUPLICATE=CASE (nested) makes a case of a record that has no records below it.",
        example: "FILE TYPE GROUPED RECORD=#kind 6 CASE=runner 1-4.\nRECORD TYPE 1.\nDATA LIST /club 8-9 (A).\nRECORD TYPE 2.\nDATA LIST /sprint 8-10.\nEND FILE TYPE.",
        parse: input_program::file_type,
    },
    CommandSpec {
        name: "RECORD TYPE",
        summary: "Inside FILE TYPE: the record types the following DATA LIST reads.",
        syntax: "RECORD TYPE value [value ...] | OTHER [SKIP] [CASE=columns] [DUPLICATE=WARN | NOWARN]\n\
                 \x20   [MISSING=WARN | NOWARN] [SPREAD=YES | NO].\n\
                 Values are numbers, or quoted strings for a string record identifier. OTHER stands\n\
                 for every type not named before and comes last. SKIP passes over the records. CASE\n\
                 gives other columns for the case identifier; DUPLICATE (grouped) and MISSING apply to\n\
                 these types only. SPREAD=NO (nested): the values go to the first case built under the\n\
                 record only.",
        example: "RECORD TYPE 11, 12, 13.\nDATA LIST /team 4 age 5-6.",
        parse: input_program::record_type,
    },
    CommandSpec {
        name: "END FILE TYPE",
        summary: "End a FILE TYPE definition and read the file.",
        syntax: "END FILE TYPE.",
        example: "FILE TYPE MIXED RECORD=rectype 1-2.\nRECORD TYPE 13.\nDATA LIST /team 4 minutes 7-9.\nEND FILE TYPE.",
        parse: input_program::end_file_type,
    },
    CommandSpec {
        name: "MATRIX DATA",
        summary: "Read matrix materials (correlations, means, counts) as a matrix data file.",
        syntax: "MATRIX DATA VARIABLES=varlist [/FILE=INLINE | 'path'] [/FORMAT=[LIST | FREE] [LOWER | UPPER | FULL]\n\
                 \x20           [DIAGONAL | NODIAGONAL]] [/SPLIT=varlist] [/FACTORS=varlist] [/CELLS=n] [/N=n]\n\
                 \x20           [/CONTENTS=[CORR] [COV] [MAT] [MSE] [DFE] [MEAN] [STDDEV | SD] [N_VECTOR | N] [N_SCALAR]\n\
                 \x20           [N_MATRIX] [COUNT] [PROX] [(types of within-cells records) ...]].\n\
                 The new active dataset holds the split variables (F4.0), ROWTYPE_ (A8), the factors (F4.0),\n\
                 VARNAME_ (A8, wider for longer names) and the continuous variables (F10.4): those of\n\
                 VARIABLES that are neither ROWTYPE_ nor a split or factor, which come last on VARIABLES.\n\
                 With ROWTYPE_ on VARIABLES each record starts with its fields in the order of VARIABLES:\n\
                 the record type (a name or a three-letter abbreviation, quoted or not) and the split and\n\
                 factor values (. for a pooled record). Without it CONTENTS gives the records in order, CORR\n\
                 when left out; types in parentheses are within-cells records, read for each of the CELLS\n\
                 cells with the factor values first. A matrix has one record per variable: LOWER and UPPER\n\
                 give a triangle, which is mirrored, FULL the whole matrix; with NODIAGONAL a correlation\n\
                 diagonal is 1 and any other diagonal missing. N_SCALAR gives one value for every variable.\n\
                 N=n adds an N record to each split. LIST starts each row on a new record (a row may go on\n\
                 over the next records; values left on a record are ignored with a warning), FREE reads the\n\
                 values as one stream. Values are separated by blanks or commas, and by + and - when not\n\
                 after D or E; . is missing. SPLIT may name one variable that is not on VARIABLES: the sets\n\
                 of materials are then numbered 1, 2, ... The result is split by the split variables. The\n\
                 data follow between BEGIN DATA and END DATA, or come from FILE. FACTOR, CLUSTER and the\n\
                 other procedures with MATRIX=IN(*) read it.",
        example: "MATRIX DATA VARIABLES=ROWTYPE_ q1 TO q3 /FORMAT=UPPER NODIAGONAL.\nBEGIN DATA\nMEAN 12 7 30\nSD 2 1.5 4\nN 40 40 40\nCORR .25 .4\nCORR -.3\nEND DATA.",
        parse: matrix_data::matrix_data,
    },
    CommandSpec {
        name: "MATRIX",
        summary: "Run a program in the matrix language, up to END MATRIX.",
        syntax: "MATRIX.\n\
                 \x20 COMPUTE m = expr | m(rows, cols) = expr.\n\
                 \x20 CALL EIGEN(m, vectors, values) | SETDIAG(m, v) | SVD(m, u, q, v).\n\
                 \x20 PRINT [m] [/FORMAT=format] [/TITLE='text'] [/SPACE=n | NEWPAGE]\n\
                 \x20       [/RLABELS=labels | /RNAMES=vector] [/CLABELS=labels | /CNAMES=vector].\n\
                 \x20 DO IF expr. ... [ELSE IF expr. ...] [ELSE. ...] END IF.\n\
                 \x20 LOOP [i = first TO last [BY step]] [IF expr]. ... [BREAK.] ... END LOOP [IF expr].\n\
                 \x20 READ m [/FILE=file] /FIELD=c1 TO c2 [BY w] [/SIZE=expr] [/MODE=RECTANGULAR | SYMMETRIC]\n\
                 \x20       [/REREAD] [/FORMAT=format].\n\
                 \x20 WRITE m [/OUTFILE=file] /FIELD=c1 TO c2 [BY w] [/MODE=RECTANGULAR | TRIANGULAR] [/HOLD]\n\
                 \x20       [/FORMAT=format].\n\
                 \x20 GET m [/FILE=file | *] [/VARIABLES=varlist] [/NAMES=name] [/MISSING=ACCEPT | OMIT | value]\n\
                 \x20       [/SYSMIS=OMIT | value].\n\
                 \x20 SAVE m [/OUTFILE=file | *] [/VARIABLES=names] [/NAMES=vector] [/STRINGS=names].\n\
                 \x20 MGET [/FILE=file | *] [/TYPE=COV CORR MEAN STDDEV N COUNT].\n\
                 \x20 MSAVE m /TYPE=COV | CORR | MEAN | STDDEV | N | COUNT [/OUTFILE=file] [/VARIABLES=names]\n\
                 \x20       [/SNAMES=names] [/SPLIT=vector] [/FNAMES=names] [/FACTOR=vector].\n\
                 \x20 DISPLAY [DICTIONARY] [STATUS].\n\
                 \x20 RELEASE m [, m ...].\n\
                 END MATRIX.\n\
                 Matrices: {1, 2; 3, 4} (commas between elements, semicolons between rows; the parts may be\n\
                 matrices), a:b and a:b:step for whole numbers, m(i, j) and v(i) with vectors of positions or\n\
                 : for all.\n\
                 Operators from the tightest: unary -, :, ** (matrix power) and &**, * (matrix product), /\n\
                 &* and &/ (elementwise), + -, < <= > >= = <> (or LT LE GT GE EQ NE), NOT, AND, OR and XOR;\n\
                 a scalar goes with any matrix. PRINT, WRITE, SAVE and MSAVE take a name, a call, a matrix in\n\
                 braces or a power of one; other expressions go in parentheses, as (A+B). In the expression of\n\
                 a subcommand a slash starts the next subcommand, so a division goes in brackets.\n\
                 Functions: ABS ALL ANY ARSIN ARTAN BLOCK CHOL CMAX CMIN COS CSSQ CSUM DESIGN DET DIAG EOF EVAL\n\
                 EXP GINV GRADE GSCH IDENT INV KRONEKER LG10 LN MAGIC MAKE MDIAG MMAX MMIN MOD MSSQ MSUM NCOL\n\
                 NROW RANK RESHAPE RMAX RMIN RND RNKORDER RSSQ RSUM SIN SOLVE SQRT SSCP SVAL SWEEP T (TRANSPOS)\n\
                 TRACE TRUNC; UNIFORM(rows, cols) and NORMAL(rows, cols) give random matrices, UNIFORM(max) and\n\
                 NORMAL(sd) random elements; CDFNORM CHICDF FCDF TCDF PROBIT and the families CDF.x, IDF.x,\n\
                 PDF.x, NCDF.x, NPDF.x, SIG.x and RV.x work element by element, a scalar argument going with any\n\
                 matrix.\n\
                 PRINT gives a table; without FORMAT the numbers share one format and are divided by a power\n\
                 of 10, named in the caption, when they are very large or small. A string element holds up to\n\
                 8 bytes and prints with an A format. Every loop, also one with an index, stops after SET\n\
                 MXLOOPS iterations (default 40) with a warning. The first READ, WRITE, SAVE and MSAVE name\n\
                 their file; later ones use the last one named. SAVE, MSAVE and WRITE write their files at END\n\
                 MATRIX; SAVE /OUTFILE=* replaces the active dataset then. An error stops the program at its\n\
                 statement, and nothing is saved.",
        example: "MATRIX.\nCOMPUTE A = {4, 2; 2, 3}.\nCALL EIGEN(A, V, E).\nPRINT E /TITLE='Eigenvalues'.\nPRINT INV(A) /FORMAT=F8.3.\nEND MATRIX.",
        parse: matrix::matrix_command,
    },
    CommandSpec {
        name: "GET",
        summary: "Read a data file and make it the active dataset.",
        syntax: "GET FILE='path' | handle [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...] [/MAP].\n\
                 KEEP, DROP and RENAME work in the order written, as in ADD FILES. MAP lists the variables as the\n\
                 options before it leave them, each with its name in the file; it may come as often as you like.",
        example: "GET FILE='survey.sav'.",
        parse: commands::get,
    },
    CommandSpec {
        name: "SAVE",
        summary: "Write the active dataset to a data file.",
        syntax: "SAVE OUTFILE='path' | handle [/COMPRESSED | /UNCOMPRESSED | /ZCOMPRESSED] [/KEEP=varlist] [/DROP=varlist]\n\
                 \x20    [/RENAME=(old = new) ...] [/MAP].\n\
                 KEEP, DROP and RENAME shape the saved file only; the active dataset stays as it is. MAP lists the\n\
                 variables of the file as the options before it leave them, each with its name in the active\n\
                 dataset; it may come as often as you like.\n\
                 COMPRESSED compresses case by case, ZCOMPRESSED in zlib blocks, UNCOMPRESSED not at all;\n\
                 without one a file named .zsav gets zlib blocks and any other the case by case form.",
        example: "SAVE OUTFILE='survey_clean.sav' /COMPRESSED.",
        parse: commands::save,
    },
    CommandSpec {
        name: "VARIABLE LABELS",
        summary: "Set the labels of variables.",
        syntax: "VARIABLE LABELS var 'label' [[/] var 'label' ...].",
        example: "VARIABLE LABELS age 'Age in years' / region 'Home region'.",
        parse: commands::variable_labels,
    },
    CommandSpec {
        name: "VALUE LABELS",
        summary: "Replace the value labels of variables.",
        syntax: "VALUE LABELS varlist value 'label' [value 'label' ...] [/ varlist ...].\n\
                 Values are numbers or quoted strings. A varlist with no labels removes its labels.",
        example: "VALUE LABELS gender 1 'Male' 2 'Female' / plan 'b' 'Basic' 'p' 'Premium'.",
        parse: commands::value_labels,
    },
    CommandSpec {
        name: "MISSING VALUES",
        summary: "Declare user-missing values.",
        syntax: "MISSING VALUES varlist (values) [[/] varlist (values) ...].\n\
                 values: up to three values, or one range with at most one extra value.\n\
                 range: low THRU high, LOWEST THRU high, low THRU HIGHEST (LO and HI also work).\n\
                 Empty parentheses clear the missing values.",
        example: "MISSING VALUES q1 q2 (98, 99) / income (LOWEST THRU -1, 999999) / plan ().",
        parse: commands::missing_values,
    },
    CommandSpec {
        name: "VARIABLE LEVEL",
        summary: "Set the measurement level of variables.",
        syntax: "VARIABLE LEVEL varlist (SCALE | ORDINAL | NOMINAL) [[/] varlist (...) ...].",
        example: "VARIABLE LEVEL age (SCALE) / q1 TO q5 (ORDINAL).",
        parse: commands::variable_level,
    },
    CommandSpec {
        name: "VARIABLE WIDTH",
        summary: "Set how many characters wide variables show in the data grid.",
        syntax: "VARIABLE WIDTH varlist (n) [[/] varlist (n) ...].\n\
                 n: a whole number of characters, at least 1. Saved with the data; formats and output stay as they are.",
        example: "VARIABLE WIDTH comment (40) / id age (5).",
        parse: commands::variable_width,
    },
    CommandSpec {
        name: "FORMATS",
        summary: "Set the display format of variables.",
        syntax: "FORMATS varlist (format) [[/] varlist (format) ...].\n\
                 format: type, width and optional decimals, for example F8.2, A20, DATE11.\n\
                 Output shows values in the format: DATE11 28-OCT-1990, DATE9 28-OCT-90, ADATE10 10/28/1990,\n\
                 TIME8 01:02:34, TIME5 01:02 (what the width cannot show is cut off), CCA to CCE as SET defines.\n\
                 Procedures show the mean, median, mode, minimum, maximum and percentiles of a date, time or\n\
                 custom currency variable in its format; a spread of dates as days and time (DTIME).",
        example: "FORMATS income (COMMA10.2) / age (F3).",
        parse: commands::formats,
    },
    CommandSpec {
        name: "FREQUENCIES",
        summary: "Frequency tables, summary statistics and percentiles.",
        syntax: "FREQUENCIES [/]VARIABLES=varlist [/FORMAT=[NOTABLE | LIMIT(n)] [AVALUE | DVALUE | AFREQ | DFREQ]]\n\
                 \x20           [/MISSING=INCLUDE] [/STATISTICS=[stat ...]] [/PERCENTILES=p ...] [/NTILES=n]\n\
                 \x20           [/GROUPED=varlist [(width) | (boundary, ...)]] [/ORDER=ANALYSIS | VARIABLE]\n\
                 \x20           [/BARCHART=[MIN(n)] [MAX(n)] [FREQ[(n)] | PERCENT[(n)]]]\n\
                 \x20           [/PIECHART=[MIN(n)] [MAX(n)] [FREQ | PERCENT] [MISSING | NOMISSING]]\n\
                 \x20           [/HISTOGRAM=[MIN(n)] [MAX(n)] [FREQ[(n)]] [NORMAL | NONORMAL]].\n\
                 stat: MEAN SEMEAN MEDIAN MODE STDDEV VARIANCE SKEWNESS SESKEW KURTOSIS SEKURT RANGE MINIMUM\n\
                 MAXIMUM SUM DEFAULT ALL NONE. STATISTICS alone or DEFAULT is MEAN STDDEV MINIMUM MAXIMUM; /STATS works too.\n\
                 A frequency table per variable: valid values, then missing ones, with percent, valid percent and\n\
                 cumulative percent. NOTABLE leaves the tables out, LIMIT(n) the tables with more than n values.\n\
                 DVALUE, AFREQ and DFREQ order the values by descending value or by frequency (ties ascending by\n\
                 value); they are ignored when percentiles or a histogram are asked for. A Statistics table, one for\n\
                 all variables (ORDER=VARIABLE: one per variable, before its frequency table), shows the valid and\n\
                 missing counts and the statistics; it appears when statistics or percentiles are asked for, or when\n\
                 a frequency table is left out. Percentiles use (W + 1) p, the median is the 50th percentile and is\n\
                 left out with AFREQ and DFREQ; MODE shows the smallest of tied modes, with a note. NTILES=n adds the\n\
                 n - 1 points that cut the cases into n equal groups (n up to 1000); several PERCENTILES and NTILES\n\
                 add up.\n\
                 GROUPED treats the values as groups for the median and percentiles: alone the values are group\n\
                 midpoints, (width) groups that wide around each value, (boundaries) the given groups; a value\n\
                 outside the boundaries falls in an open group, where percentiles are undefined (shown empty).\n\
                 INCLUDE makes user-missing values valid. Weighted when a weight variable is set; a case with a\n\
                 missing, zero or negative weight is left out. String variables get the table and the counts only.\n\
                 Charts: one per variable for each chart subcommand; BARCHART and HISTOGRAM exclude each other.\n\
                 BARCHART has a bar per valid value, in the order of the table: counts, or with PERCENT shares of\n\
                 the valid cases. PIECHART has a slice per valid value plus one for all missing values (none with\n\
                 NOMISSING): counts, or with PERCENT shares of the whole pie. HISTOGRAM (numeric variables) counts\n\
                 the valid values in equal bins: Sturges' number of bins, a width of 1, 2 or 5 times a power of\n\
                 ten (at least 1 for whole values); NORMAL adds the normal curve of all valid values. MIN(n) and\n\
                 MAX(n) leave out the values outside; the n of FREQ(n) or PERCENT(n) is the top of the axis. Of\n\
                 FREQ and PERCENT, MISSING and NOMISSING, NORMAL and NONORMAL the first given counts. A chart\n\
                 comes as its numbers: a data table in the text output, its description with --json, and a\n\
                 Vega-Lite specification with --vega-lite.",
        example: "FREQUENCIES VARIABLES=age region /FORMAT=LIMIT(10) /STATISTICS=MEAN MEDIAN MODE /NTILES=4.",
        parse: frequencies::frequencies,
    },
    CommandSpec {
        name: "EXECUTE",
        summary: "Run the pending transformations.",
        syntax: "EXECUTE.",
        example: "EXECUTE.",
        parse: commands::execute,
    },
    CommandSpec {
        name: "COUNT",
        summary: "Count, per case, how many variables hold one of the listed values.",
        syntax: "COUNT target = varlist (values) [varlist (values) ...] [/ target = ...].\n\
                 values: values, low THRU high (with LOWEST or HIGHEST), MISSING, SYSMIS.\n\
                 A value or range counts user-missing values too; a string variable takes quoted values only.",
        example: "COUNT channels = q5_1 TO q5_5 (1) / unanswered = q1 q2 (MISSING).",
        parse: data_commands::count,
    },
    CommandSpec {
        name: "SORT CASES",
        summary: "Sort the cases by the values of variables.",
        syntax: "SORT CASES [BY] varlist [(A | D)] [varlist [(A | D)] ...].\n\
                 A is ascending (the default), D descending; a direction covers the variables before it.\n\
                 System-missing sorts lowest. Cases with equal keys keep their order.",
        example: "SORT CASES BY region (A) age (D).",
        parse: data_commands::sort_cases,
    },
    CommandSpec {
        name: "SORT VARIABLES",
        summary: "Put the variables in order by a property of their dictionary entries.",
        syntax: "SORT VARIABLES [BY] NAME | TYPE | FORMAT | LABEL | VALUES | MISSING | MEASURE | ROLE | COLUMNS\n\
                 \x20              | ALIGNMENT | ATTRIBUTE name [(A | D)].\n\
                 A or UP is ascending (the default), D or DOWN reverses the whole order.\n\
                 NAME: letters without regard to case, then a number at the end by its value (v2 before v10).\n\
                 TYPE: numeric, then strings from narrow to wide. FORMAT: by format type in the order of the\n\
                 data file codes (A, COMMA, DOLLAR, F, ... DATE ...), then width and decimals.\n\
                 LABEL, VALUES, MISSING, ATTRIBUTE: variables without one come first; then labels and\n\
                 attribute values without regard to case; value labels and missing values so that equal\n\
                 definitions sit together. MEASURE: nominal, ordinal, scale. ROLE: input, target, both, none,\n\
                 partition, split. COLUMNS: display width (default 8 for numbers, the width for strings).\n\
                 ALIGNMENT: left, right, center. Variables that tie keep their order.",
        example: "SORT VARIABLES BY NAME (A).",
        parse: restructure::sort_variables,
    },
    CommandSpec {
        name: "RENAME VARIABLES",
        summary: "Change the names of variables.",
        syntax: "RENAME VARIABLES (oldlist = newlist) [(oldlist = newlist) ...].\n\
                 RENAME VARIABLES old = new [old = new ...].\n\
                 All renames happen at once, so (a b = b a) swaps two names. new1 TO new3 counts up.",
        example: "RENAME VARIABLES (q1 q2 = satisfaction recommend) (v1 TO v3 = item1 TO item3).",
        parse: data_commands::rename_variables,
    },
    CommandSpec {
        name: "DELETE VARIABLES",
        summary: "Remove variables from the dataset.",
        syntax: "DELETE VARIABLES varlist.",
        example: "DELETE VARIABLES tmp1 tmp2 q7_1 TO q7_9.",
        parse: data_commands::delete_variables,
    },
    CommandSpec {
        name: "WEIGHT",
        summary: "Weight the cases by a variable, or turn weighting off.",
        syntax: "WEIGHT BY var.\n\
                 WEIGHT OFF.",
        example: "WEIGHT BY weight.",
        parse: data_commands::weight,
    },
    CommandSpec {
        name: "AGGREGATE",
        summary: "Aggregate groups of cases into new variables or a new dataset.",
        syntax: "AGGREGATE [/OUTFILE=* | 'path' | dataset [MODE=ADDVARIABLES | REPLACE]] [/MISSING=COLUMNWISE] [/PRESORTED]\n\
                 \x20         [/BREAK=varlist [(A | D)] ...]\n\
                 \x20         /target ['label'] [target ...] = function(varlist) [/target ...].\n\
                 OUTFILE=* replaces the active dataset with one case per break group (break variables, then\n\
                 targets); with MODE=ADDVARIABLES it keeps the cases and adds the targets to each of them.\n\
                 OUTFILE='path' writes the aggregated dataset to a file and leaves the active dataset alone;\n\
                 OUTFILE=dataset puts it into a dataset that DATASET DECLARE or DATASET NAME defined.\n\
                 Without OUTFILE the targets are added to every case, as with OUTFILE=* MODE=ADDVARIABLES.\n\
                 functions: SUM MEAN SD MEDIAN MIN MAX FIRST LAST N NU NMISS NUMISS; N and NU also without (varlist).\n\
                 PGT PLT FGT FLT CGT CLT (varlist, value) and PIN POUT FIN FOUT CIN COUT (varlist, low, high): the\n\
                 percentage, fraction or count of the valid values above, below, inside (bounds included) or\n\
                 outside (bounds excluded); bounds the wrong way round are swapped, strings compare as text.\n\
                 Missing values are skipped; with MISSING=COLUMNWISE a group with one missing value gets a missing\n\
                 result (N, NU, NMISS and NUMISS still count). PRESORTED keeps the cases in file order and makes a\n\
                 group of each run of adjacent cases with the same break values. With a weight variable SUM,\n\
                 MEAN, SD, MEDIAN, N and NMISS are weighted.",
        example: "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=region /region_mean = MEAN(income) /region_n = N.",
        parse: data_commands::aggregate,
    },
    CommandSpec {
        name: "VARSTOCASES",
        summary: "Turn groups of variables into cases: one new case per variable of the group.",
        syntax: "VARSTOCASES /MAKE new ['label'] [FROM] varlist [/MAKE ...]\n\
                 \x20           [/INDEX=i ['label'] | i ['label'] (make) | i ['label'] (n) j ['label'] (n) ...]\n\
                 \x20           [/ID=new ['label']] [/NULL=DROP | KEEP] [/COUNT=new ['label']]\n\
                 \x20           [/KEEP=ALL | varlist | /DROP=varlist].\n\
                 Every case becomes one case per variable of the FROM lists, in their order; FROM lists\n\
                 have equal lengths and one type each. A MAKE variable takes the dictionary of its first\n\
                 FROM variable (strings their widest width), the label given replacing the label. The other\n\
                 variables are copied to every new case (KEEP: only those, in that order). INDEX numbers the\n\
                 new cases 1, 2, ...; with (make) it holds the names of that MAKE's FROM variables; with (n)\n\
                 several factors whose levels multiply to the length of the lists, the last varying fastest.\n\
                 ID is the case number of the original case (F8.0), COUNT the number of new cases it gave\n\
                 (F4.0). NULL=DROP (the default) leaves out a new case whose MAKE values are all\n\
                 system-missing or blank. Order of the new file: ID, the copied variables, COUNT, INDEX,\n\
                 MAKE; ID and INDEX are nominal, COUNT scale. All cases are used whatever FILTER says;\n\
                 WEIGHT, SPLIT FILE and FILTER stay unless their variables are gone. Not after TEMPORARY.",
        example: "VARSTOCASES /MAKE score 'Score' FROM score1 TO score4 /INDEX=wave /ID=person.",
        parse: restructure::vars_to_cases,
    },
    CommandSpec {
        name: "CASESTOVARS",
        summary: "Turn groups of cases into one case each, with a variable per value of an index.",
        syntax: "CASESTOVARS [/ID=varlist] [/INDEX=varlist] [/FIXED=varlist] [/AUTOFIX=YES | NO]\n\
                 \x20           [/VIND [ROOT=name]] [/COUNT=new ['label']] [/RENAME var=root ...]\n\
                 \x20           [/SEPARATOR='text'] [/GROUPBY=VARIABLE | INDEX] [/DROP=varlist].\n\
                 A row group is a run of cases with the same values of the split and ID variables; the\n\
                 data must be sorted by them. Cases with a system-missing or blank ID are left out with a\n\
                 warning. Every other variable that varies within a group becomes a variable group: one\n\
                 variable per index value (INDEX: the values or combinations that occur, ascending; without\n\
                 INDEX the position in the group), named root, separator ('.') and the index values. With\n\
                 one string index the values are the names (root, separator and value when several\n\
                 variables are restructured); names that cannot be used become V1, V2, ... AUTOFIX=YES\n\
                 copies variables that do not vary (one value plus system-missing or blanks counts as not\n\
                 varying), and warns about FIXED variables that vary; AUTOFIX=NO makes such a FIXED\n\
                 variable an error. New variables keep the dictionary of their source; the label adds the\n\
                 index values. VIND adds a 0/1 indicator (F1.0) per index value, COUNT the cases of the\n\
                 group (F4.0). Order: split, ID and fixed variables as in the file, the variable groups,\n\
                 the indicators, COUNT. User-missing values count as values. All cases are used whatever\n\
                 FILTER says; WEIGHT, SPLIT FILE and FILTER stay unless their variables are gone or\n\
                 spread. Not after TEMPORARY.",
        example: "CASESTOVARS /ID=person /INDEX=wave /GROUPBY=VARIABLE.",
        parse: restructure::cases_to_vars,
    },
    CommandSpec {
        name: "FLIP",
        summary: "Transpose the data: variables become cases and cases become variables.",
        syntax: "FLIP [VARIABLES=ALL | varlist] [/NEWNAMES=var].\n\
                 Each variable becomes a case, named in the new string variable CASE_LBL; each case becomes a\n\
                 numeric variable (F8.2), named VAR001, VAR002, ... or by the values of NEWNAMES (the values\n\
                 of CASE_LBL after an earlier FLIP). Names are upper case; a number gets K_ in front, as does a\n\
                 name that does not start with a letter; other characters a name cannot hold become _, and a\n\
                 repeated name gets _A, _B, ...; a missing or blank value keeps the default name. String\n\
                 values and user-missing values become system-missing. Every case is transposed whatever\n\
                 FILTER says. Labels, value labels, the weight, the split and the filter are gone.\n\
                 Transformations before FLIP become permanent: TEMPORARY ends. A table lists the new names.",
        example: "FLIP VARIABLES=income price year /NEWNAMES=country.",
        parse: restructure::flip,
    },
    CommandSpec {
        name: "SPLIT FILE",
        summary: "Run each procedure once per run of consecutive cases that share the split values.",
        syntax: "SPLIT FILE [LAYERED | SEPARATE] BY varlist.\n\
                 SPLIT FILE OFF.\n\
                 A group ends where a split variable changes value, so sort the cases first.\n\
                 LAYERED (the default) shows the groups in one table, with the split variables as the\n\
                 outermost row labels; SEPARATE gives every group tables of its own. AGGREGATE ignores\n\
                 the split; SORT CASES turns it off. The split is kept with the dataset.",
        example: "SPLIT FILE SEPARATE BY region.",
        parse: selection::split_file,
    },
    CommandSpec {
        name: "FILTER",
        summary: "Leave out of procedures the cases whose filter value is 0 or missing.",
        syntax: "FILTER BY var.\n\
                 FILTER OFF.\n\
                 The cases stay in the dataset. The filter is kept with the dataset, follows a rename\n\
                 and ends when the variable is deleted or the dataset replaced.",
        example: "FILTER BY adult.",
        parse: selection::filter,
    },
    CommandSpec {
        name: "TEMPORARY",
        summary: "Make the changes that follow last only until the next procedure.",
        syntax: "TEMPORARY.\n\
                 Transformations, labels, formats, SELECT IF, SAMPLE, N OF CASES, WEIGHT, FILTER and\n\
                 SPLIT FILE after it are undone once the following data pass (a procedure, EXECUTE or\n\
                 SAVE) is over, or at the end of the run. Variables that procedure adds stay.",
        example: "TEMPORARY.",
        parse: selection::temporary,
    },
    CommandSpec {
        name: "N OF CASES",
        summary: "Keep only the first n cases from the next data pass on.",
        syntax: "N OF CASES n.\n\
                 The limit applies after SELECT IF and SAMPLE, wherever they stand in the pass.",
        example: "N OF CASES 100.",
        parse: selection::n_of_cases,
    },
    CommandSpec {
        name: "N",
        summary: "Short for N OF CASES.",
        syntax: "N n.",
        example: "N 100.",
        parse: selection::n_of_cases,
    },
    CommandSpec {
        name: "SAMPLE",
        summary: "Keep a random sample of the cases.",
        syntax: "SAMPLE proportion.\n\
                 SAMPLE n FROM m.\n\
                 A proportion between 0 and 1 keeps each case with that chance; n FROM m keeps exactly n\n\
                 of the first m cases (fewer when there are fewer than m). Inside DO IF only the cases of\n\
                 the branch are sampled. The random numbers come from the generator SET RNG chooses, started\n\
                 by SET SEED or MTINDEX; a new session starts from the default seed.",
        example: "SAMPLE 500 FROM 3420.",
        parse: selection::sample,
    },
    CommandSpec {
        name: "BOOTSTRAP",
        summary: "Give the next procedure bootstrap bias, standard errors and confidence intervals.",
        syntax: "BOOTSTRAP [/SAMPLING METHOD=SIMPLE | STRATIFIED(STRATA=varlist)]\n\
                 \x20 [/VARIABLES [TARGET=varlist] [INPUT=varlist]]\n\
                 \x20 [/CRITERIA [CILEVEL=95 | level] [CITYPE=PERCENTILE | BCA] [NSAMPLES=1000 | n]]\n\
                 \x20 [/MISSING [USERMISSING=EXCLUDE | INCLUDE]].\n\
                 Keywords are written in full. The next procedure runs on the data and then again on n\n\
                 resamples drawn with replacement (within each stratum for STRATIFIED), with the generator\n\
                 SET SEED or MTINDEX starts. Cases with a missing value in a TARGET, INPUT or STRATA variable\n\
                 leave the analysis; user-missing values of nominal and ordinal variables count as valid\n\
                 with USERMISSING=INCLUDE. DESCRIPTIVES, FREQUENCIES, MEANS, CORRELATIONS, T-TEST,\n\
                 REGRESSION, CROSSTABS, EXAMINE, ONEWAY, NONPAR CORR, PARTIAL CORR, LOGISTIC REGRESSION,\n\
                 NOMREG, PLUM, UNIANOVA and GLM add Bias, Std. Error and the interval (percentile or BCa)\n\
                 to their main statistics; any other procedure warns and runs without resampling. RESIDUAL\n\
                 and WILD resampling are not supported yet.",
        example: "BOOTSTRAP /VARIABLES INPUT=height weight /CRITERIA CITYPE=BCA NSAMPLES=2000.",
        parse: bootstrap::bootstrap,
    },
    CommandSpec {
        name: "AUTORECODE",
        summary: "Recode values into consecutive integers in new variables, labeled with the old values.",
        syntax: "AUTORECODE [VARIABLES=] varlist /INTO newlist [/BLANK=VALID | MISSING] [/GROUP]\n\
                 \x20          [/DESCENDING] [/PRINT].\n\
                 Valid values get 1, 2, ... in ascending order (DESCENDING: descending), user-missing values\n\
                 the codes after them, still user-missing; system-missing stays missing. Each code is\n\
                 labeled with the value label or the value. GROUP uses one scheme for all variables,\n\
                 BLANK=MISSING makes a blank string user-missing. All cases are recoded, also those a\n\
                 filter leaves out. APPLY TEMPLATE and SAVE TEMPLATE are not supported yet.",
        example: "AUTORECODE VARIABLES=country /INTO country_code /PRINT.",
        parse: data_commands::autorecode,
    },
    CommandSpec {
        name: "RANK",
        summary: "Ranks, percentile groups, proportion estimates, normal and Savage scores as new variables.",
        syntax: "RANK [VARIABLES=]varlist [(A | D)] [varlist [(A | D)] ...] [BY varlist]\n\
                 \x20    [/RANK] [/RFRACTION] [/PERCENT] [/PROPORTION] [/N] [/NTILES(k)] [/SAVAGE] [/NORMAL]\n\
                 \x20    [/TIES=MEAN | LOW | HIGH | CONDENSE] [/FRACTION=BLOM | RANKIT | TUKEY | VW]\n\
                 \x20    [/PRINT=YES | NO] [/MISSING=EXCLUDE | INCLUDE].\n\
                 Each function, given at most once and followed by INTO names if wished, adds one new variable per\n\
                 ranked variable; without any, RANK is done. A (ascending, the default) or D (the largest value first)\n\
                 applies to the variables before it back to the previous A or D. BY ranks within each combination of\n\
                 the BY values; a missing BY value forms a group of its own. With the sum of weights W: RFRACTION is\n\
                 the rank r divided by W, PERCENT that times 100, N is W, NTILES(k) the group 1 to k, the integer part\n\
                 of r k / (W + 1) plus 1. PROPORTION follows FRACTION: BLOM (r - 3/8) / (W + 1/4) (the default),\n\
                 RANKIT (r - 1/2) / W, TUKEY (r - 1/3) / (W + 1/3), VW r / (W + 1); NORMAL is its standard normal\n\
                 quantile and SAVAGE the exponential score. TIES gives tied values the mean (default), lowest or\n\
                 highest rank; CONDENSE ranks the distinct values 1, 2, 3. A missing value (user-missing unless\n\
                 INCLUDE) or a missing, zero or negative weight gives system-missing. Names after INTO are used in\n\
                 order; the others are the function's first letter and the first seven characters of the name, then\n\
                 the function's first three letters and 001, 002 when taken, then RNK, two letters and 01. Labels read\n\
                 RANK of salary (by the BY variables). PRINT=NO leaves out the table of the new variables. Within\n\
                 SPLIT FILE groups the ranks start over.",
        example: "RANK VARIABLES=salary (D) BY region /RANK /NTILES(4) INTO salary_q /TIES=LOW.",
        parse: rank::rank,
    },
    CommandSpec {
        name: "COMPUTE",
        summary: "Assign the value of an expression to a variable.",
        syntax: "COMPUTE target = expression.",
        example: "COMPUTE bmi = weight / (height / 100) ** 2.",
        parse: commands::compute,
    },
    CommandSpec {
        name: "IF",
        summary: "Assign a value only for the cases where a condition holds.",
        syntax: "IF (condition) target = expression.\n\
                 The parentheses around the condition are optional.",
        example: "IF (age >= 65 AND region = 2) senior_south = 1.",
        parse: commands::if_command,
    },
    CommandSpec {
        name: "SELECT IF",
        summary: "Keep only the cases where a condition holds.",
        syntax: "SELECT IF condition.",
        example: "SELECT IF age >= 18 AND NOT MISSING(q1).",
        parse: commands::select_if,
    },
    CommandSpec {
        name: "RECODE",
        summary: "Map values of variables to new values.",
        syntax: "RECODE varlist (inputs = output) [(inputs = output) ...] [INTO newvars] [/ varlist ...].\n\
                 inputs: values, low THRU high (with LOWEST or HIGHEST), MISSING, SYSMIS, ELSE.\n\
                 (CONVERT), for a string into a numeric target: at its place, a string that reads as a number\n\
                 becomes that number and a blank one system-missing; other strings go on to the next (inputs).\n\
                 output: a value, SYSMIS or COPY.\n\
                 The first matching (inputs = output) wins. A value or range also matches user-missing values.\n\
                 Without a match a variable keeps its value, and so does an existing INTO target; a new target becomes system-missing.\n\
                 A string target of INTO must exist already. A quoted value longer than its variable is an error.",
        example: "RECODE age (LOWEST THRU 29 = 1) (30 THRU 59 = 2) (60 THRU HIGHEST = 3) INTO age_group.",
        parse: commands::recode,
    },
    CommandSpec {
        name: "STRING",
        summary: "Declare new string variables, filled with empty strings.",
        syntax: "STRING names (Awidth) [/ names (Awidth) ...].\n\
                 A string variable must exist before COMPUTE or IF can assign text to it.",
        example: "STRING full_name (A40) / code1 code2 (A8).",
        parse: commands::string,
    },
    CommandSpec {
        name: "NUMERIC",
        summary: "Declare new numeric variables, filled with system-missing.",
        syntax: "NUMERIC names [(format)] [/ names [(format)] ...].\n\
                 Without a format the variables get F8.2.",
        example: "NUMERIC score1 score2 (F5.1) / flag.",
        parse: commands::numeric,
    },
    CommandSpec {
        name: "DO IF",
        summary: "Start a block of transformations that run only where a condition holds.",
        syntax: "DO IF (condition).\n\
                 Followed by COMPUTE, IF, RECODE or nested DO IF commands, then optional\n\
                 ELSE IF (condition). and ELSE. branches, then END IF.\n\
                 For each case the first branch whose condition is true runs. After a missing\n\
                 condition no later branch runs for that case.",
        example: "DO IF (age < 18).",
        parse: commands::do_if,
    },
    CommandSpec {
        name: "ELSE IF",
        summary: "Start the next conditional branch of a DO IF block.",
        syntax: "ELSE IF (condition).",
        example: "ELSE IF (age < 65).",
        parse: commands::else_if,
    },
    CommandSpec {
        name: "ELSE",
        summary: "Start the branch of a DO IF block for the cases no condition matched.",
        syntax: "ELSE.",
        example: "ELSE.",
        parse: commands::else_command,
    },
    CommandSpec {
        name: "END IF",
        summary: "End a DO IF block.",
        syntax: "END IF.",
        example: "END IF.",
        parse: commands::end_if,
    },
    CommandSpec {
        name: "DO REPEAT",
        summary: "Repeat transformations with stand-in names replaced by variables or values.",
        syntax: "DO REPEAT standin = varlist | values [/ standin = varlist | values ...].\n\
                 \x20 transformation commands\n\
                 END REPEAT [PRINT].\n\
                 Every list must have as many items; the commands run once per item, each stand-in\n\
                 replaced by its item. A list holds existing variables (TO in file order, ALL) or new\n\
                 names (v1 TO v5), or numbers and quoted strings (1 TO 5 counts up). PRINT shows the\n\
                 generated commands.",
        example: "DO REPEAT r = region1 TO region5 / v = 1 TO 5.\nCOMPUTE r = (region = v).\nEND REPEAT.",
        parse: loops::do_repeat,
    },
    CommandSpec {
        name: "LOOP",
        summary: "Repeat transformations for each case, with an index, a condition or both.",
        syntax: "LOOP [var = first TO last [BY step]] [IF condition].\n\
                 \x20 transformation commands\n\
                 END LOOP [IF condition].\n\
                 The index runs from first to last by step (1 by default); the expressions may differ\n\
                 per case, a missing one skips the loop. IF on LOOP is checked before each iteration,\n\
                 IF on END LOOP after it. A loop without an index stops after SET MXLOOPS iterations\n\
                 (default 40). BREAK leaves the innermost loop.",
        example: "LOOP #i = 1 TO 5.\nCOMPUTE total = total + #i.\nEND LOOP.",
        parse: loops::loop_command,
    },
    CommandSpec {
        name: "END LOOP",
        summary: "End a LOOP, optionally when a condition holds.",
        syntax: "END LOOP [IF condition].",
        example: "LOOP.\nCOMPUTE x = x + 1.\nEND LOOP IF x >= 10.",
        parse: loops::end_loop,
    },
    CommandSpec {
        name: "BREAK",
        summary: "Leave the innermost LOOP, usually inside DO IF.",
        syntax: "BREAK.",
        example: "LOOP #i = 1 TO 10.\nDO IF v(#i) = 0.\nBREAK.\nEND IF.\nEND LOOP.",
        parse: loops::break_command,
    },
    CommandSpec {
        name: "VECTOR",
        summary: "Name a range of variables, or new variables, as a vector for indexed use.",
        syntax: "VECTOR name = first TO last [/ ...].\n\
                 VECTOR prefix [, prefix ...] (n [format]) [/ ...].\n\
                 The first form refers to existing variables, all numeric or all strings of one width;\n\
                 the second creates prefix1 to prefixn (F8.2, or the format given). v(i) reads an element\n\
                 in expressions and is a target of COMPUTE and IF. A vector lasts until the next procedure.",
        example: "VECTOR q = q1 TO q10 / new (5, F2).",
        parse: loops::vector,
    },
    CommandSpec {
        name: "CTABLES",
        summary: "Custom tables: counts, percentages and statistics by categories.",
        syntax: "CTABLES [/FORMAT EMPTY=ZERO|BLANK|'text' MISSING='text']\n\
                 \x20       [/VLABELS VARIABLES=varlist DISPLAY=DEFAULT|NAME|LABEL|BOTH|NONE]\n\
                 \x20       [/MRSETS COUNTDUPLICATES=NO|YES]\n\
                 \x20       [/HIDESMALLCOUNTS [COUNT=5]] [/WEIGHT VARIABLE=varname] [/SMISSING VARIABLE|LISTWISE]\n\
                 \x20       [/PCOMPUTE &name=EXPR(expression)]\n\
                 \x20       [/PPROPERTIES &name ... [LABEL='text'] [FORMAT=function format ...] [HIDESOURCECATS=NO|YES]]\n\
                 \x20       /TABLE [rows] [BY [columns] [BY layers]]\n\
                 \x20       [/SLABELS POSITION=COLUMN|ROW|LAYER VISIBLE=YES|NO]\n\
                 \x20       [/CLABELS AUTO|ROWLABELS=OPPOSITE|LAYER|COLLABELS=OPPOSITE|LAYER] [/CRITERIA CILEVEL=95]\n\
                 \x20       [/CATEGORIES VARIABLES=varlist [ORDER=A|D] [KEY=VALUE|LABEL|function[(var[, n])]]\n\
                 \x20                    [MISSING=EXCLUDE|INCLUDE]\n\
                 \x20                    [TOTAL=NO|YES] [LABEL='text'] [POSITION=AFTER|BEFORE] [EMPTY=INCLUDE|EXCLUDE]]\n\
                 \x20       [/CATEGORIES VARIABLES=varlist [value, low THRU high, OTHERNM, MISSING, &name,\n\
                 \x20                    SUBTOTAL='label', HSUBTOTAL='label', ...] [TOTAL=YES ...]]\n\
                 \x20       [/TITLES TITLE='text' ['text' ...] CAPTION='text' ... CORNER='text' ...]\n\
                 \x20       [/SIGTEST TYPE=CHISQUARE [ALPHA=0.05] [INCLUDEMRSETS=YES|NO] [CATEGORIES=ALLVISIBLE|SUBTOTALS]]\n\
                 \x20       [/COMPARETEST TYPE=PROP|MEAN [ALPHA=0.05 [0.01]] [ADJUST=BONFERRONI|BH|NONE]\n\
                 \x20                     [MEANSVARIANCE=ALLCATS|TESTEDCATS] [SHOWSIG=NO|YES] [INCLUDEMRSETS=YES|NO]\n\
                 \x20                     [CATEGORIES=ALLVISIBLE|SUBTOTALS] [MERGE=NO|YES [STYLE=APA|SIMPLE]]\n\
                 \x20                     [ORIGIN=COLUMN]]\n\
                 \x20       [/TABLE ... another table with its own SLABELS, CATEGORIES, TITLES and tests].\n\
                 rows, columns: variables joined by + (stack) and > (nest), with parentheses. var [C] or var [S]\n\
                 makes a variable categorical or scale; otherwise SCALE is scale and the other levels categorical.\n\
                 A numeric variable of unknown level is scale with a currency, date or time format, a fraction, a\n\
                 negative value, 24 or more distinct values, or no value below 10; else categorical.\n\
                 /TABLE BY columns leaves the rows empty: one row without a label. Layers (rows BY columns BY\n\
                 layers, or rows BY BY layers) give a table for every layer category,\n\
                 for every combination of nested layer variables and every category of stacked ones; its title\n\
                 names the layer. Within a layer the percentages are of that layer (LAYERPCT: of all its cells;\n\
                 LAYERROWPCT and LAYERCOLPCT: across its subtables), except TABLEPCT, which spans all layers.\n\
                 SLABELS POSITION=LAYER makes every summary a layer of its own. Tests run in every layer.\n\
                 A summary list follows a scale variable or the innermost categorical variable of one axis:\n\
                 var [function ['label'] [format], ...], such as PCT8.1. Formats: F, PCT, COMMA, DOT (period and\n\
                 comma swapped), DOLLAR, N (leading zeros), E (scientific), and NEGPAREN (negative numbers in\n\
                 parentheses), NEQUAL (N= before), PAREN (in parentheses), PCTPAREN (percent in parentheses);\n\
                 these four show the number as F in the width and add their marks outside it; CCA to CCE and\n\
                 the date and time formats. By default a summary of a scale variable has the variable's format,\n\
                 but for dates: STDDEV, SEMEAN and RANGE as days and time (DTIME), SUM and VARIANCE plain;\n\
                 and VARIANCE of a time variable plain. A function given\n\
                 twice in a list is two columns, each with its own format.\n\
                 After a parenthesis, (a + b > c) [COUNT], the summaries go to a and c, before their own ones.\n\
                 TITLES: several quoted texts are several lines. )DATE in them is today's date (YYYY-MM-DD),\n\
                 )TIME the time (HH:MM:SS), both local; )TABLE the table expression without levels and\n\
                 summaries, with variable and set labels in place of names: Gender > Age BY Region.\n\
                 TOTALS [function ...] inside the list gives the totals and subtotals of that categorical\n\
                 variable their own summaries (the columns hold both lists); a function of scale variables there,\n\
                 such as MEAN or MEDIAN, summarizes the variable's own values: var [COUNT TOTALS [MEAN]].\n\
                 CLABELS ROWLABELS=OPPOSITE moves the categories of the innermost row variables to the columns,\n\
                 inside the column categories and outside the summary labels, and COLLABELS=OPPOSITE the other\n\
                 way; the moved variable counts as the innermost one of its new axis, also for percentages.\n\
                 ROWLABELS=LAYER or COLLABELS=LAYER makes a layer of every category instead; the moved variable\n\
                 then counts as a layer variable.\n\
                 Stacked variables whose labels move need the same categories with the same labels; tests are\n\
                 not available then.\n\
                 Default: COUNT for categorical tables, MEAN for a scale variable.\n\
                 Categorical: COUNT, ROWPCT, COLPCT, TABLEPCT, SUBTABLEPCT, LAYERPCT, LAYERROWPCT, LAYERCOLPCT,\n\
                 each with .COUNT, .VALIDN or .TOTALN (COLPCT.COUNT and so on); COLPCT alone means COLPCT.COUNT.\n\
                 Scale: also MEAN MEDIAN MODE MINIMUM MAXIMUM RANGE SUM STDDEV VARIANCE SEMEAN PTILE n VALIDN TOTALN\n\
                 MISSING and the percentages with .SUM. A U prefix (UCOUNT, UMEAN) ignores the weight.\n\
                 SMISSING LISTWISE: a case missing on one scale variable of a table is missing on all of them\n\
                 (a table: the sections with the same categorical variables); it still counts in COUNT and TOTALN.\n\
                 Confidence limits and standard errors: a .LCL, .UCL or .SE suffix (COUNT.LCL, COLPCT.COUNT.UCL,\n\
                 MEAN.SE); .LCL and .UCL for COUNT, the percentages of COUNT, VALIDN or TOTALN, MEAN, MEDIAN, PTILE\n\
                 and SUM, .SE for COUNT, those percentages, MEAN and SUM; not for sets or with a U prefix.\n\
                 CRITERIA CILEVEL (after its /TABLE, default 95) sets the level. A proportion has the Jeffreys\n\
                 interval (Beta quantiles of w + 0.5 and W - w + 0.5, weights rounded); a count is the table's\n\
                 weight times the interval of its share of the table; a mean or sum the t interval; a percentile\n\
                 the percentiles at the limits of the share at or below it.\n\
                 /WEIGHT VARIABLE=var (before the first /TABLE) weights every table by an effective base weight,\n\
                 in place of the WEIGHT command's variable. ECOUNT (also for categorical variables and sets),\n\
                 EVALIDN and ETOTALN are then the effective bases (sum of weights)^2 / (sum of squared weights)\n\
                 of COUNT, VALIDN and TOTALN, and SEMEAN uses the variance adjusted to the effective base;\n\
                 without /WEIGHT they equal COUNT, VALIDN and TOTALN. The tests then round nothing: the\n\
                 chi-square is multiplied by the subtable's weight over its sum of squared weights, and the\n\
                 column tests are t-tests on the effective bases, comparing two categories when both have a\n\
                 case and one of them two.\n\
                 Weighted by the weight variable when one is set; fractional weights are not rounded, and a case\n\
                 whose weight is zero, negative or missing is left out (also from U functions). MEDIAN and PTILE\n\
                 use the averaged empirical distribution. A count without cases is 0 unless EMPTY says otherwise.\n\
                 A total has percentages too: the base of a total row or column takes in every category totaled.\n\
                 In an explicit list OTHERNM stands for every valid value the list does not name, in ascending\n\
                 order, and MISSING for every user-missing value; a value named twice takes its last place.\n\
                 KEY=function sorts the categories of a list without explicit values by that summary of each,\n\
                 taken over all cases of the category whatever the other variables (COUNT, or MEAN(age),\n\
                 PTILE(age, 75) for a variable); ORDER=D sorts from the largest, and ties keep value order.\n\
                 SUBTOTAL='label' in an explicit list sums the listed categories before it, back to the previous\n\
                 subtotal (with POSITION=BEFORE the ones after it, up to the next); HSUBTOTAL also hides them.\n\
                 A subtotal counts a case once, like a total, and has every summary and percentage a total has.\n\
                 A computed category &name stands in an explicit category list of a categorical variable. For\n\
                 every summary its value is EXPR over the same summary of other categories: [value], [low THRU\n\
                 high] (the sum of the listed categories in the range), TOTAL (needs TOTAL=YES), numbers, + - * /\n\
                 ** and parentheses, SUBTOTAL (the only subtotal) or SUBTOTAL[n] (the nth), [OTHERNM] and\n\
                 [MISSING] (the sums of those categories). Every category it names must be listed. PPROPERTIES gives its label (else\n\
                 the expression; )LABEL[n] in it is the label of the nth item of the list as written, a range as\n\
                 'low THRU high'), a format per summary, and HIDESOURCECATS=YES to hide the categories it uses\n\
                 (not with tests). Totals and tests leave computed categories out.\n\
                 HIDESMALLCOUNTS COUNT=n (n a whole number from 2, default 5) shows every COUNT, VALIDN, TOTALN or\n\
                 RESPONSES below n, weighted or with U, as <n in all tables of the command.\n\
                 User-missing categories are left out unless MISSING=INCLUDE; system-missing ones always are.\n\
                 A case whose missing value is left out still counts in TOTALN and the .TOTALN percentages; a\n\
                 case whose valid value an explicit category list leaves out is not in the table at all.\n\
                 Tests run in every innermost subtable (innermost row by innermost column variable, for each\n\
                 category of the outer ones), each in a table after the main one. SIGTEST: Pearson chi-square; empty\n\
                 rows and columns are left out. Tests compare the shown categories, with an HSUBTOTAL in place of\n\
                 the categories it hides; CATEGORIES=SUBTOTALS puts every subtotal in place of its categories.\n\
                 COMPARETEST: two-sided pairwise z-tests of column proportions (PROP)\n\
                 or t-tests of column means (MEAN: a scale variable with MEAN in the rows) in every row of a subtable.\n\
                 The letter of the column with the smaller value appears in the column with the larger one: uppercase\n\
                 below the smaller ALPHA, lowercase below the larger; a period marks a column left out (a proportion\n\
                 of 0 or 1, or less than 2 cases). ADJUST defaults to BONFERRONI, MEANSVARIANCE to ALLCATS. With a\n\
                 weight variable the tests round the weighted cell counts, column totals or category weights.\n\
                 MERGE=YES puts the letters in the main table instead, in a column after each compared category\n\
                 (in the row of MEAN, or of COLPCT or COUNT, when the summaries are in the rows). STYLE=APA (the\n\
                 default) gives letters that two columns share unless they differ significantly at the smaller\n\
                 ALPHA; STYLE=SIMPLE gives the keys of the separate table. SHOWSIG=YES keeps the separate table.\n\
                 A multiple response set ($name, see MRSETS) stands where a categorical variable may. A case counts\n\
                 once in every category it has a response in; the set's total counts the cases with any response.\n\
                 A dichotomy set lists its variables in set order; /CATEGORIES may list them by name: [q5_3, q5_1].\n\
                 A set may carry the summaries even with categorical variables nested inside it; of two nested sets\n\
                 only the inner one may. Sets also have RESPONSES and, for every percentage base above,\n\
                 base.RESPONSES (responses over the area's responses), base.RESPONSES.COUNT (responses over the\n\
                 area's count) and base.COUNT.RESPONSES (count over the area's responses). A case without any\n\
                 response counts only in TOTALN and the .TOTALN percentages. COUNTDUPLICATES=YES counts a value that\n\
                 a case gives in two variables of a category set twice in the responses; counts never do.\n\
                 Tests on sets (INCLUDEMRSETS=YES, the default) count each case once in the margins and allow for the\n\
                 cases two categories of a set share; the means test then pools the variance from the two categories\n\
                 compared. INCLUDEMRSETS=NO leaves sets out of the tests, and so does COUNTDUPLICATES=YES for a\n\
                 category set.",
        example: "CTABLES /TABLE q1 [COUNT, COLPCT.COUNT PCT8.1] BY gender /CATEGORIES VARIABLES=q1 TOTAL=YES.",
        parse: ctables::ctables,
    },
    CommandSpec {
        name: "MRSETS",
        summary: "Define, delete and list multiple response sets.",
        syntax: "MRSETS [/]MDGROUP NAME=$name [LABEL='text' | LABELSOURCE=VARLABEL] [CATEGORYLABELS=VARLABELS | COUNTEDVALUES]\n\
                 \x20      VARIABLES=varlist VALUE=value\n\
                 \x20      /MCGROUP NAME=$name [LABEL='text'] VARIABLES=varlist\n\
                 \x20      /DELETE NAME=[$name ...] | ALL\n\
                 \x20      /DISPLAY NAME=[$name ...] | ALL.\n\
                 Subcommands run in the order written and may repeat. A set name starts with $; defining an\n\
                 existing name replaces that set. A set has at least two variables, all numeric or all strings.\n\
                 MDGROUP (dichotomies): every variable is one category, labeled with its variable label or name; a\n\
                 case has a response in it when the variable holds VALUE, a whole number or, for strings, a quoted\n\
                 text no wider than the narrowest variable. CATEGORYLABELS=COUNTEDVALUES labels each category with\n\
                 the value label of VALUE in its variable instead; every variable needs one and they must differ.\n\
                 LABELSOURCE=VARLABEL (only with COUNTEDVALUES, instead of LABEL) takes the set label from the first\n\
                 variable of the set that has a variable label, else the name of the first variable.\n\
                 MCGROUP (categories): the values of the variables are\n\
                 the categories, labeled with the value labels of the first variable that has one.\n\
                 DISPLAY lists the sets in a table. Sets are saved with the data file and used by CTABLES.",
        example: "MRSETS /MDGROUP NAME=$channels LABEL='Contact channels' VARIABLES=q5_1 TO q5_5 VALUE=1 /DISPLAY NAME=ALL.",
        parse: mrsets::mrsets,
    },
    CommandSpec {
        name: "DESCRIPTIVES",
        summary: "Summary statistics of numeric variables, optionally saved as z-scores.",
        syntax: "DESCRIPTIVES [/]VARIABLES=varlist [/STATISTICS=stat ...] [/SORT=[key] [(A | D)]]\n\
                 \x20            [/MISSING=VARIABLE | LISTWISE [INCLUDE]] [/SAVE].\n\
                 stat: MEAN SEMEAN STDDEV VARIANCE KURTOSIS SKEWNESS RANGE MINIMUM MAXIMUM SUM DEFAULT ALL.\n\
                 DEFAULT is MEAN STDDEV MINIMUM MAXIMUM; N is always shown. Weighted when a weight variable is set.\n\
                 SORT lists the variables by key (MEAN SEMEAN STDDEV VARIANCE KURTOSIS SKEWNESS RANGE MIN MAX SUM\n\
                 or NAME), ascending (A, the default) or descending (D); without a key it is the mean. A variable\n\
                 without the statistic goes last. The key need not be among the statistics shown.\n\
                 MISSING=VARIABLE (the default) leaves out the missing values of each variable on its own;\n\
                 LISTWISE leaves out a case missing on any listed variable; INCLUDE counts user-missing values.\n\
                 The row Valid N (listwise) counts the cases valid on every listed variable. String variables get no\n\
                 statistics; a warning names them. var (name) in the list adds a z-score under that name; SAVE adds\n\
                 one for every variable, named Z plus the variable name (ZSC001 to ZSC099, STDZ01 to STDZ09,\n\
                 ZZZZ01 to ZZZZ09, ZQZQ01 to ZQZQ09 when that name is taken), labeled Zscore: plus the label.",
        example: "DESCRIPTIVES VARIABLES=age income (z_inc) /STATISTICS=DEFAULT SUM /SAVE.",
        parse: procedures::descriptives,
    },
    CommandSpec {
        name: "CROSSTABS",
        summary: "Contingency tables with percentages, residuals and chi-square tests.",
        syntax: "CROSSTABS [/]TABLES=rowvars BY colvars [BY layervars ...] [/TABLES=...]\n\
                 \x20         [/VARIABLES=varlist (min, max) [varlist (min, max) ...]  (before TABLES)]\n\
                 \x20         [/CELLS=COUNT ROW COLUMN TOTAL EXPECTED RESID SRESID ASRESID ALL NONE]\n\
                 \x20         [/STATISTICS=CHISQ PHI CC LAMBDA UC BTAU CTAU GAMMA D ETA CORR KAPPA RISK MCNEMAR CMH[(1)] ALL NONE] [/MISSING=TABLE | INCLUDE | REPORT]\n\
                 \x20         [/FORMAT=AVALUE | DVALUE TABLES | NOTABLES]\n\
                 \x20         [/COUNT=[ASIS | CASE | CELL] [ROUND | TRUNCATE]] [/BARCHART]\n\
                 \x20         [/WRITE[=NONE | CELLS | ALL]]\n\
                 \x20         [/METHOD=MC [CIN(99)] [SAMPLES(10000)] | EXACT [TIMER(5)]].\n\
                 First a Case Processing Summary: per table the valid, missing and total cases (weighted).\n\
                 One table per pair of row and column variable; layer variables split it into blocks.\n\
                 NOTABLES leaves out the tables, as CELLS=NONE does, keeping the statistics.\n\
                 COUNT makes weighted counts whole: CASE every case's weight, CELL every cell's count, before\n\
                 anything is computed; ROUND or TRUNCATE. A keyword left out is CELL or ROUND; without COUNT,\n\
                 ASIS, weights stay as they are.\n\
                 CELLS defaults to COUNT; CELLS alone is COUNT ROW COLUMN TOTAL. Percentages are 0 to 100.\n\
                 STATISTICS alone is CHISQ. DVALUE orders the rows from the highest value; columns stay ascending.\n\
                 CHISQ: Pearson, likelihood ratio, linear-by-linear (numeric variables), and for a 2 x 2\n\
                 table Fisher's exact test and the continuity correction.\n\
                 PHI (phi and Cramer's V), CC (contingency coefficient), BTAU, CTAU (Kendall's tau-b, tau-c),\n\
                 GAMMA and CORR (Spearman and Pearson, numeric variables only) go in a Symmetric Measures table;\n\
                 LAMBDA (lambda and Goodman and Kruskal's tau), UC (uncertainty coefficient), D (Somers' d) and\n\
                 ETA (numeric variables) in a Directional Measures table. MCNEMAR adds to the chi-square table the exact McNemar test (2 x 2) or the\n\
                 McNemar-Bowker test (larger), for rows and columns with the same values. KAPPA works on\n\
                 the square table over the values of both variables (same type), unseen pairs counted 0. RISK\n\
                 gives a Risk Estimate table for 2 x 2 tables: the odds ratio and the risk for each column\n\
                 category, with 95% intervals. CMH treats the layers as strata of 2 x 2 tables: Breslow-Day\n\
                 and Tarone (homogeneity), Cochran and Mantel-Haenszel (conditional independence), and the\n\
                 common odds ratio with its 95% interval, tested against the value in brackets. Each measure\n\
                 gives its value, asymptotic standard error, approximate T and significance; ALL asks for\n\
                 every statistic (CMH with 1); a table with one row or column gets no measures.\n\
                 MISSING=TABLE leaves out a case missing on any variable of the table; INCLUDE keeps\n\
                 user-missing values as categories. Fisher's test takes whole counts: rounded, or truncated with TRUNCATE.\n\
                 BARCHART adds a clustered bar chart per table: a cluster per row category, a bar per column\n\
                 category, the counts of the table summed over its layers.\n\
                 WRITE puts one record per cell in the file named by the PROCEDURE OUTPUT before the command, after\n\
                 an Output File Summary table: split-file group (from 1), table number and count as whole numbers\n\
                 in columns 1-16, then the row, column and control values in 8-column fields (whole numbers in\n\
                 integer mode, else as the variable prints, text to the left), a column of the table at a time,\n\
                 the last control variable changing slowest. CELLS (also a bare WRITE) writes the non-empty cells\n\
                 without a missing value, or with them under MISSING=INCLUDE or REPORT; ALL (integer mode only)\n\
                 every cell of the ranges. A long string variable or more than eight dimensions is an error.\n\
                 FORMAT also reads BOX NOBOX INDEX NOINDEX PIVOT NOPIVOT, which shaped the old text listing and\n\
                 change nothing, with one warning.\n\
                 VARIABLES switches to integer mode: a range follows each list (it covers the whole list) and\n\
                 values are cut to whole numbers. Rows and columns are every value of the range, with or\n\
                 without cases; a value outside it makes the case missing. The statistics use the values\n\
                 that occur. TABLES then names only variables of VARIABLES, and TO and ALL follow their order there.\n\
                 MISSING=REPORT (integer mode only) shows the user-missing values of the rows and columns\n\
                 marked (missing): their cells hold counts, nothing else, and stay out of totals, percentages\n\
                 and statistics; the cases count as missing.\n\
                 METHOD adds significances computed from the tables that share the observed row and column\n\
                 totals, next to the asymptotic ones, on whole counts (cells rounded, or truncated with\n\
                 TRUNCATE). EXACT walks all such tables with a network of partial tables: the chi-square\n\
                 table gets the exact two-sided significance and the point probability of Pearson, the\n\
                 likelihood ratio, Fisher's test (any size) and the linear-by-linear association (also\n\
                 one-sided); the measures tables an Exact Significance column for phi, Cramer's V, the\n\
                 contingency coefficient, lambda (not the symmetric value), Goodman and Kruskal's tau, the\n\
                 uncertainty coefficient, Kendall's tau-b and tau-c, Somers' d, Spearman, Pearson's R and\n\
                 kappa. TIMER(minutes) bounds each statistic (0: no bound); one that runs over, or would\n\
                 need more memory than an eighth of what is free on the computer (at least 256 MB, at most 8 GB; less under a memory limit of a container), keeps only its asymptotic value, with a warning that gives the amount. MC instead draws SAMPLES random tables with those\n\
                 totals from the SET SEED generator and shows the share at least as extreme with its\n\
                 interval at CIN percent. Gamma, eta, McNemar-Bowker and CMH stay asymptotic.",
        example: "CROSSTABS /TABLES=region BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.",
        parse: procedures::crosstabs,
    },
    CommandSpec {
        name: "MULT RESPONSE",
        summary: "Frequency tables and crosstabulations of multiple response groups and variables.",
        syntax: "MULT RESPONSE [/]GROUPS=name ['label'] (varlist (value)) | (varlist (min, max)) [name ...]\n\
                 \x20             [/VARIABLES=varlist (min, max) [varlist (min, max) ...]]\n\
                 \x20             [/FREQUENCIES=items]\n\
                 \x20             [/TABLES=items BY items [BY items ...] [(PAIRED)] [/items BY ...]]\n\
                 \x20             [/MISSING=[TABLE | MDGROUP | MRGROUP] [INCLUDE]]\n\
                 \x20             [/FORMAT=[LABELS | NOLABELS] [TABLE | CONDENSE | ONEPAGE] [DOUBLE]]\n\
                 \x20             [/BASE=CASES | RESPONSES] [/CELLS=[COUNT] [ROW] [COLUMN] [TOTAL] [ALL]].\n\
                 GROUPS, VARIABLES, FREQUENCIES and TABLES come in this order, before the others; at least\n\
                 one of GROUPS and VARIABLES and one of FREQUENCIES and TABLES. One value makes a dichotomy\n\
                 group (its categories are the variables with that value), two a category group (the values\n\
                 from min to max, counted over all its variables). Groups exist only in this command.\n\
                 items: groups, and variables named on VARIABLES; TO follows their order there. Values are\n\
                 truncated to whole numbers; values out of range do not count. Empty categories are left out.\n\
                 A case is missing for a group with no counted value; MDGROUP and MRGROUP leave out a case\n\
                 missing on any variable of a dichotomy or category group (MRGROUP also for a value out of\n\
                 range); INCLUDE counts user-missing values. FREQUENCIES: responses, percent of responses\n\
                 and percent of cases. TABLES: counts, and the percentages CELLS asks for (ALL without\n\
                 keywords), based on cases or with BASE=RESPONSES on responses. PAIRED pairs the first\n\
                 variable of a category group with the first of another, and so on; its percentages are\n\
                 based on responses. NOLABELS and CONDENSE show values for category groups and variables;\n\
                 ONEPAGE and DOUBLE only change printed layout and are ignored with a warning. Weights count\n\
                 as they are.",
        example: "MULT RESPONSE GROUPS=media 'Media used' (tv radio web (1))\n\
                  \x20 brands 'Brands known' (brand1 TO brand3 (1, 9))\n\
                  \x20 /VARIABLES=gender (1, 2)\n\
                  \x20 /FREQUENCIES=media brands\n\
                  \x20 /TABLES=media brands BY gender\n\
                  \x20 /CELLS=COLUMN.",
        parse: mult_response::mult_response,
    },
    CommandSpec {
        name: "MEANS",
        summary: "Statistics of dependent variables within groups of control variables.",
        syntax: "MEANS [TABLES=]deps [BY controls [BY ...]] [/deps BY ...] [/CELLS=cell ...]\n\
                 \x20     [/STATISTICS=ANOVA | LINEARITY | NONE] [/MISSING=TABLE | INCLUDE | DEPENDENT].\n\
                 cell: MEAN COUNT STDDEV MEDIAN GMEDIAN SEMEAN SUM MIN MAX RANGE VARIANCE KURT SEKURT SKEW\n\
                 SESKEW FIRST LAST NPCT SPCT NPCT(var) SPCT(var) HARMONIC GEOMETRIC DEFAULT ALL; DEFAULT is MEAN\n\
                 COUNT STDDEV. GMEDIAN is the grouped median: each value is the midpoint of its interval.\n\
                 NPCT(var) and SPCT(var) give the percent of the N and of the sum of the cases in the same\n\
                 category of var, which must be a control of every table list (all cases on its Total rows).\n\
                 One Report table per dependent and combination of controls: every group of the first control,\n\
                 the next nested inside, each level with a Total. A Case Processing Summary per table list.\n\
                 A string dependent gets COUNT, FIRST and LAST only. ANOVA gives the analysis of variance by the\n\
                 first control with eta; LINEARITY adds the linear term and r (three or more numeric groups).\n\
                 Without /MISSING each dependent with its controls leaves out its own missing cases; any /MISSING\n\
                 leaves out cases missing on any variable of the list (INCLUDE: user-missing values count;\n\
                 DEPENDENT: they count for the controls only). Weighted when a weight variable is set.",
        example: "MEANS TABLES=income BY region BY gender /CELLS=MEAN COUNT STDDEV MEDIAN /STATISTICS=ANOVA.",
        parse: stats::means,
    },
    CommandSpec {
        name: "SUMMARIZE",
        summary: "Statistics of dependents within groups of control variables, optionally listing the cases.",
        syntax: "SUMMARIZE [TABLES=]deps [BY controls [BY ...]] [/[TABLES=]deps BY ...] [/TITLE='text']\n\
                 \x20         [/FOOTNOTE='text'] [/CELLS=cell ... | NONE]\n\
                 \x20         [/MISSING=[EXCLUDE | INCLUDE | DEPENDENT] [VARIABLE | TABLE]]\n\
                 \x20         [/FORMAT=[NOLIST | LIST [LIMIT=n] | VALIDLIST [LIMIT=n]] [CASENUM | NOCASENUM]\n\
                 \x20                  [TOTAL | NOTOTAL] [MISSING='text']]\n\
                 \x20         [/STATISTICS=ANOVA | LINEARITY | ALL | NONE].\n\
                 cell: as on MEANS; DEFAULT (MEAN COUNT STDDEV) when CELLS is left out, ALL for every cell.\n\
                 Each table list gives a Case Processing Summary and one Case Summaries table per combination of\n\
                 controls (one from each BY): the groups of the first control with the next nested inside, each\n\
                 level ending in a Total, one row per cell statistic and one column per dependent. LIST adds the\n\
                 cases of each innermost group, numbered unless NOCASENUM; VALIDLIST lists only cases valid on\n\
                 every dependent; LIMIT=n works on the first n cases only. LIST and VALIDLIST allow one table\n\
                 list. NOTOTAL keeps the total rows without their label. MISSING='text' replaces the period of a\n\
                 system-missing value in the listing. VARIABLE (default) drops a case from the dependents it is\n\
                 missing on, TABLE from all of them when it is missing on any. A case missing on a control is\n\
                 always dropped; INCLUDE counts user-missing values as valid, DEPENDENT only for controls. A\n\
                 string dependent gets COUNT, FIRST, LAST and NPCT only. ANOVA and LINEARITY work as on MEANS,\n\
                 by each control of the first BY. TITLE and FOOTNOTE (\\n breaks a line) name the summary\n\
                 tables. A subcommand given twice: the last one counts. Weighted when a weight variable is set.",
        example: "SUMMARIZE TABLES=income spend BY region /FORMAT=LIST LIMIT=20 NOCASENUM\n\
                  \x20 /CELLS=COUNT MEAN MAX /TITLE='Spending by region'.",
        parse: summarize::summarize,
    },
    CommandSpec {
        name: "OLAP CUBES",
        summary: "Statistics of scale variables in nested groups, with differences between groups or variables.",
        syntax: "OLAP CUBES summaryvars BY groupvars [BY groupvars ...] [/CELLS=cell ... | NONE]\n\
                 \x20          [/CREATE ['label' | DEFAULT ...] GAC | GPC | GAC GPC (groupvar (value value) ...)]\n\
                 \x20          [/CREATE ['label' | DEFAULT ...] VAC | VPC | VAC VPC (var var) ...]\n\
                 \x20          [/TITLE='text'] [/FOOTNOTE='text'] [/HIDESMALLCOUNTS [COUNT=5]].\n\
                 cell: as on MEANS; DEFAULT (SUM COUNT MEAN STDDEV SPCT NPCT) when CELLS is left out, ALL for\n\
                 every cell. The summary variables must be numeric. A Case Processing Summary has a row per\n\
                 summary variable and group combination; one OLAP Cubes table per combination of grouping\n\
                 variables (one from each BY) nests the categories of the first one outermost, each level with a\n\
                 Total, and gives a row per summary variable. GAC adds a row per pair of categories of one\n\
                 grouping variable with every statistic of the second minus that of the first; GPC the same\n\
                 change as a percentage of the first. VAC and VPC do the same between two summary variables in\n\
                 every group. Labels name the differences in order (each pair gives its types in the order\n\
                 written); DEFAULT keeps the generated name. A case counts for a summary variable when it is\n\
                 valid on it and on the grouping variables of the table. HIDESMALLCOUNTS shows counts below n\n\
                 (from 2, default 5) as <n. TITLE and FOOTNOTE: one line per quoted string, \\n breaks a line.\n\
                 A subcommand given twice: the last one counts. Weighted when a weight variable is set.",
        example: "OLAP CUBES revenue cost BY year BY store /CELLS=SUM MEAN COUNT\n\
                  \x20 /CREATE 'Growth' DEFAULT GAC GPC (year (2024 2025)) /TITLE='Store results'.",
        parse: olap_cubes::olap_cubes,
    },
    CommandSpec {
        name: "T-TEST",
        summary: "One-sample, independent-samples and paired-samples t tests with effect sizes.",
        syntax: "T-TEST /TESTVAL=value /VARIABLES=varlist\n\
                 T-TEST /GROUPS=var[(value) | (value, value)] /VARIABLES=varlist\n\
                 T-TEST /PAIRS=varlist [WITH varlist [(PAIRED)]] [/varlist ...]\n\
                 \x20      [/CRITERIA=CI(0.95)] [/MISSING=ANALYSIS | LISTWISE [INCLUDE]]\n\
                 \x20      [/ES DISPLAY(TRUE | FALSE) STANDARDIZER(SD | CORRECTED_SD | AVERAGE)].\n\
                 GROUPS without values compares 1 with 2; one value splits at it (at or above against below).\n\
                 The independent test shows Levene's test and rows for equal and unequal variances. PAIRS without\n\
                 WITH pairs every variable with every later one; WITH pairs each left with each right one, and\n\
                 (PAIRED) the n-th with the n-th. ANALYSIS (default) leaves out cases missing in each analysis,\n\
                 LISTWISE those missing on any variable of the test type. Effect sizes: Cohen's d and Hedges'\n\
                 correction (and Glass's delta for groups) with confidence intervals; they use weights rounded to\n\
                 whole numbers. Weighted when a weight variable is set.",
        example: "T-TEST GROUPS=gender(1,2) /VARIABLES=age income /CRITERIA=CI(.90).",
        parse: stats::t_test,
    },
    CommandSpec {
        name: "ONEWAY",
        summary: "One-way analysis of variance with contrasts and post hoc comparisons.",
        syntax: "ONEWAY deps BY factor [/STATISTICS=DESCRIPTIVES EFFECTS HOMOGENEITY WELCH BROWNFORSYTHE ALL NONE]\n\
                 \x20      [/CONTRAST=coefficients] [/POLYNOMIAL=degree] [/POSTHOC=test ... [ALPHA(0.05)]]\n\
                 \x20      [/RANGES=LSD | DUNCAN | SNK | TUKEYB | TUKEY | MODLSD | SCHEFFE [(alpha)]] ...\n\
                 \x20      [/MISSING=ANALYSIS | LISTWISE [INCLUDE | EXCLUDE]] [/CRITERIA=CILEVEL(0.95)] [/PLOT MEANS].\n\
                 test: LSD BONFERRONI SIDAK TUKEY SCHEFFE GT2 GABRIEL GH T2 T3 C (pairwise comparisons),\n\
                 DUNNETT DUNNETTL DUNNETTR [(refcat)] (each group against the reference group, by default the last;\n\
                 L: smaller, R: larger), SNK BTUKEY DUNCAN FREGW QREGW (homogeneous subsets only); TUKEY SCHEFFE\n\
                 GT2 GABRIEL give both. DUNCAN and the Dunnett keywords are written in full. Every value of the\n\
                 factor is a group. CONTRAST gives one coefficient per group in ascending order (n*c repeats c n\n\
                 times); it is tested with pooled and separate variances. HOMOGENEITY gives Levene's test four ways, one row\n\
                 each under Based on Mean, Median, Median and with adjusted df, and trimmed mean. Multiple Comparisons show every ordered pair of groups (Dunnett C has no significance).\n\
                 Homogeneous subsets use the harmonic mean of the group sizes (the R-E-G-W tests the sizes\n\
                 themselves); Tukey B shows no significance. Weighted when a weight variable is set. PLOT MEANS\n\
                 gives each dependent a line chart through the mean of every group, as chart data.\n\
                 POLYNOMIAL (1 to 5, the last one given counts; lowered with a warning to the groups minus 1)\n\
                 splits the between-groups sum of squares into trend terms in the ANOVA table, with the group\n\
                 value as the metric: a Weighted row per degree (sequential, the groups weighted by size) and a\n\
                 Deviation row of what is left (none after the last possible degree); when the group sizes differ\n\
                 and the groups are equally spaced there is also an Unweighted row per degree.\n\
                 RANGES names one of these post hoc tests (MODLSD, also LSDMOD, is Bonferroni), at most 10 times;\n\
                 its tests add to those of POSTHOC and the last alpha given anywhere counts for all of them.\n\
                 EFFECTS adds the rows Fixed Effects (pooled standard deviation, and the error and interval of the\n\
                 overall mean from the within mean square) and Random Effects (error and interval from the\n\
                 between-groups component, which has its own column) to the Descriptives table; without DESCRIPTIVES\n\
                 it holds only those two rows. A negative component counts as 0 in the error, with a warning.\n\
                 Not supported yet: WALLER.",
        example: "ONEWAY income BY region /STATISTICS=DESCRIPTIVES HOMOGENEITY WELCH /POSTHOC=TUKEY GH.",
        parse: stats::oneway,
    },
    CommandSpec {
        name: "CORRELATIONS",
        summary: "Pearson correlations with significance and case counts.",
        syntax: "CORRELATIONS [/VARIABLES=]varlist [WITH varlist] [/varlist ...]\n\
                 \x20            [/PRINT=TWOTAIL | ONETAIL  SIG | NOSIG  FULL | LOWER | LNODIAG]\n\
                 \x20            [/STATISTICS=DESCRIPTIVES XPROD ALL] [/MISSING=PAIRWISE | LISTWISE [INCLUDE]].\n\
                 A list alone gives a square matrix, WITH a rectangular one (left variables in rows). NOSIG marks\n\
                 coefficients significant at 0.05 with * and at 0.01 with **. XPROD adds cross-products and\n\
                 covariances; DESCRIPTIVES a table of means, standard deviations and counts. PAIRWISE (default)\n\
                 uses the cases valid on both variables of a pair; LISTWISE the cases valid on every variable\n\
                 of every list. Weighted when a weight variable is set.",
        example: "CORRELATIONS /VARIABLES=age income WITH satisfaction /PRINT=ONETAIL NOSIG.",
        parse: stats::correlations,
    },
    CommandSpec {
        name: "NONPAR CORR",
        summary: "Spearman's rho and Kendall's tau-b rank correlations with significance.",
        syntax: "NONPAR CORR [/VARIABLES=]varlist [WITH varlist] [/varlist ...]\n\
                 \x20           [/PRINT=SPEARMAN | KENDALL | BOTH  TWOTAIL | ONETAIL  SIG | NOSIG\n\
                 \x20                   FULL | LOWER | LNODIAG] [/MISSING=PAIRWISE | LISTWISE [INCLUDE]] [/SAMPLE].\n\
                 A list alone gives a square matrix, WITH a rectangular one (left variables in rows). Ties get\n\
                 their average rank; rho is tested with t on N - 2 df, tau-b with the normal approximation and\n\
                 the tie-corrected variance. NOSIG marks coefficients significant at 0.05 with * and at 0.01\n\
                 with **. PAIRWISE (default) uses the cases valid on both variables of a pair; LISTWISE the\n\
                 cases valid on every variable of the list, each list on its own. A weight variable repeats\n\
                 each case as often as its weight rounded to a whole number. SAMPLE changes nothing.\n\
                 Not supported yet: MATRIX, CI.",
        example: "NONPAR CORR /VARIABLES=satisfaction recommend WITH age /PRINT=BOTH NOSIG.",
        parse: stats::nonpar_corr,
    },
    CommandSpec {
        name: "PARTIAL CORR",
        summary: "Partial correlations of variables controlling for others, with significance.",
        syntax: "PARTIAL CORR [VARIABLES=]varlist [WITH varlist] BY controls [(order ...)] [/...]\n\
                 \x20            [/SIGNIFICANCE=TWOTAIL | ONETAIL] [/STATISTICS=NONE CORR DESCRIPTIVES BADCORR ALL]\n\
                 \x20            [/FORMAT=MATRIX | CONDENSED | SERIAL] [/MISSING=LISTWISE | ANALYSIS  EXCLUDE | INCLUDE].\n\
                 A list alone gives a square matrix, WITH a rectangular one. Without orders the partials control\n\
                 for every control variable at once; an order k gives one block per set of k controls (up to five\n\
                 orders, 25 lists). Coefficients come from the zero-order Pearson correlations step by step, and are\n\
                 tested with t on M - k - 2 df, M the smallest weighted count of the correlations used. A coefficient\n\
                 that cannot be computed is empty. CORR shows the zero-order correlations first, BADCORR only when one\n\
                 of them is empty; DESCRIPTIVES the mean, standard deviation and N. CONDENSED shows the coefficients\n\
                 alone, marked * at 0.05 and ** at 0.01; SERIAL shows each pair once. LISTWISE (default) uses the cases\n\
                 valid on every variable of the list, controls included; ANALYSIS each pair's valid cases. Weighted\n\
                 when a weight variable is set. Not supported yet: MATRIX.",
        example: "PARTIAL CORR /VARIABLES=satisfaction recommend BY age /STATISTICS=CORR /SIGNIFICANCE=ONETAIL.",
        parse: npar::partial_corr,
    },
    CommandSpec {
        name: "NPAR TESTS",
        summary: "Nonparametric tests: one-sample, two and k related samples, two and k independent samples.",
        syntax: "NPAR TESTS [/CHISQUARE=varlist [(lo,hi)] [/EXPECTED=EQUAL | f1, n*f2, ...]]\n\
                 \x20          [/BINOMIAL[(p)]=varlist[(cut) | (value1,value2)]] [/RUNS(MEAN | MEDIAN | MODE | value)=varlist]\n\
                 \x20          [/K-S(NORMAL [mean,sd] | UNIFORM [min,max] | POISSON [mean] | EXPONENTIAL [mean])=varlist]\n\
                 \x20          [/WILCOXON | /SIGN | /MCNEMAR | /MH=varlist [WITH varlist [(PAIRED)]]]\n\
                 \x20          [/FRIEDMAN | /KENDALL | /COCHRAN=varlist]\n\
                 \x20          [/M-W | /K-S | /W-W | /MOSES[(n)] | /K-W | /J-T | /MEDIAN[(value)]=varlist BY var(value1,value2)]\n\
                 \x20          [/STATISTICS=[DESCRIPTIVES] [QUARTILES] [ALL]] [/MISSING=ANALYSIS | LISTWISE [INCLUDE]]\n\
                 \x20          [/METHOD=EXACT [TIMER(n)] | MC [CIN(n)] [SAMPLES(n)]] [/SAMPLE] [/KS_SIM ...].\n\
                 Long names work too: CHI-SQUARE, KOLMOGOROV-SMIRNOV, MANN-WHITNEY, WALD-WOLFOWITZ, KRUSKAL-WALLIS,\n\
                 JONCKHEERE-TERPSTRA. Without WITH every variable is paired with every later one; (PAIRED) pairs the\n\
                 n-th with the n-th. M-W, two-sample K-S, W-W and MOSES compare the two groups (MOSES: the first is the\n\
                 control group); K-W and J-T take every value from value1 to value2 as a group; MEDIAN does so when\n\
                 value1 < value2, and compares the two values otherwise. Exact significance where the method gives it\n\
                 for small samples: binomial; sign and McNemar up to 25 changes; Mann-Whitney when n1 n2 <= 400 and\n\
                 n1 n2 / 2 + min(n1, n2) <= 220; median test up to 30 cases (Fisher); W-W up to 30 cases; Moses.\n\
                 K-S with normal parameters taken from the data uses the Lilliefors significance. METHOD adds exact\n\
                 significances for Wilcoxon, sign, McNemar, Mann-Whitney and the runs tests (MC gives them too, as\n\
                 exact results replace simulated ones); other tests stay asymptotic, with a warning. ANALYSIS (default)\n\
                 leaves out cases missing on a test's variables, LISTWISE cases missing on any variable of the command.\n\
                 A weight repeats each case as often as its weight rounded to a whole number. SAMPLE and KS_SIM change\n\
                 nothing.",
        example: "NPAR TESTS /M-W=income BY gender(1,2) /WILCOXON=before WITH after /STATISTICS=QUARTILES.",
        parse: npar::npar_tests,
    },
    CommandSpec {
        name: "NPTESTS",
        summary: "Nonparametric tests picked by measurement level: one sample, independent samples, related samples.",
        syntax: "NPTESTS [/ONESAMPLE TEST(fields) [CHISQUARE[(EXPECTED=EQUAL | CUSTOM(FREQUENCIES=list CATEGORIES=list))]]\n\
                 \x20         [BINOMIAL[(TESTVALUE=p SUCCESSCATEGORICAL=FIRST | LIST(values)\n\
                 \x20                   SUCCESSCONTINUOUS=CUTPOINT(MIDPOINT | value) CLOPPERPEARSON JEFFREYS LIKELIHOOD)]]\n\
                 \x20         [KOLMOGOROV_SMIRNOV[(NSAMPLES=n MC_CILEVEL=level NORMAL=SAMPLE[(SIMULATION=TRUE | FALSE)] |\n\
                 \x20                   CUSTOM(MEAN=m SD=s) UNIFORM=SAMPLE | CUSTOM(MIN=a MAX=b)\n\
                 \x20                   EXPONENTIAL=SAMPLE | CUSTOM(MEAN=m) POISSON=CUSTOM(MEAN=m))]]\n\
                 \x20         [RUNS[(GROUPCATEGORICAL=SAMPLE | LIST(values)\n\
                 \x20                   GROUPCONTINUOUS=CUTPOINT(SAMPLEMEDIAN | SAMPLEMEAN | value))]]\n\
                 \x20         [WILCOXON(TESTVALUE=value)]]\n\
                 \x20       [/INDEPENDENT TEST(fields) GROUP(field) [MANN_WHITNEY] [WALD_WOLFOWITZ] [KOLMOGOROV_SMIRNOV]\n\
                 \x20         [HODGES_LEHMAN] [MOSES[(TRIMOUTLIERS=SAMPLE | n)]] [KRUSKAL_WALLIS[(COMPARE=c)]]\n\
                 \x20         [MEDIAN[(TESTVALUE=SAMPLE | value COMPARE=c)]] [JONCKHEERE_TERPSTRA[(ORDER=ASCENDING | DESCENDING\n\
                 \x20         COMPARE=c)]]]\n\
                 \x20       [/RELATED TEST(fields) [MCNEMAR[(SUCCESS=FIRST | LIST(values))]] [WILCOXON] [SIGN]\n\
                 \x20         [MARGINAL_HOMOGENEITY] [HODGES_LEHMAN] [COCHRAN[(SUCCESS=FIRST | LIST(values) COMPARE=c)]]\n\
                 \x20         [FRIEDMAN[(COMPARE=c)]] [KENDALL[(COMPARE=c)]]]\n\
                 \x20       [/MISSING [SCOPE=ANALYSIS | LISTWISE] [USERMISSING=EXCLUDE | INCLUDE]]\n\
                 \x20       [/CRITERIA [ALPHA=0.05] [CILEVEL=95] [SEED=2000000 | RANDOM]].\n\
                 c is PAIRWISE (the default), STEPWISE or NONE. Keywords are written out in full and each appears once.\n\
                 Nominal and ordinal fields count as categorical, scale fields as continuous; a field of unknown level\n\
                 gets one from its values. A test subcommand with only TEST (and GROUP) picks the tests itself: one\n\
                 sample, a binomial test for a categorical field with two values, chi-square for one with more, and a\n\
                 normal Kolmogorov-Smirnov test for a continuous field; independent samples, Mann-Whitney for two\n\
                 groups and Kruskal-Wallis for more; related samples, the fields split by level, then Wilcoxon (two)\n\
                 or Friedman (more) on continuous fields and McNemar (two) or Cochran (more) on categorical fields\n\
                 with two values between them. Named tests run only on fields that suit them, with a warning for\n\
                 the others: chi-square on categorical fields, K-S and Wilcoxon on continuous and ordinal ones;\n\
                 independent tests on continuous and ordinal fields, the two-group tests only when GROUP has two\n\
                 values (the lower value is the first group, the Moses control group); McNemar and Cochran on the\n\
                 nominal fields and on the ordinal fields separately; Wilcoxon, SIGN, HODGES_LEHMAN, FRIEDMAN and\n\
                 KENDALL on the continuous and ordinal fields together; MARGINAL_HOMOGENEITY on two ordinal fields.\n\
                 FIRST makes the first value met in the data the success and needs exactly two values. A continuous\n\
                 field is cut at a point: values up to it are the success (binomial) or the first group (runs).\n\
                 Binomial, sign and McNemar are exact up to 25 cases, Mann-Whitney for small groups, Wald-Wolfowitz\n\
                 up to 30 cases, the median test of two groups up to 30 cases (Fisher), Moses always. K-S with\n\
                 parameters from the data draws NSAMPLES samples (default 1000) from SEED for a Monte Carlo\n\
                 significance with a MC_CILEVEL interval; NORMAL=SAMPLE(SIMULATION=FALSE) uses the Lilliefors table.\n\
                 Jonckheere-Terpstra decides one-sided in the ORDER direction. After a rejected test of three or more\n\
                 samples, COMPARE gives pairwise comparisons (Bonferroni-adjusted) or homogeneous subsets found by\n\
                 stepwise step-down tests. Output: the hypothesis test summary (ALPHA), a confidence interval summary\n\
                 (binomial intervals and Hodges-Lehmann estimates at CILEVEL), a detail table per test and the\n\
                 comparisons. USERMISSING applies to categorical fields; SCOPE=LISTWISE drops a case missing on any\n\
                 field named. Weights are rounded to whole numbers; the weight variable cannot be tested.",
        example: "NPTESTS /ONESAMPLE TEST(score rating) /INDEPENDENT TEST(score) GROUP(site) KRUSKAL_WALLIS(COMPARE=STEPWISE) /CRITERIA ALPHA=0.01.",
        parse: nptests::nptests,
    },
    CommandSpec {
        name: "PROPORTIONS",
        summary: "Tests and confidence intervals for a proportion or a difference of two proportions.",
        syntax: "PROPORTIONS /ONESAMPLE varlist [TESTVAL=0.5] [TESTTYPES=type ...] [CITYPES=type ...]\n\
                 PROPORTIONS /PAIREDSAMPLES varlist [WITH varlist [(PAIRED)]] [TESTTYPES=...] [CITYPES=...]\n\
                 PROPORTIONS /INDEPENDENTSAMPLES varlist BY var [SELECT=LEVEL(1, 2) | MIDPOINT | CUTPOINT(value)]\n\
                 \x20           [TESTTYPES=...] [CITYPES=...]\n\
                 \x20           [/SUCCESSVALUE=LAST | FIRST | LEVEL(values) | MIDPOINT | CUTPOINT(value)]\n\
                 \x20           [/MISSING [SCOPE=ANALYSIS | LISTWISE] [USERMISSING=EXCLUDE | INCLUDE]] [/CRITERIA CILEVEL=95].\n\
                 One analysis per command. Tests, defaults marked *: one sample EXACT MIDP* SCORE* SCORECC WALD WALDCC;\n\
                 paired EXACT MIDP* MCNEMAR* MCNEMARCC WALD WALDCC; independent HAUCK_ANDERSON WALD WALDCC WALDH0*\n\
                 WALDH0CC. Intervals: one sample AGRESTI_COULL* ANSCOMBE CLOPPER_PEARSON JEFFREYS* LOGIT WALD WALDCC\n\
                 WILSON_SCORE* WILSON_SCORECC; paired AGRESTI_MIN BONETT_PRICE* NEWCOMBE* WALD WALDCC; independent\n\
                 AGRESTI_CAFFO* BROWN_LI_JEFFREYS HAUCK_ANDERSON NEWCOMBE* NEWCOMBECC WALD WALDCC. NONE turns tests or\n\
                 intervals off. TESTVAL, SELECT, TESTTYPES and CITYPES may also be written as subcommands. A case is a\n\
                 success when its value is the highest (LAST) or lowest (FIRST) valid value the analysis sees, one of\n\
                 the LEVEL values, no less than (lowest + highest) / 2 (MIDPOINT) or no less than the cut\n\
                 point; every other valid value is a failure. Groups: the two SELECT values, or the cases below the\n\
                 middle of the grouping variable's range or below the cut point as group 1 and the rest as group 2;\n\
                 without SELECT the grouping variable is numeric with groups 1 and 2. A paired list without WITH\n\
                 pairs every variable with every later one; WITH pairs every left with every right variable,\n\
                 (PAIRED) the n-th with the n-th. ANALYSIS drops a case per analysis, LISTWISE drops it from all\n\
                 when any variable named is missing. Weights count as frequencies; exact and mid-p probabilities\n\
                 use the counts rounded to whole numbers. Bounds are kept within 0 to 1 (-1 to 1 for differences).\n\
                 Two-sided p values are twice the smaller tail, at most 1. CILEVEL is a percent; a value below 1 is\n\
                 read as a fraction.",
        example: "PROPORTIONS /INDEPENDENTSAMPLES passed BY school SELECT=LEVEL('north', 'south')\n\
                  \x20 TESTTYPES=WALDH0 HAUCK_ANDERSON CITYPES=NEWCOMBE WALD /CRITERIA CILEVEL=90.",
        parse: proportions::proportions,
    },
    CommandSpec {
        name: "EXAMINE",
        summary: "Descriptives, percentiles, M-estimators, extreme values and normality tests, by factor cells.",
        syntax: "EXAMINE [VARIABLES=]deps [BY factor [BY factor] [factor ...]] [/TOTAL | NOTOTAL] [/ID=var]\n\
                 \x20       [/PERCENTILES[(list)][=HAVERAGE | WAVERAGE | ROUND | EMPIRICAL | AEMPIRICAL | NONE]]\n\
                 \x20       [/STATISTICS=DESCRIPTIVES EXTREME[(n)] ALL NONE] [/CINTERVAL=95]\n\
                 \x20       [/MESTIMATORS[=HUBER[(c)] TUKEY[(c)] HAMPEL[(a,b,c)] ANDREW[(c)] ALL NONE]]\n\
                 \x20       [/PLOT=[BOXPLOT] [STEMLEAF] [HISTOGRAM] [NPPLOT] [SPREADLEVEL[(power)]] | ALL | NONE]\n\
                 \x20       [/COMPARE=GROUPS | VARIABLES]\n\
                 \x20       [/MISSING=LISTWISE | PAIRWISE  EXCLUDE | INCLUDE  NOREPORT | REPORT].\n\
                 Each dependent is analyzed over all cases (unless NOTOTAL) and in every cell of each factor term,\n\
                 two factors crossed with BY. Descriptives (the default): mean with its confidence interval (CINTERVAL,\n\
                 50 to 99.99), 5% trimmed mean, median, variance, standard deviation, minimum, maximum, range,\n\
                 interquartile range, skewness and kurtosis with standard errors. PERCENTILES alone gives 5 10 25 50\n\
                 75 90 95 by HAVERAGE, with Tukey's hinges when 25, 50 or 75 is listed; the method also gives the median\n\
                 and the interquartile range. MESTIMATORS alone gives all four (Huber 1.339, Tukey 4.685, Hampel\n\
                 1.7 3.4 8.5, Andrews 1.34 times pi), each started at the median and scaled by the median absolute\n\
                 deviation. EXTREME lists the n highest and lowest values by case number or by the ID variable.\n\
                 NPPLOT gives the Tests of Normality table (Kolmogorov-Smirnov with the Lilliefors significance and\n\
                 Shapiro-Wilk by Royston's algorithm, 3 to 5000 cases).\n\
                 Plots come as chart data (a data table, and Vega-Lite with --vega-lite). Without PLOT every cell gets\n\
                 a stem-and-leaf plot and a boxplot; a PLOT subcommand names the ones wanted. BOXPLOT: the box spans\n\
                 Tukey's hinges around the median; a case 1.5 to 3 box lengths past a hinge is an outlier, 3 or more\n\
                 an extreme, each labeled by case number or by the ID variable, and the whiskers stop at the last\n\
                 unmarked values. COMPARE=GROUPS (default) gives each dependent one plot with a box per cell;\n\
                 VARIABLES puts all dependents in one plot, grouped by cell. STEMLEAF lists the cases past the\n\
                 outlier bounds as extremes. NPPLOT adds the normal Q-Q plot (each distinct value against the normal\n\
                 score of its mean rank over W + 1) and the detrended one. SPREADLEVEL needs a factor: per cell the\n\
                 log of the interquartile range against the log of the median, with the slope and the power it\n\
                 suggests (1 minus the slope); with a power p the values are raised to p first (0 takes logs), and\n\
                 the Test of Homogeneity of Variance table gives Levene's tests on the same values. ALL is every plot.\n\
                 LISTWISE (default) leaves out a case missing on any dependent or factor, PAIRWISE only from the\n\
                 analyses of that variable. INCLUDE makes user-missing values of the dependents valid, REPORT makes\n\
                 missing values of the factors categories. Weighted when a weight variable is set (Shapiro-Wilk rounds\n\
                 the weights and takes 3 to 50 cases with non-integer weights).",
        example: "EXAMINE VARIABLES=income BY region /PERCENTILES(25,50,75)=HAVERAGE /STATISTICS=ALL /MESTIMATORS /PLOT=NPPLOT.",
        parse: examine::examine,
    },
    CommandSpec {
        name: "REGRESSION",
        summary: "Linear regression with forced entry or removal and stepwise selection, residuals and diagnostics.",
        syntax: "REGRESSION [/VARIABLES=varlist | (COLLECT)] [/DESCRIPTIVES=[DEFAULTS MEAN STDDEV VARIANCE CORR SIG\n\
                 \x20          BADCORR COV XPROD N ALL NONE]] [/MISSING=LISTWISE | PAIRWISE | MEANSUBSTITUTION [INCLUDE]]\n\
                 \x20          [/SELECT=var EQ | NE | LT | LE | GT | GE value] [/REGWGT=var] [/STATISTICS=[DEFAULTS R ANOVA COEFF OUTS ZPP CHANGE CI[(95)] BCOV COLLIN TOL\n\
                 \x20          F SES XTX SELECTION LABEL ALL]] [/CRITERIA=[DEFAULTS PIN[(0.05)] | FIN[(3.84)] POUT[(0.10)] |\n\
                 \x20          FOUT[(2.71)] TOLERANCE[(0.0001)] MAXSTEPS(n) CIN[(95)]]] [/ORIGIN | NOORIGIN]\n\
                 \x20          /DEPENDENT=varlist /[METHOD=]ENTER [varlist] | REMOVE varlist | STEPWISE [varlist] |\n\
                 \x20          FORWARD [varlist] | BACKWARD [varlist] | TEST(varlist)(varlist)... [/METHOD=...]\n\
                 \x20          [/RESIDUALS=[DEFAULTS DURBIN OUTLIERS(tempvars) ID(var) HISTOGRAM NORMPROB]]\n\
                 \x20          [/CASEWISE=[DEFAULTS OUTLIERS(3) | ALL PLOT(tempvar) DEPENDENT tempvars]]\n\
                 \x20          [/SAVE=tempvar[(name)] ... [FITS]] [/SCATTERPLOT=(y, x) ...] [/PARTIALPLOT=[ALL | varlist]].\n\
                 tempvar: PRED RESID ZPRED ZRESID SRESID DRESID SDRESID ADJPRED SEPRED MAHAL COOK LEVER DFBETA SDBETA\n\
                 DFFIT SDFIT COVRATIO MCIN ICIN. One equation per dependent; each METHOD works on the equation the\n\
                 ones before it left, and each step is a model. ENTER takes the variables in order of tolerance;\n\
                 STEPWISE removes the weakest variable while its F-to-remove fails POUT, then enters the strongest\n\
                 that passes PIN; FORWARD only enters, BACKWARD only removes (entering its list first when the\n\
                 equation is empty). A variable enters only when it and the variables in the equation keep the\n\
                 TOLERANCE. Without keywords STATISTICS is R ANOVA COEFF OUTS and DESCRIPTIVES is MEAN STDDEV CORR.\n\
                 The tables: Variables Entered/Removed, Model Summary (CHANGE, DURBIN, SELECTION add columns),\n\
                 ANOVA, Coefficients (CI, ZPP, TOL, COLLIN, SES, F add columns), Excluded Variables, Coefficient\n\
                 Correlations (BCOV), Collinearity Diagnostics (COLLIN), the swept correlation matrix (XTX),\n\
                 Residuals Statistics (with RESIDUALS, CASEWISE or a plot), Casewise Diagnostics and Outlier\n\
                 Statistics. LISTWISE (default) uses the cases valid on every variable; PAIRWISE builds the\n\
                 correlations pair by pair, with the smallest pair count as N; MEANSUBSTITUTION puts the mean in\n\
                 place of a missing value. SAVE adds variables named PRE_1, RES_1 and so on, or as given; DFBETA and\n\
                 SDBETA save one per coefficient (name0, name1, ...), MCIN and ICIN a lower and an upper bound.\n\
                 Weighted when a weight variable is set; REGWGT gives weighted least squares. Charts come as data:\n\
                 HISTOGRAM(tempvars) a histogram with a normal curve; NORMPROB(tempvars) a normal P-P plot (PRED,\n\
                 RESID and DRESID are plotted standardized); RESIDUALS alone gives both of ZRESID. SCATTERPLOT\n\
                 plots each pair, the first on the vertical axis; a temporary variable takes a star (*ZRESID).\n\
                 PARTIALPLOT plots, for each independent (or those listed), the dependent's residuals against the\n\
                 independent's, both regressed on the other independents; it needs two in the equation. TEMPLATE\n\
                 is ignored with a warning. LABEL changes nothing. TEST enters every variable named in its sets together (Model Variables\n\
                 Entered/Removed says Test), then gives a Subset Tests table: for each set the change in R square\n\
                 when it leaves the equation, the change in the regression sum of squares, df, mean square and F\n\
                 (against the residual mean square of the full equation) with its significance, and a Residual row.\n\
                 SELECT makes the correlation matrix, the equation and its tests from the cases where the condition\n\
                 holds (a missing value on the variable leaves the case out; the variable is not on VARIABLES,\n\
                 DEPENDENT or METHOD, and its value is quoted for a string variable). The other cases still get\n\
                 predictions, residuals, leverage, Mahalanobis and Cook's distance (formulas of the Algorithms\n\
                 manual for cases outside the fit); the residual statistics, the outlier lists and the plots are of\n\
                 the selected and the unselected cases apart (SEPARATE, the default) or of all of them (POOLED),\n\
                 Casewise Diagnostics list both with a Selected Status, and SAVE fills every case.\n\
                 Not supported yet: MATRIX, OUTFILE.",
        example: "REGRESSION /STATISTICS=COEFF OUTS R ANOVA CHANGE /DEPENDENT=spend /METHOD=ENTER age /METHOD=STEPWISE q1 TO q5 /SAVE=PRED ZRESID.",
        parse: regression::regression,
    },
    CommandSpec {
        name: "RELIABILITY",
        summary: "Reliability of a scale: Cronbach's alpha, split-half and other models, item statistics and ICC.",
        syntax: "RELIABILITY /VARIABLES=varlist [/SCALE(label)=varlist | ALL]\n\
                 \x20           [/MODEL=ALPHA | OMEGA | SPLIT[(n)] | GUTTMAN | PARALLEL | STRICTPARALLEL]\n\
                 \x20           [/STATISTICS=DESCRIPTIVE SCALE CORR COV ANOVA [FRIEDMAN | COCHRAN] TUKEY HOTELLING ALL]\n\
                 \x20           [/SUMMARY=MEANS VARIANCE COV CORR TOTAL ALL]\n\
                 \x20           [/ICC=[MODEL(MIXED | RANDOM | ONEWAY)] [TYPE(CONSISTENCY | ABSOLUTE)] [CIN=95] [TESTVAL=0]]\n\
                 \x20           [/METHOD=COVARIANCE] [/MISSING=EXCLUDE | INCLUDE].\n\
                 A case missing on any variable of VARIABLES is left out. ALPHA (default) gives Cronbach's alpha\n\
                 and the alpha of the standardized items; OMEGA gives McDonald's omega for one common factor, from the\n\
                 closed form built on the covariances (it needs 3 items; with SUMMARY TOTAL the last column of\n\
                 Item-Total Statistics is omega if the item is deleted, which needs 4); SPLIT puts n items (default half, the odd one in the\n\
                 first part) in the second part and gives the alpha of each part, the correlation between them,\n\
                 Spearman-Brown and Guttman's split-half; GUTTMAN gives lambda 1 to 6; PARALLEL and STRICTPARALLEL\n\
                 give the maximum likelihood estimates and a chi-square test of the model. SUMMARY TOTAL is the\n\
                 Item-Total Statistics: scale mean, variance and alpha if the item is deleted and the corrected\n\
                 item-total correlation. ANOVA is the repeated measures table; FRIEDMAN (with Kendall's W) and\n\
                 COCHRAN give their chi-square in place of F, computed from the values as they are (give ranks\n\
                 or 0/1 items); TUKEY adds the test for nonadditivity; HOTELLING tests that the item means are\n\
                 equal. ICC gives single and average measure coefficients with confidence intervals and F tests.\n\
                 Items without variance are dropped, with a warning, when the model or a statistic needs the\n\
                 covariance matrix (all but ALPHA and SPLIT with DESCRIPTIVE, SCALE, ANOVA and TOTAL).\n\
                 Weighted when a weight variable is set. Not supported yet: MATRIX.",
        example: "RELIABILITY /VARIABLES=q1 TO q10 /SCALE('Satisfaction') ALL /MODEL=ALPHA /STATISTICS=DESCRIPTIVE SCALE /SUMMARY=TOTAL.",
        parse: reliability::reliability,
    },
    CommandSpec {
        name: "WEIGHTED KAPPA",
        summary: "Cohen's weighted kappa: how well two raters agree on an ordered scale.",
        syntax: "WEIGHTED KAPPA varlist | rowlist BY columnlist\n\
                 \x20              [/MISSING CLASSMISSING=TABLE | INCLUDE]\n\
                 \x20              [/CRITERIA [ASYMPTOTICCILEVEL=95] [WEIGHTING=LINEAR | QUADRATIC] [IGNORECASE=FALSE | TRUE]]\n\
                 \x20              [/PRINT [TABULATION(ALL | rowlist BY columnlist)] [CASE(UPPER | LOWER)]\n\
                 \x20                      [ORDER(ASCENDING | DESCENDING)]].\n\
                 One list pairs each variable with every later one, the earlier as the row; two lists pair every\n\
                 row variable with every column variable, never a variable with itself. Both variables of a pair\n\
                 must be numeric, or both string. The k categories of a pair are the values either rater used,\n\
                 numbers by size and strings alphabetically, and two categories d places apart get the agreement\n\
                 weight 1 - d/(k - 1) (LINEAR) or 1 - d^2/(k - 1)^2 (QUADRATIC). Each pair drops the cases missing\n\
                 on either of its variables; INCLUDE keeps user-missing values, each as a category of its own.\n\
                 The kappa table has a row per pair: N, the kappa, its asymptotic standard error, z with its\n\
                 two-sided significance (from the standard error under chance agreement) and the confidence\n\
                 interval at ASYMPTOTICCILEVEL percent. IGNORECASE=TRUE compares strings regardless of case and\n\
                 shows them in the case CASE names. TABULATION adds the table of counts of every pair, or of the\n\
                 pairs the two subsets make, its categories in ORDER. Case weights are rounded to whole numbers\n\
                 (0.5 or less drops the case). A pair with fewer than two cases or only one category is skipped\n\
                 with a warning. KEYWORD(value) works as KEYWORD=value; ASYMPTOTIC_CILEVEL and IGNORE_CASE are\n\
                 other spellings.",
        example: "WEIGHTED KAPPA nurse_a nurse_b BY doctor /CRITERIA WEIGHTING=QUADRATIC ASYMPTOTICCILEVEL=90\n\
                  \x20 /PRINT TABULATION(ALL).",
        parse: agreement::weighted_kappa,
    },
    CommandSpec {
        name: "FLEISS MULTIRATER KAPPA",
        summary: "Fleiss' kappa: how well several raters agree when they sort subjects into categories.",
        syntax: "FLEISS MULTIRATER KAPPA [VARIABLES=]varlist\n\
                 \x20                       [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20                       [/CRITERIA [ASYMPTOTICCILEVEL=95] [IGNORECASE=FALSE | TRUE]]\n\
                 \x20                       [/PRINT [CATEGORY] [CASE(UPPER | LOWER)]].\n\
                 Every variable is a rater and every case a subject; at least two raters, all numeric or all\n\
                 string, each named once. A case missing on any rater is dropped; INCLUDE keeps user-missing\n\
                 values as categories. Numeric ratings lose their fractions. The overall table gives the kappa,\n\
                 its asymptotic standard error under chance agreement, z with its two-sided significance and the\n\
                 confidence interval at ASYMPTOTICCILEVEL percent. CATEGORY adds a row per category: the\n\
                 conditional probability that a second rating of a subject falls in it when one did, its kappa\n\
                 and the same test. Case weights are rounded to whole numbers (0.5 or less drops the case). At\n\
                 least two subjects and two categories are needed. IGNORECASE and CASE work as on WEIGHTED KAPPA.",
        example: "FLEISS MULTIRATER KAPPA judge1 TO judge4 /PRINT CATEGORY /CRITERIA ASYMPTOTICCILEVEL=99.",
        parse: agreement::fleiss_kappa,
    },
    CommandSpec {
        name: "RATIO STATISTICS",
        summary: "Statistics of the ratio of two positive variables, overall and within groups.",
        syntax: "RATIO STATISTICS numerator WITH denominator [BY group [(ASCENDING | DESCENDING | NOSORT)]]\n\
                 \x20                [/MISSING=EXCLUDE | INCLUDE] [/PRINT=statistic ...]\n\
                 \x20                [/OUTFILE('file or dataset')=statistic ...].\n\
                 statistic: MEAN MEDIAN WGTMEAN MIN MAX RANGE STDDEV AAD COD PRD MNCOV MDCOV CIN[(95)]\n\
                 \x20          BCOC((low, high) ...) WCOC(percent ...)\n\
                 A case counts when both variables are valid and above 0 and its weight, rounded to a whole\n\
                 number, is above 0. WGTMEAN is the sum of numerators over the sum of denominators, PRD the\n\
                 mean over WGTMEAN. AAD is the mean distance from the median and COD that in percent of the\n\
                 median; MNCOV is the standard deviation in percent of the mean, MDCOV the root mean square\n\
                 distance from the median (over N - 1) in percent of the median. BCOC is the percent of ratios\n\
                 from low to high, ends included; WCOC the percent within that many percent of the median. CIN\n\
                 adds intervals to MEAN and WGTMEAN (t with N - 1 df, a delta method variance for WGTMEAN) and\n\
                 to MEDIAN (from order statistics, with the actual coverage, at least the level asked). Without\n\
                 PRINT only the case processing summary shows. The groups may include a missing value. OUTFILE\n\
                 writes the overall record first, the group variable empty, then one record per group; it is\n\
                 not available under SPLIT FILE. A subcommand without content is ignored.",
        example: "RATIO STATISTICS rent WITH income BY district (DESCENDING)\n\
                  \x20 /PRINT=MEDIAN COD PRD WCOC(10 25) CIN(90).",
        parse: ratio_statistics::ratio_statistics,
    },
    CommandSpec {
        name: "BAYES ONESAMPLE",
        summary: "Bayesian inference on one mean, proportion or Poisson rate: Bayes factors and posteriors.",
        syntax: "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL | BINOMIAL | POISSON VARIABLES=varlist\n\
                 \x20                          [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]\n\
                 \x20  [/DATA VARIABLES=sublist [VARIANCE=v] [NULLVALUE=0] [ALTERGVALUE=g]          (NORMAL)\n\
                 \x20        [SUCCESS=LAST | FIRST | MIDPOINT | CUTPOINT(v) | LEVEL(values)]    (BINOMIAL)\n\
                 \x20        [NULLPRIORPARAM=2 2] [ALTERPRIORPARAM=a b]] ...         (BINOMIAL, POISSON)\n\
                 \x20  [/PRIOR [VARDIST=DIFFUSE | INVCHISQ(df scale) | INVGAMMA(shape scale) | JEFFREYS2 | JEFFREYS4]\n\
                 \x20          [PRECDIST=GAMMA(shape rate) | CHISQ(df)] [MEANDIST=DIFFUSE | NORMAL(mean [variance])]\n\
                 \x20          [KAPPA=1] [TYPE=CONJUGATE(a b)]]\n\
                 \x20  [/CRITERIA [CILEVEL=95] [METHOD=AGL | MCA] [SEED=RANDOM | n] [TOL=0.000001] [MAXITER=2000]\n\
                 \x20             [MCSAMPLES=10000]] [/MISSING SCOPE=ANALYSIS | LISTWISE].\n\
                 Each variable is analysed alone. A DATA subcommand sets its keywords for the variables of\n\
                 its sublist; the others keep the defaults. NORMAL: the Bayes factor BF01 weighs a mean of\n\
                 NULLVALUE against a normal spread of means around it. With a known VARIANCE the spread\n\
                 is g times that variance (g = ALTERGVALUE, default 1). Without it, a given g fixes the\n\
                 spread relative to the unknown variance, and no g puts a Cauchy prior on the effect size\n\
                 (the JZS factor), integrated by adaptive quadrature or, with METHOD=MCA, by MCSAMPLES\n\
                 draws. The posterior of the mean: from the variance prior with a flat mean prior\n\
                 (DIFFUSE, JEFFREYS2, JEFFREYS4), or a conjugate normal mean prior centered on the first\n\
                 value of MEANDIST=NORMAL with KAPPA prior observations (INVCHISQ, INVGAMMA, PRECDIST);\n\
                 a conjugate prior with MEANDIST=DIFFUSE falls back to VARDIST=DIFFUSE. With a known\n\
                 variance the mean prior is flat or NORMAL(mean variance). BINOMIAL: SUCCESS picks the\n\
                 values that count as a success (LAST and FIRST: the highest and lowest value present;\n\
                 MIDPOINT: from halfway between the smallest and largest value up; CUTPOINT: from the\n\
                 value up). BF01 compares a beta prior NULLPRIORPARAM with ALTERPRIORPARAM, and the\n\
                 posterior comes from the beta prior TYPE=CONJUGATE(a b) (both above 0, or 0 0). POISSON:\n\
                 the same with gamma priors of shape and rate; the values must be counts.\n\
                 ALTERPRIORPARAM is needed for a Bayes factor and TYPE for a posterior of these two.\n\
                 Credible intervals have equal tails. Case weights are rounded to whole numbers; a weight\n\
                 below 0.5 drops the case.",
        example: "BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=commute ANALYSIS=BOTH\n\
                  \x20 /DATA VARIABLES=commute NULLVALUE=30\n\
                  \x20 /PRIOR VARDIST=INVGAMMA(3 200) MEANDIST=NORMAL(28) KAPPA=4.",
        parse: bayes::onesample,
    },
    CommandSpec {
        name: "BAYES RELATED",
        summary: "Bayesian inference on the mean difference of paired measurements.",
        syntax: "BAYES RELATED /PAIR VARIABLES=first second [VARIANCE=v] [NULLVALUE=0] [ALTERGVALUE=g]\n\
                 \x20             [/PAIR ...] [/INFERENCE [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]]\n\
                 \x20             [/PRIOR ...] [/CRITERIA ...] [/MISSING SCOPE=ANALYSIS | LISTWISE].\n\
                 Each pair is analysed as one sample of differences, first minus second, with the rules of\n\
                 BAYES ONESAMPLE DISTRIBUTION=NORMAL: NULLVALUE is the difference tested, VARIANCE a known\n\
                 variance of the differences. PRIOR and CRITERIA take the keywords of BAYES ONESAMPLE except\n\
                 TYPE. LISTWISE drops a case missing on any variable of any pair.",
        example: "BAYES RELATED /PAIR VARIABLES=pulse_before pulse_after NULLVALUE=0\n\
                  \x20 /PAIR VARIABLES=grip_left grip_right /INFERENCE ANALYSIS=BAYESFACTOR.",
        parse: bayes::related,
    },
    CommandSpec {
        name: "BAYES INDEPENDENT",
        summary: "Bayesian comparison of the means of two independent groups.",
        syntax: "BAYES INDEPENDENT /INFERENCE VARIABLES=varlist GROUP=var\n\
                 \x20                            SELECT=LEVEL(a b) | MIDPOINT | CUTPOINT(v)\n\
                 \x20                            [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH] [DATAVAR=v1 v2]\n\
                 \x20  [/ESTBF COMPUTATION=ROUDER[(1)] | GONEN([0] variance) | HYPER[(-0.75)]]\n\
                 \x20  [/PRIOR [EQUALDATAVAR=FALSE | TRUE] [VARDIST=DIFFUSE | JEFFREYS | INVCHISQ(df scale)]\n\
                 \x20          [MEANDIST=DIFFUSE | NORMAL(mean1 mean2 variance1 variance2)]]\n\
                 \x20  [/CRITERIA [CILEVEL=95] [TOL=0.000001] [MAXITER=2000]] [/MISSING SCOPE=ANALYSIS | LISTWISE].\n\
                 LEVEL takes the two values of GROUP that form group 1 and group 2; MIDPOINT (the middle of\n\
                 the range of GROUP) and CUTPOINT put values below the point in group 1 and the rest in\n\
                 group 2. The difference is group 2 minus group 1. Bayes factors use the pooled t: ROUDER\n\
                 gives BF01 for a Cauchy prior of that scale on the effect size, GONEN BF01 for a normal\n\
                 prior with that mean and variance, HYPER BF10 for a hyper-prior with parameter a (above -1,\n\
                 at most -0.5). The posterior of the difference: with DATAVAR (known variances) normal, from\n\
                 flat or NORMAL mean priors; else with EQUALDATAVAR=TRUE a t from VARDIST=JEFFREYS or\n\
                 INVCHISQ, and in all other cases the unequal-variances posterior of flat priors (each\n\
                 group needs more than five cases). Weights as on BAYES ONESAMPLE.",
        example: "BAYES INDEPENDENT /INFERENCE VARIABLES=yield GROUP=fertilizer SELECT=LEVEL(1 2)\n\
                  \x20 ANALYSIS=BOTH /ESTBF COMPUTATION=GONEN(0.5)\n\
                  \x20 /PRIOR EQUALDATAVAR=TRUE VARDIST=JEFFREYS.",
        parse: bayes::independent,
    },
    CommandSpec {
        name: "BAYES CORRELATION",
        summary: "Bayesian inference on the Pearson correlation of every pair of variables.",
        syntax: "BAYES CORRELATION /INFERENCE VARIABLES=varlist [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]\n\
                 \x20                            [CVALUE=0] [MAXPLOTS=10]\n\
                 \x20  [/ESTBF [TYPE=JZS | FRACTION[(0.5)]] [NULLVALUE=0]]\n\
                 \x20  [/CRITERIA [CILEVEL=95] [SEED=RANDOM | n] [TOL=0.0001] [MAXITER=2000] [MCSAMPLES=10000]\n\
                 \x20             [POSTSAMPLES=10000]] [/MISSING SCOPE=PAIRWISE | LISTWISE].\n\
                 Every variable is paired with every later one. JZS gives BF01 of a zero correlation\n\
                 against the default prior of the linear model; FRACTION(b) the fractional Bayes factor\n\
                 BF01 of NULLVALUE, with the fraction b of the data training the prior (b times N must\n\
                 exceed 1). A correlation of 1 or -1 gets no Bayes factor. The posterior of rho is\n\
                 proportional to (1 - rho^2)^CVALUE times the approximate likelihood; its mode, mean,\n\
                 variance and highest-density interval are computed by quadrature, so SEED and\n\
                 POSTSAMPLES do not change them. Unless ANALYSIS=BAYESFACTOR, the first MAXPLOTS pairs get\n\
                 a chart of the prior, likelihood and posterior densities of rho as chart data. MCSAMPLES is\n\
                 accepted and not used. Weights as on BAYES ONESAMPLE.",
        example: "BAYES CORRELATION /INFERENCE VARIABLES=sleep_hours focus_score mood ANALYSIS=BOTH\n\
                  \x20 /ESTBF TYPE=FRACTION(0.4).",
        parse: bayes::correlation,
    },
    CommandSpec {
        name: "BAYES ANOVA",
        summary: "Bayesian one-way analysis of variance: a Bayes factor for the groups and posteriors of their means.",
        syntax: "BAYES ANOVA dependent BY factor\n\
                 \x20  [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]\n\
                 \x20  [/ESTBF COMPUTATION=JZS | ZELLNER(g) | HYPER[(3)] | ROUDER[(1)]]\n\
                 \x20  [/PRIOR [TYPE=REFERENCE | CONJUGATE] [SHAPEPARAM=a0] [SCALEPARAM=b0]\n\
                 \x20          [MEANVECTOR=values] [VMATRIX=IDENTITY | row1; row2; ...]]\n\
                 \x20  [/PLOT [MEANS=values] [ERRORVAR=FALSE | TRUE]] [/DESIGN REGWGT=var]\n\
                 \x20  [/CRITERIA [CILEVEL=95] [TOL=0.000001] [MAXITER=2000]].\n\
                 Each value of the factor forms a group. BF10 compares the model with one mean per group\n\
                 against one common mean; it depends on N, the number of groups and R square. ZELLNER\n\
                 fixes g of the g-prior; JZS mixes g over an inverse gamma (1/2, N/2); ROUDER(s) uses the\n\
                 scale s in that mixture; HYPER(a), a above 2, uses the hyper-g prior. The posterior\n\
                 table gives each group mean and the error variance. REFERENCE is the flat prior\n\
                 1/variance. CONJUGATE needs SHAPEPARAM and SCALEPARAM of an inverse gamma on the error\n\
                 variance and MEANVECTOR, one prior mean per group in ascending order of the factor;\n\
                 VMATRIX is the lower triangle of the prior matrix of the means (times the variance),\n\
                 rows ending in ';'; one that is not positive definite becomes the identity with a\n\
                 warning. PLOT draws the prior, the likelihood and the posterior of the listed group means\n\
                 and of the error variance as chart data. REGWGT multiplies the case weights in the sums\n\
                 of squares; cases with a regression weight of 0 or below are left out.",
        example: "BAYES ANOVA reaction_ms BY font_size\n\
                  \x20 /INFERENCE ANALYSIS=BOTH /ESTBF COMPUTATION=ROUDER(0.7) /PLOT MEANS=10 14.",
        parse: bayes::anova,
    },
    CommandSpec {
        name: "BAYES REGRESSION",
        summary: "Bayesian linear regression: a Bayes factor for the model, posteriors, predictions and F tests.",
        syntax: "BAYES REGRESSION dependent [BY factors] [WITH covariates]\n\
                 \x20  [/DESIGN [FACTORS=sublist] [COVARIATES=sublist] [REGWGT=var]]\n\
                 \x20  [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]\n\
                 \x20  [/ESTBF [COMPUTATION=JZS | ZELLNER(g) | HYPER[(3)] | ROUDER[(1)]] [COMPARE=NULL | FULL]]\n\
                 \x20  [/PRIOR ...as on BAYES ANOVA] [/PREDICT REGRESSORS=values]\n\
                 \x20  [/SAVE [MEANPRED[(name)]] [VARIANCE[(name)]] [MODE[(name)]] [CIMEANPREDL[(name)]]\n\
                 \x20         [CIMEANPREDU[(name)]]] [/OUTFILE PARAMETER='file']\n\
                 \x20  [/PLOT [COVARIATES=sublist] [FACTORS=sublist] [MAX_CATEGORY=2] [INTERCEPT=FALSE | TRUE]\n\
                 \x20         [ERRORVAR=FALSE | TRUE] [BAYESPRED=FALSE | TRUE]]\n\
                 \x20  [/FTEST [LABEL='text'] [VARIABLES=sublist] [INTERCEPT=FALSE | TRUE] [TESTVALUES=values]] ...\n\
                 \x20  [/CRITERIA [CILEVEL=95] [TOL=0.000001] [MAXITER=2000]].\n\
                 The model holds an intercept, one indicator per category of each factor (ascending; the\n\
                 last is redundant and set to 0) and the covariates; DESIGN lists narrow it. BF10 weighs\n\
                 the model against the intercept alone with the g-priors of BAYES ANOVA. COMPARE=FULL adds\n\
                 a table that weighs each model without one factor or covariate, and the intercept alone,\n\
                 against the full model. The posterior table gives every parameter and the error variance;\n\
                 PRIOR works as on BAYES ANOVA, with MEANVECTOR holding one value per parameter, intercept\n\
                 first, redundant ones included. PREDICT takes one value per factor (a category) and per\n\
                 covariate, factors first, and gives the predictive distribution; SAVE writes its mean,\n\
                 variance, mode and credible bounds for every case with valid regressors (default names\n\
                 such as MEANPRED_1). Each FTEST tests whether the parameters of its variables (and the\n\
                 intercept) equal TESTVALUES, 0 by default. PLOT draws prior, likelihood and posterior as\n\
                 chart data: of covariates, of the first MAX_CATEGORY categories of factors, of the\n\
                 intercept, of the error variance, and the predictive density. OUTFILE is accepted, but no\n\
                 model file is written yet. Cases missing on any variable of the model are left out.",
        example: "BAYES REGRESSION bill_total BY table_zone WITH party_size\n\
                  \x20 /INFERENCE ANALYSIS=BOTH /ESTBF COMPARE=FULL\n\
                  \x20 /PREDICT REGRESSORS=2 4 /FTEST LABEL='zone' VARIABLES=table_zone.",
        parse: bayes::regression,
    },
    CommandSpec {
        name: "BAYES REPEATED",
        summary: "Bayesian one-way repeated measures analysis: a Bayes factor and posteriors of the condition means.",
        syntax: "BAYES REPEATED varlist\n\
                 \x20  [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH] [/ESTBF COMPUTATION=BIC | ROUDER]\n\
                 \x20  [/DESIGN REGWGT=var] [/PLOT MEANS=sublist]\n\
                 \x20  [/CRITERIA [CILEVEL=95] [SEED=RANDOM | n] [MCSAMPLES=30000]].\n\
                 Each case is a subject measured once under each of the two or more variables; a case\n\
                 missing on any of them is left out. BF10 weighs different condition means against equal\n\
                 ones with the subjects as a random effect. BIC uses a closed-form approximation from the\n\
                 sums of squares of conditions, subjects and the total. ROUDER averages the factor of a\n\
                 mixed model over MCSAMPLES draws of the two scale parameters, from SEED; it uses the\n\
                 regression weights only. The posterior of each condition mean is the normal large-sample\n\
                 approximation, which needs no prior. PLOT draws the posterior densities of the listed\n\
                 means as chart data.",
        example: "BAYES REPEATED trial1 trial2 trial3 trial4\n\
                  \x20 /INFERENCE ANALYSIS=BOTH /ESTBF COMPUTATION=ROUDER /CRITERIA SEED=731.",
        parse: bayes::repeated,
    },
    CommandSpec {
        name: "BAYES LOGLINEAR",
        summary: "Bayesian test of independence in a two-way table, with simulated interaction terms.",
        syntax: "BAYES LOGLINEAR /TABLE rowvar BY colvar\n\
                 \x20  [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]\n\
                 \x20  [/ESTBF [MODEL=MULTINOMIAL | POISSON | NONPARAMETRIC] [FIX_MARGIN=TOTAL | ROW | COLUMN]\n\
                 \x20          [PRIOR_TYPE=CONJUGATE | MIXTURE_DIRICHLET | INTRINSIC] [SHAPEPARAMS=values]\n\
                 \x20          [SCALEPARAM=DATA | b] [LAMBDAS=values]]\n\
                 \x20  [/PRINT [SUPPRESS_TABLE=FALSE | TRUE] [CELLS=OBSERVED ROW COLUMN EXPECTED TOTAL ALL]\n\
                 \x20          [STATISTICS=CHISQ LRT]]\n\
                 \x20  [/CRITERIA [CILEVEL=95] [SEED=RANDOM | n] [TOL=0.000001] [MAXITER=2000]\n\
                 \x20             [POSTSAMPLES=10000] [ORDER=ASCENDING | DESCENDING]].\n\
                 The table counts cases valid on both variables with rounded case weights; only categories\n\
                 that occur count, and ORDER sorts both of them. Every Bayes factor is BF01, for\n\
                 independence. CONJUGATE puts gamma priors with the shapes of SHAPEPARAMS (1 for every cell;\n\
                 one value, or one row of values per table row with ';' between rows) on the cells;\n\
                 FIX_MARGIN says whether the grand total, the row totals or the column totals are fixed by\n\
                 the design. POISSON fixes nothing and uses the gamma scale SCALEPARAM (DATA: rows times\n\
                 columns times the smallest shape over N). MIXTURE_DIRICHLET mixes symmetric Dirichlet priors\n\
                 over a log-Cauchy weight, by quadrature with TOL and MAXITER. INTRINSIC needs a 2 x 2 table\n\
                 (others fall back to the mixture with a warning). NONPARAMETRIC needs FIX_MARGIN=ROW with\n\
                 two rows or COLUMN with two columns; LAMBDAS holds one Dirichlet parameter, or one per free\n\
                 category.\n\
                 The posterior part draws POSTSAMPLES cell probability sets from SEED and reports each log\n\
                 odds ratio against the last row and column, with a simultaneous credible region for all of\n\
                 them. PRINT shows the crosstab (observed counts by default) and, for CHISQ, Pearson's\n\
                 chi-square, the continuity correction of a 2 x 2 table and Fisher's test when an expected count\n\
                 is below 5; LRT adds the likelihood ratio. Keywords with an underscore may be written\n\
                 without it or as two words.",
        example: "BAYES LOGLINEAR /TABLE ferry_route BY ticket_class\n\
                  \x20 /INFERENCE ANALYSIS=BOTH /ESTBF MODEL=MULTINOMIAL FIX_MARGIN=ROW PRIOR_TYPE=CONJUGATE\n\
                  \x20 /PRINT CELLS=OBSERVED ROW STATISTICS=CHISQ LRT /CRITERIA SEED=4417.",
        parse: bayes::loglinear,
    },
    CommandSpec {
        name: "POWER MEANS ONESAMPLE",
        summary: "Power or sample size of a one-sample t-test of a mean.",
        syntax: "POWER MEANS ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                     N=n | POWER=powers  SD=sd  MEAN=mean  [NULL=0]\n\
                 \x20                     [/PLOT [N[(low high)]] [ES[(low high)]] [N_XAXIS[(low high)] ES_YAXIS[(low high)]]\n\
                 \x20                            [N_YAXIS[(low high)] ES_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].\n\
                 powers: one or more values between 0 and 1, and at most one grid low TO high BY step.\n\
                 Needs no data. With N (a whole number above 1) the table gives the power of the test of\n\
                 MEAN against NULL when the population standard deviation is SD; with POWER it gives, for\n\
                 each power, the smallest N that reaches it and the power that N actually has. The power\n\
                 comes from the noncentral t with N - 1 df and noncentrality (MEAN - NULL) / SD * sqrt(N);\n\
                 the effect size (MEAN - NULL) / SD shows unsigned for a two-sided test. A one-sided test\n\
                 looks in the direction of the effect. PLOT gives power curves as chart data: N over sample\n\
                 sizes (default from 2 to twice the table's N), ES over effect sizes (default 0 to twice the\n\
                 effect, or from minus to plus that for a one-sided test; within -5 and 5), and a pair of\n\
                 _XAXIS and _YAXIS keywords a surface over both. With several powers only the N chart is\n\
                 drawn, over the sizes the table found. VROTATE and HROTATE are accepted; the chart data has\n\
                 no viewing angle.",
        example: "POWER MEANS ONESAMPLE /PARAMETERS SD=12 MEAN=104 NULL=100 POWER=0.8 0.9\n\
                  \x20 /PLOT N.",
        parse: power::means_onesample,
    },
    CommandSpec {
        name: "POWER MEANS INDEPENDENT",
        summary: "Power or group sizes of a t-test comparing the means of two independent groups.",
        syntax: "POWER MEANS INDEPENDENT /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                       N=n1 n2 | POWER=powers [NRATIO=1]  SD=sd | sd1 sd2  MEAN=difference | mean1 mean2\n\
                 \x20                       [/PLOT [NRATIO[(low high)]] [ES[(low high)]] [NRATIO_XAXIS[(low high)] ES_YAXIS[(low high)]]\n\
                 \x20                              [NRATIO_YAXIS[(low high)] ES_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. One SD assumes equal variances: df n1 + n2 - 2 and effect size difference /\n\
                 SD. Two different SDs give the separate-variances test with Welch's df, and the table shows\n\
                 the mean difference instead of an effect size. Two MEAN values are the group means, one is\n\
                 their difference. With POWER the table gives n1 (the smallest that reaches the power) and\n\
                 n2 = NRATIO * n1 rounded up (at least 2), with the power they actually have. NRATIO takes\n\
                 0.01 to 100. PLOT: NRATIO over n2 / n1 with n1 + n2 kept (default 0.2 to 5), ES over the\n\
                 effect size, or the mean difference with two SDs (default 0 to twice the table's value). The\n\
                 other rules are those of POWER MEANS ONESAMPLE.",
        example: "POWER MEANS INDEPENDENT /PARAMETERS SD=8 11 MEAN=31 25 POWER=0.85 NRATIO=2\n\
                  \x20 /PLOT ES(0 10).",
        parse: power::means_independent,
    },
    CommandSpec {
        name: "POWER MEANS RELATED",
        summary: "Power or number of pairs of a paired t-test.",
        syntax: "POWER MEANS RELATED /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                   NPAIRS=n | POWER=powers  SD=sd | sd1 sd2  [RHO=0.5]  MEAN=difference | mean1 mean2\n\
                 \x20                   [/PLOT [NPAIRS[(low high)]] [ES[(low high)]] [NPAIRS_XAXIS[(low high)] ES_YAXIS[(low high)]]\n\
                 \x20                          [NPAIRS_YAXIS[(low high)] ES_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. The test is the one-sample t-test of the differences within pairs. One SD is\n\
                 the standard deviation of those differences; two are those of the two measures, combined\n\
                 with their correlation RHO (-1 to 1, not 0) as sqrt(sd1^2 + sd2^2 - 2 RHO sd1 sd2). RHO is\n\
                 ignored with one SD. NPAIRS is a whole number above 1; the NPAIRS chart range lies within 2\n\
                 and 5000. The other rules are those of POWER MEANS ONESAMPLE.",
        example: "POWER MEANS RELATED /PARAMETERS SD=6.5 7 RHO=0.7 MEAN=3 NPAIRS=40 TEST=DIRECTIONAL\n\
                  \x20 /PLOT NPAIRS(10 80).",
        parse: power::means_related,
    },
    CommandSpec {
        name: "POWER PROPORTIONS ONESAMPLE",
        summary: "Power or sample size of a test of one proportion against a hypothesized value.",
        syntax: "POWER PROPORTIONS ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                           N=n | POWER=powers  [NULL=0.5]  ALTERNATIVE=p\n\
                 \x20                           [ESTIMATE=NORMAL | BINOMIAL] [CONTINUITY=FALSE | TRUE] [TIMER=minutes]\n\
                 \x20                           [/PLOT [N[(low high)]] [NULL] [ALTERNATIVE] [DIFF] [N_XAXIS[(low high)] DIFF_YAXIS]\n\
                 \x20                                  [N_YAXIS[(low high)] DIFF_XAXIS] [NULL_XAXIS ALTERNATIVE_YAXIS]\n\
                 \x20                                  [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. NULL and ALTERNATIVE lie between 0 and 1, and differ when POWER is given.\n\
                 NORMAL uses the normal approximation, with a continuity correction of 1 / (2 sqrt(n)) when\n\
                 CONTINUITY=TRUE. BINOMIAL counts the exact binomial rejection region at NULL and its\n\
                 probability at ALTERNATIVE, up to 500 trials; above that the normal approximation is used\n\
                 with a warning. Because exact power rises in steps, the sample size search checks smaller\n\
                 sizes and then counts upwards until the power is reached. TIMER stops that search after so\n\
                 many minutes, with no result. A one-sided test looks in the direction of ALTERNATIVE. PLOT\n\
                 charts power over N, over NULL or ALTERNATIVE (the other kept), over DIFF = ALTERNATIVE -\n\
                 NULL with NULL kept, and surfaces over N and DIFF or NULL and ALTERNATIVE; proportion axes\n\
                 run from 0.01 to 0.99.",
        example: "POWER PROPORTIONS ONESAMPLE /PARAMETERS NULL=0.3 ALTERNATIVE=0.42 POWER=0.8 0.9\n\
                  \x20 ESTIMATE=BINOMIAL.",
        parse: power::proportions_onesample,
    },
    CommandSpec {
        name: "POWER PROPORTIONS INDEPENDENT",
        summary: "Power or group sizes of a test comparing two independent proportions.",
        syntax: "POWER PROPORTIONS INDEPENDENT /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                             N=n1 n2 | POWER=powers [NRATIO=1]  PROPORTIONS=p1 p2\n\
                 \x20                             [METHOD=CHISQ | T | LRT | FISHER] [ESTIMATE=NORMAL | BINOMIAL]\n\
                 \x20                             [CONTINUITY=FALSE | TRUE] [POOLED=TRUE | FALSE] [TIMER=minutes]\n\
                 \x20                             [/PLOT [NRATIO[(low high)]] [RISK_DIFF] [RISK_RATIO[(low high)]] [ODDS_RATIO[(low high)]]\n\
                 \x20                                    [N1_XAXIS[(low high)] N2_YAXIS[(low high)]] [N1_YAXIS[(low high)] N2_XAXIS[(low high)]]\n\
                 \x20                                    [P1_XAXIS P2_YAXIS] [P1_YAXIS P2_XAXIS] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. The table shows the risk difference p1 - p2, the risk ratio p1 / p2 and the\n\
                 odds ratio. METHOD picks the test: CHISQ the z (chi-square) test, with a pooled or separate\n\
                 standard error (POOLED) and optionally a continuity correction; T a t-test of the 0/1\n\
                 values (pooled or not); LRT the likelihood ratio test; FISHER Fisher's exact test, by the\n\
                 arcsine approximation (it needs at least 0.5 / p successes and failures in the right\n\
                 groups, else the power is missing). With ESTIMATE=BINOMIAL and at most 500 trials in all,\n\
                 the power sums the binomial probabilities of every pair of outcomes the test rejects (for\n\
                 FISHER, the exact conditional test); with POWER and FISHER the size search uses the chi-square\n\
                 test instead, with a warning. With POWER the table gives n1 and n2 = NRATIO * n1 rounded up\n\
                 (at least 1); the directional chi-square sizes come from a closed formula. A one-sided test\n\
                 looks in the direction of p1 - p2. PLOT: NRATIO with n1 + n2 kept (default 0.2 to 5),\n\
                 RISK_DIFF, RISK_RATIO and ODDS_RATIO by moving p1 with p2 kept, and surfaces over the two\n\
                 group sizes (default 1 to twice each size) or the two proportions.",
        example: "POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.22 0.35 N=90 110\n\
                  \x20 METHOD=T POOLED=FALSE /PLOT RISK_RATIO.",
        parse: power::proportions_independent,
    },
    CommandSpec {
        name: "POWER PROPORTIONS RELATED",
        summary: "Power or number of pairs of McNemar's test of two paired proportions.",
        syntax: "POWER PROPORTIONS RELATED /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                         NPAIRS=n | POWER=powers  VALUES=PROPORTIONS(a b) | COUNTS(a b)\n\
                 \x20                         [MARGINAL=FALSE | TRUE] [CORRELATION=0.5] [ESTIMATE=NORMAL | BINOMIAL] [TIMER=minutes]\n\
                 \x20                         [/PLOT [NPAIRS[(low high)]] [RISK_DIFF] [RISK_RATIO[(low high)]] [ODDS_RATIO[(low high)]]\n\
                 \x20                                [CORRELATION] [DISCORDANT] [MARGINAL] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. A pair is a success or failure on a first and a second occasion. With\n\
                 MARGINAL=FALSE, a is the share of pairs with success first and failure second and b the\n\
                 reverse; with TRUE, a and b are the success rates of the first and second occasion, turned\n\
                 into those two shares through their CORRELATION (between -1 and 1). COUNTS give the numbers\n\
                 of such pairs and need NPAIRS. The risk difference is b - a in both readings, the risk and\n\
                 odds ratios compare the second discordant share with the first. NORMAL uses the normal\n\
                 approximation; BINOMIAL averages the exact conditional binomial test over the number of\n\
                 discordant pairs, up to 500 pairs. PLOT: NPAIRS (within 2 and 2500), RISK_DIFF, RISK_RATIO\n\
                 and ODDS_RATIO by moving b with a kept, CORRELATION (MARGINAL=TRUE only), and surfaces over\n\
                 the two discordant shares (DISCORDANT) or the two success rates (MARGINAL, with TRUE).",
        example: "POWER PROPORTIONS RELATED /PARAMETERS VALUES=PROPORTIONS(0.6 0.45) MARGINAL=TRUE\n\
                  \x20 CORRELATION=0.3 POWER=0.9 /PLOT CORRELATION.",
        parse: power::proportions_related,
    },
    CommandSpec {
        name: "POWER ONEWAY ANOVA",
        summary: "Power or group sizes of a one-way analysis of variance, its contrasts and pairwise comparisons.",
        syntax: "POWER ONEWAY ANOVA /PARAMETERS [SIGNIFICANCE=0.05]  GROUP_SIZES=n1 n2 ... | POWER=powers\n\
                 \x20                  POOLED_SD=sd  GROUP_MEANS=m1 m2 ...  [GROUP_WEIGHTS=EQUAL | w1 w2 ...]\n\
                 \x20                  [/CONTRAST [COEFFICIENTS=c1 c2 ...] [TEST=NONDIRECTIONAL | DIRECTIONAL]\n\
                 \x20                             [PAIRWISE=FALSE | TRUE] [ADJUST=BONFERRONI | SIDAK | LSD]]\n\
                 \x20                  [/PLOT [TOTAL_N[(low high)]] [POOLED_SD[(low high)]]\n\
                 \x20                         [TOTAL_N_XAXIS[(low high)] POOLED_SD_YAXIS[(low high)]]\n\
                 \x20                         [TOTAL_N_YAXIS[(low high)] POOLED_SD_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. Each group has a size (a whole number of at least 2) and a mean; all share\n\
                 POOLED_SD. The overall F test of equal means gets its power from the noncentral F with J - 1\n\
                 and N - J df, its effect size being the root mean square standardized effect. COEFFICIENTS\n\
                 add a test of one contrast (noncentral t, one- or two-sided by TEST); coefficients that do not\n\
                 add up to 0 are corrected in the last group (the one before it when only the last is not 0),\n\
                 with a warning. PAIRWISE=TRUE adds a two-sided test of every pair of groups at the significance\n\
                 level split by Bonferroni, Sidak or not at all (LSD). With POWER each test gets its own sizes:\n\
                 the total is searched with the groups in the proportions of GROUP_WEIGHTS (each 1 to 100,\n\
                 equal by default), then every group is rounded up (at least 2); a Group Sizes table lists them.\n\
                 PLOT charts the overall test: TOTAL_N over the total size with the groups kept in proportion\n\
                 (default from the smallest total that keeps every group at 2 or more to twice the table's\n\
                 total), POOLED_SD over the standard deviation (default half to twice the value given), and the\n\
                 pair of _XAXIS and _YAXIS keywords a surface over both.",
        example: "POWER ONEWAY ANOVA /PARAMETERS GROUP_MEANS=48 52 57 POOLED_SD=9 POWER=0.85\n\
                  \x20 GROUP_WEIGHTS=2 1 1 /CONTRAST COEFFICIENTS=-2 1 1 PAIRWISE=TRUE ADJUST=SIDAK.",
        parse: power::oneway_anova,
    },
    CommandSpec {
        name: "POWER PEARSON ONESAMPLE",
        summary: "Power or sample size of a test of one Pearson correlation against a hypothesized value.",
        syntax: "POWER PEARSON ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                       N=n | POWER=powers  [NULL=0]  ALTERNATIVE=r  [ADJUST_BIAS=TRUE | FALSE]\n\
                 \x20                       [/PLOT [N[(low high)]] [NULL] [ALTERNATIVE] [DIFF] [N_XAXIS[(low high)] DIFF_YAXIS]\n\
                 \x20                              [N_YAXIS[(low high)] DIFF_XAXIS] [NULL_XAXIS ALTERNATIVE_YAXIS]\n\
                 \x20                              [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. The test compares Fisher's z of the sample correlation with that of NULL; its\n\
                 standard error is 1 / sqrt(N - 3) and the power comes from the normal approximation. N is a\n\
                 whole number above 3. NULL and ALTERNATIVE lie from -1 to 1; with POWER they lie strictly\n\
                 inside and differ. ADJUST_BIAS=TRUE (the default) adds the small-sample corrections to the\n\
                 mean and spread of z; FALSE leaves them out. A one-sided test looks in the direction of\n\
                 ALTERNATIVE - NULL. PLOT charts power over N, NULL, ALTERNATIVE or DIFF = ALTERNATIVE - NULL\n\
                 (NULL kept), and surfaces over N and DIFF or NULL and ALTERNATIVE; correlation axes run from\n\
                 -0.99 to 0.99.",
        example: "POWER PEARSON ONESAMPLE /PARAMETERS NULL=0.15 ALTERNATIVE=0.4 POWER=0.8 0.9\n\
                  \x20 /PLOT N.",
        parse: power::pearson_onesample,
    },
    CommandSpec {
        name: "POWER SPEARMAN ONESAMPLE",
        summary: "Power or sample size of a test of one Spearman rank correlation against a hypothesized value.",
        syntax: "POWER SPEARMAN ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                        N=n | POWER=powers  [NULL=0]  ALTERNATIVE=r  [VARIANCE=BW | FHP | CC]\n\
                 \x20                        [/PLOT [N[(low high)]] [NULL] [ALTERNATIVE] [DIFF] [N_XAXIS[(low high)] DIFF_YAXIS]\n\
                 \x20                               [N_YAXIS[(low high)] DIFF_XAXIS] [NULL_XAXIS ALTERNATIVE_YAXIS]\n\
                 \x20                               [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. As POWER PEARSON ONESAMPLE, with Fisher's z of the rank correlation, whose\n\
                 variance VARIANCE chooses: BW (Bonett and Wright, the default) (1 + ALTERNATIVE^2 / 2) /\n\
                 (N - 3); FHP (Fieller, Hartley and Pearson) 1.06 / (N - 3); CC (Caruso and Cliff) 1 / (N - 2)\n\
                 + |z of ALTERNATIVE| / (6 N + 4 sqrt(N)). N is above 3, above 2 with CC. There is no bias\n\
                 adjustment.",
        example: "POWER SPEARMAN ONESAMPLE /PARAMETERS ALTERNATIVE=-0.35 N=45 VARIANCE=CC\n\
                  \x20 TEST=DIRECTIONAL /PLOT DIFF.",
        parse: power::spearman_onesample,
    },
    CommandSpec {
        name: "POWER PARTIALCORR",
        summary: "Power or sample size of a test of one partial correlation against a hypothesized value.",
        syntax: "POWER PARTIALCORR /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]\n\
                 \x20                 PARTIALOUT=k  N=n | POWER=powers  [NULL=0]  ALTERNATIVE=r\n\
                 \x20                 [/PLOT [N[(low high)]] [PARTIALOUT] [NULL] [ALTERNATIVE] [DIFF]\n\
                 \x20                        [N_XAXIS[(low high)] DIFF_YAXIS] [N_YAXIS[(low high)] DIFF_XAXIS]\n\
                 \x20                        [NULL_XAXIS ALTERNATIVE_YAXIS] [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. As POWER PEARSON ONESAMPLE with the bias adjustment, for the correlation of two\n\
                 variables with PARTIALOUT others (a whole number, 0 or more) held fixed: each of them costs one\n\
                 degree of freedom, so the standard error of z is 1 / sqrt(N - 3 - k) and N must exceed k + 3.\n\
                 PLOT adds PARTIALOUT, power over the number held fixed (from 0 to N - 4).",
        example: "POWER PARTIALCORR /PARAMETERS PARTIALOUT=4 ALTERNATIVE=0.3 NULL=0.05 N=120\n\
                  \x20 /PLOT PARTIALOUT N_XAXIS DIFF_YAXIS.",
        parse: power::partialcorr,
    },
    CommandSpec {
        name: "POWER UNIVARIATE LINEAR",
        summary: "Power or sample size of the F test of a set of predictors in a linear regression.",
        syntax: "POWER UNIVARIATE LINEAR /PARAMETERS [SIGNIFICANCE=0.05] [MODEL=FIXED | RANDOM]  N=n | POWER=powers\n\
                 \x20                       TOTAL_PREDICTORS=p  TEST_PREDICTORS=q\n\
                 \x20                       PARTIAL_CORR=r | FULL_MODEL=r2 NESTED_MODEL=r2  [INTERCEPT=TRUE | FALSE]\n\
                 \x20                       [/PLOT [N[(low high)]] [PARTIAL_CORR] [N_XAXIS[(low high)] PARTIAL_CORR_YAXIS]\n\
                 \x20                              [N_YAXIS[(low high)] PARTIAL_CORR_XAXIS] [VROTATE(10)] [HROTATE(325)]].\n\
                 Needs no data. A regression with p predictors (besides the intercept) tests whether q of them\n\
                 add nothing to the other p - q. The effect is the multiple partial correlation of the outcome\n\
                 with the q, or comes from the squared multiple correlations of the full and the nested model\n\
                 as (full - nested) / (1 - nested). The F test has q and N - p - 1 df (N - p without the\n\
                 intercept). MODEL=FIXED takes the predictor values as fixed: noncentral F with noncentrality\n\
                 N r^2 / (1 - r^2). RANDOM takes outcome and predictors as jointly normal and uses the exact\n\
                 distribution of the sample squared multiple partial correlation. q is from 1 to p (below p with\n\
                 FULL_MODEL); PARTIAL_CORR lies strictly between -1 and 1 and is not 0 with POWER; the squared\n\
                 correlations lie strictly between 0 and 1, full not below nested (above it with POWER). N is at\n\
                 least p + 2 (p + 1 without the intercept). PLOT charts power over N and over the partial\n\
                 correlation (default 0 to 0.99), and a surface over both.",
        example: "POWER UNIVARIATE LINEAR /PARAMETERS TOTAL_PREDICTORS=7 TEST_PREDICTORS=3 FULL_MODEL=0.31\n\
                  \x20 NESTED_MODEL=0.24 MODEL=RANDOM POWER=0.8 /PLOT N.",
        parse: power::univariate_linear,
    },
    CommandSpec {
        name: "SIMPLAN",
        summary: "A simulation plan: distributions of the inputs, fitted to the data or given, their correlations and the targets of a model.",
        syntax: "SIMPLAN CREATE [SOURCE=AUTOFIT]\n\
                 \x20 [/MODEL FILE='file' [CUSTOM=NO | YES]]\n\
                 \x20 [/TARGETOPTS TARGET=field[(FORMAT=type[,d])] ... [COMMENT='text'] [OUTPUT=YES | NO]\n\
                 \x20             [TARGETFILTER=NO | YES([MIN=LO | value[(INCLUSIVE | EXCLUSIVE)]] [MAX=HI | value[(...)]])]] ...\n\
                 \x20 /SIMINPUT INPUT=field[(MAPTO=name FORMAT=type[,d])] ... [OUTPUT=YES | NO]\n\
                 \x20           [TYPE=AUTOFIT[(DEFAULT | distributions)] | MANUAL] [DISTRIBUTION=name(PARAM=values ...)]\n\
                 \x20           [MINVAL=n] [MAXVAL=n] ...\n\
                 \x20 [/FIXEDINPUT INPUT=field ... VALUE=n | ITERATIONS=VALUES(n ...) | RANGE(MIN=n MAX=n N=k)] ...\n\
                 \x20 [/CORRELATIONS VARORDER=fields CORRMATRIX=1; r, 1; r, r, 1 ...]\n\
                 \x20 [/AUTOFIT [NCASES=ALL | n] [FIT=AD | KS] [MAXITERATIONS=100] [BINS=100]]\n\
                 \x20 [/STOPCRITERIA [MAXCASES=100000] [TARGET=field\n\
                 \x20               TYPE=MEAN(THRESH=n UNIT=PERCENT | VALUE [CILEVEL=95]) |\n\
                 \x20                    TAIL(UNIT=PERCENTILE | VALUE THRESH=n [SIDE=RIGHT | LEFT] FREQUENCY=n)]]\n\
                 \x20 [/MISSING CLASSMISSING=EXCLUDE | INCLUDE] [/VALUELABELS VARIABLES=fields VALUES=value 'label' ...] ...\n\
                 \x20 /PLAN FILE='file' [DISPLAY=YES | NO].\n\
                 SIMPLAN VIEW /PLAN FILE='file'.\n\
                 Distributions and their parameters: BERNOULLI(PROB) BETA(SHAPE1 SHAPE2) BINOM(N PROB)\n\
                 CATEGORICAL(CATS=values PROBS=values) EMPIRICAL EXP(SCALE) GAMMA(SHAPE SCALE) LNORMAL(A B)\n\
                 NEGBIN([TYPE=FAILURES | TRIALS] THRESHOLD PROB) NORMAL(MEAN STDDEV) POISSON(MEAN)\n\
                 TRIANGULAR(MIN MAX MODE) UNIFORM(MIN MAX) USER_RANGES(MIN=values MAX=values PROBS=values)\n\
                 WEIBULL(A B [C]). The SCALE of EXP and GAMMA is the rate, as in the RV and CDF functions; LNORMAL A is\n\
                 the median and B the standard deviation of the logarithm; WEIBULL A is the scale, B the shape and C\n\
                 the origin. The model is a file that BEGIN EXPR wrote, or a plan whose model is taken. Every input\n\
                 of a model is simulated (SIMINPUT) or fixed (FIXEDINPUT); without MODEL the plan only generates the\n\
                 SIMINPUT fields. AUTOFIT (default) fits each input to its field in the active dataset (MAPTO names\n\
                 another field) by maximum likelihood: a nominal field gets the categorical distribution, an ordinal one\n\
                 the binomial, negative binomial (failures) and Poisson by the chi-square statistic, a scale one the\n\
                 beta, exponential, gamma, lognormal, normal, triangular, uniform and two-parameter Weibull by the\n\
                 Anderson-Darling statistic (or Kolmogorov-Smirnov with FIT=KS); the smallest statistic wins and a\n\
                 Goodness of Fit table shows them all with their p-values where known. Weights count; cases missing\n\
                 on any fitted input are left out (user-missing values of a categorical fit count with\n\
                 CLASSMISSING=INCLUDE). Without CORRELATIONS the Pearson correlations of the fields in the data are\n\
                 used. MINVAL and MAXVAL cut the distribution. Several values of one parameter, several PROBS sets or\n\
                 FIXEDINPUT ITERATIONS run one simulation per value (sensitivity analysis). The plan is Statscraft's\n\
                 own JSON. SPLIT FILE is not used. Not supported yet: model files of other procedures (PMML),\n\
                 SOURCE='file', CONTINGENCY MULTIWAY=YES, string inputs.",
        example: "SIMPLAN CREATE /MODEL FILE='margin.json' CUSTOM=YES\n\
                  \x20 /SIMINPUT INPUT=units TYPE=MANUAL DISTRIBUTION=TRIANGULAR(MIN=800 MAX=1400 MODE=1000)\n\
                  \x20 /SIMINPUT INPUT=unit_cost /FIXEDINPUT INPUT=unit_price VALUE=12\n\
                  \x20 /PLAN FILE='margin.splan'.",
        parse: simulation::simplan,
    },
    CommandSpec {
        name: "SIMRUN",
        summary: "Runs a simulation plan: draws the inputs, computes the targets and describes their distributions.",
        syntax: "SIMRUN /PLAN FILE='file' [/CRITERIA [REPRESULTS=TRUE | FALSE] [SEED=629111597]]\n\
                 \x20 [/DISTRIBUTION [DISPLAY=[PDF] [CDF] [NONE]] [CDFORDER=[ASCENDING] [DESCENDING]] [SCALE=...] [CATEGORICAL=...]]\n\
                 \x20 [/SCATTERPLOT [DISPLAY=NO | YES] [ROWSIZE=n]] [/BOXPLOT [DISPLAY=NO | YES] [OVERLAYTARGETS=...]]\n\
                 \x20 [/TORNADO [DISPLAY=YES | NO] [TYPE=[CORR] [DELTASTDDEV(k)] [VARIANCE]]]\n\
                 \x20 [/PRINT [DESCRIPTIVES=YES[(CILEVEL=95)] | NO] [PERCENTILES=NO | YES([NTILES=k] [PTILES=values])]\n\
                 \x20         [ASSOCIATIONS=NO | YES]]\n\
                 \x20 [/OPTIONS VIZSTYLESHEET='name'] [/OUTFILE FILE='file' | dataset | *].\n\
                 Each case draws standard normal numbers from a Mersenne Twister started by SEED, correlates them\n\
                 (the correlation of each pair of normals is searched so that the inputs, after the inverse\n\
                 distribution function, reach the planned correlation) and turns them into the inputs; the model's\n\
                 expressions then give the targets. Cases are drawn up to MAXCASES, or until the stopping rule\n\
                 of the plan holds, for each value of a sensitivity analysis. Tables: Target Statistics (mean,\n\
                 standard deviation, median, minimum, maximum, the confidence interval of the mean, 5th and 95th\n\
                 percentiles), Target Categories for an ordinal or nominal target, Input Statistics, Target\n\
                 Percentiles, Correlations of Inputs (ASSOCIATIONS), and the tornado tables: the correlation with\n\
                 each input, the mean change of the target when an input moves DELTASTDDEV standard deviations\n\
                 down and up, and each input's total share of the target's variance (Sobol, from a second\n\
                 sample). Charts: the histogram (PDF) and the cumulative distribution (CDF) of each target, the\n\
                 boxplot, scatterplots of each target with each input (the first 5000 cases) and tornado bar\n\
                 charts. SCALE, CATEGORICAL, ROWSIZE, OVERLAYTARGETS and OPTIONS are accepted and change\n\
                 nothing. OUTFILE writes every case: the targets, the inputs and, with a sensitivity analysis,\n\
                 Iteration. Without a model only the input statistics, ASSOCIATIONS and OUTFILE apply.",
        example: "SIMRUN /PLAN FILE='margin.splan' /CRITERIA SEED=4711\n\
                  \x20 /TORNADO TYPE=CORR VARIANCE /PRINT PERCENTILES=YES(NTILES=4) /OUTFILE FILE='draws.sav'.",
        parse: simulation::simrun,
    },
    CommandSpec {
        name: "BEGIN EXPR",
        summary: "Starts a block of expressions that END EXPR saves as a model for SIMPLAN.",
        syntax: "BEGIN EXPR /OUTFILE PREPXML='file'.\n\
                 NUMERIC and STRING, VARIABLE LEVEL, VALUE LABELS and COMPUTE commands, then END EXPR.\n\
                 Nothing in the block touches the active dataset. Each COMPUTE gives a target, in order, and may\n\
                 use the targets before it; a variable is computed once. NUMERIC names the inputs the active\n\
                 dataset lacks, and only those. Targets and declared inputs are\n\
                 scale unless VARIABLE LEVEL says otherwise. The file is Statscraft's own JSON (not XML); a name\n\
                 starting with *\\ is kept in memory for the session.",
        example: "BEGIN EXPR /OUTFILE PREPXML='margin.json'.\n\
                  NUMERIC units unit_price.\n\
                  COMPUTE margin = units * (unit_price - unit_cost).\n\
                  END EXPR.",
        parse: simulation::begin_expr,
    },
    CommandSpec {
        name: "END EXPR",
        summary: "Ends the BEGIN EXPR block and writes its model file.",
        syntax: "END EXPR.",
        example: "END EXPR.",
        parse: simulation::end_expr,
    },
    CommandSpec {
        name: "FACTOR",
        summary: "Factor analysis: principal components and six factoring methods, rotations, factor scores.",
        syntax: "FACTOR VARIABLES=varlist | MATRIX=IN(COR=* | 'file' | COV=* | 'file')\n\
                 \x20      [/MISSING=[LISTWISE | PAIRWISE | MEANSUB] [INCLUDE]] [/SELECT=var(value)]\n\
                 \x20      [/ANALYSIS=varlist] [/METHOD=CORRELATION]\n\
                 \x20      [/PRINT=[DEFAULT] [INITIAL] [EXTRACTION] [ROTATION] [UNIVARIATE] [CORRELATION] [COVARIANCE]\n\
                 \x20              [SIG] [DET] [INV] [REPR] [AIC] [KMO] [FSCORE] [ALL]]\n\
                 \x20      [/FORMAT=[SORT] [BLANK(n)] [DEFAULT]] [/DIAGONAL=values | DEFAULT]\n\
                 \x20      [/PLOT=[EIGEN] [ROTATION[(f1,f2) ...]]]\n\
                 \x20      [/CRITERIA=[FACTORS(n)] [MINEIGEN(1)] [ITERATE(25)] [ECONVERGE(.001)] [RCONVERGE(.0001)]\n\
                 \x20                 [KAISER | NOKAISER] [DELTA(0)] [KAPPA(4)] [DEFAULT]]\n\
                 \x20      [/EXTRACTION=PC | PAF | ALPHA | IMAGE | ULS | GLS | ML]\n\
                 \x20      [/ROTATION=VARIMAX | EQUAMAX | QUARTIMAX | OBLIMIN[(delta)] | PROMAX[(kappa)] | NOROTATE]\n\
                 \x20      [/SAVE=[REG | BART | AR] [(n | ALL [rootname])]].\n\
                 The analysis is of the correlation matrix of the ANALYSIS variables (default all of VARIABLES).\n\
                 LISTWISE (default) uses the cases valid on every variable of VARIABLES; PAIRWISE each pair's own\n\
                 cases; MEANSUB puts the mean in place of a missing value. SELECT computes the matrix from the\n\
                 cases with that value, while scores are saved for all cases. CRITERIA applies to the EXTRACTION\n\
                 and ROTATION that follow it. The number of factors is FACTORS(n), else the count of eigenvalues\n\
                 above MINEIGEN. Without EXTRACTION the extraction is PC with VARIMAX; with EXTRACTION but no\n\
                 ROTATION the factors are not rotated. A later ANALYSIS cancels the PRINT, DIAGONAL, EXTRACTION,\n\
                 ROTATION and SAVE before it, with a warning. Tables: Descriptive Statistics (UNIVARIATE),\n\
                 Correlation Matrix (CORRELATION, SIG, DET), Inverse of Correlation Matrix (INV), Covariance Matrix,\n\
                 KMO and Bartlett's Test (KMO), Anti-image Matrices (AIC), Communalities, Total Variance Explained,\n\
                 Component or Factor Matrix, Goodness-of-fit Test (ML, GLS), Reproduced Correlations (REPR), the\n\
                 rotated matrix with its transformation matrix, or Pattern, Structure and factor correlation\n\
                 matrices for OBLIMIN and PROMAX, Score Coefficient and Score Covariance Matrix (FSCORE). SORT\n\
                 orders the variables by the factor of their largest loading; BLANK(n) leaves out loadings below\n\
                 n in absolute value. SAVE adds scores by regression, Bartlett or Anderson-Rubin, named FAC1_1,\n\
                 FAC2_1 and so on, or rootname1, rootname2. MATRIX=IN reads a matrix file (ROWTYPE_ N, MEAN,\n\
                 STDDEV, CORR or COV, VARNAME_) from the active dataset or a file. Weighted when a weight variable\n\
                 is set. PLOT gives chart data: EIGEN the scree plot, every eigenvalue of the correlation matrix\n\
                 against its number; ROTATION the loading plot, each variable at its loadings (rotated when there\n\
                 is a rotation) on the first two factors, with the third as depth when there are three or more, or\n\
                 one plot per pair given. Not supported yet: METHOD=COVARIANCE, MATRIX=OUT, MATRIX=IN(FAC=...).",
        example: "FACTOR VARIABLES=q1 TO q10 /PRINT=INITIAL EXTRACTION ROTATION KMO /FORMAT=SORT BLANK(.3) /CRITERIA=MINEIGEN(1) /EXTRACTION=PC /ROTATION=VARIMAX /SAVE=REG(ALL).",
        parse: factor::factor,
    },
    CommandSpec {
        name: "QUICK CLUSTER",
        summary: "K-means clustering of cases into a given number of clusters.",
        syntax: "QUICK CLUSTER varlist | ALL [/MISSING=[LISTWISE | PAIRWISE | DEFAULT] [INCLUDE]]\n\
                 \x20             [/FILE='file' | dataset] [/INITIAL=(values)]\n\
                 \x20             [/CRITERIA=[CLUSTER(2)] [NOINITIAL] [MXITER(10)] [CONVERGE(0)]]\n\
                 \x20             [/METHOD=KMEANS[(NOUPDATE | UPDATE)] | CLASSIFY]\n\
                 \x20             [/PRINT=[INITIAL] [CLUSTER] [ID(var)] [DISTANCE] [ANOVA] [NONE]]\n\
                 \x20             [/OUTFILE='file' | dataset] [/SAVE=[CLUSTER[(name)]] [DISTANCE[(name)]]].\n\
                 Distances are Euclidean. Initial centers: by default the first k complete cases, then a case\n\
                 replaces a center when it lies farther from the centers than the two closest centers lie from\n\
                 each other (NOINITIAL keeps the first k cases); INITIAL gives them cluster by cluster, FILE reads\n\
                 them from the clustering variables of a data file. KMEANS moves each center to the mean of its\n\
                 cases after each pass (UPDATE after each case) until no center moves more than CONVERGE times the\n\
                 smallest distance between the initial centers, or MXITER passes; CLASSIFY keeps the initial\n\
                 centers. Then every case goes to its nearest center, and the final centers are the means of\n\
                 their cases. Tables: Initial Cluster Centers, Iteration History, Cluster Membership (CLUSTER,\n\
                 with ID), Final Cluster Centers, Distances between Final Cluster Centers (DISTANCE), ANOVA and\n\
                 Number of Cases in each Cluster. LISTWISE (default) leaves out a case missing on any variable;\n\
                 PAIRWISE uses the variables a case has. SAVE adds the cluster number and the distance to the\n\
                 classification center, named QCL_1, QCL_2 and so on unless given. OUTFILE writes the final\n\
                 centers with CLUSTER_. Weighted when a weight variable is set.",
        example: "QUICK CLUSTER q1 TO q5 /CRITERIA=CLUSTER(3) MXITER(20) /PRINT=INITIAL ANOVA /SAVE=CLUSTER(segment) DISTANCE.",
        parse: factor::quick_cluster,
    },
    CommandSpec {
        name: "TWOSTEP CLUSTER",
        summary: "Clusters of cases from continuous and categorical variables, the number of clusters chosen by BIC or AIC.",
        syntax: "TWOSTEP CLUSTER [/CONTINUOUS VARIABLES=varlist] [/CATEGORICAL VARIABLES=varlist]\n\
                 \x20               [/DISTANCE=LIKELIHOOD | EUCLIDEAN] [/NUMCLUSTERS=AUTO [15] [BIC | AIC] | FIXED=n]\n\
                 \x20               [/CRITERIA=[INITHRESHOLD(0)] [MXBRANCH(8)] [MXLEVEL(3)]] [/HANDLENOISE=0]\n\
                 \x20               [/NOSTANDARDIZE [VARIABLES=varlist]] [/MISSING=EXCLUDE | INCLUDE] [/MEMALLOCATE=64]\n\
                 \x20               [/PRINT=[IC] [COUNT] [SUMMARY]] [/VIEWMODEL [DISPLAY=YES | NO] [EVALUATIONFIELDS=varlist]]\n\
                 \x20               [/SAVE CLUSTER [VARIABLE=name]] [/OUTFILE [MODEL='file'] [STATE='file']].\n\
                 Step one reads the cases in file order into a tree of small subclusters: a case joins the\n\
                 nearest subcluster when it lies within the threshold, else it starts its own; a node holds at\n\
                 most MXBRANCH entries and the tree at most MXLEVEL levels below its root. A full tree is rebuilt\n\
                 with a larger threshold; with HANDLENOISE p it first puts aside the subclusters smaller than p\n\
                 percent of the largest, takes back those that later fit and calls the rest noise. Step two\n\
                 joins the subclusters, nearest pair first. LIKELIHOOD (default) measures how much the\n\
                 log-likelihood drops when two groups join, with normal continuous and multinomial categorical\n\
                 variables; EUCLIDEAN, the distance between group means, takes continuous variables only. AUTO\n\
                 compares 1 to 15 clusters (or the maximum given): BIC (default) or AIC gives a first count, at\n\
                 the first step whose change is less than 0.04 of the change from 1 to 2 clusters, and the\n\
                 ratios of the join distances settle it. Then every case goes to its nearest cluster; when the\n\
                 tree filled under HANDLENOISE, a case too far from every cluster goes to cluster -1. Continuous\n\
                 variables are standardized unless NOSTANDARDIZE names them; a constant one is left out.\n\
                 Tables: Model Summary (unless DISPLAY=NO), Auto-Clustering (IC), Cluster Distribution (COUNT),\n\
                 Centroids and one frequency table per categorical variable (SUMMARY). Cases missing on any\n\
                 variable are left out; the weight variable is not used. SAVE adds the cluster numbers, named\n\
                 TSC_1, TSC_2 and so on unless given. OUTFILE writes Statscraft's own JSON: MODEL the variables,\n\
                 their standardization and the final clusters, STATE the subclusters and settings of the tree\n\
                 (not with SPLIT FILE). The result depends on the order of the cases. DMCLUSTER is another\n\
                 name for the command. Not supported yet: INFILE.",
        example: "TWOSTEP CLUSTER /CONTINUOUS VARIABLES=spend visits /CATEGORICAL VARIABLES=region plan\n\
                  \x20 /NUMCLUSTERS=AUTO 8 BIC /PRINT=COUNT SUMMARY IC /SAVE CLUSTER VARIABLE=segment.",
        parse: twostep::twostep,
    },
    CommandSpec {
        name: "DMCLUSTER",
        summary: "Another name for TWOSTEP CLUSTER.",
        syntax: "DMCLUSTER takes the subcommands of TWOSTEP CLUSTER and runs it.",
        example: "DMCLUSTER /CONTINUOUS VARIABLES=spend visits /PRINT=COUNT.",
        parse: twostep::twostep,
    },
    CommandSpec {
        name: "DETECTANOMALY",
        summary: "Unusual cases: peer groups by two-step clustering, an anomaly index and the variables behind it.",
        syntax: "DETECTANOMALY [/VARIABLES [CATEGORICAL=varlist] [SCALE=varlist] [ID=var] [EXCEPT=varlist]]\n\
                 \x20             [/HANDLEMISSING [APPLY=NO | YES] [CREATEMISPROPVAR=NO | YES]]\n\
                 \x20             [/CRITERIA [MINNUMPEERS=1] [MAXNUMPEERS=15] [MLWEIGHT=6] [NUMREASONS=1]\n\
                 \x20                        [PCTANOMALOUSCASES=5] [NUMANOMALOUSCASES=n] [ANOMALYCUTPOINT=2 | NONE]]\n\
                 \x20             [/SAVE [ANOMALY[(name)]] [PEERID[(name)]] [PEERSIZE[(name)]] [PEERPCTSIZE[(name)]]\n\
                 \x20                    [REASONVAR[(root)]] [REASONMEASURE[(root)]] [REASONVALUE[(root)]] [REASONNORM[(root)]]]\n\
                 \x20             [/PRINT [CPS] [ANOMALYLIST] [NORMS] [ANOMALYSUMMARY] [REASONSUMMARY] [NONE]] [/OUTFILE MODEL='file'].\n\
                 Without CATEGORICAL and SCALE every variable but ID and EXCEPT is used, strings and nominal or\n\
                 ordinal variables as categorical. Keywords are written in full, each subcommand once. The cases\n\
                 are grouped into peer groups by two-step clustering (log-likelihood distance, the variances\n\
                 eased by the variable's overall variance over MLWEIGHT, BIC choosing between MINNUMPEERS and\n\
                 MAXNUMPEERS). For each case, each variable's share of the distance between the case and its\n\
                 peer group measures how far it strays; their sum over the peer group's mean sum is the anomaly\n\
                 index, and a variable's share of the sum its impact. The anomaly list takes the cases of\n\
                 highest index, PCTANOMALOUSCASES percent (rounded down) or NUMANOMALOUSCASES of them, keeping\n\
                 those with an index of at least ANOMALYCUTPOINT; its reasons are each case's NUMREASONS\n\
                 variables of largest impact, with the value and the peer group's norm (mean or mode).\n\
                 Missing values leave the case out unless APPLY=YES, which puts the mean in for a continuous\n\
                 variable and makes missing a category of a categorical one; CREATEMISPROPVAR=YES then adds the\n\
                 share of missing values as a variable. A variable that does not vary is left out. Tables:\n\
                 Case Processing Summary (CPS), the anomaly index, peer and reason lists (ANOMALYLIST, the\n\
                 default), Scale and Categorical Variable Norms (NORMS), Anomaly Index Summary, Reason Summary.\n\
                 SAVE adds AnomalyIndex, PeerId, PeerSize, PeerPctSize and per reason k ReasonVar_k,\n\
                 ReasonMeasure_k, ReasonValue_k and ReasonNorm_k, a free suffix added to a default name in use.\n\
                 WEIGHT and SPLIT FILE are not used. OUTFILE MODEL writes Statscraft's own JSON: the variables\n\
                 with their means and categories, and the peer groups.",
        example: "DETECTANOMALY /VARIABLES SCALE=amount days CATEGORICAL=channel ID=order_no\n\
                  \x20 /CRITERIA NUMREASONS=2 PCTANOMALOUSCASES=2 /SAVE ANOMALY PEERID REASONVAR /PRINT CPS ANOMALYLIST NORMS.",
        parse: twostep::detectanomaly,
    },
    CommandSpec {
        name: "ADP",
        summary: "Prepares a target and its inputs for modelling and writes the preparation as syntax.",
        syntax: "ADP /FIELDS [TARGET=var] INPUT=varlist [ANALYSISWEIGHT=var]\n\
                 \x20   [/PREPDATETIME [DATEDURATION=YES[(REFERENCE=CURRENT | YMD('y-m-d') UNIT=AUTO | YEARS | MONTHS | DAYS[(SUFFIX='s')])] | NO]\n\
                 \x20                  [TIMEDURATION=YES[(REFERENCE=CURRENT | HMS('h:m:s') UNIT=AUTO | HOURS | MINUTES | SECONDS[(SUFFIX='s')])] | NO]\n\
                 \x20                  [EXTRACTYEAR | EXTRACTMONTH | EXTRACTDAY | EXTRACTHOUR | EXTRACTMINUTE | EXTRACTSECOND=NO | YES[(SUFFIX='s')]]]\n\
                 \x20   [/SCREENING [PCTMISSING=YES[(MAXPCT=50)] | NO] [UNIQUECAT=YES[(MAXCAT=100)] | NO] [SINGLECAT=NO | YES[(MAXPCT=95)]]]\n\
                 \x20   [/ADJUSTLEVEL [INPUT=YES | NO] [TARGET=YES | NO] [MAXVALORDINAL=10] [MINVALCONTINUOUS=5]]\n\
                 \x20   [/OUTLIERHANDLING [INPUT=YES | NO] [TARGET=YES | NO] [CUTOFF=SD(3)] [REPLACEWITH=CUTOFFVALUE | MISSING]]\n\
                 \x20   [/REPLACEMISSING [INPUT=YES[(EXCLUDE([CONTINUOUS] [NOMINAL] [ORDINAL]))] | NO] [TARGET=NO | YES[(EXCLUDE(...))]]]\n\
                 \x20   [/REORDERNOMINAL [INPUT=NO | YES] [TARGET=NO | YES]]\n\
                 \x20   [/RESCALE [INPUT=ZSCORE[(MEAN=0 SD=1)] | MINMAX[(MIN=0 MAX=100)] | NONE] [TARGET=BOXCOX[(MEAN=0 SD=1)] | NONE]]\n\
                 \x20   [/TRANSFORM [MERGESUPERVISED=YES[(PVALUE=0.05)] | NO] [MERGEUNSUPERVISED=NO | YES[([ORDINAL] [NOMINAL] [MINPCT=10])]]\n\
                 \x20               [BINNING=SUPERVISED[(PVALUE=0.05)] | NONE] [SELECTION=YES[(PVALUE=0.05)] | NO] [CONSTRUCTION=NO]]\n\
                 \x20   [/CRITERIA SUFFIX([TARGET='_transformed'] [INPUT='_transformed'])]\n\
                 \x20   /OUTFILE PREPXML='file'.\n\
                 Every word is written in full and each subcommand given once. ADP looks at the data and\n\
                 writes syntax (RECODE, COMPUTE, IF and VARIABLE LEVEL) that makes a prepared copy of each\n\
                 field it changes, named with the suffix; run that file to add the fields. The data are not\n\
                 changed by ADP itself, and the file holds syntax rather than XML. The steps, in order: date\n\
                 inputs become the time since the reference date (AUTO: days below 31 days, months of 30.4375\n\
                 days below 366, else years of 365.25 days) and time inputs the time since the reference time\n\
                 (seconds below 60, minutes below 3600, else hours), with the parts asked for as ordinal\n\
                 fields; the date or time field itself is dropped. Screening drops fields with too many\n\
                 missing values, a single value, one dominant category or too many nominal categories. Levels:\n\
                 continuous fields with few values become ordinal, ordinal fields with many continuous.\n\
                 Outliers of continuous fields lie more than CUTOFF robust SDs (found after setting aside thin\n\
                 tails of at most 5% of the cases) from the robust mean. Missing values take the mean, median\n\
                 or mode by level. A nominal\n\
                 target can be renumbered by count; a continuous target goes through Box-Cox (lambda -3 to 3\n\
                 by 0.5) and then to MEAN and SD. Continuous inputs get z scores or min-max. With a target,\n\
                 categories of an input whose relation to the target does not differ at PVALUE are merged\n\
                 (F test, chi-square or the row effects test by the target's level) and an input not related\n\
                 to the target is dropped; continuous inputs are binned at the Scheffe subsets of the\n\
                 target classes' means for a categorical target, or dropped when their correlation with a\n\
                 continuous target is not significant. Without a target, MERGEUNSUPERVISED joins small\n\
                 neighbouring categories until each holds MINPCT of the cases. CONSTRUCTION=YES is not\n\
                 supported yet. Frequency weights are rounded; analysis weights weigh means and spreads.\n\
                 SPLIT FILE is not used. The table lists each field, its prepared field and the actions.",
        example: "ADP /FIELDS TARGET=churned INPUT=tenure plan region signup\n\
                  \x20 /PREPDATETIME DATEDURATION=YES(REFERENCE=YMD('2025-01-01') UNIT=MONTHS)\n\
                  \x20 /OUTFILE PREPXML='prepare.sps'.",
        parse: adp::adp,
    },
    CommandSpec {
        name: "OPTIMAL BINNING",
        summary: "Bins of scale variables: cuts chosen by MDLP against a guide variable, or by equal frequency.",
        syntax: "OPTIMAL BINNING /VARIABLES [GUIDE=var] BIN=varlist [SAVE=NO | YES [(INTO=new varlist)]]\n\
                 \x20               [/CRITERIA [PREPROCESS=EQUALFREQ[(BINS=1000)] | NONE] [METHOD=MDLP | EQUALFREQ[(BINS=10)]]\n\
                 \x20                          [LOWEREND=UNBOUNDED | OBSERVED] [UPPEREND=UNBOUNDED | OBSERVED]\n\
                 \x20                          [LOWERLIMIT=INCLUSIVE | EXCLUSIVE] [FORCEMERGE=0]]\n\
                 \x20               [/MISSING SCOPE=PAIRWISE | LISTWISE] [/OUTFILE RULES='file']\n\
                 \x20               [/PRINT [ENDPOINTS] [DESCRIPTIVES] [ENTROPY] [NONE]].\n\
                 Every word is written in full and each subcommand given once. MDLP, the default when GUIDE\n\
                 names a numeric or string variable (at most 256 values), splits a range of values in two at\n\
                 the point that best separates the guide's values, keeps the split when its information gain\n\
                 beats the minimum description length cost, and goes on in both halves. PREPROCESS first groups\n\
                 a variable with more distinct values than BINS into that many equal frequency groups, and MDLP\n\
                 splits only between groups. EQUALFREQ, the default without GUIDE, cuts at the 1/n, 2/n and\n\
                 further percentiles (empirical distribution with averaging); a variable with at most n distinct\n\
                 values gets a bin for each. FORCEMERGE r joins an inner bin smaller than r times its smaller\n\
                 neighbour to the neighbour that leaves the lower class entropy, in one pass. End points are\n\
                 data values: with LOWERLIMIT=INCLUSIVE a bin runs from its lower end point up to but not\n\
                 including its upper one, with EXCLUSIVE from above its lower one up to and including its upper\n\
                 one; the outer ends are unbounded, or the observed minimum and maximum with OBSERVED.\n\
                 PAIRWISE uses each variable's valid cases (with a valid guide for MDLP), LISTWISE the cases\n\
                 valid on all of them; user-missing values are missing. Weights are rounded to whole numbers.\n\
                 Tables: End Points with the cases of each bin (ENDPOINTS, the default), Descriptive Statistics\n\
                 and Guide Variable Distribution (DESCRIPTIVES), Model Entropy (ENTROPY, MDLP only). SAVE adds\n\
                 the bin numbers of every case with a valid value as guide_var_bin (var_bin without a guide)\n\
                 or the INTO names. RULES writes syntax with the same bins for other data. SPLIT FILE is not used.",
        example: "OPTIMAL BINNING /VARIABLES GUIDE=churned BIN=tenure spend SAVE=YES\n\
                  \x20 /CRITERIA FORCEMERGE=0.05 /OUTFILE RULES='bins.sps' /PRINT ENDPOINTS ENTROPY.",
        parse: optimal_binning::optimal_binning,
    },
    CommandSpec {
        name: "KNN",
        summary: "Nearest neighbor analysis: classify or predict a target from the k most similar training cases.",
        syntax: "KNN [target [(MLEVEL=S | O | N)]] [BY factors] [WITH covariates]\n\
                 \x20   [/EXCEPT VARIABLES=varlist] [/CASELABELS VARIABLE=var] [/FOCALCASES VARIABLE=var]\n\
                 \x20   [/RESCALE COVARIATE=ADJNORMALIZED | NONE] [/PARTITION TRAINING=70 HOLDOUT=30 | VARIABLE=var]\n\
                 \x20   [/MODEL [METRIC=EUCLID | CITYBLOCK] [NEIGHBORS=FIXED(K=3) | AUTO(KMIN=3, KMAX=5)]\n\
                 \x20           [FEATURES=ALL | AUTO(FORCE=varlist)]]\n\
                 \x20   [/CRITERIA [NUMFEATURES=AUTO | FIXED(n) | ERRORRATIO(MINCHANGE=0.01)] [PREDICTED=MEAN | MEDIAN]\n\
                 \x20              [WEIGHTFEATURES=NO | YES]] [/CROSSVALIDATION FOLDS=10 | VARIABLE=var]\n\
                 \x20   [/MISSING USERMISSING=EXCLUDE | INCLUDE] [/VIEWMODEL DISPLAY=YES | NO] [/PRINT CPS | NONE]\n\
                 \x20   [/SAVE [PREDVAL[(name)]] [PREDPROB[(root)]] [PARTITION[(name)]] [FOLD[(name)]] [MAXCAT(25)]]\n\
                 \x20   [/OUTFILE [MODEL='file'] [FOCALCASES='file' | dataset]].\n\
                 Names and keywords are written in full, each subcommand once and never empty. Factors are\n\
                 coded with one 0/1 column per category seen in training; covariates are mapped onto -1 to 1\n\
                 by the training minimum and maximum unless COVARIATE=NONE. Cases missing on the target or a\n\
                 predictor are left out (INCLUDE keeps user-missing codes of categorical ones). The rest are\n\
                 split at random into training and holdout cases by the TRAINING:HOLDOUT ratio (SET SEED makes\n\
                 it repeatable), or by VARIABLE: above 0 trains. A case's neighbors are the k training cases\n\
                 closest to it by EUCLID or CITYBLOCK distance, the case itself never among them; ties keep\n\
                 file order. A categorical target gets the commonest neighbor category (ties: the category\n\
                 with more training cases, then the lower value) and probabilities (count + 1) / (k + J) over\n\
                 the J training categories; a scale target gets the neighbors' MEAN or MEDIAN. NEIGHBORS=AUTO\n\
                 tries each k from KMIN to KMAX: by FOLDS-fold cross-validation of the training cases, or with\n\
                 FEATURES=AUTO by the error left after feature selection. FEATURES=AUTO starts from the FORCE\n\
                 list and adds, one at a time, the predictor whose model errs least, until NUMFEATURES are added\n\
                 (AUTO: up to 20 in all) or, with ERRORRATIO, the relative change of the error is small.\n\
                 Importance: the error without the predictor plus 1/m, scaled to sum 1; WEIGHTFEATURES=YES\n\
                 multiplies each distance term by it. Tables: Case Processing Summary (CPS), then unless\n\
                 DISPLAY=NO Model Summary, the k or feature selection error log, Feature Importance, Error\n\
                 Summary, Classification Table, and the Nearest Neighbors and Distances of focal cases\n\
                 (FOCALCASES variable above 0, named by CASELABELS). SAVE adds KNN_PredictedValue,\n\
                 KNN_Probability_1 to _MAXCAT, KNN_Partition (1 training, 0 holdout) and KNN_Fold. OUTFILE\n\
                 MODEL writes a JSON model; FOCALCASES writes each focal case's neighbors and distances. WEIGHT\n\
                 is ignored with a warning; SPLIT FILE gives one analysis per group.",
        example: "KNN plan (MLEVEL=N) BY region WITH age income /PARTITION TRAINING=8 HOLDOUT=2\n\
                  \x20 /MODEL NEIGHBORS=AUTO(KMIN=2, KMAX=6) /CROSSVALIDATION FOLDS=5 /SAVE PREDVAL PREDPROB.",
        parse: knn::knn,
    },
    CommandSpec {
        name: "NAIVEBAYES",
        summary: "Naive Bayes classification of a categorical target, with forward selection of the predictors.",
        syntax: "NAIVEBAYES target [BY factors] [WITH covariates]\n\
                 \x20          [/EXCEPT VARIABLES=varlist] [/FORCE [FACTORS=varlist] [COVARIATES=varlist]]\n\
                 \x20          [/TRAININGSAMPLE PERCENT=p | VARIABLE=var]\n\
                 \x20          [/SUBSET MAXSIZE=AUTO | n [(BESTSUBSET=PSEUDOBIC | TESTDATA)] | EXACTSIZE=n | NOSELECTION]\n\
                 \x20          [/CRITERIA [BINS=10] [MEMALLOCATE=1024] [TIMER=5]] [/MISSING USERMISSING=EXCLUDE | INCLUDE]\n\
                 \x20          [/PRINT [CPS] [EXCLUDED] [SUMMARY] [SELECTED] [CLASSIFICATION] | NONE]\n\
                 \x20          [/SAVE [PREDVAL[(name)]] [PREDPROB[(root[:n])]]] [/OUTFILE MODEL='file'].\n\
                 Without BY and WITH every other variable is a predictor (strings, nominal and ordinal ones as\n\
                 factors, scale ones as covariates), except the weight, the TRAININGSAMPLE variable and EXCEPT.\n\
                 Keywords are written in full, each subcommand once. A covariate is cut into BINS intervals of\n\
                 equal width over the training cases, empty inner intervals merged with their neighbours. Each\n\
                 probability is a smoothed count: (count + 1/N) over (total + categories/N), N the training\n\
                 cases. Selection starts from the FORCE predictors and adds, step by step, the one that raises\n\
                 the mean log-likelihood of the training cases most, up to EXACTSIZE more or the MAXSIZE limit\n\
                 (AUTO: 20, or a fifth of the predictors when that is more, at most 100); of the subsets passed\n\
                 the best has the lowest pseudo-BIC (minus the mean log-likelihood plus J log(N) / 2N), or with\n\
                 TESTDATA the highest mean log-likelihood of the test cases (the default when TRAININGSAMPLE\n\
                 splits the data). NOSELECTION uses every predictor. A case is put in the target category of\n\
                 highest posterior probability; a missing predictor is left out of a case's product. Cases\n\
                 without a target or without any predictor are left out, as are predictors that are empty or\n\
                 constant. PERCENT draws the training cases from the SET SEED generator; VARIABLE takes cases\n\
                 equal to 1. Frequency weights are rounded (below 0.5 left out). SPLIT FILE is not used.\n\
                 SAVE adds PredictedValue and PredictedProbability_1 to _n (25 by default) for the first n\n\
                 target categories. OUTFILE MODEL writes Statscraft's own JSON model. MEMALLOCATE and TIMER\n\
                 are checked; every case is kept in memory.",
        example: "NAIVEBAYES churn BY region plan WITH tenure spend /TRAININGSAMPLE PERCENT=70\n\
                  \x20 /SUBSET MAXSIZE=3 /CRITERIA BINS=5 /SAVE PREDVAL PREDPROB(prob).",
        parse: naivebayes::naivebayes,
    },
    CommandSpec {
        name: "AIM",
        summary: "Attribute importance: how much each variable sets every group (cluster) apart from all cases.",
        syntax: "AIM groupvar [/CATEGORICAL varlist] [/CONTINUOUS varlist]\n\
                 \x20   [/CRITERIA [ADJUST=BONFERRONI | NONE] [CI=95] [HIDENOTSIG=NO | YES] [SHOWREFLINE=YES | NO]]\n\
                 \x20   [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20   [/PLOT [CATEGORY] [CLUSTER[(TYPE=BAR | PIE)]] [ERRORBAR] [IMPORTANCE[([X=GROUP | VARIABLE] [Y=TEST | PVALUE])]]].\n\
                 The group variable (numeric or string) comes first; CATEGORICAL takes variables of any type,\n\
                 CONTINUOUS numeric ones, and at least one of them is needed. For every group and variable a\n\
                 test compares the group with all cases: a categorical variable by the chi-square of the\n\
                 group's category counts against the shares of all cases (categories less one degrees of\n\
                 freedom), a continuous one by Student's t of the group mean against the overall mean, over\n\
                 the group's own standard deviation (group size less one degrees of freedom, two-sided). A\n\
                 group whose values are all equal gets an infinite t and significance 0. The significance\n\
                 level is 100 less CI percent; BONFERRONI (default) divides it by the number of groups, for\n\
                 the tests and for the error bar intervals. Tables: one row per variable and group with the\n\
                 statistic, degrees of freedom, significance and whether it passes the level. PLOT CATEGORY\n\
                 shows each categorical variable's category shares within the groups, CLUSTER the size of\n\
                 the groups as bars or a pie, ERRORBAR the group means with their intervals, IMPORTANCE the\n\
                 statistics (Y=TEST) or -log10 of the significance (Y=PVALUE), one chart per variable with\n\
                 the groups along the axis (X=GROUP) or one per group with the variables (X=VARIABLE).\n\
                 HIDENOTSIG=YES leaves out of the charts what does not pass the level; SHOWREFLINE adds the\n\
                 critical values, or the overall mean to the error bars. A case missing on any of the named\n\
                 variables is left out; INCLUDE keeps user-missing values. The weight variable is not used.\n\
                 Subcommands may be given in any order; a repeated one replaces the earlier.",
        example: "AIM segment /CATEGORICAL region plan /CONTINUOUS spend visits\n\
                  \x20 /CRITERIA CI=90 /PLOT CLUSTER(TYPE=PIE) ERRORBAR IMPORTANCE(X=VARIABLE Y=PVALUE).",
        parse: aim::aim,
    },
    CommandSpec {
        name: "SELECTPRED",
        summary: "Screens a long list of predictors and ranks them one by one by how well each relates to a target.",
        syntax: "SELECTPRED target [(MLEVEL={S | O | N})] [BY factors] [WITH covariates]\n\
                 \x20   [/EXCEPT VARIABLES=varlist]\n\
                 \x20   [/SCREENING [STATUS={ON | OFF}] [PCTMISSING=70] [PCTEQUAL=95] [PCTUNEQUAL=90] [STDDEV=0] [CV=0.001]]\n\
                 \x20   [/CRITERIA [SIZE={AUTO | n}] [PCUTOFF=0.05] [RANKING={PEARSONCHISQ | LRCHISQ | CRAMERSV | LAMBDA}]\n\
                 \x20              [TIMER=5] [SHOWUNSELECTED=0]]\n\
                 \x20   [/MISSING USERMISSING={EXCLUDE | INCLUDE}]\n\
                 \x20   [/PRINT [CPS] [EXCLUDED] [SELECTED] [SUMMARY[(PEARSONCHISQ LRCHISQ CRAMERSV LAMBDA)]] [NONE]]\n\
                 \x20   [/PLOT SUMMARY].\n\
                 The target is categorical when nominal or ordinal (by MLEVEL or the dictionary), else scale.\n\
                 Factors (BY) are categorical predictors, covariates (WITH, numeric) scale ones; without BY\n\
                 and WITH every other variable but the weight is a predictor, its kind from the dictionary, and\n\
                 EXCEPT leaves some out. Cases missing on the target or on every predictor are left out.\n\
                 Predictors missing or constant in every case are left out, and with screening on also those\n\
                 with more than PCTMISSING percent missing, categorical ones with one category over PCTEQUAL\n\
                 percent of their cases or over PCTUNEQUAL percent of categories holding a single case, and\n\
                 scale ones with a standard deviation under STDDEV or an absolute coefficient of variation under\n\
                 CV. Each remaining predictor is tested alone on the cases valid on it and the target: with a\n\
                 categorical target a categorical predictor by RANKING (the Pearson or likelihood-ratio\n\
                 chi-square significance, or Cramer's V or lambda, largest first), a scale one by the one-way\n\
                 F of the predictor across the target classes; with a scale target a categorical predictor by\n\
                 the F of the target across its categories and a scale one by the significance of Pearson's r.\n\
                 When both kinds are present a chi-square significance (Pearson's unless LRCHISQ) ranks the categorical ones. Ties go\n\
                 to the larger statistic, then the manual's further rules, then file order. SIZE predictors\n\
                 are selected from the top (AUTO: the nearest whole number to the smaller of the predictor count\n\
                 and the larger of 30 and twice its square root), only those with a significance below PCUTOFF\n\
                 when significance ranks. Tables: Case Processing Summary, Excluded Predictors, Predictor\n\
                 Ranking (the selected ones and SHOWUNSELECTED more) and Selected Predictors; PLOT SUMMARY\n\
                 charts the ranking. Frequency weights are rounded, cases with a weight under 0.5 left out.\n\
                 USERMISSING=INCLUDE keeps user-missing values of categorical variables. TIMER stops a run\n\
                 longer than that many minutes (0 for no limit). SPLIT FILE is not used. Names are written\n\
                 in full and each subcommand once.",
        example: "SELECTPRED churned BY region plan WITH spend visits tenure\n\
                  \x20 /EXCEPT VARIABLES=tenure /CRITERIA SIZE=3 RANKING=LRCHISQ /PRINT SUMMARY(CRAMERSV) /PLOT SUMMARY.",
        parse: aim::selectpred,
    },
    CommandSpec {
        name: "RESPONSE RATE",
        summary: "Response rates of a numerator over a denominator in each group, with cumulative rates and limits.",
        syntax: "RESPONSE RATE /VARIABLES NUMERATOR=var DENOMINATOR=var GROUP=var\n\
                 \x20             [/MINRATE RATE=NONE | RATE=value | FORMULA=cost, revenue]\n\
                 \x20             [/MAXCOUNT COUNT=ALL | COUNT=n | PERCENT=value].\n\
                 NUMERATOR and DENOMINATOR are numeric; GROUP is numeric or string. Both are summed within\n\
                 each value of GROUP, and the rate of a group is 100 times its numerator sum over its\n\
                 denominator sum. Groups go in ascending order of their value; the cumulative rate of a group\n\
                 adds up the numerator and the denominator over it and every lower group before dividing. A\n\
                 group whose denominator sum is 0 or below is left out, with a warning. MINRATE sets the\n\
                 lowest acceptable cumulative rate in percent: a value above 0 and below 100, or 100 times\n\
                 cost over revenue per response (both above 0). MAXCOUNT sets the highest acceptable\n\
                 cumulative denominator sum: a whole number, or a percentage (above 0, below 100) of the\n\
                 denominator total. With either limit, each table row is coloured green when it keeps\n\
                 every limit given (cumulative rate at least MINRATE, cumulative sum at most MAXCOUNT) and\n\
                 red when it does not, and the caption states the limits. Table: Response Rates, one row\n\
                 per group with the sums, the rate, the cumulative sums and the cumulative rate. Charts:\n\
                 the rates and cumulative rates by group with a line at MINRATE, and the cumulative\n\
                 denominator sum by group with a line at MAXCOUNT. A case missing on any of the three\n\
                 variables is left out; a case weight multiplies both values. /MINRATE=value and\n\
                 /MAXCOUNT=n are short for RATE= and COUNT=. Every word is written in full and each\n\
                 subcommand given once.",
        example: "RESPONSE RATE /VARIABLES NUMERATOR=orders DENOMINATOR=letters GROUP=area_rank\n\
                  \x20 /MINRATE FORMULA=0.6, 30 /MAXCOUNT PERCENT=50.",
        parse: response_rate::response_rate,
    },
    CommandSpec {
        name: "CLUSTER",
        summary: "Hierarchical clustering of cases by one of 37 proximity measures and seven methods.",
        syntax: "CLUSTER varlist | /MATRIX=IN('file' | dataset | *)\n\
                 \x20       [/MEASURE=SEUCLID | EUCLID | COSINE | CORRELATION | BLOCK | CHEBYCHEV | MINKOWSKI(p) |\n\
                 \x20                 POWER(p,r) | CHISQ | PH2 | binary[(p[,np])]]\n\
                 \x20       [/METHOD=BAVERAGE | WAVERAGE | SINGLE | COMPLETE | CENTROID | MEDIAN | WARD [(rootname)] ...]\n\
                 \x20       [/PRINT=[SCHEDULE] [CLUSTER(min[,max])] [DISTANCE] [NONE]] [/SAVE=CLUSTER(min[,max])]\n\
                 \x20       [/ID=var] [/MISSING=EXCLUDE | INCLUDE] [/PLOT=[VICICLE | HICICLE[(min,max,inc)]] [DENDROGRAM] [NONE]].\n\
                 binary: RR SM JACCARD DICE SS1 RT SS2 K1 SS3 K2 SS4 HAMANN OCHIAI SS5 PHI LAMBDA D Y Q BEUCLID SIZE\n\
                 PATTERN BSEUCLID BSHAPE DISPER VARIANCE BLWMN, from the 2 x 2 table of presence (p, default 1) and\n\
                 absence (np, default 0; with p alone every other value is absence). The items are the cases valid on\n\
                 every variable; the weight variable is not used. Each step joins the most similar pair of clusters\n\
                 (smallest distance or largest similarity) into the lower-numbered one and updates the proximities by\n\
                 the formula of the method; BAVERAGE (default) and WAVERAGE choose by the mean proximity between or\n\
                 within the clusters. Tables: Case Processing Summary, Proximity Matrix (DISTANCE), and for each\n\
                 method the Agglomeration Schedule (default; for WARD the coefficient is the within-cluster sum of\n\
                 squares) and Cluster Membership (CLUSTER), the clusters of a solution numbered in the order of their\n\
                 first case. ID labels the cases. SAVE adds the membership of each solution, named rootname plus the\n\
                 number of clusters, or CLUn_m. MATRIX=IN reads the rows of type PROX of a matrix file, a dissimilarity\n\
                 matrix unless the value label of PROX says SIMILARITY. Plots come as chart data for each method; a\n\
                 vertical icicle plot unless PLOT names others. The icicle plot shows the cluster of every case at\n\
                 each number of clusters, from min to max by inc (default 1 to the number of cases minus 1). The\n\
                 dendrogram holds every join with its coefficient, rescaled from 1 at the first join to 25 at\n\
                 the last. Not supported yet: MATRIX=OUT, SAVE with MATRIX=IN.",
        example: "CLUSTER q1 TO q5 /MEASURE=SEUCLID /METHOD=WARD /PRINT=SCHEDULE CLUSTER(2,4) /SAVE=CLUSTER(3).",
        parse: cluster::cluster,
    },
    CommandSpec {
        name: "PROXIMITIES",
        summary: "Similarities, dissimilarities or distances between cases or variables by one of 37 measures.",
        syntax: "PROXIMITIES varlist | /MATRIX=IN('file' | dataset | *)\n\
                 \x20  [/VIEW=CASE | VARIABLE] [/STANDARDIZE=[VARIABLE | CASE] [NONE | Z | SD | RANGE | MAX | MEAN | RESCALE]]\n\
                 \x20  [/MEASURE=[EUCLID | SEUCLID | COSINE | CORRELATION | BLOCK | CHEBYCHEV | MINKOWSKI(p) | POWER(p,r) |\n\
                 \x20            CHISQ | PH2 | binary[(p[,np])] | NONE] [ABSOLUTE] [REVERSE] [RESCALE]]\n\
                 \x20  [/PRINT=PROXIMITIES | NONE] [/ID=var] [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/MATRIX=[IN('file' | dataset | *)] [OUT('file' | dataset | *)]].\n\
                 binary: the 27 binary measures of CLUSTER, from the 2 x 2 table of presence (p, default 1) and absence\n\
                 (np, default 0; with p alone every other value is absence). The items are the cases (VIEW=CASE, the\n\
                 default) or the variables. A case missing on any variable is dropped either way; the weight variable\n\
                 is not used. STANDARDIZE works within each variable (default) or each case before the measure: Z (the\n\
                 method when none is named) gives mean 0 and standard deviation 1, SD standard deviation 1, RANGE\n\
                 range 1, MAX largest value 1, MEAN mean 1, RESCALE values from 0 to 1; a constant set of values stays\n\
                 as it is, except under Z (all 0), RESCALE (all 0.5) and MEAN (plus 1). It cannot go with a binary\n\
                 measure. The default measure is EUCLID. ABSOLUTE drops the sign of every proximity, REVERSE changes\n\
                 it (a similarity becomes a dissimilarity and back) and RESCALE maps the smallest proximity to 0 and\n\
                 the largest to 1; they run in this order whatever the order written, on the whole matrix with its\n\
                 diagonal. Tables: Case Processing Summary and Proximity Matrix (left out with PRINT=NONE); a pair\n\
                 for which the measure is undefined is empty. ID labels the cases. MATRIX=OUT writes a matrix file\n\
                 that PROXIMITIES, CLUSTER, PROXSCAL and ALSCAL read with MATRIX=IN: ROWTYPE_ (PROX, with the measure\n\
                 and SIMILARITY or DISSIMILARITY as its value label), the ID variable, CASENO_ (the case number),\n\
                 VARNAME_ and one variable per item, VAR1, VAR2, ... for cases and the variables' own names for\n\
                 VIEW=VARIABLE; under SPLIT FILE every group adds its own matrix after the split variables. OUT(*)\n\
                 replaces the active dataset. MATRIX=IN reads such a file: no measure is computed from it (MEASURE\n\
                 and STANDARDIZE are ignored with a warning), only ABSOLUTE, REVERSE and RESCALE apply.",
        example: "PROXIMITIES taste aroma price /STANDARDIZE=VARIABLE RESCALE /MEASURE=BLOCK /ID=brand /MATRIX=OUT(dist).",
        parse: proximities::proximities,
    },
    CommandSpec {
        name: "PROXSCAL",
        summary: "Multidimensional scaling of one or more proximity matrices by majorization, with optimal transformations.",
        syntax: "PROXSCAL varlist | /MATRIX=IN('file' | dataset | *)\n\
                 \x20  [/TABLE=rowvar BY colvar [BY sourcevar] | sourcevar] [/SHAPE=LOWER | UPPER | BOTH]\n\
                 \x20  [/INITIAL=SIMPLEX | TORGERSON | RANDOM[(n)]] [/WEIGHTS=varlist] [/CONDITION=MATRIX | UNCONDITIONAL]\n\
                 \x20  [/TRANSFORMATION=RATIO | INTERVAL | ORDINAL[(KEEPTIES | UNTIE)] | SPLINE [DEGREE=2] [INKNOT=1]]\n\
                 \x20  [/PROXIMITIES=DISSIMILARITIES | SIMILARITIES] [/MODEL=IDENTITY | WEIGHTED | GENERALIZED | REDUCED[(2)]]\n\
                 \x20  [/ACCELERATION=NONE] [/CRITERIA=[DIMENSIONS(2[,max])] [MAXITER(100)] [DIFFSTRESS(.0001)] [MINSTRESS(.0001)]]\n\
                 \x20  [/PRINT=[NONE] [INPUT] [RANDOM] [HISTORY] [STRESS] [DECOMPOSITION] [COMMON] [DISTANCES] [WEIGHTS]\n\
                 \x20          [INDIVIDUAL] [TRANSFORMATIONS]]\n\
                 \x20  [/PLOT=[NONE] [STRESS] [COMMON] [WEIGHTS] [INDIVIDUAL[(sources | ALL)]] [TRANSFORMATIONS[(sources | ALL)]]\n\
                 \x20         [RESIDUALS[(sources | ALL)]]]\n\
                 \x20  [/OUTFILE=[COMMON('file' | dataset)] [WEIGHTS(...)] [DISTANCES(...)] [TRANSFORMATIONS(...)]].\n\
                 The proximities are dissimilarities unless PROXIMITIES=SIMILARITIES. Without TABLE each case is a row of\n\
                 a square matrix and each variable a column; more cases than variables hold several matrices, one\n\
                 after the other, each a source. TABLE row BY column [BY source] takes one proximity per case, each\n\
                 variable a source or the source in a third variable; TABLE=source names the matrix of each case.\n\
                 SHAPE picks the triangle below or above the diagonal, or BOTH for the weighted mean of the two; the\n\
                 diagonal is not used. WEIGHTS gives each proximity its weight; a missing proximity or weight counts as\n\
                 weight 0. MATRIX=IN reads a matrix file of PROXIMITIES or CLUSTER as one source.\n\
                 The scaling makes the distances fit the transformed proximities in least squares (normalized raw\n\
                 stress) by majorization. RATIO keeps the proximities as they are (INTERVAL is used, with a warning,\n\
                 for similarities or negative values), INTERVAL adds an intercept, ORDINAL keeps only their order\n\
                 (UNTIE lets tied values part) and SPLINE fits a smooth rising curve of the given degree with INKNOT\n\
                 inner knots at quantiles, plus an intercept. CONDITION=MATRIX fits one transformation per source,\n\
                 UNCONDITIONAL one for all. MODEL=WEIGHTED gives every source its own weight per dimension, GENERALIZED\n\
                 its own linear map of the common space and REDUCED(r) a map of rank r; one source always uses\n\
                 IDENTITY. The start is SIMPLEX, TORGERSON (classical scaling) or RANDOM(n): n random starts from the\n\
                 session's generator, the best one kept. DIMENSIONS(min,max) solves from max down to min, each\n\
                 solution starting from the main axes of the one before. Iterations stop after MAXITER, below\n\
                 MINSTRESS or when the stress improves by no more than DIFFSTRESS.\n\
                 Tables: Case Processing Summary; Stress and Fit Measures (normalized raw stress, Stress-I, Stress-II,\n\
                 S-Stress, dispersion accounted for and Tucker's coefficient; without STRESS among PRINT keywords only\n\
                 the first and the last two); Final Coordinates (COMMON), Dimension Weights or Space Weights (WEIGHTS),\n\
                 Individual Space Coordinates, Distances, Transformed Proximities, Iteration History, Decomposition of\n\
                 Normalized Raw Stress, the proximities read (INPUT) and the Random Starts. The identity model's\n\
                 common space is turned to principal axes; the weighted one has sum of squares n per dimension, the\n\
                 generalized one Z'Z = nI; every dimension is signed so that the first object is positive. Charts:\n\
                 Common Space (a scatterplot matrix from three dimensions), Scree Plot of stress by dimensionality\n\
                 (STRESS), Dimension Weights, Individual Space, Transformation Plot and Residuals Plot. OUTFILE writes\n\
                 DIM_1, DIM_2, ... for the common space or the weights and OBJ_1, OBJ_2, ... for the distances and the\n\
                 transformed proximities, the sources one below the other. Not supported yet: RESTRICTIONS, a start\n\
                 from a file, and the VARIABLES and CORRELATIONS keywords of PRINT and PLOT.",
        example: "PROXSCAL brand1 TO brand6 /TRANSFORMATION=ORDINAL /CRITERIA=DIMENSIONS(1,3) /PRINT=COMMON STRESS HISTORY /PLOT=COMMON STRESS.",
        parse: proxscal::proxscal,
    },
    CommandSpec {
        name: "PREFSCAL",
        summary: "Multidimensional unfolding of preferences or proximities between row and column objects, with penalized stress.",
        syntax: "PREFSCAL [VARIABLES=]varlist\n\
                 \x20  [/INPUT=[ROWS(n | rowvar)] [SOURCES(n | sourcevar)]] [/PROXIMITIES=DISSIMILARITIES | SIMILARITIES]\n\
                 \x20  [/WEIGHTS=varlist] [/INITIAL=CLASSICAL[(TRIANGLE | SPEARMAN)] | CORRESPONDENCE | ROSSCLIFF\n\
                 \x20            | CENTROIDS[(n)] | RANDOM[(n)]]\n\
                 \x20  [/CONDITION=ROW | MATRIX | UNCONDITIONAL]\n\
                 \x20  [/TRANSFORMATION=NONE[(INTERCEPT)] | LINEAR[(INTERCEPT)] | ORDINAL[(KEEPTIES | UNTIE)]\n\
                 \x20                   | SPLINE[([INTERCEPT] [ORDER=2] [INKNOT=1])]]\n\
                 \x20  [/MODEL=IDENTITY | WEIGHTED | GENERALIZED] [/PENALTY=[LAMBDA(0.5)] [OMEGA(1)]]\n\
                 \x20  [/CRITERIA=[DIMENSIONS(2[,max])] [MAXITER(5000)] [DIFFSTRESS(.000001)] [MINSTRESS(.0001)]]\n\
                 \x20  [/PRINT=[NONE] [INPUT] [MULTIPLE] [INITIAL] [HISTORY] [MEASURES] [DECOMPOSITION] [COMMON]\n\
                 \x20          [DISTANCES] [WEIGHTS] [INDIVIDUAL] [TRANSFORMATION]]\n\
                 \x20  [/PLOT=[NONE] [MULTIPLE] [INITIAL] [STRESS] [COMMON] [WEIGHTS] [INDIVIDUAL[(values)...]]\n\
                 \x20         [TRANSFORMATIONS[(values)...]] [SHEPARD[(values)...]] [FIT[(values)...]] [RESIDUALS[(values)...]]]\n\
                 \x20  [/OPTIONS=[MARKERS(var)] [COLORS(var)]]\n\
                 \x20  [/OUTFILE=[COMMON('file' | dataset)] [WEIGHTS(...)] [DISTANCES(...)] [TRANSFORMATIONS(...)]].\n\
                 Each listed variable is a column object and each case a row object of a source. INPUT says how the\n\
                 cases form sources: ROWS(n) cases per source or SOURCES(n) sources (the cases divide evenly; both\n\
                 numbers take the first rows times sources cases); a row variable numbers the rows (a smaller\n\
                 value than the one before starts the next source), a source variable numbers the sources, which\n\
                 hold consecutive cases. Without INPUT there is one source. A missing proximity or a WEIGHTS value\n\
                 of 0 leaves the cell out; the weight variable of the data weights whole rows. SPLIT FILE is not\n\
                 used. Similarities are mirrored within each transformation list (largest and smallest swap).\n\
                 The loss is penalized stress: the mean over transformation lists of the normalized stress,\n\
                 raised to LAMBDA, times the mean over lists of 1 + OMEGA v(p)^2 / v(t)^2, where v is the\n\
                 coefficient of variation and p and t are the proximities and transformed proximities of the\n\
                 list. A small LAMBDA makes the penalty stronger, OMEGA 0 turns it off. CONDITION picks the\n\
                 lists: every row of every source, every source, or all proximities. Each iteration moves the\n\
                 configuration by one majorization step and the transformation by one projected gradient step,\n\
                 so the penalized stress never rises. NONE keeps the proximities (INTERCEPT adds a constant),\n\
                 LINEAR fits a slope (INTERCEPT also a constant), ORDINAL keeps the order (UNTIE lets tied values\n\
                 part) and SPLINE fits a rising spline of the given degree with INKNOT inner knots; the\n\
                 transformed proximities are never negative. MODEL=WEIGHTED gives each source its own weight per\n\
                 dimension and GENERALIZED its own linear map; one source uses IDENTITY. Starts: CLASSICAL scales\n\
                 the full matrix after filling the row-row and column-column distances with the middle of their\n\
                 triangle bounds (SPEARMAN: root mean squared differences of the ranks within rows), ROSSCLIFF\n\
                 takes the singular vectors of the double centered squared proximities, CORRESPONDENCE the\n\
                 scores of a correspondence analysis of the mirrored proximities, CENTROIDS(n) the principal\n\
                 components of the rows with every column at the centroid of the rows that rank it among their\n\
                 first n, RANDOM(n) n random starts from the session's generator, the best kept. DIMENSIONS solves\n\
                 from max down to min. Iterations stop after MAXITER, at MINSTRESS, or when the penalized stress\n\
                 falls by no more than DIFFSTRESS times its value.\n\
                 Tables: Case Processing Summary; Measures (penalized stress with its stress and penalty parts;\n\
                 with MEASURES also normalized stress, Kruskal's Stress-I and II, Young's S-Stress-I and II,\n\
                 dispersion and variance accounted for, recovered preference orders, the mean Spearman rho and\n\
                 Kendall tau-b of the rows and the coefficients of variation); Final Common Space (rows, then\n\
                 columns); Dimension Weights or Space Weights; and on request the proximities, Random Starts,\n\
                 Initial Common Space, Iteration History, Decomposition of Normalized Stress, Individual Space\n\
                 Coordinates, Distances and Transformed Proximities. Charts: the joint plot of the common space\n\
                 (OPTIONS groups the rows by the values of a variable), Dimension Weights, Individual Space,\n\
                 Scree Plot, Initial Common Space, Random Starts, and Transformation, Shepard, Fit and Residuals\n\
                 plots; the values pick rows (CONDITION=ROW) or sources by their number or identifier, ALL for\n\
                 all. OUTFILE writes DIM_1, DIM_2, ... for the common space or the weights and ROW_1, ...,\n\
                 COL_1, ... for the distances and transformed proximities, the sources one below the other.\n\
                 Not supported yet: RESTRICTIONS, the SMOOTH transformation and a start from a file.",
        example: "PREFSCAL brand1 TO brand6 /INPUT=ROWS(person) /TRANSFORMATION=ORDINAL(UNTIE) /PRINT=MEASURES COMMON HISTORY /PLOT=COMMON TRANSFORMATIONS(1 TO 4).",
        parse: prefscal::prefscal,
    },
    CommandSpec {
        name: "ALSCAL",
        summary: "Multidimensional scaling of symmetric proximity matrices by alternating least squares on squared distances.",
        syntax: "ALSCAL VARIABLES=varlist | /MATRIX=IN('file' | dataset | *)\n\
                 \x20  [/INPUT=ROWS(ALL | n)] [/SHAPE=SYMMETRIC]\n\
                 \x20  [/LEVEL=ORDINAL[([UNTIE] [SIMILAR])] | INTERVAL[(1..4)] | RATIO[(1..4)] | NOMINAL]\n\
                 \x20  [/CONDITION=MATRIX | UNCONDITIONAL] [/MODEL=EUCLID | INDSCAL]\n\
                 \x20  [/CRITERIA=[NEGATIVE] [CUTOFF(0)] [CONVERGE(.001)] [ITER(30)] [STRESSMIN(.005)] [DIMENS(2[,max])]]\n\
                 \x20  [/PRINT=[DATA] [HEADER]] [/PLOT[=DEFAULT | ALL]] [/OUTFILE='file' | dataset].\n\
                 Each case is a row of a square matrix of dissimilarities and each variable a column; only the\n\
                 triangle below the diagonal is read, and more cases than variables hold one matrix per subject.\n\
                 MATRIX=IN reads a matrix file of PROXIMITIES or CLUSTER instead, VARIABLES picking its items; a\n\
                 similarity file is scaled as SIMILAR, or turned into dissimilarities from its largest value for\n\
                 INTERVAL and RATIO. A missing value or one below CUTOFF is left out; the weight variable is not used. The\n\
                 model fits squared distances to squared disparities and reports S-stress, the root of the mean over\n\
                 the matrices of their squared misfit relative to the fourth powers of the disparities. ORDINAL\n\
                 keeps the order of the data (UNTIE lets ties part, SIMILAR turns the order), NOMINAL only which values\n\
                 are equal; INTERVAL(n) and RATIO(n) make the squared disparities a polynomial of degree n of the\n\
                 squared data, with and without an intercept. CONDITION=MATRIX fits each matrix on its own,\n\
                 UNCONDITIONAL all at once. MODEL=INDSCAL weights the dimensions per matrix (no negative weights\n\
                 unless NEGATIVE). The start is classical scaling of the mean scalar products of all matrices, after\n\
                 each got the smallest constant that makes it a metric. Iterations stop when S-stress improves by less\n\
                 than CONVERGE, falls to STRESSMIN or after ITER; DIMENS(min,max) solves from max down to min (1 only\n\
                 for EUCLID), each smaller solution from the first dimensions of the one before.\n\
                 Tables per dimensionality: Iteration History (S-stress and its improvement), Stress and Squared\n\
                 Correlation (RSQ) in Distances per matrix (Kruskal's stress formula 1 and the squared correlation of\n\
                 disparities and distances, with their average), Stimulus Coordinates and for INDSCAL Subject Weights;\n\
                 PRINT=DATA adds the data and the optimally scaled data, HEADER the options. The coordinates are\n\
                 centered; EUCLID turns them to principal axes with a mean square of 1, INDSCAL scales each dimension\n\
                 to a sum of squares equal to the number of stimuli (the weights the other way) and orders the\n\
                 dimensions by their weights; every dimension is signed so that the first stimulus is positive. PLOT\n\
                 gives the configuration, the subject weights, the linear fit (disparities against distances), the\n\
                 nonlinear fit (data against distances) for ORDINAL and NOMINAL and the transformation (data against\n\
                 disparities), with ALL one per matrix. OUTFILE writes TYPE_ (CON for coordinates, SUB for subject\n\
                 weights), DIMENS, MATNUM (0 for coordinates) and DIM1, DIM2, ... for every dimensionality. Not\n\
                 supported yet: SHAPE=ASYMMETRIC and RECTANGULAR, CONDITION=ROW, the models ASCAL, AINDS and GEMSCAL,\n\
                 FILE.",
        example: "ALSCAL VARIABLES=s1 TO s9 /LEVEL=ORDINAL /CRITERIA=DIMENS(2,3) /PLOT.",
        parse: alscal::alscal,
    },
    CommandSpec {
        name: "ORTHOPLAN",
        summary: "Generates an orthogonal main-effects plan of profiles for a conjoint study.",
        syntax: "ORTHOPLAN [/FACTORS=name ['label'] [name ['label'] ...] (value ['label'] ... | 'level' ...) ...]\n\
                 \x20  [/REPLACE | /OUTFILE='file' | dataset] [/MINIMUM=n] [/HOLDOUT=n] [/MIXHOLD=YES | NO].\n\
                 Builds the smallest plan in which every pair of factors is balanced against each other (the\n\
                 full factorial when that is smallest), with at least MINIMUM profiles when a plan of at most 81\n\
                 can have them, else the largest one. A factor has 2 to 9 levels, 90 levels in all; the levels are\n\
                 values, each with an optional label, or quoted texts that become levels 1, 2, ... with the texts\n\
                 as labels, and a level list serves every name written since the last list. The plan's columns and\n\
                 order are random: SET SEED repeats them. HOLDOUT adds random profiles that repeat neither the plan\n\
                 nor each other, after the plan or, with MIXHOLD=YES, mixed into it. STATUS_ is 0 for the plan, 1\n\
                 for holdouts and 2 for simulation profiles; CARD_ numbers the profiles. Without FACTORS the\n\
                 variables of the active dataset are the factors and their value labels the levels. Cases in the\n\
                 active dataset whose variables are the factors are kept after the plan as simulation profiles.\n\
                 The plan replaces the active dataset with REPLACE, goes to the file or dataset of OUTFILE, and is\n\
                 otherwise added to the active dataset. SPLIT FILE and WEIGHT are not used. A table counts the\n\
                 profiles of each kind.",
        example: "ORTHOPLAN FACTORS=brand 'Brand' ('Acme' 'Brio' 'Cora') price 'Price' (5 '5 euro' 7 '7 euro' 9 '9 euro') organic (0 'no' 1 'yes') /HOLDOUT=2.",
        parse: conjoint::orthoplan,
    },
    CommandSpec {
        name: "PLANCARDS",
        summary: "Shows the profiles of a conjoint plan as a list or as one card per profile.",
        syntax: "PLANCARDS [/FACTORS=varlist] [/FORMAT=LIST | CARD | BOTH] [/TITLE='line' ...] [/FOOTER='line' ...]\n\
                 \x20  [/OUTFILE='file'].\n\
                 Shows the profiles of the active dataset, a plan from ORTHOPLAN or typed in, with the factor and\n\
                 value labels: FACTORS picks the factors and their order (default every variable but STATUS_ and\n\
                 CARD_; string variables are allowed). LIST (default) is one table per split group, holdouts marked\n\
                 and simulation profiles in a table of their own; CARD is a table per experimental or holdout\n\
                 profile, without split groups; BOTH (or ALL) gives both. TITLE and FOOTER take one or more strings,\n\
                 a line each, and repeat to add lines; on cards )CARD becomes the profile's number. OUTFILE writes\n\
                 the cards to a text file instead of showing them (also with BOTH; ignored with LIST).",
        example: "PLANCARDS /FORMAT=CARD /TITLE='Profile )CARD' /FOOTER='Your score from 0 to 10:'.",
        parse: conjoint::plancards,
    },
    CommandSpec {
        name: "CONJOINT",
        summary: "Part-worth utilities and importances from rankings or scores of the profiles of a plan.",
        syntax: "CONJOINT [PLAN=* | 'file' | dataset] [/DATA=* | 'file' | dataset] /SEQUENCE | RANK | SCORE=varlist\n\
                 \x20  [/SUBJECT=variable]\n\
                 \x20  [/FACTORS=varlist ['label'] ([DISCRETE [MORE | LESS] | LINEAR [MORE | LESS] | IDEAL | ANTIIDEAL]\n\
                 \x20            [value ['label'] ...]) ...]\n\
                 \x20  [/PRINT=ALL | ANALYSIS | SIMULATION | NONE [SUMMARYONLY]] [/UTILITY='file' | dataset]\n\
                 \x20  [/PLOT=[SUMMARY] [SUBJECT] [ALL] | NONE].\n\
                 The plan (STATUS_ 0 experimental, 1 holdout, 2 simulation, the simulations last; every variable\n\
                 but STATUS_ and CARD_ a numeric factor) and the responses come from PLAN and DATA, one of them a\n\
                 file or dataset and the other by default the active dataset. The response variables follow the\n\
                 experimental and holdout profiles in plan order: SCORE holds scores (higher is better), RANK the\n\
                 rank of each profile (1 is best), SEQUENCE the profile numbers from the best down. Consecutive\n\
                 cases with one SUBJECT value are one subject; without SUBJECT all cases are one. A subject with a\n\
                 missing response is skipped. Least squares on the experimental profiles estimates per subject\n\
                 and for the group the utilities: DISCRETE (the default) one per level summing to 0, LINEAR the\n\
                 value times a coefficient, IDEAL and ANTIIDEAL a quadratic of the value; the levels of the FACTORS\n\
                 list replace the factor's values from the smallest up, and labels its labels. MORE and LESS (and\n\
                 the quadratic models) only count subjects whose estimates go the other way. Tables: Model\n\
                 Description, recoded values, Cramer's V when the plan is not orthogonal, Utilities with standard\n\
                 errors and the constant, Importance Values (range of a factor's utilities as a percent of all\n\
                 ranges, averaged over subjects), Coefficients, Correlations (Pearson's R and Kendall's tau of the\n\
                 predicted and observed preferences, Kendall's tau for holdouts, one-tailed), reversals, and for\n\
                 simulation profiles the predicted scores and the mean probabilities of maximum utility,\n\
                 Bradley-Terry-Luce and logit choice. PRINT picks analysis or simulation results, SUMMARYONLY drops\n\
                 the subjects' tables. UTILITY writes a case per subject: split and subject variables, CONSTANT,\n\
                 the utilities (name1, name2, ... or name_L and name_Q) and the predicted SCOREn and SIMULn. PLOT\n\
                 draws bar charts of the importances and utilities, SUMMARY for the group, SUBJECT per subject.",
        example: "CONJOINT PLAN='plan.sav' /FACTORS=price (LINEAR LESS) brand /SUBJECT=id /RANK=r1 TO r11 /PLOT=SUMMARY.",
        parse: conjoint::conjoint,
    },
    CommandSpec {
        name: "DISCRIMINANT",
        summary: "Linear discriminant analysis with direct or stepwise entry and classification of cases.",
        syntax: "DISCRIMINANT GROUPS=var(min,max) /VARIABLES=varlist [/SELECT=var(value)]\n\
                 \x20  [/ANALYSIS=varlist[(level)] ...] [/METHOD=DIRECT | WILKS | MAHAL | MAXMINF | MINRESID | RAO]\n\
                 \x20  [/TOLERANCE=0.001] [/MAXSTEPS=n] [/FIN=3.84] [/FOUT=2.71] [/PIN=p] [/POUT=p] [/VIN=0]\n\
                 \x20  [/FUNCTIONS=n1[,pct[,sig]]] [/PRIORS=EQUAL | SIZE | values] [/SAVE=[CLASS[=name]] [SCORES[=root]] [PROBS[=root]]]\n\
                 \x20  [/ANALYSIS=... another block] [/MISSING=EXCLUDE | INCLUDE] [/HISTORY=STEP | NONE]\n\
                 \x20  [/CLASSIFY=[NONMISSING | UNSELECTED | UNCLASSIFIED] [POOLED | SEPARATE] [MEANSUB]]\n\
                 \x20  [/STATISTICS=[MEAN] [STDDEV] [UNIVF] [COV] [CORR] [FPAIR] [BOXM] [GCOV] [TCOV] [RAW] [COEFF] [TABLE]\n\
                 \x20               [CROSSVALID] [ALL]] [/ROTATE=NONE] [/PLOT=[COMBINED] [SEPARATE] [MAP] [CASES[(n)]] [ALL]].\n\
                 The analysis uses the cases with a whole group value in the range, no missing value on VARIABLES and,\n\
                 with SELECT, the selected value; groups without cases are ignored. DIRECT enters the variables that\n\
                 pass the tolerance; the stepwise methods enter the variable with the best criterion (smallest Wilks'\n\
                 lambda, largest smallest Mahalanobis distance or F between groups, smallest sum of unexplained\n\
                 variation, largest increase in Rao's V) among those passing FIN or PIN, higher inclusion levels\n\
                 first (even levels together, level 0 never), and remove a level 1 variable below FOUT or above POUT.\n\
                 Tables: Analysis Case Processing Summary, Group Statistics, Tests of Equality of Group Means (UNIVF),\n\
                 Pooled Within-Groups Matrices (COV, CORR), Covariance Matrices (GCOV, TCOV), Log Determinants and Box's\n\
                 M (BOXM), the stepwise tables (HISTORY), Pairwise Group Comparisons (FPAIR), Eigenvalues, Wilks'\n\
                 Lambda, Standardized Canonical Discriminant Function Coefficients, Structure Matrix, Canonical\n\
                 Discriminant Function Coefficients (RAW), Functions at Group Centroids, Classification Processing\n\
                 Summary, Prior Probabilities for Groups, Classification Function Coefficients (COEFF) and\n\
                 Classification Results (TABLE, CROSSVALID leaving each case out). SAVE adds the predicted group, the\n\
                 function scores and the posterior probabilities, named DSC_m and DSCn_m unless given. Weighted when a\n\
                 weight variable is set. PLOT alone means COMBINED and CASES. COMBINED plots the classified cases on\n\
                 the first two functions by group, with the group centroids; SEPARATE makes one such plot per group;\n\
                 with a single function both give histograms of the scores. CASES lists every classified case (the\n\
                 first n) in a Casewise Statistics table: actual group, the two most likely groups with their\n\
                 posterior probabilities and squared distances to the centroid, the chi-square significance of\n\
                 the first one's distance, and the scores. MAP splits the plane of the first two functions into\n\
                 the territory of each group, a convex polygon, under the pooled rule with the priors, drawn as\n\
                 tinted polygons with the centroids named; there is no\n\
                 map with one function or CLASSIFY=SEPARATE. Not supported yet: ROTATE=COEFF and STRUCTURE, MATRIX,\n\
                 OUTFILE, cross-validated rows in the casewise listing.",
        example: "DISCRIMINANT GROUPS=segment(1,3) /VARIABLES=q1 TO q6 /METHOD=WILKS /PRIORS=SIZE /STATISTICS=MEAN STDDEV UNIVF BOXM TABLE /SAVE=CLASS SCORES PROBS.",
        parse: discriminant::discriminant,
    },
    CommandSpec {
        name: "COXREG",
        summary: "Cox proportional hazards regression of survival times, with strata, categorical covariates, stepwise selection and time-dependent covariates.",
        syntax: "COXREG [VARIABLES=] time [WITH var | var BY var | var TO var ...]\n\
                 \x20  /STATUS=var [EVENT] (values) [LOST (values)] [/STRATA=var]\n\
                 \x20  [/CATEGORICAL=varlist] [/CONTRAST(var)=DEVIATION[(ref)] | SIMPLE[(ref)] | DIFFERENCE |\n\
                 \x20  HELMERT | REPEATED | POLYNOMIAL[(metric)] | SPECIAL(matrix) | INDICATOR[(ref)]]\n\
                 \x20  [/METHOD=ENTER | FSTEP[(COND | LR | WALD)] | BSTEP[(COND | LR | WALD)] [ALL | terms]] ...\n\
                 \x20  [/MISSING=EXCLUDE | INCLUDE] [/PRINT=[DEFAULT] [SUMMARY] [BASELINE] [CORR] [CI[(95)]] [ALL]]\n\
                 \x20  [/CRITERIA=[BCON(1E-4)] [LCON(1E-5)] [ITERATE(20)] [PIN(0.05)] [POUT(0.1)]]\n\
                 \x20  [/PLOT=[NONE] [SURVIVAL] [HAZARD] [LML] [OMS]] [/PATTERN=var(value) ... [BY var]] ...\n\
                 \x20  [/SAVE=tempvar[(name)] ...] [/EXTERNAL].\n\
                 tempvar: SURVIVAL SE HAZARD (or RESID) LML XBETA DFBETA PRESID. A case is used when time, status,\n\
                 strata and covariates are valid and its weight is positive; a negative time drops it, and so does\n\
                 a censored time earlier than every event of its stratum. Status values on the EVENT list mark an\n\
                 event; all others are censored (LOST changes nothing). String covariates are categorical; a\n\
                 categorical variable with k values becomes k - 1 coded columns that enter and leave together\n\
                 (DEVIATION, last category as reference, by default). Tied times follow Breslow. The fit is\n\
                 Newton-Raphson from all coefficients 0 on covariates centered at their means; it stops when no\n\
                 coefficient changes by BCON or more of its previous size, the log likelihood changes by less\n\
                 than LCON of its previous value, or after ITERATE iterations. Each METHOD starts from the model\n\
                 the one before it left: ENTER enters its terms, FSTEP enters by the score test (PIN) and removes\n\
                 by the chosen statistic (POUT), BSTEP enters its terms first and then removes and enters the\n\
                 same way. With no covariate only the beginning block and the baseline are shown. Tables: Case\n\
                 Processing Summary, Categorical Variable Codings, Omnibus Tests of Model Coefficients (block 0:\n\
                 -2 log likelihood; then per step the overall score test at all coefficients 0 and the changes\n\
                 from the previous step and block), Variables in the Equation (with PRINT CI the interval of\n\
                 Exp(B)), Model if Term Removed (COND and LR), Variables not in the Equation with the residual\n\
                 chi-square, and with PRINT the Correlation Matrix of Regression Coefficients, Covariate Means\n\
                 and a Survival Table per stratum: at every event time the baseline cumulative hazard with all\n\
                 covariates 0, and survival, its standard error and the cumulative hazard at the means. The\n\
                 baseline survival is the maximum likelihood estimate given the coefficients, so with no\n\
                 covariate it is the Kaplan-Meier curve. SUMMARY keeps a line per step in the Omnibus Tests and\n\
                 shows the other tables for the last step of each block only. String covariates tell their\n\
                 categories apart by their first eight bytes.\n\
                 PLOT gives chart data of the step curves of each stratum at the means and at each PATTERN:\n\
                 named covariates at the values given, the others at their means; BY draws one line per category\n\
                 of a categorical covariate. SAVE adds, at each case's own time and covariates, SUR_1, SE_1, HAZ_1,\n\
                 LML_1 and XBE_1 (the linear predictor with centered covariates), DFB1_1, DFB2_1, ... (the change\n\
                 in each coefficient when the case is left out, from its score residual) and PRE1_1, PRE2_1, ... for\n\
                 the Schoenfeld residuals of events, one per coded covariate and missing for those out of the final\n\
                 model, or the names given (DFBETA(name) gives name1, name2, ...). Weighted when a\n\
                 weight variable is set. After a TIME PROGRAM, covariates it creates change with time; SAVE then\n\
                 gives DFBETA only and the survival table, PATTERN and plots are left out. EXTERNAL changes\n\
                 nothing. Not supported yet: OUTFILE.",
        example: "COXREG months WITH plan spend /STATUS=ended(1) /STRATA=site /CATEGORICAL=plan /PRINT=CI(95) BASELINE /PLOT=SURVIVAL.",
        parse: coxreg::coxreg,
    },
    CommandSpec {
        name: "TIME PROGRAM",
        summary: "Start the transformations that define time-dependent covariates for the next COXREG.",
        syntax: "TIME PROGRAM.\n\
                 The transformations that follow (COMPUTE, IF, RECODE, DO IF, LOOP and the like), up to the\n\
                 next command of another kind, such as a label or format command, are kept for the next COXREG\n\
                 instead of changing the data. COXREG runs them for every case at each event time of its\n\
                 stratum up to the case's own time, with the variable T_ holding that time, so a covariate they\n\
                 compute can change with time. The data may not have a variable T_, and T_ itself is not a\n\
                 covariate. A missing value from the program where the case is otherwise valid is an error, and\n\
                 so is a program that adds or drops cases, leaves a LOOP or DO IF open, or holds PRINT, WRITE or\n\
                 XSAVE; a COXREG that stops for a fault in the program drops it, one that stops for another\n\
                 error keeps it. Any other command that reads the data drops the program, as do CLEAR TIME\n\
                 PROGRAM and CLEAR TRANSFORMATIONS. A case the program gives a missing value while it is\n\
                 missing on a variable the program names is left out.\n\
                 Not supported yet in a program: LAG, $CASENUM and values carried from case to case.",
        example: "TIME PROGRAM.\nCOMPUTE spend_t = spend * LN(T_ + 1).",
        parse: coxreg::time_program,
    },
    CommandSpec {
        name: "CLEAR TIME PROGRAM",
        summary: "Drop the time-dependent covariates of the last TIME PROGRAM.",
        syntax: "CLEAR TIME PROGRAM.\n\
                 The transformations of the last TIME PROGRAM are dropped, so a new TIME PROGRAM can define the\n\
                 time-dependent covariates afresh. COXREG uses a program up when it runs, and so does any other\n\
                 command that reads the data.",
        example: "CLEAR TIME PROGRAM.",
        parse: coxreg::clear_time_program,
    },
    CommandSpec {
        name: "LOGISTIC REGRESSION",
        summary: "Binary logistic regression with categorical covariates, stepwise selection and case diagnostics.",
        syntax: "LOGISTIC REGRESSION [VARIABLES=] dependent [WITH var | var BY var ...]\n\
                 \x20  [/CATEGORICAL=varlist] [/CONTRAST(var)=INDICATOR[(ref)] | DEVIATION[(ref)] | SIMPLE[(ref)] |\n\
                 \x20  DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)] | SPECIAL(matrix)]\n\
                 \x20  [/METHOD=ENTER | FSTEP[(COND | LR | WALD)] | BSTEP[(COND | LR | WALD)] [ALL | terms]] ...\n\
                 \x20  [/ORIGIN | NOORIGIN] [/ID=var] [/PRINT=[DEFAULT] [SUMMARY] [CORR] [ITER[(n)]] [GOODFIT] [CI(95)] [ALL]]\n\
                 \x20  [/CRITERIA=[BCON(0.001)] [ITERATE(20)] [LCON(0)] [PIN(0.05)] [POUT(0.10)] [EPS(1E-8)] [CUT(0.5)]]\n\
                 \x20  [/MISSING=EXCLUDE | INCLUDE] [/CASEWISE=[tempvars] [OUTLIER(2)]]\n\
                 \x20  [/SELECT=ALL | var EQ | NE | LT | LE | GT | GE value]\n\
                 \x20  [/SAVE=tempvar[(name)] ...] [/CLASSPLOT] [/EXTERNAL].\n\
                 tempvar: PRED PGROUP RESID DEV LRESID SRESID ZRESID LEVER COOK DFBETA. The dependent variable has\n\
                 exactly two values among the cases used: the lower is coded 0 and the higher 1 (strings in\n\
                 alphabetical order). A case missing on any variable is left out; one missing only on the dependent\n\
                 still gets PRED and PGROUP. String covariates are categorical; a categorical variable with k\n\
                 values becomes k - 1 coded variables that enter and leave together (INDICATOR, last category as\n\
                 reference, by default). The fit is Newton-Raphson from all coefficients 0 and stops when no\n\
                 coefficient changes by BCON or more, the -2 log likelihood changes by less than LCON percent, or\n\
                 after ITERATE iterations. Each METHOD starts from the model the one before it left: ENTER enters\n\
                 its terms, FSTEP enters by the score test (PIN) and removes by the chosen statistic (POUT), BSTEP\n\
                 enters its terms first and then removes and enters the same way. Tables: Case Processing Summary,\n\
                 Dependent Variable Encoding, Categorical Variables Codings, then for block 0 and each block the\n\
                 Classification Table, Variables in the Equation, Variables not in the Equation, Omnibus Tests of\n\
                 Model Coefficients, Model Summary (-2LL, Cox and Snell and Nagelkerke R square), Model if Term\n\
                 Removed (LR), and with PRINT the Iteration History, Hosmer and Lemeshow Test with its contingency\n\
                 table, the Correlation Matrix and the intervals of Exp(B). SUMMARY shows the last step of each\n\
                 block only. CASEWISE lists every case, or those whose studentized residual is OUTLIER or more\n\
                 away from 0. SAVE adds PRE_1, PGR_1, RES_1, DEV_1, LRE_1, SRE_1, ZRE_1, LEV_1, COO_1 and DFB0_1,\n\
                 DFB1_1, ... (one per coefficient), or the names given (DFBETA(name) gives name0, name1, ...).\n\
                 Weighted when a weight variable is set. CLASSPLOT gives, for every step shown, chart data of the\n\
                 cases counted by their predicted probability of the second category in 40 bins of 0.025, stacked\n\
                 by the observed category. EXTERNAL changes nothing. SELECT fits only the cases where its condition\n\
                 holds (the variable is numeric and none of those on VARIABLES, CATEGORICAL or METHOD; a missing\n\
                 value leaves the case unselected); the Case Processing Summary counts the unselected cases, the\n\
                 Classification Table has a part for each group, and SAVE and CASEWISE (with S or U in Selected\n\
                 Status) cover the unselected cases that have a prediction and an observed value.\n\
                 Not supported yet: OUTFILE.",
        example: "LOGISTIC REGRESSION VARIABLES=bought WITH age income region /CATEGORICAL=region /METHOD=FSTEP(LR) /PRINT=GOODFIT CI(95) /SAVE=PRED.",
        parse: logistic::logistic,
    },
    CommandSpec {
        name: "NOMREG",
        summary: "Multinomial logistic regression: a nominal dependent variable on factors and covariates.",
        syntax: "NOMREG dependent [(BASE=FIRST | LAST | value  ORDER=ASCENDING | DATA | DESCENDING)]\n\
                 \x20  [BY factors] [WITH covariates]\n\
                 \x20  [/MODEL=[effects] [| BACKWARD | FORWARD | BSTEP | FSTEP = effects] | /FULLFACTORIAL]\n\
                 \x20  [/STEPWISE=[RULE(SINGLE | SFACTOR | CONTAINMENT | NONE)] [MINEFFECT(0)] [MAXEFFECT(n)]\n\
                 \x20             [PIN(0.05)] [POUT(0.1)] [ENTRYMETHOD(LR | SCORE)] [REMOVALMETHOD(LR | WALD)]]\n\
                 \x20  [/INTERCEPT=INCLUDE | EXCLUDE]\n\
                 \x20  [/CRITERIA=[CIN(95)] [DELTA(0)] [MXITER(100)] [MXSTEP(5)] [LCONVERGE(0)] [PCONVERGE(1E-6)]\n\
                 \x20             [SINGULAR(1E-8)] [BIAS(0)] [CHKSEP(20)]]\n\
                 \x20  [/PRINT=[CPS] [MFI] [PARAMETER] [LRT] [FIT] [SUMMARY] [CLASSTABLE] [IC] [KERNEL] [COVB] [CORB]\n\
                 \x20         [STEP] [NONE]]\n\
                 \x20  [/SAVE=[ESTPROB[(root[:n])]] [PREDCAT[(name)]] [PCPROB[(name)]] [ACPROB[(name)]]]\n\
                 \x20  [/SCALE=1 | n | DEVIANCE | PEARSON] [/SUBPOP=varlist] [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/TEST[(constants)]=['label'] effect values ... | ALL values [; ...]].\n\
                 effects: a factor or covariate, a*b or a BY b, a(b) or a WITHIN b, x*x. The dependent and the\n\
                 factors may be numeric or string; covariates are numeric. The categories of the dependent are\n\
                 sorted by ORDER (DATA keeps the order in which they first occur); one of them, the last unless\n\
                 BASE says otherwise, is the reference and has no parameters. Without MODEL the model is the\n\
                 intercept, the covariates and the main effects of the factors; FULLFACTORIAL adds every\n\
                 interaction of the factors. A factor gets one parameter per level and an interaction one per\n\
                 level combination that occurs; a parameter that repeats others (SINGULAR) is redundant and\n\
                 set to 0, so the last level of a factor is the reference. A case missing on any variable is\n\
                 left out, as is one with a weight of 0 or less; weights are used as they are. The cases are\n\
                 grouped into subpopulations by the factors and covariates, or by the SUBPOP variables, which\n\
                 must include every variable of the model. The fit is Newton-Raphson with step-halving (MXSTEP),\n\
                 starting from the intercept-only fit; it stops when the log likelihood changes by less than\n\
                 LCONVERGE, no parameter changes by PCONVERGE or more, or after MXITER iterations. DELTA is added\n\
                 to empty cells of a subpopulation and BIAS to every cell. Tables: Case Processing Summary (CPS),\n\
                 Model Fitting Information against the intercept-only model, or the model of equal\n\
                 probabilities without intercept (MFI, with AIC and BIC under IC), Pseudo R-Square with Cox and\n\
                 Snell, Nagelkerke and McFadden (SUMMARY), Goodness-of-Fit with Pearson and deviance\n\
                 chi-square (FIT), Likelihood Ratio Tests of each effect against the model without it (LRT),\n\
                 Parameter Estimates with Wald tests and the CIN interval of Exp(B) (PARAMETER), the\n\
                 Classification table (CLASSTABLE) and the covariance or correlation matrix of the estimates\n\
                 (COVB, CORB). Without PRINT: CPS, MFI and STEP. -2 log likelihoods include the multinomial constant\n\
                 of the subpopulations; KERNEL leaves it out. SCALE multiplies the covariance matrix by the\n\
                 value, or by the Pearson or deviance chi-square over its df. SAVE adds the probability of each\n\
                 response category (EST1_1, EST2_1, ... or root1, root2, ...; the first n, 25 by default), the\n\
                 predicted category (PRE_1), its probability (PCP_1) and the probability of the observed\n\
                 category (ACP_1), for every case with valid factors and covariates.\n\
                 Stepwise: the intercept and the effects before | are always in; the method picks among the\n\
                 effects after it. FORWARD adds, BACKWARD starts with all of them and drops, FSTEP adds one and\n\
                 then drops while it can, BSTEP starts with all, drops while it can and then adds one. An effect\n\
                 is added when its p (likelihood ratio, or the score test with ENTRYMETHOD(SCORE)) is the smallest\n\
                 and below PIN, and dropped when its p (likelihood ratio, or Wald with REMOVALMETHOD(WALD)) is the\n\
                 largest and above POUT; on a tie the effect listed first goes (entry p values too small to\n\
                 hold are compared by their logarithms). The steps stop when nothing qualifies or a model would\n\
                 repeat. RULE(SINGLE) holds an effect back until the listed effects made of a part of its\n\
                 variables are in, SFACTOR does that only for effects of factors alone, CONTAINMENT waits for the\n\
                 effects it contains (same covariates, fewer factors), NONE never waits; an effect that another\n\
                 one in the model needs is not dropped. MAXEFFECT limits adding under FORWARD and FSTEP,\n\
                 MINEFFECT dropping under BACKWARD and BSTEP, counting effects without the intercept. The Step\n\
                 Summary (STEP, shown by default) lists each step with the -2 log likelihood after it and the\n\
                 test that decided it; the other tables describe the final model, which without the intercept\n\
                 can be the null model of equal probabilities. HISTORY(n) adds an Iteration History table of the final\n\
                 model: -2 log likelihood and the parameters at iteration 0 (the intercept-only start), every n-th\n\
                 iteration and the last. ASSOCIATION, for a response with two categories, counts the pairs of cases with\n\
                 different observed categories as concordant, discordant or tied by the predicted probability\n\
                 of the non-base category (cut to 1/500) and gives Somers' D, Goodman and Kruskal's gamma,\n\
                 Kendall's tau-a and the concordance index C. CELLPROB gives, for every subpopulation and response\n\
                 category, the observed and predicted count and proportion and the raw and standardized residual\n\
                 (observed minus n times the predicted probability, over the square root of n p (1 - p)).\n\
                 TEST=[(constants)] ['label'] effect values ; ... (or ALL values) tests linear combinations of the\n\
                 parameters, the values one per parameter of the effect (redundant ones included, INTERCEPT one), by\n\
                 Wald chi-squares for each logit (df the rank of the rows) and for all logits together; constants\n\
                 default to 0 and a number may be a fraction. Not supported yet: OUTFILE.",
        example: "NOMREG commute (BASE=FIRST) BY district WITH distance /PRINT=PARAMETER LRT FIT SUMMARY CLASSTABLE /SAVE=ESTPROB PREDCAT.",
        parse: nomreg::nomreg,
    },
    CommandSpec {
        name: "MLP",
        summary: "Multilayer perceptron: a neural network with one or two hidden layers that predicts scale dependents and classifies categorical ones.",
        syntax: "MLP dependent [(MLEVEL=S | O | N)] [dependent ...] [BY factors] [WITH covariates]\n\
                 \x20  [/EXCEPT VARIABLES=varlist]\n\
                 \x20  [/RESCALE [COVARIATE=STANDARDIZED | NORMALIZED | ADJNORMALIZED | NONE]\n\
                 \x20            [DEPENDENT=STANDARDIZED | NORMALIZED [(CORRECTION=0.02)] | ADJNORMALIZED [(CORRECTION=0.02)] | NONE]]\n\
                 \x20  [/PARTITION TRAINING=70 TESTING=30 HOLDOUT=0 | VARIABLE=var]\n\
                 \x20  [/ARCHITECTURE [AUTOMATIC=YES [(MINUNITS=1, MAXUNITS=50)] | NO]\n\
                 \x20                 [HIDDENLAYERS=1 | 2 [(NUMUNITS=AUTO | n [, n])]] [HIDDENFUNCTION=TANH | SIGMOID]\n\
                 \x20                 [OUTPUTFUNCTION=IDENTITY | SIGMOID | SOFTMAX | TANH]]\n\
                 \x20  [/CRITERIA [TRAINING=BATCH | ONLINE | MINIBATCH] [MINIBATCHSIZE=AUTO | n] [MEMSIZE=1000]\n\
                 \x20             [OPTIMIZATION=GRADIENTDESCENT | SCALEDCONJUGATE] [LEARNINGINITIAL=0.4]\n\
                 \x20             [LEARNINGLOWER=0.001] [MOMENTUM=0.9] [LEARNINGEPOCHS=10] [LAMBDAINITIAL=0.0000005]\n\
                 \x20             [SIGMAINITIAL=0.00005] [INTERVALCENTER=0] [INTERVALOFFSET=0.5]]\n\
                 \x20  [/STOPPINGRULES [ERRORSTEPS=1 [(DATA=AUTO | BOTH)]] [TRAININGTIMER=ON [(MAXTIME=15)] | OFF]\n\
                 \x20                  [MAXEPOCHS=AUTO | n] [ERRORCHANGE=0.0001] [ERRORRATIO=0.001]]\n\
                 \x20  [/MISSING USERMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [CPS] [NETWORKINFO] [SUMMARY] [CLASSIFICATION] [SOLUTION] [IMPORTANCE] [NONE]]\n\
                 \x20  [/PLOT [NETWORK] [PREDICTED] [RESIDUAL] [ROC] [GAIN] [LIFT] [NONE]]\n\
                 \x20  [/SAVE [PREDVAL[(names)]] [PSEUDOPROB[(root[:25] ...)]]] [/OUTFILE MODEL='file' ...].\n\
                 Keywords are written in full and each subcommand comes once. A dependent is scale or categorical by\n\
                 MLEVEL, else by its measurement level; categorical dependents and factors get one unit per category\n\
                 of the training cases. Cases missing on any variable are left out (user-missing values of\n\
                 categorical variables count with INCLUDE); the rest go to training, testing and holdout at random by\n\
                 the shares (SET SEED makes it repeatable), or by the sign of the VARIABLE: positive trains, 0 tests,\n\
                 negative holds out. Covariates and scale dependents are rescaled by the training cases. Hidden units\n\
                 use TANH (default) or SIGMOID; the output units IDENTITY when a dependent is scale, else SOFTMAX with\n\
                 cross-entropy error; other outputs use the sum of squares. SIGMOID outputs need NORMALIZED\n\
                 dependents and TANH outputs ADJNORMALIZED, the defaults for them. AUTOMATIC=YES (default) uses one\n\
                 TANH layer and searches the number of units between MINUNITS and MAXUNITS on a sample of at most\n\
                 MEMSIZE (and 1000) cases, removing the weakest unit or adding one while the testing error drops.\n\
                 NUMUNITS=AUTO starts from a count based on the numbers of input and output units and of training\n\
                 cases. Starting weights come from random draws in INTERVALCENTER +- INTERVALOFFSET, trained and\n\
                 shaken by simulated annealing on that sample. BATCH training uses SCALEDCONJUGATE by default;\n\
                 ONLINE and MINIBATCH use GRADIENTDESCENT on the cases in a random order, the learning rate falling\n\
                 from LEARNINGINITIAL to LEARNINGLOWER over LEARNINGEPOCHS passes. Training stops when the error\n\
                 (testing cases when there are any) has not reached a new low for ERRORSTEPS steps, at MAXTIME\n\
                 minutes, after MAXEPOCHS passes (AUTO: twice the weights plus 1, at least 100), or when the\n\
                 training error changes by less than ERRORCHANGE or falls below ERRORRATIO times that of predicting\n\
                 the means; the weights kept are those of the lowest error. Tables: Case Processing Summary (CPS),\n\
                 Network Information, Model Summary with the error, the relative error or percent incorrect and the\n\
                 rule that stopped training (SUMMARY), Classification per categorical dependent, Parameter Estimates\n\
                 (SOLUTION) and Independent Variable Importance with a chart (IMPORTANCE). Charts: the units of the\n\
                 network (NETWORK, default), predicted by observed (PREDICTED), residual by predicted (RESIDUAL),\n\
                 ROC with the areas, cumulative gains and lift of each category, from the training and testing\n\
                 cases. SAVE adds the predicted value or category (MLP_PredictedValue) and the first n outputs of each\n\
                 categorical dependent (MLP_PseudoProbability_1, ...). OUTFILE writes Statscraft's own JSON model,\n\
                 one file per dependent, not with SPLIT FILE. The weight variable is not used.",
        example: "SET SEED=20251.\n\
                  MLP churned (MLEVEL=N) BY plan region WITH tenure spend\n\
                  \x20 /PARTITION TRAINING=6 TESTING=2 HOLDOUT=2 /ARCHITECTURE AUTOMATIC=NO HIDDENLAYERS=1 (NUMUNITS=4)\n\
                  \x20 /PRINT CPS NETWORKINFO SUMMARY CLASSIFICATION IMPORTANCE /PLOT ROC /SAVE PREDVAL PSEUDOPROB.",
        parse: neural::mlp,
    },
    CommandSpec {
        name: "RBF",
        summary: "Radial basis function network: Gaussian hidden units at two-step clusters and least squares output weights.",
        syntax: "RBF dependent [(MLEVEL=S | O | N)] [dependent ...] [BY factors] [WITH covariates]\n\
                 \x20  [/EXCEPT VARIABLES=varlist]\n\
                 \x20  [/RESCALE [COVARIATE=STANDARDIZED | NORMALIZED | ADJNORMALIZED | NONE]\n\
                 \x20            [DEPENDENT=STANDARDIZED | NORMALIZED | ADJNORMALIZED | NONE]]\n\
                 \x20  [/PARTITION TRAINING=70 TESTING=30 HOLDOUT=0 | VARIABLE=var]\n\
                 \x20  [/ARCHITECTURE [MINUNITS=AUTO | n MAXUNITS=AUTO | n | NUMUNITS=n] [HIDDENFUNCTION=NRBF | ORBF]]\n\
                 \x20  [/CRITERIA OVERLAP=AUTO | number] [/MISSING USERMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [CPS] [NETWORKINFO] [SUMMARY] [CLASSIFICATION] [SOLUTION] [IMPORTANCE] [NONE]]\n\
                 \x20  [/PLOT [NETWORK] [PREDICTED] [RESIDUAL] [ROC] [GAIN] [LIFT] [NONE]]\n\
                 \x20  [/SAVE [PREDVAL[(names)]] [PSEUDOPROB[(root[:25] ...)]]] [/OUTFILE MODEL='file' ...].\n\
                 Variables, missing values, partitions, rescaling (STANDARDIZED by default for scale dependents too),\n\
                 tables, charts, SAVE (RBF_PredictedValue, RBF_PseudoProbability) and OUTFILE work as in MLP. The\n\
                 training cases are clustered by the two-step method (covariates as continuous, factors as\n\
                 categorical variables). Each hidden unit is a Gaussian at a cluster's means and category shares,\n\
                 its width the square root of OVERLAP (AUTO: 1 + 0.1 per input unit) times the root mean square of\n\
                 the cluster's standard deviations and of p(1 - p) for categories. NRBF (default) divides the units\n\
                 by their sum; ORBF keeps them and adds a bias. The output weights are least squares, with identity\n\
                 outputs and the sum of squares error for every dependent. Without NUMUNITS the number of units is\n\
                 searched between MINUNITS and MAXUNITS (AUTO: from the two-step cluster count K and the R output\n\
                 units, min(K, R), at least 2 for NRBF, to max(10, 2K, R)): the lowest error of the testing cases\n\
                 wins, or without them the lowest BIC of the training cases. The weight variable is not used.",
        example: "SET SEED=4417.\n\
                  RBF spend BY plan WITH tenure visits /ARCHITECTURE HIDDENFUNCTION=ORBF\n\
                  \x20 /PRINT CPS NETWORKINFO SUMMARY SOLUTION /PLOT PREDICTED RESIDUAL /SAVE PREDVAL(spend_hat).",
        parse: neural::rbf,
    },
    CommandSpec {
        name: "MVA",
        summary: "Missing value analysis: where values are missing, how that relates to the other variables, and estimates that allow for it.",
        syntax: MVA_SYNTAX,
        example: "MVA VARIABLES=district mode age km minutes /CATEGORICAL=district /ID=resp /TTEST PROB /TPATTERN /EM (OUTFILE='filled.sav').",
        parse: mva::mva,
    },
    CommandSpec {
        name: "MULTIPLE IMPUTATION",
        summary: "Multiple imputation: several completed copies of the data with plausible values in place of the missing ones.",
        syntax: MULTIPLE_IMPUTATION_SYNTAX,
        example: "MULTIPLE IMPUTATION rent hhsize tenure commute /IMPUTE METHOD=FCS NIMPUTATIONS=10 /CONSTRAINTS hhsize (MIN=1 RND=1) /OUTFILE IMPUTATIONS='filled.sav'.",
        parse: multiple_imputation::multiple_imputation,
    },
    CommandSpec {
        name: "HILOGLINEAR",
        summary: "Hierarchical loglinear models of a multiway table of counts, with backward elimination.",
        syntax: "HILOGLINEAR varlist (min,max) [varlist (min,max) ...]\n\
                 \x20  [/METHOD[=BACKWARD]] [/MAXORDER=k]\n\
                 \x20  [/CRITERIA=[CONVERGE(n)] [ITERATE(20)] [P(0.05)] [DELTA(0.5)] [MAXSTEPS(10)] [DEFAULT]]\n\
                 \x20  [/CWEIGHT=variable | (w w n*w ...)]\n\
                 \x20  [/PRINT=[FREQ] [RESID] [ESTIM] [ASSOCIATION] | DEFAULT | ALL | NONE]\n\
                 \x20  [/PLOT=[RESID] [NORMPROB] | DEFAULT | ALL | NONE] [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/DESIGN=term [term ...]] [/DESIGN ...].\n\
                 terms: a variable or variables joined by * (or BY). At least two and at most ten numeric\n\
                 variables, each group followed by its whole-number range; every integer of the range is a\n\
                 category, values are cut to integers, and a case with a value outside a range is counted as\n\
                 out of range and left out. A case missing on a variable is left out unless INCLUDE makes\n\
                 user-missing values valid; case weights are used as they are.\n\
                 A DESIGN lists the generating class: the highest terms, each of which brings in all the terms\n\
                 made of part of its variables. Every DESIGN fits its own model with the CRITERIA, PRINT,\n\
                 PLOT, MAXORDER and CWEIGHT given earlier in the command (they carry over); METHOD applies to\n\
                 the next DESIGN only. A DESIGN with no terms, or the one added (with a warning) when the\n\
                 command does not end with DESIGN, is the saturated model; MAXORDER k cuts every term down to\n\
                 its k-variable parts. The model is fitted by iterative proportional fitting until no cell\n\
                 moves by CONVERGE (by default the larger of 0.25 and a thousandth of the largest weighted\n\
                 count) or ITERATE cycles have run. CWEIGHT gives a weight per cell, from a variable (its mean\n\
                 in the cell, 1 for an empty cell) or as a list in cell order with the last variable changing\n\
                 fastest; a weight of 0 makes a structural zero and weights are cut to the range 0 to 1.\n\
                 Tables: Data Information, Convergence Information, Cell Counts and Residuals (FREQ: observed\n\
                 and expected counts; RESID: raw and standardized residuals) and the likelihood-ratio and\n\
                 Pearson Goodness-of-Fit Tests, whose df leaves out the cells fitted as 0. A saturated model\n\
                 is fitted to the counts plus DELTA and adds the tests that all effects of order k, or of\n\
                 order k and higher, are zero; ESTIM gives its effect-coded parameters with standard errors\n\
                 (when no cell is 0) and ASSOCIATION the partial chi-square of each lower-order term. By\n\
                 default an unsaturated model shows FREQ and RESID, a saturated one all four. METHOD=BACKWARD\n\
                 removes, one per step and at most MAXSTEPS times, the term of the generating class whose\n\
                 removal has the largest likelihood-ratio p, while that p is above P, and lists the steps.\n\
                 PLOT (unsaturated models only) draws the standardized residuals against the observed and\n\
                 expected counts (RESID) and a normal Q-Q plot of the adjusted residuals (NORMPROB, also\n\
                 NORMPLOT); PLOT alone means both.",
        example: "HILOGLINEAR route(1,3) season(1,4) delay(0,1) /METHOD=BACKWARD /CRITERIA=P(0.01) /DESIGN.",
        parse: loglinear::hiloglinear,
    },
    CommandSpec {
        name: "MANOVA",
        summary: "Analysis of variance and covariance for one or several dependents, with nested and pooled terms, partitions and chosen error terms.",
        syntax: "MANOVA dependents [BY factors (min,max) [factors (min,max) ...]] [WITH covariates] [(covariates)]\n\
                 \x20  [/WSFACTORS=name [name ...] (levels) ...] [/WSDESIGN=term [term ...]] [/MEASURE=names]\n\
                 \x20  [/TRANSFORM[(varlist [/varlist ...])]=[ORTHONORM] [CONTRAST | BASIS] type] [/RENAME=name | * ...]\n\
                 \x20  [/ERROR=WITHIN | RESIDUAL | WR | n] [/METHOD=[UNIQUE | SEQUENTIAL] [CONSTANT | NOCONSTANT] [QR | CHOLESKY]]\n\
                 \x20  [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] | DIFFERENCE | HELMERT | REPEATED |\n\
                 \x20                     POLYNOMIAL[(metric)] | SPECIAL(matrix)] ... [/PARTITION(factor)[=(n, m*k ...)]] ...\n\
                 \x20  [/PRINT | NOPRINT=[CELLINFO[(MEANS SSCP COV COR ALL)]] [HOMOGENEITY[(BARTLETT COCHRAN BOXM ALL)]]\n\
                 \x20        [PARAMETERS[(ESTIM NEGSUM ORTHO COR EFSIZE ALL)]] [DESIGN[(OVERALL ONEWAY)]] [TRANSFORM]\n\
                 \x20        [SIGNIF(MULTIV UNIV EIGEN DIMENR HYPOTH STEPDOWN BRIEF SINGLEDF AVERF AVONLY GG HF EFSIZE)]\n\
                 \x20        [ERROR[(SSCP COV COR STDDEV)]]]\n\
                 \x20  [/OMEANS=[VARIABLES(varlist)] TABLES(factor [BY factor] | CONSTANT, ...)]\n\
                 \x20  [/PMEANS[=[VARIABLES(dependents)] [TABLES(factor [BY factor] | CONSTANT, ...)]]]\n\
                 \x20  [/CINTERVAL=[INDIVIDUAL | JOINT][(level)]\n\
                 \x20             [UNIVARIATE(SCHEFFE | BONFER) | MULTIVARIATE(ROY | PILLAI | WILKS | HOTELLING | BONFER)]]\n\
                 \x20  [/POWER[=[F(alpha)] [T(alpha)] [APPROXIMATE | EXACT]]] [/RESIDUALS[=CASEWISE]]\n\
                 \x20  [/PCOMPS=[COR] [COV] [ROTATE[(VARIMAX | EQUAMAX | QUARTIMAX | NOROTATE)]] [NCOMP(n)]\n\
                 \x20           [MINEIGEN(value)] [ALL]]\n\
                 \x20  [/DISCRIM[=[RAW] [STAN] [ESTIM] [COR] [ROTATE[(VARIMAX | EQUAMAX | QUARTIMAX)]] [ALL]\n\
                 \x20            [ALPHA(value)]]]\n\
                 \x20  [/ANALYSIS=dependents [WITH covariates]] [/MISSING=[LISTWISE] [EXCLUDE | INCLUDE]]\n\
                 \x20  [/DESIGN=term [term ...]] [/DESIGN ...].\n\
                 term: effect [+ effect ...] [= n] [VS WITHIN | RESIDUAL | WR | n], or CONSTANT, or MUPLUS before a term;\n\
                 effect: item [BY item ...] [WITHIN item [BY item ...] ...], or MWITHIN factor(n); item: factor[(n)],\n\
                 a continuous variable or POOL(varlist).\n\
                 Factors take every whole number of their range (values are cut to whole numbers); a case outside\n\
                 a range or missing on any listed variable is left out. Case weights are used as they are. Settings\n\
                 carry over to every later DESIGN; when the command does not end with DESIGN, a full factorial\n\
                 design (all main effects and interactions) is added. Each factor is coded by its contrast\n\
                 (DEVIATION by default, the last level left out): the design columns are the inverse of the matrix\n\
                 whose first row is the level mean and whose other rows are the contrasts, so each parameter\n\
                 estimates its contrast. In a term, a(n) keeps partition n of the factor's degrees of freedom (one\n\
                 each, or as PARTITION groups them); after WITHIN (or W, or MWITHIN) a factor stands for each of its\n\
                 levels, or level n alone for b(n), and a term that starts with MWITHIN b(n) is the constant at that\n\
                 level; '+' pools effects into one test; '= n' makes the term error term n (1 to 10);\n\
                 VS (or AGAINST) names the error a term is tested against. A continuous variable on DESIGN must be\n\
                 a dependent or covariate that ANALYSIS leaves out. The constant is fitted first and shown only\n\
                 when written as CONSTANT (or when there are no factors); NOCONSTANT drops it unless written.\n\
                 UNIQUE sums of squares adjust each term for all others; SEQUENTIAL for the terms before it.\n\
                 Error terms: WITHIN, the pooled within-cell matrix (df N less the non-empty cells); RESIDUAL,\n\
                 the part of the cell model the design leaves out; WR, both together; n, a numbered term. The\n\
                 default is WITHIN, RESIDUAL when no cell holds two cases, WR when DESIGN has a continuous variable.\n\
                 Covariates are taken out of every test by regression within the error term, which loses a df\n\
                 for each, and are tested as a regression with their coefficients.\n\
                 TRANSFORM replaces the continuous variables (all, or each list in turn with one matrix) by\n\
                 combinations named T1, T2 and so on: with CONTRAST the first is their mean and the others the\n\
                 contrasts of the type, SPECIAL takes the square matrix as written, BASIS uses the transposed\n\
                 inverse of that matrix, ORTHONORM makes its rows orthonormal and drops a row the others span.\n\
                 RENAME gives every continuous variable of the list a name in order ('*' keeps the old one);\n\
                 ANALYSIS and later subcommands use the new names, OMEANS the original variables.\n\
                 Repeated measures: WSFACTORS (first after the variable list) reads the dependents as the cells of\n\
                 the within-subjects factors, the last factor changing fastest, once per measure (MEASURE names\n\
                 them). Each factor's contrast (POLYNOMIAL unless CONTRAST says otherwise) is made orthonormal\n\
                 together with the mean, and the dependents are replaced by their Kronecker products. The\n\
                 between-subjects tests analyze the mean column; each WSDESIGN term (by default every main effect\n\
                 and interaction) is the constant of an analysis of its own columns, the DESIGN terms crossed with\n\
                 it. Covariates outside parentheses come in sets, one per within-subjects cell, and are transformed\n\
                 the same way; those in parentheses stay as they are. An effect with more than one column gets\n\
                 Mauchly's test with the Greenhouse-Geisser, Huynh-Feldt and lower-bound epsilons, and averaged\n\
                 tests (AVERF, by default): the sums of the hypothesis and error sums of squares over its columns\n\
                 with the df times their number, per measure, and with several measures their multivariate tests.\n\
                 GG and HF add the significance with both df times the epsilon; AVONLY drops the multivariate tests\n\
                 of such effects. SIGNIF(UNIV) is off by default with WSFACTORS.\n\
                 Tables: Cases; with one dependent, Tests of Significance (each error term and the terms tested\n\
                 against it, the regression, (Model), (Total), R-squared); with several, Multivariate Tests of\n\
                 Significance (Pillais, Wilks, Hotellings with their F approximations, and Roys as the largest\n\
                 root over one plus it) and Univariate F-tests. SIGNIF adds Eigenvalues and Canonical Correlations\n\
                 (EIGEN), Dimension Reduction Analysis (DIMENR), the Hypothesis SSCP Matrix (HYPOTH), Roy-Bargmann\n\
                 Stepdown F-tests (STEPDOWN), a test for each degree of freedom of a term, each column entered after\n\
                 the ones before it (SINGLEDF), partial eta squared for every F and t test (EFSIZE), or only a Wilks\n\
                 table (BRIEF); NOPRINT SIGNIF(MULTIV) or (UNIV) drops a default table. PARAMETERS gives each design\n\
                 column's estimate with its standard error from the term's error term, t, Sig. and an interval\n\
                 (CINTERVAL: INDIVIDUAL at the level given, 0.95 by default; JOINT with Scheffe or Bonferroni\n\
                 multipliers over the term's parameters), NEGSUM minus their sum per term, ORTHO the orthogonal\n\
                 estimates of the design columns in order (their squares add up to the sequential sums of squares),\n\
                 COR the covariance factors and correlations of the estimates; ERROR the error matrices after the\n\
                 covariates; TRANSFORM the transformation matrix; DESIGN the design matrix of the cells (OVERALL)\n\
                 and each factor's one-way basis (ONEWAY). CELLINFO gives cell means with 95% intervals and the\n\
                 within-cell matrices; HOMOGENEITY Cochran's C, the Bartlett-Box F and Box's M; OMEANS weighted and\n\
                 unweighted means over the factors named; PMEANS observed, adjusted and estimated cell means with\n\
                 residuals, for the VARIABLES named, and with TABLES the adjusted means over the other factors,\n\
                 weighted by cell size and not; RESIDUALS each case's observed and predicted values and residuals.\n\
                 POWER adds two columns to every F test (alpha .05, or F(alpha)) and every t test of an estimate\n\
                 (two-sided, alpha .05, or T(alpha)): the noncentrality, F times its hypothesis df or |t|, and the\n\
                 observed power from the noncentral F or t, always computed exactly. In the multivariate tests,\n\
                 APPROXIMATE (the default) uses each criterion's eta squared over one less it, times the error df\n\
                 of its F, and gives Roys none; EXACT gives all four the noncentrality of Hotelling's T-squared,\n\
                 the trace times the error df, when the effect has one hypothesis df, and gives nothing otherwise.\n\
                 With several dependents, CINTERVAL MULTIVARIATE widens the intervals of the estimates so they hold\n\
                 for all dependents together: PILLAI, WILKS and HOTELLING find the critical largest root through\n\
                 that statistic's F approximation, BONFER splits alpha over the dependents and the family. With\n\
                 INDIVIDUAL the family is one parameter, and ROY, PILLAI, WILKS and HOTELLING then all give\n\
                 Hotelling's T-squared interval; with JOINT it is the term's parameters. With one dependent\n\
                 MULTIVARIATE is ignored.\n\
                 PCOMPS, with several dependents, finds the principal components of the error matrix after the\n\
                 covariates, as correlations (COR) or as variances and covariances (COV): the eigenvalues with\n\
                 their share of the total, and loadings (each eigenvector times the root of its eigenvalue) for\n\
                 the components whose eigenvalue reaches MINEIGEN (0 by default). ROTATE turns the first NCOMP\n\
                 of them (all by default, at least 2) by VARIMAX unless another type is named, with Kaiser\n\
                 normalization; a rotation carries over to later PCOMPS until NOROTATE. ALL means COR, COV and\n\
                 ROTATE.\n\
                 DISCRIM describes every multivariate effect, and the covariates, by its discriminant functions:\n\
                 the eigenvectors of the hypothesis matrix relative to the error matrix, scaled to a variance of 1\n\
                 within the error. Functions are kept in order while the dimension reduction test of a function\n\
                 and the ones after it has Sig. at most ALPHA (0.25 by default; 1 keeps all, 0 none). RAW gives the\n\
                 coefficients, STAN the coefficients times the error standard deviations, ESTIM the effect's\n\
                 parameter estimates (or the covariates' coefficients) times the coefficients, COR each\n\
                 dependent's correlation with each function within the error, ROTATE those correlations rotated\n\
                 when two or more functions are kept. DISCRIM alone gives RAW, STAN, ESTIM and COR; ALL also ROTATE.\n\
                 Not supported yet: PLOT, MATRIX, JOINT MULTIVARIATE(ROY) intervals for a term of\n\
                 more than one df, RESIDUALS PLOT, PMEANS PLOT, PRINT DESIGN other than OVERALL and ONEWAY,\n\
                 PARAMETERS(OPTIMAL), ANALYSIS with WSFACTORS, and groups of analyses on ANALYSIS.",
        example: "MANOVA score1 score2 BY method(1,3) school(1,2) WITH pretest /PRINT=SIGNIF(EIGEN) /DESIGN.\n\
                  MANOVA t1 t2 t3 BY method(1,3) /WSFACTORS=trial(3) /PRINT=SIGNIF(GG HF) /DESIGN.",
        parse: manova::manova,
    },
    CommandSpec {
        name: "GENLOG",
        summary: "General loglinear and logit models of a table of counts, with cell weights, covariates and contrasts.",
        syntax: "GENLOG factors [BY factors] [WITH covariates]\n\
                 \x20  [/CSTRUCTURE=variable] [/GRESID=varlist] [/GLOR=varlist] [/MODEL=POISSON | MULTINOMIAL]\n\
                 \x20  [/CRITERIA=[CONVERGE(0.001)] [ITERATE(20)] [DELTA(0.5)] [CIN(95)] [EPS(1E-8)] [DEFAULT]]\n\
                 \x20  [/PRINT=[FREQ] [RESID] [ZRESID] [ADJRESID] [DEV] [ITERATION] [COV] [DESIGN] [ESTIM] [CORR]\n\
                 \x20         [ALL] [DEFAULT] [NONE]]\n\
                 \x20  [/PLOT=DEFAULT | RESID[(ADJRESID DEV)] | NORMPROB[(ADJRESID DEV)] | NONE]\n\
                 \x20  [/SAVE=[RESID [(name)]] [ZRESID [(name)]] [ADJRESID [(name)]] [DEV [(name)]] [PRED [(name)]]]\n\
                 \x20  [/MISSING=EXCLUDE | INCLUDE] [/DESIGN=term [term ...]].\n\
                 terms: a factor, a(n) for category n of a factor alone, a covariate, or parts joined by * or\n\
                 BY. Factors are numeric and their categories are the values that occur; the table has a cell\n\
                 for every combination, at most ten factors. The factors after BY make a logit model, which\n\
                 needs MULTINOMIAL. A cell's count sums the case weights; its covariates, cell weight\n\
                 (CSTRUCTURE) and GRESID and GLOR coefficients are weighted means over its cases. A cell weight\n\
                 of 0 or less makes a structural zero, which is left out of the fit; other cell weights act as\n\
                 multipliers of the expected count. Without DESIGN the model is saturated, which is not\n\
                 allowed with covariates. Dummy coding with a constant: a factor has a column for each\n\
                 category, and columns that repeat earlier ones (by EPS) are redundant and 0. Under MULTINOMIAL\n\
                 the constant, and in a logit model the terms of the BY factors alone, only fix the totals.\n\
                 Newton-Raphson starts from the counts plus DELTA (a saturated model keeps DELTA in the\n\
                 counts) and stops when the fitted values move by less than CONVERGE and the score is below\n\
                 it, or after ITERATE steps. Tables: Data Information, Model Information, Convergence\n\
                 Information (ITERATION adds every step), likelihood-ratio and Pearson Goodness-of-Fit Tests,\n\
                 Cell Counts and Residuals with the columns FREQ, RESID, ZRESID, ADJRESID and DEV ask for\n\
                 (FREQ, RESID, ADJRESID and DEV by default; NONE leaves the table out), the Design Matrix,\n\
                 Parameter Estimates with Wald z and the CIN interval (ESTIM), and their covariances (COV) and\n\
                 correlations (CORR). GRESID gives, for each coefficient variable, the observed and expected\n\
                 sums and their simple, standardized and adjusted residuals; GLOR tests each contrast of log\n\
                 expected counts (coefficients summing to 0, per BY combination in a logit model) with a Wald\n\
                 chi-square and the CIN interval. A logit model adds the analysis of dispersion with the\n\
                 entropy and concentration measures of association. PLOT draws adjusted (or deviance)\n\
                 residuals against the observed and expected counts and in normal and detrended normal\n\
                 plots; by default the adjusted ones in both. SAVE gives each case the value of its cell:\n\
                 residual (RES_n), standardized (ZRE_n), adjusted (ADJ_n), deviance (DEV_n) residual or\n\
                 expected count (PRE_n), or the name given.",
        example: "GENLOG outcome BY clinic shift /MODEL=MULTINOMIAL /PRINT=FREQ ADJRESID ESTIM /DESIGN=outcome outcome*clinic outcome*shift.",
        parse: loglinear::genlog,
    },
    CommandSpec {
        name: "PROBIT",
        summary: "Probit and logit models of response counts out of subject counts, with effective levels.",
        syntax: "PROBIT responses OF subjects WITH covariates [BY factor(min,max)]\n\
                 \x20  [/MODEL=PROBIT | LOGIT | BOTH] [/LOG=[base | NONE]]\n\
                 \x20  [/CRITERIA=[OPTOL(n) | CONVERGE(n)] [ITERATE(n)] [P(0.15)] [STEPLIMIT(0.1)]]\n\
                 \x20  [/NATRES[=rate]] [/PRINT=[FREQ] [CI] [RMP] [PARALL] [DEFAULT] [ALL] [NONE]]\n\
                 \x20  [/MISSING=EXCLUDE | INCLUDE].\n\
                 Each case is a group of subjects: how many there were and how many responded. The\n\
                 chance to respond is F(a + b'x), where F is the standard normal distribution (PROBIT, the\n\
                 default) or the logistic one (LOGIT); BOTH fits the logit model first, then the probit\n\
                 one. With a factor (BY, whole-number range; cases outside it are left out) each level has\n\
                 its own intercept and all share the slopes. Covariates are replaced by their base 10\n\
                 logarithm unless LOG says otherwise: a bare LOG means base e, NONE no transformation. A\n\
                 case with a covariate of 0 is a control group; under a logarithm it stays out of the fit.\n\
                 NATRES adds a natural response rate c, so the chance becomes c + (1 - c) F(...): alone it\n\
                 is estimated (starting from the control groups' rate, or the lowest observed rate), with\n\
                 a value it is fixed. Estimates are maximum likelihood by Newton steps from zero slopes,\n\
                 with standard errors from the observed information. More responses than subjects is an\n\
                 error; a case weight multiplies both counts.\n\
                 Tables: Data Information, Convergence Information, Parameter Estimates (with z tests and\n\
                 95% intervals), Natural Response Rate, Covariances and Correlations of Parameter\n\
                 Estimates and the Pearson goodness-of-fit test. PRINT left out or bare shows FREQ (observed\n\
                 and expected responses per case), CI (the covariate level at which each share from 0.01\n\
                 to 0.99 respond, with 95% fiducial limits, on both scales under a logarithm) and RMP (the\n\
                 relative median potency of every pair of factor levels); PARALL tests whether the levels\n\
                 share the slope. CI, RMP and PARALL need a single covariate. When the goodness-of-fit\n\
                 test's significance is below P the limits widen by the heterogeneity factor chi-square/df\n\
                 and use t instead of the normal; P(0) never does that, P(1) always. With one covariate a\n\
                 scatter plot shows the transformed observed rates. MISSING=INCLUDE keeps user-missing\n\
                 values.",
        example: "PROBIT hatched OF eggs BY strain(1,3) WITH dose /MODEL=BOTH /PRINT=ALL.",
        parse: regression_more::probit,
    },
    CommandSpec {
        name: "CURVEFIT",
        summary: "Fit up to eleven curves of one independent variable, or of time, by least squares.",
        syntax: "CURVEFIT [VARIABLES=] dependents [WITH independent]\n\
                 \x20  [/MODEL=[LINEAR] [LOGARITHMIC] [INVERSE] [QUADRATIC] [CUBIC] [COMPOUND] [POWER] [S]\n\
                 \x20         [GROWTH] [EXPONENTIAL] [LGSTIC] [ALL]]\n\
                 \x20  [/CONSTANT | /NOCONSTANT] [/UPPERBOUND=NO | n] [/CIN=95] [/PRINT=ANOVA]\n\
                 \x20  [/SAVE=[PRED] [RESID] [CIN]] [/PLOT=FIT | NONE] [/ID=variable] [/TEMPLATE='file'].\n\
                 Model names may be cut to three letters. With x the independent (or 1, 2, 3, ... in case\n\
                 order when there is no WITH): LINEAR b0 + b1 x, LOGARITHMIC b0 + b1 ln x, INVERSE b0 + b1/x,\n\
                 QUADRATIC and CUBIC add x^2 and x^3, COMPOUND b0 b1^x, POWER b0 x^b1, S exp(b0 + b1/x),\n\
                 GROWTH exp(b0 + b1 x), EXPONENTIAL b0 exp(b1 x), LGSTIC 1 / (1/u + b0 b1^x) with u from\n\
                 UPPERBOUND (no bound: 1/u is 0). The last six are fitted on the log scale, so they need\n\
                 every dependent value above 0 (and below u); their b0 and multiplicative b1 come back from\n\
                 the logarithm, with standard errors scaled the same way. NOCONSTANT drops b0. LINEAR is\n\
                 the default. A case missing on any named variable is left out of every fit; user-missing\n\
                 values count as missing unless TSET MISSING=INCLUDE. Case weights apply only when there is\n\
                 an independent variable. CIN, CONSTANT or NOCONSTANT and ID default to their TSET settings,\n\
                 and TSET TOLER is the tolerance QUADRATIC and CUBIC need.\n\
                 Each dependent gets a Model Summary and Parameter Estimates table: R square, F, df and its\n\
                 significance and the estimates of every model. PRINT=ANOVA adds per model the summary, the\n\
                 analysis of variance and the coefficients with t tests. SAVE adds per dependent and model\n\
                 FIT_n (PRED), ERR_n (RESID, in the original units) and LCL_n and UCL_n (CIN: a prediction\n\
                 interval at the CIN level), n the first number free for all of them; fits and intervals\n\
                 are also given for cases with only the dependent missing. Without SAVE, TSET NEWVAR\n\
                 decides: CURRENT (the default) saves all four, numbered from 1 in the order of dependents\n\
                 and models and replacing variables of those names; ALL saves them under free numbers;\n\
                 NONE saves none. PLOT=FIT (the default) gives a\n\
                 chart of the observed values with every fitted curve; ID labels its points. Another\n\
                 subcommand given twice keeps the last. APPLY is not supported yet; TEMPLATE is ignored.",
        example: "CURVEFIT VARIABLES=visitors WITH week /MODEL=LINEAR QUADRATIC GROWTH /PRINT=ANOVA /SAVE=PRED CIN.",
        parse: regression_more::curvefit,
    },
    CommandSpec {
        name: "WLS",
        summary: "Weighted least squares: weights from a power of a source variable, or a given weight.",
        syntax: "WLS [VARIABLES=] dependent WITH independents\n\
                 \x20  [/SOURCE=variable] [/DELTA=powers] [/WEIGHT=variable] [/CONSTANT | /NOCONSTANT]\n\
                 \x20  [/PRINT=BEST | ALL] [/SAVE=WEIGHT].\n\
                 Powers: numbers and grids first TO last BY step (or first BY step TO last), at most 150\n\
                 in all; POWER is another name for DELTA and every DELTA counts. A case's weight is its\n\
                 SOURCE value raised to minus the power. SOURCE defaults to the first independent and the\n\
                 power to 1; with neither SOURCE, DELTA nor WEIGHT a warning says so. Powers are kept\n\
                 within -6.5 and 7.5, and cases whose source is not above 0 are left out. WEIGHT names a\n\
                 variable that holds the weights instead (cases with a weight not above 0 are left out); it\n\
                 cannot go with SOURCE or DELTA. A case missing on any named variable is left out; case\n\
                 weights (WEIGHT command) multiply the regression weights and count as cases.\n\
                 For every power the log-likelihood is 0.5 (-n ln 2 pi - n ln s^2 + sum ln w - (n - p)),\n\
                 with s^2 the weighted mean square error and p the number of coefficients; the Log-Likelihood\n\
                 Values table lists it and the power with the largest one is used. For that power (or for\n\
                 the WEIGHT variable) come the Model Summary (R, R square, adjusted R square, standard error\n\
                 and log-likelihood), ANOVA and Coefficients (B, standard error, standardized Beta with its\n\
                 standard error, t and significance). PRINT=ALL adds the coefficients of every power;\n\
                 without PRINT, TSET PRINT=DETAILED does the same and BRIEF leaves out the log-likelihoods.\n\
                 SAVE=WEIGHT keeps the chosen weights as WGT_n, the first free n. Without SAVE, TSET NEWVAR\n\
                 decides: CURRENT (the default) saves WGT_1, replacing it, ALL the next free WGT_n, NONE\n\
                 nothing. CONSTANT or NOCONSTANT defaults to TSET. APPLY is not supported yet.",
        example: "WLS rent WITH floor_area rooms /SOURCE=floor_area /POWER=-1 TO 3 BY 0.25 /PRINT=ALL.",
        parse: regression_more::wls,
    },
    CommandSpec {
        name: "ACF",
        summary: "Autocorrelations of time series, with standard errors, Box-Ljung tests and charts.",
        syntax: "ACF [VARIABLES=] series [/DIFF=n] [/SDIFF=n] [/PERIOD=n] [/LN | /NOLOG] [/SEASONAL]\n\
                 \x20   [/MXAUTO=n] [/SERROR=IND | MA] [/PACF] [/APPLY].\n\
                 Before the correlations each series may be logged (LN; a series with a value not above 0\n\
                 is skipped), differenced DIFF times (DIFF alone: once) and then differenced over the period\n\
                 SDIFF times. The period comes from PERIOD (at least 2), else TSET PERIOD, else the DATE\n\
                 variables; without one SDIFF and SEASONAL are left out with a warning. MXAUTO (else TSET\n\
                 MXAUTO, else 16) is the highest lag, at most the number of values less one; SEASONAL shows\n\
                 only the lags that are multiples of the period. A missing value (user-missing too, unless\n\
                 TSET MISSING=INCLUDE) drops the products it is part of; the mean and the counts use the\n\
                 valid values. Each series gets a table: autocorrelation r_k, its standard error, and the\n\
                 Box-Ljung statistic of lags 1 to k with k df and its significance. IND (the default, or\n\
                 TSET ACFSE) takes the standard error of white noise, sqrt((n - k) / (n (n + 2))); MA takes\n\
                 Bartlett's sqrt((1 + 2 (r_1^2 + ... + r_(k-1)^2)) / n). PACF adds the partial\n\
                 autocorrelations, with standard error 1 / sqrt(n). A chart per series shows the\n\
                 coefficients with limits at two standard errors. APPLY without a name repeats the previous\n\
                 ACF, with the subcommands given now replacing its own; named models are not supported.\n\
                 DIFFERENCE is another name for DIFF. Case weights are not used.",
        example: "ACF VARIABLES=daily_orders /DIFF /MXAUTO=21 /PACF.",
        parse: timeseries::acf,
    },
    CommandSpec {
        name: "PACF",
        summary: "Partial autocorrelations of time series, with standard errors and charts.",
        syntax: "PACF [VARIABLES=] series [/DIFF=n] [/SDIFF=n] [/PERIOD=n] [/LN | /NOLOG] [/SEASONAL]\n\
                 \x20    [/MXAUTO=n] [/APPLY].\n\
                 The series are prepared and the lags chosen as for ACF. The partial autocorrelations come\n\
                 from the autocorrelations by the Durbin-Levinson recursion; each has the standard error\n\
                 1 / sqrt(n), n the number of valid values. A table and a chart (limits at two standard\n\
                 errors) per series. APPLY without a name repeats the previous PACF.",
        example: "PACF river_level /SDIFF /PERIOD=7 /MXAUTO=28.",
        parse: timeseries::pacf,
    },
    CommandSpec {
        name: "CCF",
        summary: "Cross-correlations of pairs of time series, with standard errors and charts.",
        syntax: "CCF [VARIABLES=] series [WITH series] [/DIFF=n] [/SDIFF=n] [/PERIOD=n] [/LN | /NOLOG]\n\
                 \x20   [/SEASONAL] [/MXCROSS=n] [/APPLY].\n\
                 Without WITH every series is paired with each one after it; with WITH each series before\n\
                 it is paired with each one after it. Both series of a pair are prepared as in ACF. At lag\n\
                 k the correlation is of x at time t with y at time t + k, so a positive lag means the first\n\
                 series leads; the sums use the cases where both are valid, divided by the root of the\n\
                 two sums of squares about their means. Lags run from -MXCROSS to MXCROSS (else TSET\n\
                 MXCROSS, else 7), at most the series length less one. The standard error is\n\
                 1 / sqrt(m), m the number of valid pairs at that lag. A table and a chart (limits at two\n\
                 standard errors) per pair. APPLY without a name repeats the previous CCF.",
        example: "CCF ad_spend WITH weekly_sales /DIFF /MXCROSS=10.",
        parse: timeseries::ccf,
    },
    CommandSpec {
        name: "SEASON",
        summary: "Seasonal decomposition by ratios (or differences) to a moving average.",
        syntax: "SEASON [VARIABLES=] series [/MODEL=MULTIPLICATIVE | ADDITIVE] [/MA=EQUAL | CENTERED]\n\
                 \x20      [/PERIOD=n] [/APPLY].\n\
                 The period comes from PERIOD, else TSET PERIOD, else the DATE variables; the series needs\n\
                 at least four full periods and no missing value. A moving average over one period gives the\n\
                 trend: for an even period CENTERED (the default) spans period + 1 cases with half weight at\n\
                 both ends; EQUAL weighs the period cases equally, from period/2 cases before a case to\n\
                 period/2 - 1 after it. An odd period always uses EQUAL. The multiplicative model divides the series by it (times 100), the\n\
                 additive one subtracts it. For each position in the period the mean of these values without\n\
                 the largest and the smallest is the raw factor; the factors are scaled to average 100\n\
                 (multiplicative) or centered on 0 (additive). The adjusted series SAS is the series divided\n\
                 by (or less) the factor, the trend-cycle STC a 1-2-3-2-1 weighted average of SAS (3-term\n\
                 at the second and next-to-last case, extrapolated at the ends) and the irregular part ERR\n\
                 SAS over (or less) STC. Tables: the model, the cases with every component (left out under\n\
                 TSET PRINT=BRIEF) and the seasonal factors. TSET NEWVAR saves ERR_n, SAS_n, SAF_n and STC_n\n\
                 per series; saved multiplicative factors are ratios, so SAS = series / SAF.\n\
                 APPLY without a name repeats the previous SEASON.",
        example: "SEASON VARIABLES=heating_kwh /MODEL=ADDITIVE /PERIOD=12.",
        parse: timeseries::season,
    },
    CommandSpec {
        name: "PREDICT",
        summary: "Set the forecast period of EXSMOOTH and ARIMA.",
        syntax: "PREDICT [start] [THRU [end]].\n\
                 Start and end are case numbers, or dates written with DATE keywords and values; END\n\
                 means the last case. Without a start the period begins right after the USE range (or the\n\
                 file); a start later than that is moved back to it. Without an end it runs to the last\n\
                 case of the file, or covers just its start when that lies past the file. EXSMOOTH and\n\
                 ARIMA estimate on the USE range, give one-step fitted values up to the case before the\n\
                 start and forecasts from there on, adding cases when the period runs past the end of the\n\
                 file (at most TSET MXPREDICT, default 60). Added cases are missing on every variable\n\
                 except the new ones and the DATE variables. Residuals are missing in the forecast\n\
                 period. DATE turns PREDICT off; it stays in force until the next PREDICT.",
        example: "PREDICT THRU YEAR 2026 MONTH 12.",
        parse: ts_models::predict,
    },
    CommandSpec {
        name: "EXSMOOTH",
        summary: "Exponential smoothing with or without trend and season, with fitted values and forecasts.",
        syntax: "EXSMOOTH [VARIABLES=] series [/MODEL=NN | NA | NM | LN | LA | LM | EN | EA | EM | DN | DA | DM]\n\
                 \x20        [/PERIOD=n] [/SEASFACT=(factors) | variable]\n\
                 \x20        [/ALPHA=value | GRID[(start end step)]] [/GAMMA=...] [/DELTA=...] [/PHI=...]\n\
                 \x20        [/INITIAL=CALCULATE | (start trend)] [/APPLY].\n\
                 The first letter of the model is the trend (N none, L linear, E exponential, D damped),\n\
                 the second the season (N none, A additive, M multiplicative); SINGLE means NN, HOLT LN\n\
                 and WINTERS LM. ALPHA smooths the level, GAMMA the trend (not in NN, NA, NM, DA, DM),\n\
                 DELTA the season and PHI damps the trend; each defaults to 0.1, a parameter the model\n\
                 does not use is ignored. ALPHA, GAMMA and DELTA lie from 0 to 1, PHI above 0 and below\n\
                 1. GRID tries every value from start to end by step (defaults 0 to 1 by 0.1 for ALPHA,\n\
                 by 0.2 for GAMMA and DELTA, 0.1 to 0.9 by 0.2 for PHI); every combination is fitted and\n\
                 the one with the smallest sum of squared one-step errors is kept. The output lists the\n\
                 ten smallest (all of them under TSET PRINT=DETAILED). Start values come from the\n\
                 published formulas: means of the first and last complete periods, initial seasonal\n\
                 factors from SEASON's method unless SEASFACT gives them (a list as long as the period,\n\
                 or a variable such as a saved SAF_n whose first values are read). INITIAL gives the\n\
                 start level and trend instead. A seasonal model needs a period (PERIOD, TSET PERIOD or\n\
                 DATE) and four full periods; the multiplicative and exponential models need values\n\
                 above 0. The series may not have missing values inside the USE range. TSET NEWVAR saves\n\
                 FIT_n (the one-step forecast) and ERR_n per series; with PREDICT, FIT_n holds the\n\
                 forecasts. APPLY without a name repeats the previous EXSMOOTH with the subcommands\n\
                 given now replacing its own; SEASFACT is not carried over.",
        example: "EXSMOOTH VARIABLES=room_nights /MODEL=DA /PERIOD=7 /PHI=GRID /DELTA=0.25.",
        parse: ts_models::exsmooth,
    },
    CommandSpec {
        name: "ARIMA",
        summary: "Seasonal ARIMA models with regressors by exact maximum likelihood, with forecasts.",
        syntax: "ARIMA [VARIABLES=] series [WITH regressors]\n\
                 \x20     [/MODEL=[(p,d,q)[(sp,sd,sq)[period]]] [CONSTANT | NOCONSTANT] [NOLOG | LG10 | LN]]\n\
                 \x20     [/P=n | (lags)] [/D=n] [/Q=n | (lags)] [/SP=...] [/SD=n] [/SQ=...]\n\
                 \x20     [/AR=values] [/MA=values] [/SAR=values] [/SMA=values] [/REG=values] [/CON=value]\n\
                 \x20     [/MXITER=10] [/MXLAMB=1E9] [/SSQPCT=0.001] [/PAREPS=0.001] [/CINPCT=95]\n\
                 \x20     [/FORECAST=EXACT | CLS | AUTOINIT] [/APPLY [SPECIFICATIONS | INITIAL | FIT]].\n\
                 The model is phi(B) PHI(B^s) (w_t - mu) = theta(B) THETA(B^s) a_t, w_t the series less the\n\
                 regressors times their coefficients, differenced d times and over the period sd times;\n\
                 the polynomials are 1 - c_1 B - ..., so a positive MA coefficient means a negative\n\
                 weight on the past error. P, Q, SP and SQ name the lags to fit (n for 1 to n, a list in\n\
                 parentheses for those lags only) and win over MODEL. LOG is LG10; logs need values above\n\
                 0. CONSTANT and the confidence level default to TSET. The estimates maximize the exact\n\
                 Gaussian likelihood (a Kalman filter that also takes missing values inside the series)\n\
                 by Levenberg-Marquardt steps on numerical derivatives. It stops after MXITER steps, when\n\
                 no estimate changes by PAREPS (default TSET CNVERGE), when the adjusted sum of squares\n\
                 changes by less than SSQPCT percent, or when the Marquardt constant passes MXLAMB.\n\
                 MXITER=0 takes the initial values as final. AR, MA, SAR, SMA, REG and CON give initial\n\
                 values in the order of the lags. Output: the iteration history (TSET PRINT=BRIEF leaves\n\
                 it out, DETAILED adds the estimates of each step), the estimates with standard errors,\n\
                 t and approximate significance, the residual variance and standard error, the\n\
                 log-likelihood, AIC and SBC, and the covariance and correlation of the estimates. TSET\n\
                 NEWVAR saves FIT_n, ERR_n, LCL_n, UCL_n and SEP_n: one-step fitted values (EXACT: from\n\
                 the Kalman filter; CLS: past errors 0 and missing past values at the mean of the\n\
                 differenced series; AUTOINIT: the first values of the series as their own fits), the\n\
                 residuals on the log scale, the prediction limits and standard errors. Fits, limits\n\
                 and standard errors are taken back from logs. With PREDICT the fit continues as\n\
                 forecasts; with regressors only where they are valid, and the file is not extended.\n\
                 APPLY without a name repeats the previous ARIMA with the subcommands given now replacing\n\
                 its own; INITIAL starts from its estimates, FIT uses them without estimation.",
        example: "ARIMA daily_calls WITH promo /MODEL=(1,0,1)(0,1,1) 7 NOCONSTANT /FORECAST=EXACT.",
        parse: ts_models::arima,
    },
    CommandSpec {
        name: "AREG",
        summary: "Regression whose errors follow a first-order autoregressive process.",
        syntax: "AREG [VARIABLES=] series WITH regressors\n\
                 \x20    [/METHOD=PW | CO | ML] [/CONSTANT | /NOCONSTANT] [/RHO=0] [/MXITER=10]\n\
                 \x20    [/APPLY [SPECIFICATIONS | INITIAL | FIT]].\n\
                 The errors of y = a + b x are taken to follow u_t = rho u_(t-1) + e_t. PW (the default,\n\
                 Prais-Winsten) and CO (Cochrane-Orcutt) alternate two steps: least squares on the\n\
                 quasi-differences y_t - rho y_(t-1), then a new rho from the lag-one products of the\n\
                 residuals. CO drops the first case; PW keeps it, scaled by sqrt(1 - rho^2). RHO is the\n\
                 start (above -1, below 1). The loop ends when no estimate moves by TSET CNVERGE or\n\
                 after MXITER rounds. Both take missing values at the ends of the USE range only. ML\n\
                 fits the same model by exact maximum likelihood (the ARIMA engine with one\n\
                 autoregressive term), missing values allowed anywhere. CONSTANT defaults to TSET.\n\
                 Output: per iteration rho, its standard error, Durbin-Watson and the mean square error\n\
                 (TSET PRINT=DETAILED adds the coefficients, BRIEF keeps only the final estimates), the\n\
                 R square of the transformed regression at the start and the end, an ANOVA table, and\n\
                 the estimates with t tests; ML shows what ARIMA shows. TSET NEWVAR saves FIT_n, ERR_n,\n\
                 LCL_n, UCL_n and SEP_n; after the series, PREDICT gives forecasts where the regressors\n\
                 have values. APPLY without a name repeats the previous AREG with the subcommands given\n\
                 now replacing its own; INITIAL starts from its estimates, FIT uses them as they are.",
        example: "AREG heating_cost WITH degree_days /METHOD=CO /RHO=0.3.",
        parse: ts_more::areg,
    },
    CommandSpec {
        name: "SPECTRA",
        summary: "Periodogram, smoothed spectral density and cross-spectra of series.",
        syntax: "SPECTRA VARIABLES= series [/CENTER [NO]] [/CROSS [NO]]\n\
                 \x20       [/WINDOW=HAMMING | TUKEY | PARZEN | BARTLETT | UNIT | DANIELL [(span)] | NONE | weights]\n\
                 \x20       [/PLOT=P S CS QS PH A G K ALL NONE [BY FREQ | BY PERIOD]]\n\
                 \x20       [/SAVE=FREQ(root) PER(root) SIN(root) COS(root) P(root) S(root) RC(root) IC(root)\n\
                 \x20              CS(root) QS(root) PH(root) A(root) G(root) K(root)] [/APPLY].\n\
                 The terms are listed for the frequencies K/n, K = 0 to n/2: the sine and cosine\n\
                 coefficients and the periodogram (a^2 + b^2) n/2. A series needs six cases and no\n\
                 missing values (use USE or RMV); of an odd count the first case is dropped. CENTER\n\
                 subtracts the mean, else the periodogram at frequency 0 is 2n times its square. The\n\
                 density is a moving average of the periodogram whose weights come from the window's\n\
                 kernel at the span's own frequencies, scaled to sum to 1; the default is HAMMING(5), an\n\
                 even span loses one, a span of 1 or NONE leaves the periodogram as it is, and a list\n\
                 of weights runs from the leftmost to the rightmost (an even list lacks the last one).\n\
                 Values beyond the ends are mirrored. CROSS pairs the first series with each later one:\n\
                 real and imaginary cross-periodogram, cospectrum (CS), quadrature spectrum (QS),\n\
                 phase (PH, radians), cross amplitude (A), gain of the second series over the first\n\
                 (G) and squared coherency (K). Charts come as data, by frequency and by period unless\n\
                 BY picks one; P (the default) and S are drawn on natural logs. SAVE writes one\n\
                 variable per series (root_1, root_2 by their place on VARIABLES) or pair (root_1_2),\n\
                 FREQ and PER under the root itself, from the first case used; a name that exists is\n\
                 skipped. APPLY repeats the previous SPECTRA, SAVE excepted.",
        example: "SPECTRA VARIABLES=river_flow rainfall /CENTER /CROSS /WINDOW=PARZEN(7) /PLOT=S K BY FREQ.",
        parse: ts_more::spectra,
    },
    CommandSpec {
        name: "2SLS",
        summary: "Two-stage least squares for equations with endogenous predictors.",
        syntax: "2SLS [EQUATION=] dependent WITH predictors [/[EQUATION=] dependent WITH predictors]\n\
                 \x20    /INSTRUMENTS=variables [/ENDOGENOUS=variables] [/CONSTANT | /NOCONSTANT]\n\
                 \x20    [/PRINT=COV] [/SAVE=PRED RESID] [/APPLY].\n\
                 The first stage regresses every predictor on the instruments (and the constant); the\n\
                 second regresses the dependent on those predicted values. A predictor that is also an\n\
                 instrument predicts itself, so naming every predictor as an instrument gives ordinary\n\
                 least squares. There must be at least as many instruments as predictors in the longest\n\
                 equation. Repeated INSTRUMENTS lists add up; ENDOGENOUS only documents the model. Case\n\
                 weights count, and a case missing on any variable of the command leaves every\n\
                 equation. Per equation: R, R square, the standard error and the mean of the residuals,\n\
                 an ANOVA table, the coefficients with standardized ones, t tests and tolerances, and\n\
                 the correlations of the estimates (PRINT=COV or TSET PRINT=DETAILED adds their\n\
                 covariances). Residuals use the observed predictors. TSET NEWVAR saves FIT_n and ERR_n\n\
                 per equation; SAVE picks them itself, always under free numbers. CONSTANT defaults to\n\
                 TSET. APPLY repeats the previous 2SLS with the subcommands given now replacing its own.",
        example: "2SLS crop_yield WITH fertilizer_kg /INSTRUMENTS=subsidy_rate soil_ph /SAVE=RESID.",
        parse: ts_more::two_stage,
    },
    CommandSpec {
        name: "TSPLOT",
        summary: "Chart of time series over the cases, as chart data.",
        syntax: "TSPLOT [VARIABLES=] series [/DIFF[=n]] [/SDIFF[=n]] [/PERIOD=n] [/LN | /NOLOG]\n\
                 \x20      [/ID=variable] [/MARK=variable | date] [/SPLIT=UNIFORM | SCALE]\n\
                 \x20      [/FORMAT=[NOFILL | BOTTOM] [NOREFERENCE | REFERENCE[(value)]] | NOJOIN | JOIN | HILO]\n\
                 \x20      [/APPLY].\n\
                 The cases run along the horizontal axis, labeled by ID (else TSET ID, else DATE_, else\n\
                 the case number). Each series may be logged first (LN; values not above 0 become\n\
                 missing), then differenced DIFF times and over the period SDIFF times (DIFF or SDIFF\n\
                 alone: once); the period is PERIOD, TSET PERIOD or that of the DATE variables, and\n\
                 without one SDIFF is skipped with a warning. With one series, BOTTOM fills the area under\n\
                 the line (only when no value is missing or negative) and REFERENCE adds a flat line at\n\
                 the value or at the series mean. With several, JOIN links the values of each case and\n\
                 HILO shows only the highest and lowest of each case (up to three series). MARK names\n\
                 the cases where a variable changes, or a date given with DATE keywords, in the footnote,\n\
                 which also lists the transformations. Under SPLIT FILE each group gets a chart; the\n\
                 reference mean is the group's. USE limits the cases. APPLY without a name repeats the\n\
                 previous TSPLOT with the subcommands given now replacing its own.",
        example: "TSPLOT VARIABLES=river_level rainfall /FORMAT=JOIN /MARK=dam_open.",
        parse: ts_models::tsplot,
    },
    CommandSpec {
        name: "CASEPLOT",
        summary: "Chart of time series with the cases down the vertical axis, as chart data.",
        syntax: "CASEPLOT [VARIABLES=] series [/DIFF[=n]] [/SDIFF[=n]] [/PERIOD=n] [/LN | /NOLOG]\n\
                 \x20        [/ID=variable] [/MARK=variable | date] [/SPLIT=UNIFORM | SCALE]\n\
                 \x20        [/FORMAT=[NOFILL | LEFT] [NOREFERENCE | REFERENCE[(value)]] | NOJOIN | JOIN | HILO]\n\
                 \x20        [/APPLY].\n\
                 TSPLOT turned on its side: the cases run down the vertical axis and the values across.\n\
                 LEFT fills the area between the axis and the line of a single series. Everything else\n\
                 is as for TSPLOT.",
        example: "CASEPLOT VARIABLES=hourly_load /ID=shift /FORMAT=LEFT.",
        parse: ts_models::caseplot,
    },
    CommandSpec {
        name: "TSMODEL",
        summary: "Exponential smoothing and ARIMA models of series, picked by hand or by the expert modeler, with forecasts.",
        syntax: "TSMODEL [/MODELSUMMARY [PRINT=[MODELFIT RESIDACF RESIDPACF | NONE]] [PLOT=[...]]]\n\
                 \x20       [/MODELSTATISTICS [DISPLAY=YES | NO] [MODELFIT=[SRSQUARE RSQUARE RMSE MAPE MAE MAXAPE MAXAE NORMBIC]]]\n\
                 \x20       [/MODELDETAILS [PRINT=[PARAMETERS RESIDACF RESIDPACF FORECASTS]] [PLOT=[RESIDACF RESIDPACF]]]\n\
                 \x20       [/SERIESPLOT OBSERVED FORECAST FIT FORECASTCI FITCI]\n\
                 \x20       [/OUTPUTFILTER [DISPLAY=ALLMODELS | [BESTFIT(N=n | PCT=p) WORSTFIT(N=n | PCT=p)]] [MODELFIT=measure]]\n\
                 \x20       [/SAVE PREDICTED[(root)] LCL[(root)] UCL[(root)] NRESIDUAL[(root)]]\n\
                 \x20       [/AUXILIARY [CILEVEL=95] [MAXACFLAGS=24] [SEASONLENGTH=n]] [/MISSING USERMISSING=EXCLUDE | INCLUDE]\n\
                 \x20       /MODEL DEPENDENT=series [INDEPENDENT=predictors] [OUTFILE=file] [PREFIX='Model']\n\
                 \x20       [/EXPERTMODELER [TYPE=[ARIMA EXSMOOTH]] [TRYSEASONAL=YES | NO]\n\
                 \x20        | /EXSMOOTH TYPE=SIMPLE | SIMPLESEASONAL | HOLT | BROWN | DAMPEDTREND | WINTERSADDITIVE\n\
                 \x20                    | WINTERSMULTIPLICATIVE [TRANSFORM=NONE | SQRT | LN]\n\
                 \x20        | /ARIMA AR=lags MA=lags [ARSEASONAL=lags] [MASEASONAL=lags] [DIFF=d] [DIFFSEASONAL=D]\n\
                 \x20                 [CONSTANT=YES | NO] [TRANSFORM=NONE | SQRT | LN]]\n\
                 \x20       [/TRANSFERFUNCTION [VARIABLES=ALL | predictors] [NUM=lags] [NUMSEASONAL=lags]\n\
                 \x20                          [DENOM=lags] [DENOMSEASONAL=lags] [DIFF=d] [DIFFSEASONAL=D]\n\
                 \x20                          [DELAY=b] [TRANSFORM=NONE | SQRT | LN]] ...\n\
                 \x20       [/AUTOOUTLIER DETECT=OFF | ON [TYPE=[ADDITIVE LEVELSHIFT INNOVATIONAL TRANSIENT\n\
                 \x20                    SEASONALADDITIVE LOCALTREND ADDITIVEPATCH]]]\n\
                 \x20       [/OUTLIER LOCATION=[case | date] TYPE=type] ... [/MODEL ...].\n\
                 Every series of DEPENDENT gets its own model, named PREFIX_1, PREFIX_2 and so on. Global\n\
                 subcommands come first; words are written in full and a list goes in square brackets.\n\
                 The season length is SEASONLENGTH, else the period of the DATE variables. The model is\n\
                 estimated over the USE range less missing values at its ends; forecasts run from the\n\
                 case after it to the end of PREDICT. Smoothing weights minimize the sum of squared\n\
                 one-step errors, from start values found by smoothing backwards. ARIMA models are fitted\n\
                 by exact maximum likelihood; a single lag value is an order (AR=2: lags 1 and 2), a list\n\
                 of two or more names the lags. In a custom ARIMA model each predictor passes through a\n\
                 ratio of lag polynomials: TRANSFERFUNCTION sets the numerator lags (lag 0 is always in)\n\
                 and denominator lags, plain and seasonal, and how the predictor is transformed,\n\
                 differenced and shifted by DELAY first; the last one naming a predictor counts. Without\n\
                 it a predictor enters at lag 0, and the expert modeler enters every predictor so ([E]\n\
                 after a name is accepted). OUTLIER puts an outlier of the type given at a case number\n\
                 or a DATE point; AUTOOUTLIER DETECT=ON searches the ARIMA model (custom or expert) for\n\
                 outliers of the types listed (additive and level shift by default), keeping those that\n\
                 stay significant when estimated with the model. The expert modeler fits every fitting\n\
                 smoothing method and a search of ARIMA orders and keeps the model with the smallest\n\
                 normalized BIC. Output: the model description, a summary of the fit measures across\n\
                 models, a table of the fit measures, the Ljung-Box Q(18) and the number of outliers by\n\
                 model, and on request parameters (with an Outliers table), residual autocorrelations,\n\
                 forecasts and charts. OUTFILE writes the models to a JSON file for TSAPPLY. SAVE adds\n\
                 root_series_model variables (Predicted, LCL, UCL, NResidual). Not supported yet:\n\
                 OUTPMML, the MODELSUMMARY charts and transfer functions chosen by the expert modeler.",
        example: "TSMODEL /MODELDETAILS PRINT=[PARAMETERS FORECASTS] /MODEL DEPENDENT=bookings /EXSMOOTH TYPE=HOLT.",
        parse: tsmodel::tsmodel,
    },
    CommandSpec {
        name: "TSAPPLY",
        summary: "Forecasts from time series models that TSMODEL saved, as they are or estimated again.",
        syntax: "TSAPPLY [global subcommands of TSMODEL] [/AUXILIARY [REESTIMATE=NO | YES] ...]\n\
                 \x20       /MODEL FILE=file [DROP=['id' ...] | KEEP=['id' ...]] [OUTFILE=file] [/MODEL ...].\n\
                 Reads the JSON model files TSMODEL writes and applies each model to the series of the\n\
                 same name. With REESTIMATE=NO (the default) the saved estimates and the saved series\n\
                 make the forecasts, which start after the case the model was estimated to, counted from\n\
                 the first case of the file; the fit measures are the saved ones. REESTIMATE=YES keeps the\n\
                 form of each model and estimates it again over the USE range of the active dataset.\n\
                 KEEP and DROP pick models by identifier; OUTFILE writes the models applied. Output and\n\
                 SAVE are as in TSMODEL; residual autocorrelations, fitted values and residuals need\n\
                 REESTIMATE=YES. Not supported under SPLIT FILE.",
        example: "TSAPPLY /MODELDETAILS PRINT=FORECASTS /MODEL FILE='bookings_models.json'.",
        parse: tsmodel::tsapply,
    },
    CommandSpec {
        name: "PLUM",
        summary: "Ordinal regression: a cumulative link model of an ordered dependent variable.",
        syntax: "PLUM dependent [BY factors] [WITH covariates]\n\
                 \x20  [/LINK=LOGIT | PROBIT | CLOGLOG | NLOGLOG | CAUCHIT] [/LOCATION=effects] [/SCALE=effects]\n\
                 \x20  [/CRITERIA=[CIN(95)] [DELTA(0)] [BIAS(0)] [MXITER(100)] [MXSTEP(5)] [LCONVERGE(0)]\n\
                 \x20             [PCONVERGE(1E-6)] [SINGULAR(1E-8)]] [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT=[FIT] [SUMMARY] [PARAMETER] [TPARALLEL] [CELLINFO] [COVB] [CORB] [HISTORY[(n)]] [KERNEL]]\n\
                 \x20  [/SAVE=[ESTPROB[(root[:n])]] [PREDCAT[(name)]] [PCPROB[(name)]] [ACPROB[(name)]]]\n\
                 \x20  [/TEST[(constants)]=['label'] effect values ...; ... | ALL values ...] ...\n\
                 The values of the dependent, sorted ascending (numbers or strings), are the ordered\n\
                 categories. Factors may be strings; covariates are numeric. A case missing on any variable is left out (INCLUDE\n\
                 keeps user-missing values); the weight is rounded to a whole number and a case below 1 after\n\
                 rounding is left out. The model puts the link of the probability of each category or a lower one\n\
                 equal to its threshold minus the location effects, divided by exp of the scale effects. LOCATION\n\
                 defaults to the covariates and then the main effect of each factor; effects are written as in\n\
                 UNIANOVA (a*b, a BY b, a(b), a WITHIN b, x*x), and INTERCEPT alone leaves only the thresholds. A\n\
                 factor has one parameter per level that occurs; the last is redundant and 0. The cases are pooled by\n\
                 their factor levels and covariate values: DELTA goes into each empty cell of such a group, BIAS into\n\
                 every cell. The fit is Newton-Raphson from thresholds at the cumulative shares of the categories and\n\
                 the other parameters 0 (the location part first when SCALE is given), each step halved up to MXSTEP\n\
                 times while the log likelihood would fall; it stops when no parameter moves PCONVERGE or more, the\n\
                 log likelihood changes less than LCONVERGE (absolute or relative), or after MXITER iterations.\n\
                 Tables: Case Processing Summary and Model Fitting Information (-2 log likelihood of the thresholds\n\
                 alone and of the final model, their difference as a chi-square) always; Goodness-of-Fit (FIT:\n\
                 Pearson and deviance over the pooled cells), Pseudo R-Square (SUMMARY: Cox and Snell, Nagelkerke,\n\
                 McFadden) and Parameter Estimates (PARAMETER: estimate, standard error, Wald, df, significance and\n\
                 CIN interval) without PRINT or when named; Test of Parallel Lines (TPARALLEL: a likelihood ratio\n\
                 test against separate location parameters for each threshold), Cell Information (CELLINFO),\n\
                 Parameter Covariance (COVB), Parameter Correlation (CORB) and Iteration History (HISTORY, every\n\
                 n-th, the first and the last) when named. The -2 log likelihood includes the multinomial constant of\n\
                 the pooled cells; KERNEL leaves it out. A warning counts the empty pooled cells.\n\
                 SAVE adds EST1_m, EST2_m, ... (the first n categories, 25 by default), PRE_m, PCP_m and ACP_m with\n\
                 the smallest free m, or root1, root2, ... and the names given: the probability of each category, the\n\
                 most likely category, its probability and the probability of the observed category, for every case\n\
                 with valid factors and covariates. TEST gives Wald tests of combinations of the parameters, in the\n\
                 order of the Parameter Estimates (thresholds, location, scale, redundant ones included): INTERCEPT\n\
                 takes one value per threshold, ALL one per parameter; one TEST uses either ALL in every row or\n\
                 effect names, never both. Each row is tested against its constant (0 by default) on 1 df and the\n\
                 rows together on the rank of their covariance; a coefficient on a redundant parameter multiplies\n\
                 its 0.",
        example: "PLUM comfort BY seat_row WITH trip_hours /LINK=PROBIT /PRINT=FIT PARAMETER SUMMARY TPARALLEL /SAVE=ESTPROB PREDCAT.",
        parse: plum::plum,
    },
    CommandSpec {
        name: "QUANTILE REGRESSION",
        summary: "Linear regression of a chosen quantile of the dependent variable, fitted by linear programming.",
        syntax: "QUANTILE REGRESSION dependent [BY factors] [WITH covariates]\n\
                 \x20  [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/CRITERIA [QUANTILE=values [v1 TO v2 BY step]] [IID=TRUE | FALSE]\n\
                 \x20             [BANDWIDTH=BOFINGER | HALL_SHEATHER] [CILEVEL=95]\n\
                 \x20             [METHOD=AUTO | SIMPLEX | INTERIOR_POINT] [TOL=1E-12] [CONV=1E-6] [MAXITER=2000]]\n\
                 \x20  [/MODEL [effects] [REGWGT=var] [INTERCEPT=TRUE | FALSE]]\n\
                 \x20  [/SAVE [PRED[(name)]] [RESID[(name)]] [CIPREDL[(name)]] [CIPREDU[(name)]]]\n\
                 \x20  [/PREDICT_EFFECTS NUM_TOP_EFFECTS=3] [/PREDICTION_TABLES effects] [/PREDICTION_LINES effects]\n\
                 \x20  [/PRINT [PARAMETER] [COVB] [CORB]]\n\
                 \x20  [/PLOT [MAX_CATEGORIES=10] [PREDICTED_BY_OBSERVED=FALSE | TRUE] [PARAM_EST=50 | ALL]]\n\
                 \x20  [/OUTFILE [COVB=file] [CORB=file] [FILE_SEPARATE=FALSE | TRUE] [MODEL=file | PARAMETER=file]]\n\
                 \x20  [/EXTERNAL].\n\
                 For each quantile p (0.5 unless QUANTILE lists others; a grid v1 TO v2 BY step may join\n\
                 them, a repeated value is fitted once) the coefficients make the weighted sum of p times\n\
                 each positive residual and 1-p times the size of each negative one as small as it can be.\n\
                 The weight is the case weight, rounded, times REGWGT; a case with a weight below 0.5 or a\n\
                 REGWGT of 0 or less is left out, as is a case missing on any variable (CLASSMISSING=INCLUDE\n\
                 keeps user-missing factor values). MODEL lists effects as in UNIANOVA; without them the\n\
                 model has the intercept and every covariate and factor as a main effect. A parameter that\n\
                 depends on earlier ones is redundant and 0. SIMPLEX walks from vertex to vertex of the\n\
                 linear program to the exact solution; INTERIOR_POINT runs Frisch-Newton steps (TOL for its\n\
                 matrices, CONV for the duality gap, MAXITER steps) and finishes with the simplex from there;\n\
                 AUTO takes the simplex for up to 1000 cases. With IID=TRUE the standard errors come from the\n\
                 spread of the residuals around p over a bandwidth (BOFINGER or HALL_SHEATHER); with FALSE\n\
                 the model is refitted at p plus and minus the bandwidth and each case gets its own density.\n\
                 t tests and CILEVEL intervals use N minus the number of estimated parameters as df.\n\
                 Tables: Case Processing Summary, Model Quality (pseudo R squared against the intercept-only\n\
                 fit, mean absolute error), Parameter Estimates (PARAMETER, shown unless PRINT leaves it out),\n\
                 Parameter Covariance (COVB) and Parameter Correlation (CORB). Predicted values over the cells\n\
                 of effects with one or two factors are tables, along one covariate (alone or with factors)\n\
                 line charts, other covariates at their means and other factors averaged: for the effects of\n\
                 PREDICTION_TABLES and PREDICTION_LINES, or else the first NUM_TOP_EFFECTS such effects of the\n\
                 model. PLOT PREDICTED_BY_OBSERVED adds a scatter of each case per quantile, PARAM_EST a chart\n\
                 of each of the first n estimated parameters across the quantiles (with two or more).\n\
                 SAVE adds the predicted value, residual and interval bounds (named PRED, RESID, CIPREDL and\n\
                 CIPREDU unless a name is given; with several quantiles the name gets _ and the quantile's\n\
                 digits, as in PRED_25). OUTFILE COVB or CORB writes RowType_, VarName_ and P1, P2, ... with\n\
                 rows EST, SE, SIG, DF and then COV or CORR to a data file or declared dataset (a Quantile\n\
                 column joins several quantiles; FILE_SEPARATE=TRUE writes one file each, the quantile's\n\
                 digits added to the name). MODEL or PARAMETER writes the estimates (MODEL with their\n\
                 covariances) to a JSON file of Statscraft's own, one per quantile. EXTERNAL changes nothing.",
        example: "QUANTILE REGRESSION rent BY district WITH floor_area /CRITERIA QUANTILE=0.25 0.5 0.75 IID=FALSE /PRINT PARAMETER COVB /SAVE PRED(fit) RESID.",
        parse: quantile_regression::quantile_regression,
    },
    CommandSpec {
        name: "PLS",
        summary: "Partial least squares regression of one or more dependent variables on many, possibly collinear, predictors.",
        syntax: "PLS dependent [MLEVEL=N | O | S] [REFERENCE=FIRST | LAST | value] [dependent ...]\n\
                 \x20  [BY factors] [WITH covariates]\n\
                 \x20  [/ID VARIABLE=var] [/MODEL effects]\n\
                 \x20  [/OUTDATASET [CASES=dataset] [LATENTFACTORS=dataset] [PREDICTORS=dataset]]\n\
                 \x20  [/CRITERIA LATENTFACTORS=5].\n\
                 Each subcommand and keyword at most once, written in full, with its equals sign. A\n\
                 dependent is scale or categorical: MLEVEL after it, else its measurement level, else scale\n\
                 for a number and categorical for a string (N and O both mean categorical). ALL\n\
                 MLEVEL=... or ALL REFERENCE=... sets every dependent that has no setting of its own. A categorical\n\
                 dependent becomes one 0/1 column per category except the reference one (REFERENCE: FIRST,\n\
                 LAST by default, or a value that occurs). Factors follow BY, covariates WITH; a factor\n\
                 gives a 0/1 column for every level, an interaction one column per combination of levels\n\
                 that occurs (times the covariates in it). MODEL lists main effects and interactions (a*b\n\
                 or a BY b), the main effects of all predictors by default; nested terms are not taken.\n\
                 Every column is centered and divided by its standard deviation. Cases missing on any\n\
                 variable (user-missing too) are left out; the weight is rounded and a case below 1 is left\n\
                 out. NIPALS extracts up to LATENTFACTORS factors, no more than the predictor columns or the\n\
                 cases less one. Tables: Proportion of Variance Explained (X and Y, cumulative, R-square\n\
                 and adjusted R-square), Parameters (constant and coefficient of every predictor column\n\
                 in the original units), Variable Importance in the Projection (cumulative by factor),\n\
                 Weights and Loadings. OUTDATASET writes named datasets: CASES (ID or case number,\n\
                 predicted values and residuals, the distance to the X model, the X and Y scores) for every\n\
                 case with valid predictors, LATENTFACTORS (loadings and weights of each variable) and\n\
                 PREDICTORS (coefficients and VIP of each predictor column). No plots.",
        example: "PLS yield MLEVEL=S quality MLEVEL=N REFERENCE=FIRST BY plot WITH rain sun soil_ph /CRITERIA LATENTFACTORS=3 /OUTDATASET CASES=plsCases.",
        parse: pls::pls,
    },
    CommandSpec {
        name: "CORRESPONDENCE",
        summary: "Correspondence analysis: rows and columns of a two-way table as points in a few dimensions.",
        syntax: "CORRESPONDENCE TABLE=rowvar(min,max) BY colvar(min,max) | ALL(rows,columns)\n\
                 \x20  [/SUPPLEMENTARY=[rowvar | ROW (values)] [colvar | COLUMN (values)]]\n\
                 \x20  [/EQUAL=[rowvar | ROW (values) (values) ...] [colvar | COLUMN (values) ...]]\n\
                 \x20  [/MEASURE=CHISQ | EUCLID] [/STANDARDIZE=RCMEAN | RMEAN | CMEAN | RSUM | CSUM]\n\
                 \x20  [/DIMENSION=2 | n] [/NORMALIZATION=SYMMETRICAL | PRINCIPAL | RPRINCIPAL | CPRINCIPAL | q]\n\
                 \x20  [/PRINT=[TABLE] [RPROFILES] [CPROFILES] [RPOINTS] [CPOINTS] [RCONF] [CCONF]\n\
                 \x20          [PERMUTATION[(n)]] [DEFAULT] [NONE]]\n\
                 \x20  [/PLOT=[BIPLOT[(len)]] [RPOINTS[(len)]] [CPOINTS[(len)]] [TRROWS[(len)]] [TRCOLUMNS[(len)]]\n\
                 \x20         [NDIM(first, last | MAX)] [NONE]]\n\
                 \x20  [/OUTFILE=[SCORE('file' | dataset)] [VARIANCE('file' | dataset)]]\n\
                 Each case adds its weight (0 when negative) to one cell: every whole number in a range is a\n\
                 category, values are truncated to whole numbers, and a case outside a range or missing on either\n\
                 variable is left out. With ALL the first cases are the rows and the first variables the columns\n\
                 (ROWCAT_ aside, whose values and labels name the rows); a missing or negative cell counts as 0\n\
                 and WEIGHT does not apply. SUPPLEMENTARY rows and columns are placed in the solution after it is\n\
                 found and take no part in it; each EQUAL group is added up into one category whose point all its\n\
                 members share. Empty rows and columns are left out. CHISQ analyzes the standardized residuals of\n\
                 the table; EUCLID uses STANDARDIZE: RMEAN takes out the row means, CMEAN the column means, RCMEAN\n\
                 both (the same as CHISQ), RSUM and CSUM first bring the row or column totals to one value. The\n\
                 solution has at most one dimension fewer than the smaller of the active rows and columns; the\n\
                 Summary lists every dimension with a singular value above 0.\n\
                 NORMALIZATION q gives the row scores the power (1 + q)/2 of each singular value and the column\n\
                 scores (1 - q)/2; PRINCIPAL gives both the full value, which rules out the biplot. In every\n\
                 dimension the active row with the largest absolute score scores positive.\n\
                 Tables: Summary always (singular values, inertia, the chi-square of the table with its\n\
                 significance, the share of the inertia of each dimension, and the standard deviation of each\n\
                 singular value with the correlations between them); TABLE, RPOINTS and CPOINTS without PRINT or\n\
                 with DEFAULT: the table with its active margins and the Overview Row and Column Points (mass,\n\
                 scores, inertia, each point's share of a dimension and each dimension's share of a point);\n\
                 RPROFILES and CPROFILES the profiles; RCONF and CCONF the standard deviations of the active\n\
                 scores and their correlations across dimensions (delta method under multinomial sampling);\n\
                 PERMUTATION(n) the table sorted by the scores of each of the first n dimensions (1 by default).\n\
                 Plots: BIPLOT (the default) the rows and columns together, RPOINTS and CPOINTS each side alone,\n\
                 one chart for each pair of dimensions; TRROWS and TRCOLUMNS the scores of the categories in each\n\
                 dimension. NDIM(a, b) plots dimension a against a+1 to b, and the transformation plots of a to\n\
                 b; without it, or without values, every pair. A number after a plot keyword cuts the labels to\n\
                 that length (60 at most). OUTFILE SCORE writes ROWTYPE_, LEVEL_, VARNAME_ and DIM1, DIM2 and\n\
                 so on for every category; VARIANCE writes the covariances of the singular values (VARNAME_\n\
                 SINGULAR) and of the scores of each active point outside the EQUAL groups, one case per\n\
                 dimension (DIMNMBR_).",
        example: "CORRESPONDENCE TABLE=district(1,5) BY transport(1,4) /SUPPLEMENTARY=district(5) /PRINT=DEFAULT RPROFILES RCONF /PLOT=BIPLOT.",
        parse: correspondence::correspondence,
    },
    CommandSpec {
        name: "MULTIPLE CORRESPONDENCE",
        summary: "Multiple correspondence (homogeneity) analysis: cases and categories of several nominal variables as points in a few dimensions.",
        syntax: "MULTIPLE CORRESPONDENCE [/VARIABLES=] varlist\n\
                 \x20  /ANALYSIS=varlist [(WEIGHT=n)] [varlist [(WEIGHT=n)] ...]\n\
                 \x20  [/DISCRETIZATION=[varlist [([GROUPING] [NCAT=7 | n] [DISTR=NORMAL | UNIFORM] | [EQINTV=width]\n\
                 \x20                              | RANKING | MULTIPLYING)] ...]]\n\
                 \x20  [/MISSING=[varlist | ALL [([PASSIVE] [MODEIMPU | EXTRACAT] | ACTIVE [MODEIMPU | EXTRACAT | RANDIMPU]\n\
                 \x20                              | LISTWISE)] ...]]\n\
                 \x20  [/SUPPLEMENTARY=[OBJECT(case numbers)] [VARIABLE(varlist)]]\n\
                 \x20  [/CONFIGURATION=[INITIAL | FIXED]('file' | dataset)] [/DIMENSION=2 | n]\n\
                 \x20  [/NORMALIZATION=VPRINCIPAL | OPRINCIPAL | SYMMETRICAL | INDEPENDENT | q]\n\
                 \x20  [/MAXITER=100 | n] [/CRITITER=.00001 | value]\n\
                 \x20  [/PRINT=[CORR] [DESCRIP[(varlist)]] [DISCRIM] [HISTORY] [OBJECT[([(labelvar)] varlist)]] [OCORR]\n\
                 \x20         [QUANT[(varlist)]] [NONE]]\n\
                 \x20  [/PLOT=[OBJECT[(varlist)][(len)]] [CATEGORY(varlist)[(len)]] [JOINTCAT[(varlist)][(len)]]\n\
                 \x20        [DISCRIM[(varlist)][(len)]] [BIPLOT[((varlist))][(varlist)][(len)]] [TRANS(var[(n)] ...)]\n\
                 \x20        [RESID(var[(n)] ...)[(len)]] [NDIM(first, last)] [NONE]]\n\
                 \x20  [/SAVE=[TRDATA[(root[(n)])]] [OBJECT[(root[(n)])]]]\n\
                 \x20  [/OUTFILE=[DISCRDATA('file' | dataset)] [TRDATA('file' | dataset)] [OBJECT('file' | dataset)]].\n\
                 Each analysis variable holds categories coded 1, 2, 3 and so on; a code below 1, a system-missing\n\
                 and a user-missing value are missing. String values are coded in sorted order, and a numeric\n\
                 variable with fractions is cut into 7 groups (fewer if it has fewer values) shaped like a normal\n\
                 curve. DISCRETIZATION recodes on purpose, without case weights and with codes below 1 still\n\
                 valid: GROUPING into NCAT groups, NORMAL by the optimal cut points of a normal curve applied to\n\
                 z-scores, UNIFORM with counts near n/NCAT, or EQINTV bands of a given width from the smallest\n\
                 value; RANKING numbers the distinct values; MULTIPLYING rounds 10 times the z-score and shifts\n\
                 the lowest to 1. The analysis places each case (object) and each category in a few dimensions\n\
                 so that a category is the weighted mean of its objects and objects lie close to their own\n\
                 categories; the WEIGHT on ANALYSIS gives a variable more say, and WEIGHT cases count by their\n\
                 weight. MISSING PASSIVE (the default) leaves the variable out for that object; its MODEIMPU or\n\
                 EXTRACAT only fills the correlation tables. ACTIVE fills the value before the fit with the\n\
                 weighted mode (smallest code on ties), an extra category or a random category drawn by\n\
                 frequency from the session's generator. LISTWISE drops the case. A case without any active\n\
                 value becomes supplementary. Supplementary objects and variables are placed after the fit.\n\
                 CONFIGURATION INITIAL starts the iterations from the coordinates in a file (one variable per\n\
                 dimension, one case per case of the data); FIXED keeps those coordinates, standardized and made\n\
                 uncorrelated, and fits every analysis variable as supplementary. Without a file the start is the\n\
                 exact solution of the eigenproblem, so the iterations only confirm it (with more than 600\n\
                 categories in all, a fixed pseudo-random start instead). DIMENSION is lowered with a warning to\n\
                 what the data allow. The fit stops after MAXITER iterations or when it gains less than CRITITER.\n\
                 NORMALIZATION q gives the object scores the power (1 + q)/2 of each singular value (the root of\n\
                 the inertia) and the categories the power (1 - q)/2, both measured from the variable principal\n\
                 solution: VPRINCIPAL -1 (objects standard, categories at their centroids), OPRINCIPAL 1,\n\
                 SYMMETRICAL 0, INDEPENDENT both principal. In every dimension the object with the largest\n\
                 absolute score is positive. SPLIT FILE does not apply.\n\
                 Tables: Case Processing Summary, Iteration History (the last step, or every step with HISTORY:\n\
                 variance accounted for, its increase and the loss) and Model Summary (Cronbach's alpha,\n\
                 eigenvalue, inertia and percent of variance per dimension, the total and the mean) always;\n\
                 DESCRIP the frequencies, missing count and mode of each variable; DISCRIM the discrimination\n\
                 measures with the active total; CORR the correlations of the quantified variables in each\n\
                 dimension and OCORR of the codes, each with the eigenvalues of the active part; QUANT per\n\
                 variable the centroids, mass, inertia and both kinds of contributions; OBJECT the object scores\n\
                 (optionally labelled by a variable and with codes of others beside them) and the object\n\
                 contributions. Without PRINT, or with PRINT alone: DESCRIP DISCRIM CORR.\n\
                 Charts (OBJECT and DISCRIM by default): objects, colored by the categories of each named\n\
                 variable; category points of each variable; all categories together (JOINTCAT); the\n\
                 discrimination measures; objects with centroids (BIPLOT, not with INDEPENDENT); quantification\n\
                 by code (TRANS) and quantified value against object score (RESID) for the first n dimensions.\n\
                 Two dimensions give one scatter, more a scatterplot matrix, NDIM(a, b) one scatter of a against\n\
                 each of a+1 to b. A label length of 0 shows values and names instead of labels.\n\
                 SAVE adds the quantified variables rootK_D_S (TRA by default, K the place on ANALYSIS, D the\n\
                 dimension, S the first free set number) and the object scores rootD_S (OBSCO), both as the\n\
                 normalization says; excluded cases and passive missing values are system-missing. OUTFILE\n\
                 writes the discretized codes (DISCRDATA, under the variables' names), the quantified variables\n\
                 TRAK_D and the object scores OBSCOD.",
        example: "MULTIPLE CORRESPONDENCE pet garden commute /ANALYSIS=pet garden(WEIGHT=2) commute /DISCRETIZATION=commute(GROUPING NCAT=4) /MISSING=garden(ACTIVE EXTRACAT) /PRINT=DISCRIM QUANT OBJECT((household)) /PLOT=JOINTCAT /SAVE=OBJECT.",
        parse: multiple_correspondence::multiple_correspondence,
    },
    CommandSpec {
        name: "CATPCA",
        summary: "Categorical principal components: quantify each variable by optimal scaling (spline, ordinal, nominal, multiple nominal or numeric) and reduce them to a few components.",
        syntax: "CATPCA [/VARIABLES=] varlist\n\
                 \x20  /ANALYSIS=varlist [([WEIGHT=n] [LEVEL=SPORD | SPNOM | ORDI | NOMI | MNOM | NUME]\n\
                 \x20                     [DEGREE=2 | n] [INKNOT=2 | n])] [varlist [(...)] ...]\n\
                 \x20  [/DISCRETIZATION=[varlist [([GROUPING] [NCAT=7 | n] [DISTR=NORMAL | UNIFORM] | [EQINTV=width]\n\
                 \x20                              | RANKING | MULTIPLYING)] ...]]\n\
                 \x20  [/MISSING=[varlist | ALL [([PASSIVE] [MODEIMPU | EXTRACAT | RANDIMPU]\n\
                 \x20                              | ACTIVE [MODEIMPU | EXTRACAT | RANDIMPU] | LISTWISE)] ...]]\n\
                 \x20  [/SUPPLEMENTARY=[OBJECT(case numbers)] [VARIABLE(varlist)]]\n\
                 \x20  [/CONFIGURATION=[INITIAL | FIXED]('file' | dataset)] [/DIMENSION=2 | n]\n\
                 \x20  [/NORMALIZATION=VPRINCIPAL | OPRINCIPAL | SYMMETRICAL | INDEPENDENT | q]\n\
                 \x20  [/MAXITER=100 | n] [/CRITITER=.00001 | value]\n\
                 \x20  [/ROTATION=[NOROTATE | VARIMAX | EQUAMAX | QUARTIMAX | PROMAX[(4 | kappa)] | OBLIMIN[(0 | delta)]]\n\
                 \x20             [KAISER | NOKAISER]]\n\
                 \x20  [/RESAMPLE=NONE | BOOTSTRAP[(samples, percent, BALANCED | UNBALANCED, PROCRU | REFLEC)]]\n\
                 \x20  [/PRINT=[DESCRIP[(varlist)]] [LOADING[(SORT | NOSORT)]] [CORR] [VAF] [OCORR] [QUANT[(varlist)]]\n\
                 \x20         [HISTORY] [OBJECT[([(labelvar)] varlist)]] [NONE]]\n\
                 \x20  [/PLOT=[OBJECT[(varlist)][(len)]] [LOADING[(varlist [(CENTR[(varlist)])])][(len)]]\n\
                 \x20        [CATEGORY(varlist)[(len)]] [JOINTCAT[(varlist)][(len)]] [TRANS(var[(n)] ...)[(len)]]\n\
                 \x20        [RESID(var[(n)] ...)[(len)]] [BIPLOT[(LOADING | CENTR[(varlist)])][(varlist)][(len)]]\n\
                 \x20        [TRIPLOT[(varlist[(varlist)])][(len)]] [PROJCENTR(var, varlist)[(len)]] [VAF]\n\
                 \x20        [NDIM(first, last)] [NONE]]\n\
                 \x20  [/SAVE=[TRDATA[(root[(n)])]] [OBJECT[(root[(n)])]] [APPROX[(root)]]]\n\
                 \x20  [/OUTFILE=[DISCRDATA('file' | dataset)] [TRDATA('file' | dataset)] [OBJECT('file' | dataset)]\n\
                 \x20           [APPROX('file' | dataset)]].\n\
                 Variables are coded as for MULTIPLE CORRESPONDENCE: whole codes from 1, lower codes and\n\
                 missing values missing, strings coded in sorted order, fractions cut into 7 groups unless\n\
                 DISCRETIZATION says otherwise; DISCRETIZATION, MISSING, SUPPLEMENTARY, CONFIGURATION, DIMENSION,\n\
                 MAXITER and CRITITER work as there. Each variable gets a value for each category, chosen so\n\
                 that the components explain the quantified variables as well as they can. LEVEL says what the\n\
                 values must respect: NUME equal steps between codes (plain principal components when every\n\
                 variable is numeric), ORDI the order of the codes, NOMI nothing but the grouping, SPORD (the\n\
                 default) and SPNOM a smooth curve through the codes, a spline of DEGREE pieces joined at INKNOT\n\
                 interior knots placed at quantiles of the cases' codes (SPORD keeps it rising), and MNOM a\n\
                 separate value per component, the centroid of the category's cases. WEIGHT gives a variable\n\
                 more say. The fit starts from the exact numeric solution (iteration 0), then alternates\n\
                 quantifications and component scores until the variance accounted for gains less than\n\
                 CRITITER. An extra category from ACTIVE EXTRACAT is left out of the level's restriction after\n\
                 iteration 0. The components are then turned to principal axes, and a component is reflected\n\
                 when its negative loadings have the larger mean square. Supplementary variables are fitted to\n\
                 the final scores. NORMALIZATION q gives the scores the power (1 + q)/2 of the root of each\n\
                 eigenvalue and the loadings and multiple nominal centroids the power (1 - q)/2 less one, so\n\
                 that scores times loadings stay the same: VPRINCIPAL -1 (standard scores, loadings are\n\
                 correlations), OPRINCIPAL 1, SYMMETRICAL 0; INDEPENDENT scales the scores only. ROTATION turns\n\
                 the loadings of the variables that have loadings, with Kaiser normalization unless NOKAISER,\n\
                 and the scores and centroids with them. RESAMPLE=BOOTSTRAP is not supported yet (an error);\n\
                 a rotation with BOOTSTRAP is an error too. SPLIT FILE does not apply.\n\
                 Tables: Case Processing Summary, Iteration History (iteration 0 and the last, or all with\n\
                 HISTORY: variance accounted for, its increase, and the loss split into the centroid part and\n\
                 the part from holding single variables to a line) and Model Summary (Cronbach's alpha,\n\
                 eigenvalue and percent of variance per dimension and in total) always; DESCRIP frequencies,\n\
                 missing count and mode; QUANT per variable the quantification, centroid and vector\n\
                 coordinates; VAF the variance each variable accounts for, by centroids and by vectors; OCORR\n\
                 and CORR the correlations of the original and of the quantified variables (one matrix per\n\
                 dimension when a variable is MNOM), with eigenvalues; OBJECT the component scores; LOADING the\n\
                 component loadings (SORT groups them by their largest loading), with the rotated loadings and\n\
                 transformation matrix, or pattern, structure and component correlations after an oblique\n\
                 rotation. Without PRINT, or with PRINT alone: DESCRIP LOADING CORR.\n\
                 Charts (OBJECT and LOADING by default): component scores, colored by the categories of each\n\
                 named variable; loadings, with MNOM centroids after CENTR; category centroids and vectors of\n\
                 each variable; all centroids together (JOINTCAT); quantification by code (TRANS); quantified\n\
                 value against its approximation (RESID); scores with loadings or centroids (BIPLOT) or with\n\
                 both (TRIPLOT), scores and centroids shrunk to the loadings, neither with INDEPENDENT; the\n\
                 centroids of one variable projected on the loading direction of others (PROJCENTR, also as a\n\
                 table); bars of each variable's variance accounted for per dimension and in total (VAF). Two\n\
                 dimensions give one scatter, more a scatterplot matrix, NDIM(a, b) one scatter of a against\n\
                 each of a+1 to b. A label length of 0 shows values and names instead of labels.\n\
                 SAVE adds the quantified variables rootK_S (TRA, K the place on ANALYSIS, S the first free set\n\
                 number; an MNOM variable gives rootK_D_S per dimension D with the root cut to 3 characters),\n\
                 the scores rootD_S (OBSCO) and the approximations rootK_S (APP: scores times loadings, the\n\
                 first score for an MNOM variable). Excluded cases and passive missing values are\n\
                 system-missing. OUTFILE writes the codes (DISCRDATA), TRAK or TRAK_D, OBSCOD and APPK; a file\n\
                 with no keyword means TRDATA.",
        example: "CATPCA pace crowd parking shade /ANALYSIS=pace crowd(LEVEL=ORDI) parking(LEVEL=NOMI) shade(LEVEL=SPNOM INKNOT=1) /MISSING=shade(ACTIVE EXTRACAT) /ROTATION=VARIMAX /PRINT=LOADING(SORT) VAF QUANT(parking) /PLOT=BIPLOT TRANS(shade) /SAVE=OBJECT.",
        parse: catpca::catpca,
    },
    CommandSpec {
        name: "CATREG",
        summary: "Categorical regression: quantify a response and its predictors by optimal scaling (spline, ordinal, nominal or numeric) so that a linear model fits best, with ridge, lasso and elastic net penalties.",
        syntax: "CATREG VARIABLES=varlist\n\
                 \x20  /ANALYSIS=response [([LEVEL=SPORD | SPNOM | ORDI | NOMI | NUME] [DEGREE=2 | n] [INKNOT=2 | n])]\n\
                 \x20            WITH varlist [(...)] [varlist [(...)] ...]\n\
                 \x20  [/DISCRETIZATION=[varlist [([GROUPING] [NCAT=7 | n] [DISTR=NORMAL | UNIFORM] | [EQINTV=width]\n\
                 \x20                              | RANKING | MULTIPLYING)] ...]]\n\
                 \x20  [/MISSING=[varlist | ALL [(LISTWISE | MODEIMPU | EXTRACAT)] ...]]\n\
                 \x20  [/SUPPLEMENTARY=OBJECT(case numbers)]\n\
                 \x20  [/INITIAL=NUMERICAL | RANDOM | MULTISTART[(50 | threshold | ALL)]('file' | dataset)\n\
                 \x20            | FIXSIGNS(row)('file' | dataset)]\n\
                 \x20  [/MAXITER=100 | n] [/CRITITER=.00001 | value]\n\
                 \x20  [/REGULARIZATION=NONE | RIDGE[(start, stop, step)][('file')] | LASSO[(start, stop, step)][('file')]\n\
                 \x20                   | ENET[(start, stop, step)(start, stop, step)][('file')]]\n\
                 \x20  [/RESAMPLE=NONE | CROSSVAL[(10 | folds)] | BOOTSTRAP[(50 | samples)]]\n\
                 \x20  [/PRINT=[R] [COEFF] [DESCRIP[(varlist)]] [HISTORY] [ANOVA] [CORR] [OCORR] [QUANT[(varlist)]]\n\
                 \x20         [REGU] [NONE]]\n\
                 \x20  [/PLOT=[TRANS(varlist)[(len)]] [RESID(varlist)[(len)]] [REGU[(values | ALL)]]]\n\
                 \x20  [/SAVE=[TRDATA[(root)]] [PRED[(root)]] [RES[(root)]]]\n\
                 \x20  [/OUTFILE=[TRDATA('file' | dataset)] [DISCRDATA('file' | dataset)]].\n\
                 Variables are coded as for CATPCA: whole codes from 1, lower codes and missing values\n\
                 missing, strings coded in sorted order, fractions cut into 7 groups unless DISCRETIZATION says\n\
                 otherwise. MISSING drops cases with a gap (LISTWISE, the default) or fills the gap first with\n\
                 the most frequent code (MODEIMPU) or with a new code above all others (EXTRACAT, kept out of\n\
                 the level's restriction). Each variable gets a value for each category, and each predictor a\n\
                 coefficient, so that the weighted squared error of the quantified response is least, every\n\
                 quantified variable having mean 0 and variance 1. LEVEL says what the values must respect:\n\
                 NUME equal steps (all numeric is plain regression on standardized variables), ORDI the order\n\
                 of the codes, NOMI nothing but the grouping, SPORD (the default) and SPNOM a spline of DEGREE\n\
                 joined at INKNOT interior knots (SPORD rising). The fit updates the response and then each\n\
                 predictor in turn until the error drops less than CRITITER, at most MAXITER times.\n\
                 INITIAL=NUMERICAL first fits every variable as numeric (iteration 0 of the history); RANDOM\n\
                 starts the levels at once from the standardized codes with the correlations as coefficients.\n\
                 MULTISTART tries sign patterns for the coefficients of the ordinal and spline ordinal\n\
                 predictors from a nominal start, all of them (ALL) or those a step-by-step search allows (a\n\
                 predictor may turn negative only when its order restriction costs more than the threshold\n\
                 percent of its variance), keeps the best and writes each pattern with its R square to the\n\
                 file; FIXSIGNS takes the signs from a row of such a file. REGULARIZATION shrinks the\n\
                 coefficients: RIDGE divides by 1 plus the penalty, LASSO moves them half the penalty towards\n\
                 0 (to 0 when they would cross), ENET does both; each value from start to stop by step gives a\n\
                 model. With more than one model and neither RESAMPLE nor supplementary cases only the REGU\n\
                 table and chart are shown; with them the model with the least prediction error is reported.\n\
                 RESAMPLE estimates the prediction error by K-fold cross validation or by the .632 bootstrap;\n\
                 supplementary cases give a test error. Supplementary cases get the quantifications of the\n\
                 active ones; a code only they have is interpolated between neighbouring codes for numeric and\n\
                 spline levels and extrapolated along the line for numeric ones, otherwise the case gets no\n\
                 prediction. SPLIT FILE does not apply.\n\
                 Tables: Case Processing Summary always; DESCRIP frequencies, missing count and mode; HISTORY\n\
                 R square and apparent prediction error by iteration; R the Model Summary (multiple R, R\n\
                 square, adjusted R square with the predictor count and with the optimal scaling degrees of\n\
                 freedom, the apparent and, with resampling or test cases, the expected prediction error);\n\
                 ANOVA two tables, one with the predictor count and one with the optimal scaling degrees of\n\
                 freedom; COEFF the coefficients with a standard error from 1000 bootstrap samples, degrees\n\
                 of freedom, F and significance, and the zero-order, partial and part correlations, Pratt's\n\
                 importance and the tolerance after and before the quantification; CORR and OCORR the\n\
                 correlations of the quantified and of the original predictors with eigenvalues; QUANT the\n\
                 quantification of each category; REGU per model the penalties, R square, the prediction\n\
                 error when estimated, and the coefficients, also written to the REGULARIZATION file.\n\
                 Without PRINT, or with PRINT alone: R COEFF DESCRIP ANOVA.\n\
                 Charts (none by default): TRANS the quantification by code; RESID per predictor its\n\
                 coefficient times quantification and the residuals of the other predictors by code; REGU\n\
                 the coefficient paths against the standardized sum of coefficients (one chart per listed\n\
                 ridge penalty for ENET). A label length of 0 shows codes instead of labels; above 60 it is 60.\n\
                 SAVE adds the quantified variables rootK_S (TRA, K the place on ANALYSIS with the response\n\
                 first, S the first free set number), the predictions root_S (PRE) and the residuals root_S\n\
                 (RES); excluded cases are system-missing. OUTFILE writes the quantified variables TRAK\n\
                 (TRDATA, also a file with no keyword) and the coded variables under their names (DISCRDATA).",
        example: "CATREG VARIABLES=rating taste price area /ANALYSIS=rating(LEVEL=ORDI) WITH taste price(LEVEL=NUME) area(LEVEL=NOMI) /MISSING=price(MODEIMPU) /PRINT=R COEFF ANOVA QUANT(area) /PLOT=TRANS(area) /SAVE=PRED RES.",
        parse: catreg::catreg,
    },
    CommandSpec {
        name: "HOMALS",
        summary: "Homogeneity analysis in the older syntax: MULTIPLE CORRESPONDENCE with variables read as categories 1 to a stated largest value.",
        syntax: "HOMALS [/VARIABLES=] varlist(max) [varlist(max) ...]\n\
                 \x20  [/ANALYSIS=varlist] [/NOBSERVATIONS=n] [/DIMENSION=2 | n]\n\
                 \x20  [/MAXITER=100 | n] [/CONVERGENCE=.00001 | value]\n\
                 \x20  [/PRINT=[FREQ] [EIGEN] [DISCRIM] [QUANT] [OBJECT] [HISTORY] [DEFAULT] [ALL] [NONE]]\n\
                 \x20  [/PLOT=[NDIM(first | ALL, last | MAX)] [QUANT[(varlist)][(len)]] [OBJECT[(varlist)][(len)]]\n\
                 \x20        [DISCRIM[(len)]] [DEFAULT[(len)]] [ALL[(len)]] [NONE]]\n\
                 \x20  [/SAVE=[root][(n)]] [/MATRIX=OUT(* | 'file' | dataset)]\n\
                 The number after a list is the largest category of its variables: every whole value from 1 up to\n\
                 it is a category, a fraction is cut to its whole part, user-missing codes count as values, and\n\
                 anything else (system-missing, 0, negatives, above the largest) is missing and left out of the fit\n\
                 for that variable. String variables are refused. ANALYSIS picks the variables of the fit (all of\n\
                 VARIABLES by default; named twice it is an error); the others can still label object plots.\n\
                 NOBSERVATIONS uses only the first n cases. Every other subcommand counts once, the last one.\n\
                 The model, the fit and the numbers are those of MULTIPLE CORRESPONDENCE with variable principal\n\
                 normalization, run on these categories; messages name HOMALS. Tables: Case Processing Summary,\n\
                 Iteration History and Model Summary (eigenvalue and inertia per dimension; the inertia is the\n\
                 mean discrimination measure) always; FREQ the frequencies of each variable; DISCRIM the\n\
                 discrimination measures; QUANT the category quantifications; OBJECT the object scores; HISTORY\n\
                 every iteration. Without PRINT, or with DEFAULT: FREQ EIGEN DISCRIM QUANT; ALL adds OBJECT and\n\
                 HISTORY. Charts (QUANT and OBJECT by default, none for one dimension): QUANT all categories\n\
                 together, OBJECT the objects labelled by each named variable, DISCRIM the discrimination\n\
                 measures. The label length lies from 1 to 20 (12 by default). NDIM(a, b) draws dimension a\n\
                 against a+1 to b, MAX the last dimension, ALL every pair; without it dimension 1 against 2.\n\
                 SAVE adds the object scores rootD_S (HOM by default, the root cut to 6 characters, D the\n\
                 dimension, S the first free set number) for the first n dimensions (all by default). MATRIX=OUT\n\
                 writes ROWTYPE_ (QUANT), LEVEL, VARNAME_ and DIM1 to DIMn, one case per category; an asterisk\n\
                 replaces the active dataset. SPLIT FILE does not apply.",
        example: "HOMALS VARIABLES=brand(3) shop(4) payment(2) /PRINT=FREQ DISCRIM QUANT /PLOT=QUANT(5) /SAVE=hs(2).",
        parse: legacy_scaling::homals,
    },
    CommandSpec {
        name: "PRINCALS",
        summary: "Nonlinear principal components in the older syntax: CATPCA with variables read as categories 1 to a stated largest value.",
        syntax: "PRINCALS [/VARIABLES=] varlist(max) [varlist(max) ...]\n\
                 \x20  [/ANALYSIS=varlist [(ORDI | SNOM | MNOM | NUME)] [varlist [(...)] ...]]\n\
                 \x20  [/NOBSERVATIONS=n] [/DIMENSION=2 | n] [/MAXITER=100 | n] [/CONVERGENCE=.00001 | value]\n\
                 \x20  [/PRINT=[FREQ] [EIGEN] [LOADINGS] [QUANT] [HISTORY] [CORRELATION] [OBJECT] [DEFAULT] [ALL] [NONE]]\n\
                 \x20  [/PLOT=[NDIM(first | ALL, last | MAX)] [QUANT[(varlist)][(len)]] [OBJECT[(varlist)][(len)]]\n\
                 \x20        [LOADINGS[(len)]] [DEFAULT[(len)]] [ALL[(len)]] [NONE]]\n\
                 \x20  [/SAVE=[root][(n)]] [/MATRIX=OUT(* | 'file' | dataset)]\n\
                 Variables are read as for HOMALS: whole values 1 to the stated largest are the categories, all\n\
                 else is missing. The level after a list on ANALYSIS covers the variables since the previous\n\
                 parenthesis: ORDI (the default) keeps the order of the categories, SNOM any values, NUME equal\n\
                 steps, MNOM a free point per category and dimension. The fit is that of CATPCA with these\n\
                 levels and variable principal normalization; messages name PRINCALS. Tables: Case Processing\n\
                 Summary, Iteration History and Model Summary always (the eigenvalue there is the sum over the\n\
                 variables; divided by their number it is the mean that older output showed); FREQ the\n\
                 frequencies; LOADINGS the component loadings; QUANT the quantifications with centroid and\n\
                 vector coordinates; CORRELATION the correlations of the quantified variables; OBJECT the\n\
                 object scores; HISTORY every iteration. Without PRINT: FREQ EIGEN LOADINGS; DEFAULT adds QUANT;\n\
                 ALL gives every one. Charts (QUANT and OBJECT by default, none for one dimension): QUANT the\n\
                 categories of each variable, OBJECT the objects labelled by each named variable, LOADINGS the\n\
                 loadings; label lengths and NDIM as for HOMALS. SAVE adds rootD_S (PRI by default). MATRIX=OUT\n\
                 writes ROWTYPE_ (QUANT, SCOOR_ for single and MCOOR_ for multiple coordinates, LOADING_),\n\
                 LEVEL, VARNAME_, VARTYPE_ (MULTIPLE, SINGLE N, ORDINAL or NUMERICAL) and DIM1 to DIMn; the\n\
                 quantification of a single variable is in DIM1, the other columns 0. SPLIT FILE does not apply.",
        example: "PRINCALS VARIABLES=taste smell(5) price(4) /ANALYSIS=taste smell(SNOM) price(NUME) /PRINT=QUANT LOADINGS /PLOT=LOADINGS.",
        parse: legacy_scaling::princals,
    },
    CommandSpec {
        name: "OVERALS",
        summary: "Nonlinear canonical correlation analysis: what two or more sets of variables, each quantified at its own level, have in common.",
        syntax: "OVERALS [/VARIABLES=] varlist(max) [varlist(max) ...]\n\
                 \x20  /ANALYSIS=varlist [(ORDI | SNOM | MNOM | NUME)] [varlist [(...)] ...]\n\
                 \x20  /SETS=n(count, count, ...)\n\
                 \x20  [/NOBSERVATIONS=n] [/DIMENSION=2 | n] [/INITIAL=NUMERICAL | RANDOM]\n\
                 \x20  [/MAXITER=100 | n] [/CONVERGENCE=.00001 | value]\n\
                 \x20  [/PRINT=[FREQ] [QUANT] [CENTROID] [HISTORY] [WEIGHTS] [OBJECT] [FIT] [DEFAULT] [NONE]]\n\
                 \x20  [/PLOT=[NDIM(first | ALL, last | MAX)] [OBJECT[(varlist)][(len)]] [QUANT[(varlist)][(len)]]\n\
                 \x20        [LOADINGS[(len)]] [TRANS[(varlist)]] [CENTROID[(varlist)][(len)]] [DEFAULT[(len)]] [NONE]]\n\
                 \x20  [/SAVE=[root][(n)]] [/MATRIX=OUT(* | 'file' | dataset)]\n\
                 Variables are read as for HOMALS and quantified at the level ANALYSIS gives (ORDI by default):\n\
                 MNOM a free point per category and dimension; SNOM, ORDI and NUME one value per category (free,\n\
                 in the order of the codes, or in equal steps) times a weight per dimension. SETS splits the\n\
                 ANALYSIS variables, in order, into at least two sets. A case missing on any variable of a set\n\
                 leaves that set out; a case in no set takes no part; each set needs three cases. The object\n\
                 scores and quantifications make the sum of each set's quantified variables as close as possible\n\
                 to the object scores, summed over the sets; scores are centered and uncorrelated with sum of\n\
                 squares K times n in the metric of the number of sets each case is in. WEIGHT cases count as\n\
                 replications. NUMERICAL (the default) first fits every single variable as numerical and then\n\
                 continues at the stated levels; RANDOM starts from random scores of the session's generator. The\n\
                 fit stops after MAXITER iterations or when it gains less than CONVERGENCE, and is turned to its\n\
                 principal axes; in each dimension the case with the largest absolute score is positive.\n\
                 DIMENSION is lowered with a warning to what the sets allow. One variable per set gives the\n\
                 solution of HOMALS (all MNOM) or PRINCALS (all single); two sets of numerical variables give\n\
                 eigenvalues (1 + r)/2 for the canonical correlations r.\n\
                 Tables: Variables by Set and Summary of Analysis (loss of each set per dimension over n, the\n\
                 mean, and the eigenvalue 1 minus the mean; the fit is their sum) always; FREQ the frequencies\n\
                 with the cases missing from the set; HISTORY the fit per iteration; WEIGHTS the weights and\n\
                 component loadings of the single variables; FIT multiple fit, single fit (squared weights) and\n\
                 single loss; CENTROID per variable the quantification, projected centroids and centroids;\n\
                 QUANT the quantification, single and multiple coordinates; OBJECT the object scores. Without\n\
                 PRINT, or with DEFAULT: FREQ CENTROID WEIGHTS; NONE leaves the summary. Charts (OBJECT and\n\
                 LOADINGS by default): object scores, labelled by each named variable; loadings; TRANS the\n\
                 quantification against the category (one chart per dimension for MNOM); QUANT the category\n\
                 coordinates; CENTROID the centroids. Label lengths and NDIM as for HOMALS. SAVE adds the object\n\
                 scores rootD_S (OVE by default). MATRIX=OUT writes ROWTYPE_ (QUANT, SCOOR_, MCOOR_, CENTRO_,\n\
                 PCENTRO_, WEIGHT_, LOADING_), LEVEL, VARNAME_, VARTYPE_, SET_ and DIM1 to DIMn; values that do\n\
                 not apply are 0. SPLIT FILE does not apply.",
        example: "OVERALS VARIABLES=habit1 habit2(4) score1 score2(10) /ANALYSIS=habit1 habit2(SNOM) score1 score2(NUME) /SETS=2(2,2) /PRINT=FIT QUANT /PLOT=OBJECT CENTROID(habit1).",
        parse: legacy_scaling::overals,
    },
    CommandSpec {
        name: "ANACOR",
        summary: "Correspondence analysis in the older syntax: CORRESPONDENCE with its own normalization names and defaults.",
        syntax: "ANACOR TABLE=rowvar(min,max) BY colvar(min,max) | ALL(rows,columns)\n\
                 \x20  [/DIMENSION=2 | n]\n\
                 \x20  [/NORMALIZATION=CANONICAL | PRINCIPAL | RPRINCIPAL | CPRINCIPAL | q]\n\
                 \x20  [/VARIANCES=[SINGULAR] [ROWS] [COLUMNS]]\n\
                 \x20  [/PRINT=[TABLE] [PROFILES] [SCORES] [CONTRIBUTIONS] [PERMUTATION] [DEFAULT] [NONE]]\n\
                 \x20  [/PLOT=[NDIM(first | ALL, last | MAX)] [ROWS[(len)]] [COLUMNS[(len)]] [JOINT[(len)]]\n\
                 \x20        [TRROWS] [TRCOLUMNS] [DEFAULT[(len)]] [NONE]]\n\
                 \x20  [/MATRIX OUT=[SCORE(* | 'file' | dataset)] [VARIANCE(* | 'file' | dataset)]]\n\
                 The table is read and analyzed as CORRESPONDENCE reads it with MEASURE=CHISQ; messages name\n\
                 ANACOR. NORMALIZATION q gives the row scores the power (1 + q)/2 of each singular value and the\n\
                 column scores (1 - q)/2, q from -2 to 2: CANONICAL (or DEFAULT, the default) is 0, RPRINCIPAL 1,\n\
                 CPRINCIPAL -1; PRINCIPAL gives both the full value. The Summary always shows the singular values\n\
                 with their spread and correlations, so VARIANCES SINGULAR changes nothing; ROWS and COLUMNS add\n\
                 the spread and correlations of the row and column scores. PRINT: TABLE the table with its\n\
                 margins, PROFILES the row and column profiles, SCORES and CONTRIBUTIONS the overview of row and\n\
                 column points (mass, scores, inertia and both kinds of contributions), PERMUTATION the table\n\
                 sorted by the scores of each dimension; DEFAULT (and no PRINT) TABLE SCORES CONTRIBUTIONS.\n\
                 PLOT: ROWS and COLUMNS (the default) each side alone, JOINT both together (not with\n\
                 PRINCIPAL), TRROWS and TRCOLUMNS the scores per category; labels are cut to 3 characters unless\n\
                 a length is given (20 for the transformation plots). NDIM(a, b) draws dimension a against a+1\n\
                 to b, ALL every pair, MAX the last dimension; without it dimension 1 against 2. MATRIX OUT\n\
                 writes the scores and the variances as OUTFILE of CORRESPONDENCE does; an asterisk replaces\n\
                 the active dataset.",
        example: "ANACOR TABLE=region(1,4) BY channel(1,3) /NORMALIZATION=RPRINCIPAL /PRINT=SCORES PROFILES /PLOT=JOINT(6).",
        parse: legacy_scaling::anacor,
    },
    CommandSpec {
        name: "GENLIN",
        summary: "Generalized linear models: normal, binomial, Poisson, gamma, inverse Gaussian, negative binomial and ordinal responses.",
        syntax: "GENLIN dependent [(REFERENCE=LAST | FIRST | value)] [(ORDER=ASCENDING | DESCENDING | DATA)]\n\
                 \x20  | events OF trials-variable | n\n\
                 \x20  [BY factors [(ORDER=ASCENDING | DESCENDING | DATA)]] [WITH covariates]\n\
                 \x20  [/MODEL [effects] [INTERCEPT=YES | NO] [OFFSET=variable | value] [SCALEWEIGHT=variable]\n\
                 \x20          [DISTRIBUTION=NORMAL | BINOMIAL | GAMMA | IGAUSS | POISSON | NEGBIN[(k | MLE)] | MULTINOMIAL]\n\
                 \x20          [LINK=IDENTITY | LOG | LOGIT | PROBIT | CLOGLOG | NLOGLOG | LOGC | NEGBIN | POWER(a) |\n\
                 \x20                ODDSPOWER(a) | CUMLOGIT | CUMPROBIT | CUMCLOGLOG | CUMNLOGLOG | CUMCAUCHIT]]\n\
                 \x20  [/CRITERIA [ANALYSISTYPE=3 | 1 | ALL [(WALD | LR)]] [CHECKSEP=20] [CILEVEL=95] [CITYPE=WALD]\n\
                 \x20             [COVB=MODEL | ROBUST] [HCONVERGE=0] [LCONVERGE=0] [PCONVERGE=1E-6] (each [(ABSOLUTE | RELATIVE)])\n\
                 \x20             [LIKELIHOOD=FULL | KERNEL] [MAXITERATIONS=100] [MAXSTEPHALVING=5]\n\
                 \x20             [METHOD=FISHER[(n)] | NEWTON] [SCALE=MLE | DEVIANCE | PEARSON | value] [SINGULAR=1E-12]]\n\
                 \x20  [/EMMEANS [TABLES=factor[*factor...]] [CONTROL=covariate(value | MEAN) ...]\n\
                 \x20            [SCALE=ORIGINAL | TRANSFORMED] [COMPARE=factor[*factor...]]\n\
                 \x20            [CONTRAST=PAIRWISE | DEVIATION[(value)] | DIFFERENCE | HELMERT | POLYNOMIAL[(metric)] |\n\
                 \x20                      REPEATED | SIMPLE[(value)]]\n\
                 \x20            [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]] ...\n\
                 \x20  [/REPEATED SUBJECT=variable[*variable...] [WITHINSUBJECT=variable[*variable...]] [SORT=YES | NO]\n\
                 \x20            [CORRTYPE=INDEPENDENT | AR(1) | EXCHANGEABLE | MDEPENDENT(m) | UNSTRUCTURED | FIXED(list)]\n\
                 \x20            [ADJUSTCORR=YES | NO] [COVB=ROBUST | MODEL] [HCONVERGE=0] [MAXITERATIONS=100]\n\
                 \x20            [PCONVERGE=1E-6[(ABSOLUTE | RELATIVE)]] [UPDATECORR=1]]\n\
                 \x20  [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [CPS] [DESCRIPTIVES] [MODELINFO] [FIT] [SUMMARY] [SOLUTION[(EXPONENTIATED)]] [COVB] [CORB]\n\
                 \x20          [WORKINGCORR] | NONE]\n\
                 \x20  [/SAVE [XBPRED] [XBSTDERROR] [MEANPRED] [CIMEANPREDL] [CIMEANPREDU] [PREDVAL] [LEVERAGE] [RESID]\n\
                 \x20         [PEARSONRESID] [DEVIANCERESID] [STDPEARSONRESID] [STDDEVIANCERESID] [LIKELIHOODRESID] [COOK]].\n\
                 Every keyword is written in full and every subcommand but EMMEANS appears at most once. The\n\
                 response is one dependent variable or a count of events out of a number of trials (a variable or\n\
                 a constant); covariates, offset and scale weight are numeric, factors and a categorical dependent\n\
                 may be strings. Without DISTRIBUTION, events/trials is binomial and anything else normal; without\n\
                 LINK each distribution takes its own (identity, logit, inverse, inverse square, log, cumulative\n\
                 logit), and pairs the model cannot use are an error. A case is left out when its rounded weight is\n\
                 below 1, a variable is missing (CLASSMISSING=INCLUDE keeps user-missing factor values), the scale\n\
                 weight is not positive, or the response is out of range: not above 0 for GAMMA and IGAUSS, not a\n\
                 whole number of 0 or more for POISSON and NEGBIN, events above the trials. A binary dependent must\n\
                 have two values; the procedure models the lower one unless REFERENCE names another reference.\n\
                 Effects are written as in UNIANOVA; a factor has one column per level and the column of the last\n\
                 level (in the ORDER given) is redundant and shown as 0. The estimates maximize the log likelihood\n\
                 by Newton steps from weighted least squares start values: Fisher scoring for the first n\n\
                 iterations of METHOD=FISHER(n) (1 by default), the observed information after; a step is halved\n\
                 while the log likelihood falls. The scale is estimated with the coefficients (MLE, the default for\n\
                 NORMAL, GAMMA and IGAUSS), set from the deviance or Pearson chi-square over its degrees of freedom,\n\
                 or fixed (1 for the other distributions). NEGBIN(MLE) estimates k jointly; NEGBIN alone fixes k at\n\
                 1. ROBUST gives the sandwich covariance.\n\
                 Tables: Model Information, Case Processing Summary, Categorical and Continuous Variable Information,\n\
                 Goodness of Fit (deviance and Pearson chi-square, raw and scaled, over the subpopulations of a\n\
                 binary response; the log likelihood, full or KERNEL; AIC, AICC, BIC and CAIC), Omnibus Test (a\n\
                 likelihood ratio test against the intercept-only model), Tests of Model Effects (Type I and III,\n\
                 Wald or likelihood ratio; with SCALE=DEVIANCE or PEARSON the likelihood ratio is divided by the\n\
                 scale and an F test added) and Parameter Estimates with Wald intervals and Exp(B), then the\n\
                 covariance and correlation of the estimates. The ordinal multinomial model puts the link of the\n\
                 cumulative probability at each threshold minus the effects, without an intercept. EMMEANS gives\n\
                 the predicted means of the factor cells (covariates at their means or CONTROL values), on the\n\
                 response or the linear predictor scale, with pairwise or contrast comparisons and an overall\n\
                 Wald test. SAVE names the new variables after the keyword (XBPredicted, MeanPredicted, Residual\n\
                 and so on, a suffix _1 when taken) or as given; the multinomial ones are numbered per category.\n\
                 REPEATED fits generalized estimating equations for measurements that are correlated within a\n\
                 subject. A subject is a combination of the SUBJECT values; WITHINSUBJECT values put its cases in\n\
                 order, and a value a subject lacks counts as a missing measurement. Without WITHINSUBJECT the file\n\
                 order is kept. SORT=NO takes each run of equal SUBJECT values in the file as one subject and\n\
                 ignores WITHINSUBJECT. The fit starts from the plain model and then updates the estimates and the\n\
                 working correlation matrix in turn: AR(1), EXCHANGEABLE, MDEPENDENT(m) and UNSTRUCTURED are\n\
                 estimated from the Pearson residuals (with ADJUSTCORR=YES each count in a denominator is less the\n\
                 number of parameters), FIXED takes the correlations below the diagonal row by row. UPDATECORR=n\n\
                 re-estimates the matrix every n iterations, 0 never. SCALE=MLE becomes the Pearson moment of the\n\
                 residuals; another scale and an estimated NEGBIN k stay at the plain model's value. COVB=ROBUST\n\
                 (the default) gives sandwich standard errors, MODEL the model-based ones; the CRITERIA COVB is not\n\
                 used. FIT then shows QIC and QICC instead of the likelihood statistics, SUMMARY only the Wald\n\
                 tests of the effects, CPS adds the Correlated Data Summary, WORKINGCORR the Working Correlation\n\
                 Matrix. A frequency weight must be the same for all cases of a subject. SAVE has no leverage,\n\
                 deviance-based or standardized residuals or Cook's distance here.\n\
                 Not supported yet: OUTFILE, TWEEDIE, INITIAL, CITYPE=PROFILE, PRINT GEF, HISTORY, LAGRANGE and\n\
                 LMATRIX, and with REPEATED the multinomial distribution and generalized score tests\n\
                 (ANALYSISTYPE with LR).",
        example: "GENLIN visits BY clinic WITH age /MODEL clinic age DISTRIBUTION=NEGBIN(MLE) LINK=LOG OFFSET=logdays /PRINT FIT SUMMARY SOLUTION(EXPONENTIATED) /EMMEANS TABLES=clinic COMPARE=clinic.",
        parse: genlin::genlin,
    },
    CommandSpec {
        name: "UNIANOVA",
        summary: "General linear model of one dependent variable: factorial ANOVA, ANCOVA and regression.",
        syntax: glm_syntax!("UNIANOVA"),
        example: "UNIANOVA score BY method school WITH pretest /PRINT=DESCRIPTIVE ETASQ PARAMETER /EMMEANS=TABLES(method) COMPARE ADJ(BONFERRONI) /DESIGN=pretest method school method*school.",
        parse: glm::unianova,
    },
    CommandSpec {
        name: "GLM",
        summary: "General linear model: one dependent variable as in UNIANOVA, several (multivariate analysis of variance and covariance), or repeated measures with WSFACTOR.",
        syntax: glm_syntax!("GLM"),
        example: "GLM t1 t2 t3 BY group /WSFACTOR=time 3 POLYNOMIAL /WSDESIGN=time /DESIGN=group.",
        parse: glm::glm,
    },
    CommandSpec {
        name: "MIXED",
        summary: "Linear mixed model: fixed effects with random effects of subjects and correlated repeated measures.",
        syntax: "MIXED dependent [BY factors] [WITH covariates]\n\
                 \x20  [/FIXED=[effects] [| [NOINT] [SSTYPE(3 | 1)]]] [/METHOD=REML | ML] [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/RANDOM=effects [| [SUBJECT(var[*var...])] [COVTYPE(type)] [SOLUTION]]] ...\n\
                 \x20  [/REPEATED=var[*var...] | SUBJECT(var[*var...]) [COVTYPE(type)]] [/REGWGT=var]\n\
                 \x20  [/CRITERIA=[CIN(95)] [MXITER(100)] [MXSTEP(10)] [SCORING(1)] [SINGULAR(1E-12)]\n\
                 \x20            [PCONVERGE(1E-6[, ABSOLUTE | RELATIVE])] [LCONVERGE(0[, ...])] [HCONVERGE(0[, ...])]\n\
                 \x20            [DFMETHOD(SATTERTHWAITE | RESIDUAL | KENWARDROGER)]]\n\
                 \x20  [/PRINT=[SOLUTION] [TESTCOV] [G] [R] [COVB] [CORB] [CPS] [DESCRIPTIVES] [HISTORY[(n)]] [LMATRIX]]\n\
                 \x20  [/SAVE=tempvar[(name)] ...]\n\
                 \x20  [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]\n\
                 \x20            [COMPARE[(factor)] [REFCAT(value | FIRST | LAST)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...\n\
                 \x20  [/TEST[(constants)]=['label'] effect values ... [| effect values ...] [DIVISOR=n]; ...\n\
                 \x20         | ALL values ... [| values ...] ...] ...\n\
                 effects: INTERCEPT (first), a factor or covariate, a*b or a BY b, a(b) or a WITHIN b.\n\
                 type: VC (the RANDOM default: one variance per effect), ID (one variance for the block), DIAG (the\n\
                 REPEATED default: one variance per level), UN (every variance and covariance), CS (a diagonal\n\
                 offset plus one common covariance), CSR (one variance and one correlation), CSH (a variance per\n\
                 level and one correlation), AR1 (a variance and a correlation that is raised to the power of the\n\
                 distance between levels), ARH1 (the same with a variance per level), ARMA11 (a variance, a\n\
                 correlation gamma between neighbours and a factor rho for each step further), TP and TPH (a\n\
                 correlation for each distance, with one variance or a variance per level), AD1 (a variance per\n\
                 level; two levels correlate by the product of the neighbour correlations between them), UNR (a\n\
                 variance per level and a correlation per pair), HF (a variance per level; two levels have their\n\
                 mean variance less lambda as covariance), FA1 and FAH1 (the loadings of one factor plus a\n\
                 diagonal offset, common or per level). Variances that scale correlations stay above 0, other\n\
                 variances and offsets at least 0, correlations between -1 and 1. VC on REPEATED is DIAG.\n\
                 tempvar: FIXPRED (X b), PRED (X b plus the predicted random effects), RESID (dependent minus\n\
                 PRED), SEFIXP and DFFIXP (the standard error and degrees of freedom of FIXPRED), SEPRED and\n\
                 DFPRED (the same for PRED, counting the prediction error of the random effects).\n\
                 The fixed part is the intercept unless NOINT, plus the FIXED effects. Each RANDOM adds one\n\
                 covariance block per subject, the same for every subject; subjects are independent, and so are\n\
                 the RANDOM subcommands. Without SUBJECT the whole file is one subject. REPEATED orders the\n\
                 levels of its variables and gives each subject a residual block of that structure; without it\n\
                 the residuals have one variance. Each subject variable of RANDOM must be a subject variable of\n\
                 REPEATED. A case missing on any variable named is left out; the weight is rounded and counts\n\
                 the case that many times (with REPEATED only a weight of 1). REGWGT divides the residual\n\
                 covariance of a case by its value. The covariance parameters maximize the restricted (REML) or\n\
                 full (ML) likelihood by Newton steps, the first SCORING ones with the expected Hessian, halving\n\
                 a step up to MXSTEP times when it fails to improve; the fit stops when the change of every\n\
                 parameter is below PCONVERGE, or by LCONVERGE or HCONVERGE when set. Tables: Model Dimension,\n\
                 Information Criteria (-2 log likelihood, AIC, AICC, CAIC, BIC; for REML n is the cases less the\n\
                 fixed parameters and only covariance parameters count), Type III Tests of Fixed Effects (Type I\n\
                 with SSTYPE(1): each effect after the effects listed before it), Estimates of Fixed Effects\n\
                 (SOLUTION) and Estimates of Covariance Parameters (with TESTCOV the standard errors from the\n\
                 observed Hessian, Wald Z and intervals on the log scale for variances and the Fisher z scale for\n\
                 correlations). Degrees of freedom are Satterthwaite's, or the cases less the rank of the fixed\n\
                 design with DFMETHOD(RESIDUAL). DFMETHOD(KENWARDROGER) gives every standard error and test of\n\
                 fixed effects the Kenward-Roger adjusted covariance, and each F its scale and denominator df\n\
                 from that method, built on the expected information of the covariance parameters. EMMEANS\n\
                 averages the fitted cells equally over the factors left out, covariates at their means or the\n\
                 WITH values; each mean and each difference gets its own degrees of freedom. COMPARE lists every\n\
                 pair of levels, or each level against REFCAT, with Bonferroni or Sidak adjustment over that\n\
                 number of comparisons, and an F test of the compared factor at each level of the others. TEST\n\
                 takes coefficients for the parameters of named fixed effects (redundant ones included) or ALL\n\
                 of them, one row per combination; the constants in parentheses are the values tested against\n\
                 (0 by default) and DIVISOR divides a row. ALL and effect names never mix in one TEST: the\n\
                 first of them decides whether the rows use ALL or name their effects. After | come\n\
                 coefficients for one subject's random effects, by effect or as bare values for all of them\n\
                 over the RANDOM subcommands in order; the bare values fit either form, even as a whole row,\n\
                 and in a TEST that uses ALL, ALL may come before them. An effect on two RANDOM subcommands\n\
                 needs the bare values. With SUBJECT they are shared equally among the subjects, so the row\n\
                 speaks of their average. TEST gives each row's estimate with a t test and the joint F test; a\n\
                 row whose fixed part is not estimable is dropped with a warning. SOLUTION on RANDOM lists the\n\
                 predicted random effects of every subject with the\n\
                 standard error of their prediction error, df, a t test and an interval. Rows with random\n\
                 effects, and the other rows of their TEST, keep the unadjusted covariance and Satterthwaite's df\n\
                 (from numerical derivatives) even with KENWARDROGER. PRINT G and R show the covariance of one\n\
                 subject's random effects and repeated levels, COVB and CORB the covariance and correlation of\n\
                 the fixed estimates (adjusted with KENWARDROGER). CPS counts the cases at each value of the\n\
                 factors and of the repeated and subject variables; DESCRIPTIVES gives the count, mean and\n\
                 standard deviation of the dependent and covariates in each cell of the factors; HISTORY(n) lists\n\
                 the start and every n-th iteration (update type, step halvings, -2 log likelihood and every\n\
                 parameter) and always the last;\n\
                 LMATRIX shows the coefficients of each test of a fixed effect and of each TEST. Not supported\n\
                 yet: the spatial and Kronecker types of REPEATED, KRONECKER and SPCOORDS.",
        example: "MIXED yield BY plot_type WITH rain /FIXED=plot_type rain /RANDOM=INTERCEPT rain | SUBJECT(farm) COVTYPE(UN) /PRINT=SOLUTION TESTCOV.",
        parse: mixed::mixed,
    },
    CommandSpec {
        name: "VARCOMP",
        summary: "Variance components: how much each random effect of a linear model adds to the variance of the dependent variable.",
        syntax: "VARCOMP dependent BY factors [WITH covariates] /RANDOM=factor [factor ...] ...\n\
                 \x20  [/METHOD=MINQUE[(1 | 0)] | ML | REML | SSTYPE[(3 | 1)]] [/INTERCEPT=INCLUDE | EXCLUDE]\n\
                 \x20  [/MISSING=EXCLUDE | INCLUDE] [/REGWGT=var]\n\
                 \x20  [/CRITERIA=[CONVERGE(1E-8)] [EPS(1E-8)] [ITERATE(50)]]\n\
                 \x20  [/PRINT=[EMS] [SS] [HISTORY[(1)]]]\n\
                 \x20  [/OUTFILE=[VAREST] [COVB | CORB] ('file' | dataset)]\n\
                 \x20  [/DESIGN=[INTERCEPT] [effect ...]]\n\
                 effects: a factor or covariate, a*b or a BY b, a(b) or a WITHIN b.\n\
                 Every effect that holds a factor named on RANDOM is random, with its own variance; the\n\
                 other effects are fixed. RANDOM may repeat and its factors add up. Without DESIGN the model\n\
                 has the intercept, the covariates and every main effect and interaction of the factors.\n\
                 The design is coded as in UNIANOVA: one indicator column per level combination that occurs.\n\
                 METHOD picks the estimator. MINQUE (the default) solves the minimum norm quadratic unbiased\n\
                 equations with prior weight 1 for every random effect and the residual, or with 0 for the\n\
                 random effects and 1 for the residual. ML and REML maximize the full or the restricted\n\
                 likelihood, starting from the variance of each effect's least squares estimates and the\n\
                 residual variance of the full model; Fisher scoring takes the first step and Newton steps\n\
                 follow while the Hessian allows, a step is halved up to 10 times until the likelihood rises,\n\
                 a variance that would turn negative is set to 0, and the fit stops when both the\n\
                 likelihood and the parameters change by less than CONVERGE (relative) or after ITERATE\n\
                 iterations. SSTYPE equates the mean square of each random effect and of the error, by Type\n\
                 I or III sums of squares, with its expected value and solves for the variances; these may\n\
                 come out negative. A case missing on any variable is left out; INCLUDE keeps user-missing\n\
                 values. The case weight is rounded and counts the case that many times; REGWGT weights the\n\
                 residual of each case (cases with a weight of 0 or less are left out). EPS is the tolerance\n\
                 for redundant design columns. Tables: Factor Level Information, Variance Estimates, with ML\n\
                 and REML the Asymptotic Covariance Matrix of the estimates (the inverse of the expected\n\
                 information); PRINT SS shows the sums of squares, df and mean squares and PRINT EMS the\n\
                 expected mean squares (SSTYPE only; the fixed effects that enter appear as a quadratic\n\
                 term); HISTORY(n) lists the log likelihood and the estimates at every n-th iteration and\n\
                 the last (ML and REML only). OUTFILE writes one row per component with its label\n\
                 (VarName_), its estimate (Estimate, with VAREST) and its row of the covariance or correlation\n\
                 matrix (COVB or CORB, ML and REML only) in columns VC1, VC2, ...",
        example: "VARCOMP strength BY mixture batch operator /RANDOM=batch operator /METHOD=REML /PRINT=HISTORY.",
        parse: varcomp::varcomp,
    },
    CommandSpec {
        name: "GENLINMIXED",
        summary: "Generalized linear mixed model: a target with a normal, binomial, multinomial, gamma, inverse Gaussian, Poisson or negative binomial distribution and a link, fixed effects, random effects of subjects and repeated measures.",
        syntax: "GENLINMIXED [/DATA_STRUCTURE SUBJECTS=field[*field...] [REPEATED_MEASURES=field[*field...]]\n\
                 \x20         [COVARIANCE_TYPE=DIAGONAL | type]]\n\
                 \x20  /FIELDS TARGET=field [TRIALS=NONE | VALUE(n) | FIELD(field)] [ANALYSIS_WEIGHT=field]\n\
                 \x20         [OFFSET=NONE | VALUE(x) | FIELD(field)]\n\
                 \x20  [/TARGET_OPTIONS [REFERENCE=value]\n\
                 \x20         [DISTRIBUTION=NORMAL | BINOMIAL | MULTINOMIAL | GAMMA | INVERSE_GAUSSIAN | POISSON |\n\
                 \x20         NEGATIVE_BINOMIAL] [LINK=IDENTITY | CAUCHIT | CLOGLOG | LOG | LOGC | LOGIT | NLOGLOG |\n\
                 \x20         POWER | PROBIT] [LINK_PARAMETER=x]]\n\
                 \x20  [/FIXED [EFFECTS=effects] [USE_INTERCEPT=TRUE | FALSE]]\n\
                 \x20  [/RANDOM [EFFECTS=effects] [USE_INTERCEPT=FALSE | TRUE] [SUBJECTS=field[*field...]]\n\
                 \x20          [COVARIANCE_TYPE=type] [SOLUTION=FALSE | TRUE]] ...\n\
                 \x20  [/BUILD_OPTIONS [TARGET_CATEGORY_ORDER=ASCENDING | DESCENDING | DATA] [MAX_ITERATIONS=100]\n\
                 \x20         [CONFIDENCE_LEVEL=95] [DF_METHOD=RESIDUAL | SATTERTHWAITE | KENWARD_ROGER]\n\
                 \x20         [COVB=MODEL | ROBUST] [PCONVERGE=1E-6[(ABSOLUTE | RELATIVE)]] [LCONVERGE=0[(...)]]\n\
                 \x20         [HCONVERGE=0[(...)]] [SCORING=0] [SINGULAR=1E-12]]\n\
                 \x20  [/EMMEANS [TABLES=factor[*factor...]] [COMPARE=factor]\n\
                 \x20         [CONTRAST=NONE | PAIRWISE | DEVIATION | SIMPLE]] ...\n\
                 \x20  [/EMMEANS_OPTIONS [CONTROL=covariate(x) ...] [SCALE=ORIGINAL | TRANSFORMED]\n\
                 \x20         [PADJUST=LSD | SEQBONFERRONI | SEQSIDAK]]\n\
                 \x20  [/OUTFILE MODEL='file']\n\
                 \x20  [/SAVE [PREDICTED_VALUES[(name)]] [PREDICTED_PROBABILITY[(root)]] [MAX_CATEGORIES(25)]\n\
                 \x20         [CONFIDENCE[(name)]] [CONFIDENCE_METHOD(PREDICTEDPROBABILITY | PROBABILITYIMPROVEMENT)]\n\
                 \x20         [CONFIDENCE_INTERVALS[(root)]] [PEARSON_RESIDUALS[(name)]]]\n\
                 effects: a field, a*b or a BY b, a(b); a field of scale level is a covariate, any other a factor.\n\
                 type: VARIANCE_COMPONENTS (the default on RANDOM: one variance per effect; on DATA_STRUCTURE the\n\
                 same as DIAGONAL), IDENTITY, DIAGONAL, UNSTRUCTURED, COMPOUND_SYMMETRY, AR1, ARH1, ARMA11, CSH or\n\
                 TOEPLITZ, as the types of MIXED.\n\
                 Without DISTRIBUTION a scale target is NORMAL, and one with TRIALS or a categorical one of two\n\
                 values BINOMIAL; the link defaults to IDENTITY, LOGIT, POWER(-1) for GAMMA, POWER(-2) for\n\
                 INVERSE_GAUSSIAN and LOG for POISSON and NEGATIVE_BINOMIAL. BINOMIAL takes either events with\n\
                 TRIALS or a target of two values; then the last value in TARGET_CATEGORY_ORDER (or REFERENCE) is\n\
                 the reference and the model is for the other one.\n\
                 Cases whose target does not fit the distribution, or that miss a field, the offset or a weight\n\
                 above 0, are left out. The fit linearizes the model around the current means and fits the\n\
                 linear mixed model of the working target with the weights that follow from the variance\n\
                 function, by restricted likelihood as MIXED does, then updates the means with the predicted\n\
                 random effects, until no covariance parameter changes by more than PCONVERGE relative to its\n\
                 size. REPEATED_MEASURES orders the cases of each subject by these fields and lets their residuals\n\
                 be correlated as COVARIANCE_TYPE says; a model needs RANDOM or REPEATED_MEASURES. The scale is 1\n\
                 for BINOMIAL, POISSON and NEGATIVE_BINOMIAL without repeated measures and estimated otherwise;\n\
                 NORMAL with IDENTITY is MIXED itself. NEGATIVE_BINOMIAL takes its k from the model without\n\
                 random effects and keeps it. Each RANDOM must name subjects from DATA_STRUCTURE. MAX_ITERATIONS\n\
                 bounds both loops. COVB=ROBUST tests with the sandwich covariance over the subjects.\n\
                 Tables: Model Summary, Model Dimension, Information Criteria (of the working model), Type III\n\
                 Tests of Fixed Effects, Estimates of Fixed Effects, Estimates of Covariance Parameters with\n\
                 standard errors and Wald intervals, and the predicted random effects of each RANDOM with\n\
                 SOLUTION=TRUE. Degrees of freedom are the cases less the rank of the fixed design unless\n\
                 DF_METHOD says otherwise. EMMEANS gives the means of a fixed effect of factors (the overall\n\
                 mean without TABLES) with covariates at their mean or CONTROL value, on the scale of the target\n\
                 or of the link, and with COMPARE the contrasts among the levels of one factor, adjusted by\n\
                 PADJUST, and their joint F test. SAVE adds the predicted mean (the events for events and trials,\n\
                 the category for a target of two values), its interval and the Pearson residual; probabilities\n\
                 and confidence need a categorical target. OUTFILE writes the fixed effects as a JSON model file.\n\
                 A categorical target of three or more values is MULTINOMIAL. An ordinal one gets a threshold per\n\
                 category but the last in place of the intercept and a cumulative link (LOGIT, PROBIT, CLOGLOG,\n\
                 NLOGLOG or CAUCHIT); a nominal one is compared with its REFERENCE category (the last by default)\n\
                 by the logit, with its own parameters and random effects for each other category. Neither takes\n\
                 REPEATED_MEASURES; EMMEANS is ignored with a warning. SAVE gives the predicted category, the\n\
                 probabilities (cumulative for an ordinal target), the confidence and the intervals of the\n\
                 probabilities. Not supported yet: SAVE PEARSON_RESIDUALS of a multinomial target, a model with\n\
                 neither RANDOM nor REPEATED_MEASURES, the spatial and Kronecker covariance types,\n\
                 KRONECKER_MEASURES, SPCOORDS, GROUPING and INPUTS_CATEGORY_ORDER other than ASCENDING.",
        example: "GENLINMIXED /DATA_STRUCTURE SUBJECTS=clinic /FIELDS TARGET=cured /TARGET_OPTIONS DISTRIBUTION=BINOMIAL LINK=LOGIT /FIXED EFFECTS=dose /RANDOM USE_INTERCEPT=TRUE SUBJECTS=clinic.",
        parse: genlinmixed::genlinmixed,
    },
    CommandSpec {
        name: "ANOVA",
        summary: "Analysis of variance for factorial designs with covariates, the older form with MCA tables.",
        syntax: "ANOVA [VARIABLES=] dependents BY factors(min,max) [factors(min,max) ...] [WITH covariates] [/next list ...]\n\
                 \x20     [/COVARIATES=FIRST | WITH | AFTER] [/MAXORDERS=ALL | n | NONE]\n\
                 \x20     [/METHOD=UNIQUE | EXPERIMENTAL | HIERARCHICAL] [/STATISTICS=[MCA] [REG] [MEAN] [ALL] [NONE]]\n\
                 \x20     [/MISSING=EXCLUDE | INCLUDE].\n\
                 A range follows the factors it applies to; factor values are truncated to whole numbers and cases\n\
                 outside a range are left out, as is a case missing on any variable of its list. The model has the\n\
                 covariates, the main effects and the interactions up to MAXORDERS (five by default); an order of\n\
                 interactions with empty cells is dropped with a warning. UNIQUE (default) adjusts every effect for all\n\
                 others (Type III); EXPERIMENTAL assesses covariates, main effects and each order of interactions in\n\
                 turn, each adjusted for its own kind and the kinds before it; HIERARCHICAL adjusts each covariate and\n\
                 factor only for those before it. COVARIATES sets where the covariates enter. Tables: ANOVA (with REG the\n\
                 coefficients of the covariates where they enter), and, not with UNIQUE, Cell Means (MEAN), MCA,\n\
                 Factor Summary and Model Goodness of Fit (MCA). STATISTICS without keywords is MEAN and REG. Weighted\n\
                 when a weight variable is set.",
        example: "ANOVA VARIABLES=score BY region(1,4) sex(1,2) WITH age /METHOD=EXPERIMENTAL /STATISTICS=MEAN MCA.",
        parse: anova::anova,
    },
    CommandSpec {
        name: "TREE",
        summary: "A classification tree grown by CHAID or exhaustive CHAID.",
        syntax: "TREE dependent [level] BY predictor [level] ... [FORCE = var [level]]\n\
                 \x20    [/TREE DISPLAY=TOPDOWN | LEFTTORIGHT | RIGHTTOLEFT | NONE  NODES=STATISTICS | CHART | BOTH\n\
                 \x20           BRANCHSTATISTICS=YES | NO  NODEDEFS=YES | NO  SCALE=AUTO | n]\n\
                 \x20    [/DEPCATEGORIES USEVALUES=[VALID MISSING value ...] TARGET=[value ...]]\n\
                 \x20    [/PRINT=MODELSUMMARY CLASSIFICATION RISK CPS TREETABLE | NONE]\n\
                 \x20    [/GAIN CATEGORYTABLE=YES | NO  TYPE=[NODE]  SORT=DESCENDING | ASCENDING  CUMULATIVE=YES | NO]\n\
                 \x20    [/SAVE NODEID[(name)] PREDVAL[(name)] PREDPROB[(root)]]\n\
                 \x20    [/METHOD TYPE=CHAID | EXHAUSTIVECHAID]  [/VALIDATION TYPE=NONE]\n\
                 \x20    [/GROWTHLIMIT MAXDEPTH=AUTO | n  MINPARENTSIZE=100  MINCHILDSIZE=50]\n\
                 \x20    [/CHAID ALPHASPLIT=.05 ALPHAMERGE=.05 SPLITMERGED=NO CHISQUARE=PEARSON | LR CONVERGE=.001\n\
                 \x20            MAXITERATIONS=100 ADJUST=BONFERRONI | NONE INTERVALS=10 | varlist(n) ...]\n\
                 \x20    [/COSTS EQUAL] [/SCORES EQUALINCREMENTS | CUSTOM=value [score] ...]\n\
                 \x20    [/MISSING NOMINALMISSING=MISSING | VALID]\n\
                 \x20    [/PLOT GAIN RESPONSE INDEX INCREMENT=10].\n\
                 Levels: [S] scale, [O] ordinal, [N] or [C] nominal; without one the dictionary's level holds,\n\
                 and an unknown level comes from the data as in a data pass. The dependent is nominal\n\
                 (Pearson or likelihood-ratio chi-square) or ordinal (likelihood ratio against Goodman's row\n\
                 effects model, scored 1, 2, ... or by SCORES). Scale predictors are cut into INTERVALS groups of\n\
                 about equal size. Predictor categories that do not differ at ALPHAMERGE are merged (ordinal ones\n\
                 only when adjacent); a missing value is a category of its own that may merge with any. A category\n\
                 smaller than MINCHILDSIZE is merged with its most similar neighbor. The predictor with the\n\
                 smallest Bonferroni-adjusted significance splits the node when that is at most ALPHASPLIT. A node\n\
                 is not split when it is pure, smaller than MINPARENTSIZE or at MAXDEPTH (AUTO is 3). Exhaustive\n\
                 CHAID merges down to two categories and keeps the most significant set. The tree diagram comes as\n\
                 chart data: every node with its parent, the condition that leads to it, its category counts and\n\
                 percentages, its share of all cases, its predicted category and the split below it. TREETABLE lists the nodes\n\
                 in order with their parents. Gain tables (one per TARGET category) show terminal nodes by index.\n\
                 SAVE adds the terminal node, the predicted category and one probability per category for the\n\
                 cases in the analysis (default names NodeID, PredictedValue, PredictedProbability_1, ...).\n\
                 A weight counts each case as often as its rounded value. SPLIT FILE is ignored.\n\
                 PLOT GAIN, RESPONSE and INDEX give line charts with one line per TARGET category over cumulative\n\
                 percentile groups INCREMENT percent wide (1, 2, 5, 10, 20 or 25): the terminal nodes in the order\n\
                 of the gain tables, the node that holds a cut counted in proportion. PLOT MEAN, PROFIT, ROI and\n\
                 IMPORTANCE, PRIORS, CRT, QUEST, the settings of CRT and QUEST and PRINT IMPORTANCE, SURROGATES and\n\
                 CATEGORYSPECS are accepted and ignored with a warning. Not supported\n\
                 yet: METHOD TYPE=CRT and QUEST, a scale dependent, VALIDATION SPLITSAMPLE and CROSSVALIDATION,\n\
                 SPLITMERGED=YES, COSTS CUSTOM, GAIN TYPE PTILE, RULES, PROFITS, INFLUENCE, OUTFILE, TARGETRESPONSE.",
        example: "TREE churn [n] BY region [n] age_group [o] income [s] /GROWTHLIMIT MAXDEPTH=2 /SAVE NODEID PREDVAL.",
        parse: tree::tree,
    },
    CommandSpec {
        name: "GRAPH",
        summary: "One chart: summaries by category as bars, lines, areas, a pie, points with error bars, a Pareto chart or ranges, a histogram, a scatterplot or a scatterplot matrix.",
        syntax: "GRAPH [/TITLE='line' ['line']] [/SUBTITLE='line'] [/FOOTNOTE='line' ['line']]\n\
                 \x20     /BAR[(SIMPLE | GROUPED | STACKED | RANGE)]=spec | /PIE=spec\n\
                 \x20     | /LINE[(SIMPLE | MULTIPLE | AREA | DROP | DIFFERENCE)]=spec | /HILO[(SIMPLE | GROUPED)]=spec\n\
                 \x20     | /PARETO[(CUM | NOCUM)][(SIMPLE | STACKED)]=spec\n\
                 \x20     | /ERRORBAR[(CI [95] | STERROR [2] | STDDEV [2])]=varlist [BY var] | var BY var BY var\n\
                 \x20     | /HISTOGRAM[(NORMAL)]=var\n\
                 \x20     | /SCATTERPLOT[(BIVARIATE | OVERLAY | MATRIX | XYZ)]=x WITH y | xlist WITH ylist [(PAIR)]\n\
                 \x20         | varlist | x WITH y WITH z, then [BY group] [BY id (NAME | IDENTIFY)]\n\
                 \x20     [/INTERVAL CI[(95)] | SE[(2)] | STDDEV[(2)]]\n\
                 \x20     [/PANEL [COLVAR=varlist [COLOP=CROSS | NEST]] [ROWVAR=varlist [ROWOP=CROSS | NEST]]]\n\
                 \x20     [/MISSING=[LISTWISE | VARIABLE] [NOREPORT | REPORT] [EXCLUDE | INCLUDE]].\n\
                 spec: COUNT | PCT | CUFREQ | CUPCT BY var [BY var]; var alone (COUNT BY var); func(varlist) ...\n\
                 [BY var [BY var]]; VALUE(varlist) [BY var]. func: MEAN MEDIAN GMEDIAN MODE SUM CUSUM N MINIMUM\n\
                 MAXIMUM STDDEV VARIANCE, PTILE(varlist, p), PLT PGT NLT NGT(varlist, x), PIN NIN(varlist, low, high).\n\
                 Keywords may be cut to three letters (MIN, MAX, ERR, STE); the first subcommand needs no slash.\n\
                 The shape of the spec sets the layout: one function by one variable gives one series; one function\n\
                 by two variables gives a series per value of the second; several functions by one variable give a\n\
                 series per function; several functions without BY give one bar or slice each. A list such as\n\
                 SUM(a b) is SUM(a) SUM(b). SIMPLE, GROUPED and MULTIPLE change nothing; STACKED stacks the series\n\
                 and AREA fills under the lines and stacks them. A pie takes a single series.\n\
                 COUNT and N add up the case weights; PCT is a share of all cases in the chart; CUFREQ, CUSUM and\n\
                 CUPCT add up along the categories of each series, CUPCT ending at 100 per series. PLT, PGT and PIN\n\
                 give the share of a category's valid values below x, above x or from low to high; NLT, NGT and NIN\n\
                 the weighted count. MEDIAN and PTILE use (W + 1) p as FREQUENCIES does; MODE is the smallest of\n\
                 tied values; GMEDIAN takes the values as group midpoints. VALUE draws a bar per case and a series\n\
                 per variable, unweighted; missing values leave a gap and the BY variable names the cases.\n\
                 Categories ascend by value. A case missing a category value is left out unless REPORT, which adds\n\
                 categories for missing values after the valid ones. LISTWISE drops a case missing on any function\n\
                 variable of the chart, VARIABLE only from that variable's functions; INCLUDE makes user-missing\n\
                 values valid. A category empty in every series is left out; at most 3000 categories.\n\
                 ERRORBAR shows each mean as a point with a bar around it, laid out as MEAN(varlist) would be: a\n\
                 list by one BY variable or one variable by two puts the points side by side. CI is a confidence\n\
                 interval at the level (50 to 99.9) from Student's t with W - 1 degrees of freedom; STERROR (also\n\
                 SE) and STDDEV reach n standard errors or deviations each way. The number may sit in parentheses.\n\
                 A mean whose cases weigh 1 or less in total has no bar.\n\
                 INTERVAL adds such bars to a simple or clustered bar or line chart of MEAN or MEDIAN: CI, SE\n\
                 and STDDEV for a mean as on ERRORBAR, CI only for a median. A median's interval runs between\n\
                 order statistics, interpolated (Hettmansperger and Sheather), so it need not be symmetric; it\n\
                 needs whole case weights. Counts, percentages and the other functions get no bars yet.\n\
                 PARETO counts (COUNT or a bare variable) or adds up (SUM, VALUE), puts the largest category first\n\
                 and stacks several series. CUM, the default, draws the running total as a percentage of the grand\n\
                 total: a line against a second axis from 0 to 100 on the right. NOCUM drops the line and the axis.\n\
                 The range charts read their ends from the series: BAR(RANGE) floats a bar between each pair (two\n\
                 functions, or a second BY variable with two values); LINE(DROP) joins the lowest and the highest\n\
                 series of a category; LINE(DIFFERENCE) shades between two series; HILO takes high, low and an\n\
                 optional close as two or three series.\n\
                 HISTOGRAM bins the valid values as FREQUENCIES does; NORMAL adds the normal curve. SCATTERPLOT\n\
                 draws each case valid on both variables once, colored by the BY group variable; BY id (NAME)\n\
                 labels the points with that variable's values, (IDENTIFY) leaves them off. The variables set\n\
                 the kind, as for bars: lists around WITH lay a colored group of points for each pair into one\n\
                 frame, the variable before WITH across (every pair of the two lists, or with PAIR the first\n\
                 with the first and so on; no BY group). A list alone draws a matrix: a cell for each pair, the\n\
                 column's variable across and the row's up, the names on the diagonal. x WITH y WITH z has a\n\
                 third axis; it is not drawn yet and shows as the table of its points. LISTWISE leaves out a\n\
                 case missing on any of the variables; with VARIABLE a pair takes the cases valid on both.\n\
                 PANEL repeats the chart for each combination of the categories of the ROWVAR and COLVAR variables,\n\
                 rows first, each a separate chart named in its title. Combinations without cases get no chart, so\n\
                 COLOP and ROWOP change nothing. A case missing on a panel variable is left out unless REPORT.\n\
                 TITLE names the chart (otherwise it is named after what it shows); SUBTITLE goes under the title\n\
                 and FOOTNOTE below the chart. Lines have at most 72 characters. With SPLIT FILE each group gets a\n\
                 chart. Weighted when a weight variable is set; cases with a zero or missing weight are left out.\n\
                 Not supported yet: TEMPLATE and HILO with more than three series (GROUPED); each gives an\n\
                 error.",
        example: "GRAPH /TITLE='Visits per store' /BAR(STACKED)=COUNT BY store BY daypart.",
        parse: graph::graph,
    },
    CommandSpec {
        name: "GGRAPH",
        summary: "A chart from a graph dataset and a chart program in BEGIN GPL: bars (clustered, stacked), lines, areas, points, a pie or a histogram.",
        syntax: "GGRAPH /GRAPHDATASET NAME=\"name\" VARIABLES=item ... [TRANSFORM=NO | VARSTOCASES(SUMMARY=\"n\" INDEX=\"n\")]\n\
                 \x20             [MISSING=LISTWISE | VARIABLEWISE] [REPORTMISSING=NO | YES] [CASELIMIT=n]\n\
                 \x20      /GRAPHSPEC SOURCE=INLINE [LABEL=\"text\"].\n\
                 BEGIN GPL\n\
                 chart program\n\
                 END GPL.\n\
                 item: a variable (ALL and TO allowed) with an optional [NAME=\"new\" LEVEL=SCALE | NOMINAL | ORDINAL],\n\
                 or a function with an optional [NAME=\"new\"]: COUNT(), MINIMUM MAXIMUM VALIDN SUM MEAN STDDEV\n\
                 VARIANCE MEDIAN GMEDIAN MODE (var), PTILE PLT PGT NLT NGT NLE PLE NEQ PEQ NGE PGE (var, x), PIN NIN\n\
                 (var, low, high), CASEVALUE(var). Keywords are written in full. A function makes one row per\n\
                 combination of the other variables' values; its column is called by its name and arguments joined\n\
                 with underscores (MEAN_price, PTILE_price_90) unless NAME renames it. Without a function every\n\
                 case with a positive weight is a row, and $CASENUM numbers them. Weights count in every function.\n\
                 LISTWISE drops a case missing on any listed variable; VARIABLEWISE drops it only from the functions\n\
                 of the variable it misses. REPORTMISSING=YES turns user-missing values of variables that are not at\n\
                 the scale level into categories of their own. VARSTOCASES makes a row of each function, numbered in\n\
                 the INDEX column with the value in the SUMMARY column. CASELIMIT keeps the first n rows.\n\
                 Each GRAPHSPEC takes the next BEGIN GPL block; both stand right after the command. In the chart\n\
                 program: SOURCE: s=userSource(id(\"name\")); DATA: x=col(source(s), name(\"column\")\n\
                 [, unit.category()]); GUIDE: axis(dim(1 | 2), label(\"text\") | null()), legend(aesthetic(...),\n\
                 label(\"text\")), text.title, text.subtitle, text.footnote(label(\"text\")); SCALE: linear(dim(2),\n\
                 include(0), max(n)), cat(dim(n) | aesthetic(...), include(\"value\", ...)), which adds categories;\n\
                 COORD: rect(dim(1,2), cluster(3)) clusters, transpose() lays bars across, polar.theta() makes a pie;\n\
                 ELEMENT: interval, interval.dodge, interval.stack, line, area, area.stack or point with position(x*y)\n\
                 or position(x*y*z) with cluster, and color(variable) for a series per category or color(\"name\") to\n\
                 name the series. Several rows on one point need summary.count, mean, sum, median, mode, min, max or\n\
                 percent around the position; position(summary.count(bin.rect(x))) is a histogram and\n\
                 line(position(density.normal(x))) adds its normal curve. point over two scale variables is a\n\
                 scatterplot. Other statements and settings give a warning and are skipped.",
        example: "GGRAPH /GRAPHDATASET NAME=\"sales\" VARIABLES=store daypart MEAN(basket)\n\
                 \x20 /GRAPHSPEC SOURCE=INLINE.\n\
                 BEGIN GPL\n\
                 SOURCE: s=userSource(id(\"sales\"))\n\
                 DATA: store=col(source(s), name(\"store\"), unit.category())\n\
                 DATA: daypart=col(source(s), name(\"daypart\"), unit.category())\n\
                 DATA: basket=col(source(s), name(\"MEAN_basket\"))\n\
                 GUIDE: axis(dim(2), label(\"Average basket\"))\n\
                 ELEMENT: interval.stack(position(store*basket), color(daypart))\n\
                 END GPL.",
        parse: ggraph::ggraph,
    },
    CommandSpec {
        name: "BEGIN GPL",
        summary: "The chart program of the GGRAPH command right before it, up to END GPL.",
        syntax: "BEGIN GPL\n\
                 chart program\n\
                 END GPL.\n\
                 BEGIN GPL and END GPL each stand alone on their line. The block belongs to the GGRAPH before it\n\
                 (one block for each GRAPHSPEC with SOURCE=INLINE); see GGRAPH for the statements it reads.",
        example: "BEGIN GPL\n\
                 SOURCE: s=userSource(id(\"sales\"))\n\
                 DATA: price=col(source(s), name(\"price\"))\n\
                 ELEMENT: interval(position(summary.count(bin.rect(price))))\n\
                 END GPL.",
        parse: ggraph::begin_gpl,
    },
    CommandSpec {
        name: "IGRAPH",
        summary: "The older chart command: bars, lines, areas, pies, boxes, error bars, histograms and scatterplots built from variables placed on axes and legends.",
        syntax: "IGRAPH [/Y=VAR(var) | $COUNT | $PCT [TYPE=SCALE [(MIN=n MAX=n)] | CATEGORICAL] [TITLE='text']]\n\
                 \x20      [/X1=VAR(var) | $CASE [TYPE=...] [TITLE='text']] [/X2=VAR(var) [TYPE=...] [TITLE='text']]\n\
                 \x20      [/COLOR=VAR(var) [TYPE=...] [LEGEND=ON | OFF] [TITLE='text'] [CLUSTER | STACK]]\n\
                 \x20      [/STYLE=VAR(var) [LEGEND=ON | OFF] [TITLE='text'] [CLUSTER | STACK]] [/SIZE=VAR(var) ...]\n\
                 \x20      [/SUMMARYVAR=VAR(var) | $COUNT | $PCT] [/STACK=VAR(var)] [/PANEL VAR(var) [VAR(var) ...]]\n\
                 \x20      [/CATORDER VAR(var) ([ASCENDING | DESCENDING] [VALUE | LABEL | COUNT | OCCURRENCE]\n\
                 \x20          [OMITEMPTY | SHOWEMPTY])] [/POINTLABEL=VAR(var) [ALL | NONE]] [/CASELABEL=VAR(var)]\n\
                 \x20      [/COORDINATE=VERTICAL | HORIZONTAL | THREE] [/NORMALIZE] [/TITLE='text'] [/SUBTITLE='text']\n\
                 \x20      [/CAPTION='text'] [/VIEWNAME='text'] [/MISSING=[LISTWISE | VARIABLEWISE] [NOREPORT | REPORT]\n\
                 \x20          [EXCLUDE | INCLUDE]]\n\
                 \x20      /BAR [(function)] | /LINE [(function)] | /AREA [(function)] | /PIE [(function)]\n\
                 \x20      | /BOX [OUTLIERS=ON | OFF] [EXTREME=ON | OFF] | /ERRORBAR CI(level) | SD(n) | SE(n)\n\
                 \x20      | /HISTOGRAM [CUM] [CURVE=ON | OFF] | /SCATTER [/FITLINE METHOD=REGRESSION LINEAR\n\
                 \x20          | ORIGIN LINEAR | MEAN | NONE [LINE=TOTAL MEFFECT]].\n\
                 function: MEAN MEDIAN MODE MINIMUM MAXIMUM SUM SUMAV SUMSQ STDDEV VARIANCE SEMEAN SKEW\n\
                 KURTOSIS SESKEW SEKURT FIRST LAST, PTILE(p), NLT NLE NEQ NGE NGT(x), PLT PLE PEQ PGE PGT(x),\n\
                 NIN PIN(low, high). A variable may also be written without VAR( ).\n\
                 Each chart is drawn as GRAPH would draw it. BAR, LINE and AREA put the categories of X1 along\n\
                 the axis and a summary of Y on the other: the function given (MEAN when none is) of a Y variable,\n\
                 or the count or percentage of cases with Y=$COUNT or $PCT. A categorical COLOR or STYLE variable\n\
                 gives a series per category, clustered unless STACK is given; NORMALIZE then stacks to 100%.\n\
                 X1=$CASE draws a bar per case, named by the CASELABEL variable. A Y variable of TYPE=CATEGORICAL\n\
                 takes MODE only. ERRORBAR alone draws means of Y with bars of a confidence interval or of n\n\
                 standard deviations or errors; given with BAR or LINE it adds those bars to the means.\n\
                 PIE takes its slices from the COLOR (or STYLE) categories, sized by SUMMARYVAR: $COUNT (the\n\
                 default), $PCT, or a variable with a function such as PIE (SUM). X1, Y and STACK variables\n\
                 give a separate pie per category, as PANEL does.\n\
                 BOX draws Tukey boxes of Y per category of X1, side by side per COLOR category: hinges,\n\
                 median, whiskers to the furthest values within 1.5 box lengths, outliers beyond them and\n\
                 extremes beyond 3 box lengths, each marked by case number. OUTLIERS=OFF and EXTREME=OFF leave\n\
                 those marks out. HISTOGRAM bins X1 with automatic widths as GRAPH does; Y=$PCT shows\n\
                 percentages, CUM running totals and CURVE=ON the normal curve.\n\
                 SCATTER plots Y against X1, a group per COLOR or STYLE category; a scale X2 adds a third axis.\n\
                 POINTLABEL with ALL labels the points. FITLINE adds a least squares line, a line through the\n\
                 origin, or the mean of Y, over all points (LINE=TOTAL, the default) or per group (MEFFECT);\n\
                 the footnote gives each line's intercept and slope. The points and lines are unweighted.\n\
                 A variable takes its TYPE from its measurement level when none is given. A scale COLOR\n\
                 variable only colors the elements, so it is ignored. CATORDER sorts the categories of a\n\
                 variable by value (the default), label, weighted count or first case. PANEL repeats the chart\n\
                 for each combination of the panel categories. TITLE, SUBTITLE and CAPTION go above and below\n\
                 the chart (a \\n inside becomes a space); VIEWNAME names it in the outline. X1 and Y TITLE\n\
                 name the axes, MAX sets the top of the axis. A case missing on a variable of the chart is left\n\
                 out; MISSING works as in GRAPH. With SPLIT FILE each group gets a chart.\n\
                 Keywords that only change the look (CHARTLOOK, the LENGTH subcommands, EFFECT, SIZE, KEY=OFF,\n\
                 LEGEND=OFF, MIN, label, baseline, whisker, cap and line style settings, JITTER, SHOWEMPTY,\n\
                 X1INTERVAL and X1START, FORMAT) are accepted and named in one warning. REFLINE, SPIKE,\n\
                 FITLINE METHOD=LLR, FITLINE INTERVAL and SPIKE=ON cannot be drawn and give an error, as do\n\
                 X2 outside a scatterplot and a mix of elements other than those above.",
        example: "IGRAPH /X1=VAR(store) TYPE=CATEGORICAL /Y=VAR(basket) TYPE=SCALE /COLOR=VAR(daypart) /BAR (MEAN).",
        parse: igraph::igraph,
    },
    CommandSpec {
        name: "XGRAPH",
        summary: "Charts from one expression: summaries as bars or lines, clustered, stacked and paneled, population pyramids, binned histograms, dot plots and boxes.",
        syntax: "XGRAPH [CHART=] (y [+ y ...]) [> var | 1] [BY x [> var] [BY z [> var]]]\n\
                 \x20  [/BIN START=AUTO | n SIZE=AUTO | WIDTH(n) | COUNT(n)] [/DISPLAY DOT=ASYMMETRIC | SYMMETRIC | FLAT]\n\
                 \x20  [/DISTRIBUTION TYPE=NORMAL] [/COORDINATE SPLIT=NO | YES] [/ERRORBAR CI(level) | STDDEV(n) | SE(n)]\n\
                 \x20  [/MISSING USE=LISTWISE | VARIABLEWISE REPORT=NO | YES]\n\
                 \x20  [/PANEL COLVAR=varlist [COLOP=CROSS | NEST] ROWVAR=varlist [ROWOP=CROSS | NEST]]\n\
                 \x20  [/TEMPLATE FILE='file'] [/TITLES TITLE='line' ['line'] SUBTITLE='line' FOOTNOTE='line' ['line']].\n\
                 y: var [function] [element], (var + var ...) [function] [element], or [count function] [element].\n\
                 x, z: var [c] or [s], $CASENUM [LABEL=var], or 1, the stand-in for the y variables.\n\
                 function: COUNT PCT CUPCT CUFREQ (no variable); VALUE; MINIMUM MAXIMUM VALIDN SUM CUSUM MEAN\n\
                 STDDEV VARIANCE MEDIAN GMEDIAN MODE, PTILE(p), PLT PGT NLT NGT(x), PIN NIN(low, high).\n\
                 element: BAR (the default), LINE, HISTOBAR, POINT, BOX. Square brackets are typed as shown,\n\
                 one keyword in each; names and keywords are written in full, each subcommand at most once.\n\
                 BAR and LINE draw the function of y for every category of x. A variable after > on y stacks\n\
                 the bars; one after > on x clusters them; z gives the series when neither is there. Every\n\
                 further dimension (a second cluster, or z next to a stack or cluster) repeats the chart for\n\
                 each of its categories, as PANEL does. Several y variables, joined with +, need the 1 where\n\
                 they form the categories, the series or the stack. [VALUE] draws a bar for every case, the\n\
                 cases numbered on the axis that holds $CASENUM or named by its LABEL variable; a variable on\n\
                 the other axis gives a chart for each of its categories. [COLOR] and [PATTERN] after a stack\n\
                 or cluster variable are accepted and change nothing.\n\
                 COORDINATE SPLIT=YES turns a bar chart into a population pyramid: horizontal bars with a\n\
                 series for each category of z. HISTOBAR counts the cases in bins of a scale x and draws a\n\
                 histogram for each category of z, all with the same bins, which BIN sets (by default as\n\
                 FREQUENCIES makes them; COUNT(n) gives n bins from the start to the largest value).\n\
                 DISTRIBUTION adds the normal curve of each histogram. POINT puts a dot for every case at its\n\
                 x value; DISPLAY stacks equal values up from 1 (ASYMMETRIC), around 0 (SYMMETRIC) or not at\n\
                 all (FLAT). BOX draws Tukey boxes of the y variable by x, side by side by the x cluster or z.\n\
                 ERRORBAR adds bars to counts and percentages (CI: Jeffreys' interval; SE and STDDEV: n times\n\
                 the binomial spread of the count), means, and medians (CI only). MISSING USE=LISTWISE leaves\n\
                 out a case missing on any y variable, VARIABLEWISE only from that variable; REPORT=YES keeps\n\
                 categories of missing values. TITLES lines may hold )DATE, )TIME and )CHART. TEMPLATE is\n\
                 accepted and ignored with a warning. With SPLIT FILE each group gets its charts.",
        example: "XGRAPH CHART=(spend [MEAN] [BAR]) BY region[c] > channel[c] BY season[c]\n\
                  \x20 /ERRORBAR SE(1) /TITLES TITLE='Spend by region'.",
        parse: xgraph::xgraph,
    },
    CommandSpec {
        name: "PPLOT",
        summary: "Probability plots: Q-Q or P-P plots of variables against a test distribution.",
        syntax: PPLOT_SYNTAX,
        example: "PPLOT VARIABLES=wait_min /DISTRIBUTION=GAMMA /TYPE=P-P /PLOT=NORMAL.",
        parse: pplot::pplot,
    },
    CommandSpec {
        name: "NPPLOT",
        summary: "Another name for PPLOT.",
        syntax: "NPPLOT [VARIABLES=] varlist [/subcommands of PPLOT].\n\
                 The same command as PPLOT, with the same subcommands and defaults.",
        example: "NPPLOT battery_hours /DISTRIBUTION=WEIBULL /FRACTION=VW.",
        parse: pplot::pplot,
    },
    CommandSpec {
        name: "ROC",
        summary: "ROC curve of test variables against a state, with the area under the curve.",
        syntax: "ROC testvars BY state(value)\n\
                 \x20  [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/CRITERIA=[CUTOFF(INCLUDE | EXCLUDE)] [TESTPOS(LARGE | SMALL)] [CI(95)]\n\
                 \x20             [DISTRIBUTION(FREE | NEGEXPO)]]\n\
                 \x20  [/PLOT=CURVE[(REFERENCE)] | NONE] [/PRINT=[SE[(MODELQUALITY)]] [COORDINATES]].\n\
                 The test variables are numeric; the state variable may be a string, and the value in\n\
                 parentheses is its positive state, every other valid state negative. Cases with a missing\n\
                 state or test value are left out of every curve (MISSING=INCLUDE keeps user-missing values);\n\
                 weights count cases. TESTPOS(SMALL) makes low test values point to the positive state. Each\n\
                 cutoff lies halfway between neighboring test values, plus one below the smallest and one\n\
                 above the largest; a value equal to a cutoff tests positive unless CUTOFF(EXCLUDE).\n\
                 Tables: Case Processing Summary; Area Under the Curve, which counts every positive and\n\
                 negative pair (a tie as half) and with SE adds the nonparametric standard error (or the\n\
                 bi-negative exponential one under NEGEXPO when both groups are equally large), the\n\
                 two-sided test of area 0.5 and the CI interval; COORDINATES lists sensitivity and\n\
                 1 - specificity at every cutoff. CURVE draws the curves in one chart, REFERENCE adds the\n\
                 diagonal; MODELQUALITY adds a bar chart of the lower bounds of the intervals.",
        example: "ROC risk_score lab_index BY relapsed (1) /PLOT=CURVE(REFERENCE) /PRINT=SE COORDINATES.",
        parse: pplot::roc,
    },
    CommandSpec {
        name: "ROC ANALYSIS",
        summary: "ROC curves with paired or two-group comparisons of their areas, precision-recall curves and classifier metrics.",
        syntax: "ROC ANALYSIS testvars BY state(value)\n\
                 \x20  [/MISSING USERMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/CRITERIA [CUTOFF=INCLUDE | EXCLUDE] [TESTPOS=LARGE | SMALL] [CI=95]\n\
                 \x20             [DISTRIBUTION=FREE | NEGEXPO]]\n\
                 \x20  [/DESIGN [PAIR=FALSE | TRUE] [GROUP=var]\n\
                 \x20           [SELECT=LEVEL(value value) | MIDPOINT | CUTPOINT(value)]]\n\
                 \x20  [/PLOT [CURVE=ROC[(REFERENCE)] [PR[(REFERENCE)]] | NONE] [PR_INTERPOLATE=TP | FP]\n\
                 \x20         [MODELQUALITY=FALSE | TRUE]]\n\
                 \x20  [/PRINT [SE=FALSE | TRUE] [COORDINATES=[ROC] [PR]] [CLASSIFIER=FALSE | TRUE]].\n\
                 Every keyword may also be written KEY(value). Test variables are numeric; the state\n\
                 variable may be a string, its value in parentheses marks the positive cases and every\n\
                 other valid state is negative. A case takes part when its state, all test variables\n\
                 and the grouping variable are valid (USERMISSING=INCLUDE keeps user-missing values).\n\
                 Weights are rounded to whole numbers and count cases; a weight of 0.5 or less, or a\n\
                 missing one, makes the case missing. Cutoffs, directions and areas as in\n\
                 ROC; when every test value lies strictly between 0 and 1 the outer cutoffs are 0 and 1.\n\
                 Without DESIGN each test variable gets its own area. PAIR=TRUE compares every pair of\n\
                 two or more test variables measured on the same cases: the errors of the areas and of\n\
                 their differences follow DeLong's method, and NEGEXPO does not apply. GROUP with SELECT\n\
                 compares two independent groups of cases for each test variable (PAIR is then\n\
                 ignored): LEVEL names the value of each group; MIDPOINT (halfway between the smallest\n\
                 and largest group value) and CUTPOINT split a numeric grouping variable into the cases\n\
                 at or above the point and those below it. NEGEXPO needs equally many positive and\n\
                 negative cases in both groups, else both groups use the nonparametric error.\n\
                 Tables: Case Processing Summary; Area Under the ROC Curve (SE=TRUE adds the standard\n\
                 error, the test of area 0.5 and the CI interval); CLASSIFIER adds the Gini index\n\
                 2 x area - 1 and the largest gap between sensitivity and 1 - specificity (Max K-S)\n\
                 with its cutoff; the paired or independent design adds a table of area differences\n\
                 with z tests and intervals; COORDINATES lists sensitivity and 1 - specificity (ROC) or\n\
                 precision and recall (PR) at every cutoff. CURVE draws ROC curves (the default), PR\n\
                 curves or none; with groups each test variable gets its own chart. PR curves are\n\
                 filled in between cutoffs one true positive (TP) or one false positive (FP) at a\n\
                 time; PR(REFERENCE) draws the share of positive cases. MODELQUALITY=TRUE adds a bar\n\
                 chart of the lower bounds of the area intervals.",
        example: "ROC ANALYSIS score_a score_b BY diseased (1) /DESIGN PAIR=TRUE /PRINT SE=TRUE CLASSIFIER=TRUE /PLOT CURVE=ROC(REFERENCE) PR.",
        parse: roc_analysis::roc_analysis,
    },
    CommandSpec {
        name: "SPCHART",
        summary: "Control charts (X-bar with R or s, individuals, p, np, c, u) and the process capability chart.",
        syntax: spchart::SPCHART_SYNTAX,
        example: "SPCHART /XR=fill_ml BY batch /RULES=ALL /STATISTICS=CP CPK PP PPK /LSL=495 /USL=505.",
        parse: spchart::spchart,
    },
    CommandSpec {
        name: "KM",
        summary: "Kaplan-Meier survival estimates by factor level and stratum, with tests that compare the levels.",
        syntax: "KM time [BY factor] /STATUS=var [EVENT](values) [LOST(values)]\n\
                 \x20  [/STRATA=var] [/ID=var] [/PLOT=[SURVIVAL] [LOGSURV] [HAZARD] [OMS]]\n\
                 \x20  [/PRINT=[TABLE] [MEAN] [NONE]] [/PERCENTILES=[(]values[)]]\n\
                 \x20  [/TEST=[LOGRANK] [BRESLOW] [TARONE]] [/COMPARE=[OVERALL | PAIRWISE] [POOLED | STRATA]]\n\
                 \x20  [/TREND[=(weights)]] [/SAVE=[SURVIVAL[(name)]] [HAZARD[(name)]] [SE[(name)]] [CUMEVENT[(name)]]].\n\
                 K-M is another name. A case whose status is one of the EVENT values (single values or\n\
                 a THRU b, quoted for a string status) had the event at its time; any other status counts\n\
                 as censored, and LOST values are counted apart in the tables though they are censored\n\
                 too. Cases with a missing time, status, factor or stratum, or a negative time, are left\n\
                 out; weights count cases. Each factor level within each stratum gets its own curve.\n\
                 Tables: Case Processing Summary; a Survival Table per curve, a row per case with the\n\
                 estimate and its Greenwood standard error at each event time, the events so far and the\n\
                 cases still at risk (ID labels the rows); Means and Medians, the mean being the area\n\
                 under the curve up to the longest time; Percentiles (25, 50 and 75 when no values are\n\
                 given), where the p% value is the first event time at which the estimate is p/100 or\n\
                 lower. PRINT names what to show: TABLE, MEAN or NONE.\n\
                 With a factor, the log rank test (or TEST: Breslow, weighted by the cases at risk, and\n\
                 Tarone-Ware, by their square root) compares the levels: all at once or each pair\n\
                 (PAIRWISE), pooled over the strata or within each (STRATA). TREND tests a linear trend\n\
                 over the levels in ascending order with one degree of freedom, with the given weights or\n\
                 evenly spaced ones centered on 0. PLOT draws, for each stratum, the step curves of the\n\
                 levels with censored times marked: SURVIVAL, LOGSURV (its log), HAZARD (minus its log,\n\
                 the cumulative hazard) and OMS (one minus it). SAVE adds the estimate, the cumulative\n\
                 hazard, the standard error and the events up to each case's time; the default names are\n\
                 SUR_n, HAZ_n, SE_n and CUM_n.",
        example: "KM weeks BY plan /STATUS=outcome EVENT(1) LOST(2) /STRATA=region /TEST=LOGRANK BRESLOW /COMPARE=PAIRWISE /PLOT=SURVIVAL.",
        parse: survival::km,
    },
    CommandSpec {
        name: "K-M",
        summary: "Another name for KM.",
        syntax: "K-M time [BY factor] /STATUS=var [EVENT](values) [/subcommands of KM].\n\
                 The same command as KM, with the same subcommands and defaults.",
        example: "K-M days_to_repair /STATUS=fixed(1).",
        parse: survival::km,
    },
    CommandSpec {
        name: "SURVIVAL",
        summary: "Actuarial life tables over fixed time intervals, with the Wilcoxon (Gehan) comparison of groups.",
        syntax: "SURVIVAL [TABLES=] times [BY controls(min, max) ...] [BY controls(min, max) ...]\n\
                 \x20  /INTERVAL=THRU end BY width [THRU end BY width ...]\n\
                 \x20  /STATUS=var(min[, max]) [FOR times] [/STATUS=...]\n\
                 \x20  [/PLOTS[(ALL | SURVIVAL | LOGSURV | HAZARD | DENSITY | OMS ...)][=times BY controls BY controls]]\n\
                 \x20  [/PRINT=TABLE | NOTABLE] [/COMPARE[=times BY controls BY controls]]\n\
                 \x20  [/CALCULATE=[EXACT | CONDITIONAL | APPROXIMATE] [PAIRWISE] [COMPARE]]\n\
                 \x20  [/MISSING=[GROUPWISE | LISTWISE] [INCLUDE]] [/WRITE=NONE].\n\
                 Intervals start at 0; each THRU ... BY period continues where the last one ended, and a\n\
                 period that the width does not divide evenly is stretched to the next whole interval.\n\
                 One more interval, open at the top, holds the longer times. A time falls into the\n\
                 interval whose start it reaches. A case whose status lies in the range had the event;\n\
                 other cases are censored and count as half exposed in their interval. STATUS without FOR\n\
                 serves every time variable that no other STATUS names. Each control's values are\n\
                 truncated to whole numbers and kept inside its range; first order controls each split\n\
                 the cases on their own, a second BY splits every one of those again.\n\
                 The life table shows, per interval with cases entering: those entering, withdrawing,\n\
                 exposed to risk and having the event, the proportions ending and surviving, the\n\
                 cumulative survival at the interval's end with its standard error, the density and the\n\
                 hazard with theirs (none for the open interval). Median Survival Times interpolates\n\
                 inside the interval where the cumulative survival passes 0.5, or shows the start of the\n\
                 last interval cases entered with a plus sign. Weights count cases, so counts per\n\
                 interval may be entered as weighted records. Negative times are missing.\n\
                 COMPARE tests whether the groups of each first order control share one survival curve,\n\
                 with Gehan's scores (a tie between an event and a censored time ranks the event first);\n\
                 CALCULATE PAIRWISE adds each pair of values, APPROXIMATE scores the interval of each time\n\
                 instead of the time, COMPARE gives the comparisons only. MISSING=GROUPWISE leaves a case\n\
                 out only of the tables whose variables it misses; LISTWISE leaves out a case missing on\n\
                 any TABLES variable (a missing status only affects its own time variable); INCLUDE keeps\n\
                 user-missing values. PLOTS draws, per time variable, first order control and second order\n\
                 value, the survival function, its log, the hazard, the density or one minus survival, the\n\
                 groups as lines. Not supported yet: WRITE=TABLES and BOTH (only CROSSTABS writes through PROCEDURE OUTPUT so far).",
        example: "SURVIVAL TABLES=weeks BY plan(1,3) /INTERVAL=THRU 30 BY 5 /STATUS=outcome(1) /COMPARE /CALCULATE=PAIRWISE /PLOTS(SURVIVAL,HAZARD).",
        parse: survival::survival,
    },
    CommandSpec {
        name: "CSPLAN",
        summary: "Write or show the plan of a complex sample: how to draw it, or how to analyze it.",
        syntax: "CSPLAN SAMPLE /PLAN FILE='file' [/PLANVARS [SAMPLEWEIGHT=var] [PREVIOUSWEIGHT=var]]\n\
                 \x20  [/PRINT [PLAN] [MATRIX]]\n\
                 \x20  /DESIGN [STAGELABEL='label'] [STRATA=vars] [CLUSTER=vars]\n\
                 \x20  /METHOD TYPE=SIMPLE_WOR | SIMPLE_WR | SIMPLE_SYSTEMATIC | SIMPLE_CHROMY | PPS_WOR | PPS_WR\n\
                 \x20              | PPS_SYSTEMATIC | PPS_BREWER | PPS_MURTHY | PPS_SAMPFORD | PPS_CHROMY\n\
                 \x20              [ESTIMATION=DEFAULT | WR]\n\
                 \x20  /SIZE VALUE=n | VARIABLE=var | MATRIX=vars; categories n; ...\n\
                 \x20  | /RATE VALUE=f | VARIABLE=var | MATRIX=vars; categories f; ... [MINSIZE=n] [MAXSIZE=n]\n\
                 \x20  [/MOS VARIABLE=var | SOURCE=FROMDATA [MIN=m] [MAX=m]]\n\
                 \x20  [/STAGEVARS [INCLPROB[(var)]] [CUMWEIGHT[(var)]] [INDEX[(var)]] [POPSIZE[(var)]]\n\
                 \x20             [SAMPSIZE[(var)]] [RATE[(var)]] [WEIGHT[(var)]]]\n\
                 \x20  [/DESIGN ... /METHOD ... (a second and a third stage)].\n\
                 CSPLAN ANALYSIS /PLAN FILE='file' /PLANVARS ANALYSISWEIGHT=var [/SRSESTIMATOR TYPE=WOR | WR]\n\
                 \x20  [/PRINT [PLAN] [MATRIX]]\n\
                 \x20  /DESIGN [STAGELABEL='label'] [STRATA=vars] [CLUSTER=vars]\n\
                 \x20  /ESTIMATOR TYPE=EQUAL_WOR | UNEQUAL_WOR | WR\n\
                 \x20  [/POPSIZE VALUE=n | VARIABLE=var | MATRIX=vars; categories n; ...]\n\
                 \x20  [/INCLPROB VALUE=p | VARIABLE=var | MATRIX=vars; categories p; ...]\n\
                 \x20  [/DESIGN ... /ESTIMATOR ... (a second and a third stage)].\n\
                 CSPLAN VIEW /PLAN FILE='file' [/PRINT [PLAN] [MATRIX]].\n\
                 The plan is a small JSON file that the other CS commands read; CSPLAN does not read\n\
                 the data. Each DESIGN starts a stage, and the subcommands after it belong to that stage.\n\
                 A stage followed by another needs CLUSTER.\n\
                 SAMPLE plans are carried out by CSSELECT. Per stage, METHOD says how units are drawn inside\n\
                 each stratum: SIMPLE_WOR at random without replacement, SIMPLE_WR with replacement (last\n\
                 stage only), SIMPLE_SYSTEMATIC every N/n-th unit from a random start, SIMPLE_CHROMY\n\
                 sequentially with Chromy's method. The PPS methods (first stage only) give each unit a chance\n\
                 in proportion to its measure of size, MOS: a variable, or with SOURCE=FROMDATA the number of\n\
                 cases in the cluster, held between MIN and MAX. PPS_WOR (Hanurav and Vijayan), PPS_SAMPFORD,\n\
                 PPS_BREWER and PPS_MURTHY (these two draw two units) draw without replacement, need CLUSTER\n\
                 and are analyzed as UNEQUAL_WOR; PPS_WR, PPS_SYSTEMATIC and PPS_CHROMY are analyzed as\n\
                 WR. A unit too large for its share of the sample is taken for sure. SIZE gives the number of\n\
                 units per stratum, RATE the\n\
                 share (n is the share times the population rounded, then raised to MINSIZE or lowered to\n\
                 MAXSIZE); either one as a value, a variable that holds it, or a matrix with a row per\n\
                 stratum. STAGEVARS renames the variables CSSELECT writes and asks for more: the inclusion\n\
                 probability and the cumulative weight are always written (InclusionProbability_1_ and\n\
                 SampleWeightCumulative_1_ for stage 1), the duplicate index for a stage drawn with\n\
                 replacement, and on request the population size, sample size, rate and weight of the stage.\n\
                 The final weight (SAMPLEWEIGHT, default SampleWeight_Final_) multiplies the stage weights and\n\
                 PREVIOUSWEIGHT. A sample plan is an analysis plan too: SIMPLE_WOR stages are analyzed as\n\
                 EQUAL_WOR with the inclusion probability variable, the PPS methods as said above, the\n\
                 others as WR (and every stage as WR with ESTIMATION=WR, first stage only), so CSDESCRIPTIVES\n\
                 can read the plan of a drawn sample.\n\
                 ANALYSIS plans describe a sample drawn elsewhere. WR treats the units of the stage as drawn\n\
                 with replacement and ends the design: later stages add nothing to the variance. EQUAL_WOR\n\
                 treats them as drawn without replacement with equal chances and needs either the population\n\
                 size of each stratum (POPSIZE, whole numbers) or the share of units drawn (INCLPROB, above 0\n\
                 and at most 1); it brings the finite population correction. UNEQUAL_WOR (first stage, with\n\
                 CLUSTER) treats the units as drawn without replacement with unequal chances: the analysis\n\
                 reads their inclusion and joint inclusion probabilities from the file CSSELECT writes (see\n\
                 JOINTPROB of CSDESCRIPTIVES). SRSESTIMATOR picks the simple\n\
                 random sampling variance that design effects compare with: WOR (the default) includes the\n\
                 finite population correction, WR does not.\n\
                 PRINT PLAN (the default) shows the plan, MATRIX the matrix rows.",
        example: "CSPLAN SAMPLE /PLAN FILE='orchards_draw.json' /DESIGN STRATA=valley CLUSTER=orchard /METHOD TYPE=SIMPLE_WOR /SIZE VALUE=3 /DESIGN /METHOD TYPE=SIMPLE_SYSTEMATIC /RATE VALUE=0.25 MINSIZE=2.",
        parse: complex_samples::csplan,
    },
    CommandSpec {
        name: "CSSELECT",
        summary: "Draw a complex sample from the active dataset by a plan written by CSPLAN SAMPLE.",
        syntax: "CSSELECT /PLAN FILE='file' [/CRITERIA [STAGES=n [n [n]]] [SEED=RANDOM | n]]\n\
                 \x20  [/CLASSMISSING EXCLUDE | INCLUDE] [/DATA [RENAMEVARS] [PRESORTED]]\n\
                 \x20  [/SAMPLEFILE OUTFILE='file' | dataset [KEEP=vars] [DROP=vars]]\n\
                 \x20  [/JOINTPROB OUTFILE='file'] [/SELECTRULE OUTFILE='file'] [/PRINT [SELECTION] [CPS]].\n\
                 The active dataset is the frame. Stage by stage, the units (cases, or the combinations of\n\
                 the CLUSTER values) are drawn inside each stratum; a later stage draws only inside the units\n\
                 the earlier one selected. STAGES runs part of the plan: a later stage then needs the\n\
                 cumulative weight variable of the stage before it in the data. A case with a missing\n\
                 stratum or cluster (user-missing ones count under CLASSMISSING=INCLUDE), or a missing size,\n\
                 rate or previous weight, is not part of the frame. FILTER and USE limit the frame; WEIGHT\n\
                 and SPLIT FILE are ignored with a warning.\n\
                 SEED=n makes the draw repeatable and leaves the session's generator alone. Without it the\n\
                 seed comes from the session's generator when SET SEED was given in the session, so the\n\
                 same syntax draws the same sample again; otherwise from the clock.\n\
                 Without SAMPLEFILE the new variables join the active dataset, which keeps every\n\
                 case: a case outside the sample has missing weights. SAMPLEFILE writes the selected cases\n\
                 only, with the variables KEEP and DROP leave (applied in their order) and the new ones; a\n\
                 unit drawn several times with replacement appears once per draw, numbered by the index\n\
                 variable. A plan that draws with replacement needs SAMPLEFILE. A new variable whose name\n\
                 already exists is an error; with RENAMEVARS the existing variable is renamed to the first\n\
                 free name of the form name_1, name_2, ... with a warning.\n\
                 Stage inclusion probabilities are n/N (with replacement 1 - (1 - 1/N)^n) and stage weights\n\
                 N/n per draw; with a PPS method n times the unit's share of the total size (PPS_WR:\n\
                 1 - (1 - share)^n, weight one over n times the share; PPS_MURTHY its own formula), and a\n\
                 measure of size must be the same in all cases of a unit. PRINT SELECTION (the default) shows\n\
                 per stage and stratum the population size, the sample size and the rate; CPS counts the\n\
                 cases. A first stage drawn by PPS_WOR, PPS_BREWER, PPS_MURTHY or PPS_SAMPFORD also writes\n\
                 the joint inclusion probabilities of the selected units to a data file: JOINTPROB OUTFILE,\n\
                 or else the plan's name with the extension .joint.sav. It holds the first-stage strata and\n\
                 clusters, Unit_No_ (the unit's number in its stratum) and Joint_Prob_k_ for k = 1, 2 and\n\
                 so on (the joint probability with unit k; the inclusion probability on the diagonal); with\n\
                 other plans JOINTPROB is ignored with a warning.\n\
                 SELECTRULE OUTFILE writes a text file (replacing one of that name) with a condition that\n\
                 is true for the units drawn by the last stage run that has clusters, ready to turn into a\n\
                 SELECT IF or a database query for the next stage. Each unit is a line of comparisons\n\
                 (name EQ value) on the strata and clusters of its stage and of the stages above it, joined\n\
                 by AND; the lines are joined by OR; string values are quoted. When no stage run has\n\
                 clusters, the cases themselves are the units and nothing names them, so no file is\n\
                 written and a warning says so.",
        example: "CSSELECT /PLAN FILE='orchards_draw.json' /CRITERIA SEED=4711 /SAMPLEFILE OUTFILE='orchards_sample.sav' DROP=notes /PRINT SELECTION CPS.",
        parse: complex_samples::csselect,
    },
    CommandSpec {
        name: "CSDESCRIPTIVES",
        summary: "Means, sums and ratios of a complex sample with design-based standard errors and design effects.",
        syntax: "CSDESCRIPTIVES /PLAN FILE='file' [/SUMMARY VARIABLES=vars]\n\
                 \x20  [/MEAN [TTEST=value | values]] [/SUM [TTEST=value | values]]\n\
                 \x20  [/RATIO NUMERATOR=vars DENOMINATOR=vars [TTEST=value | values]] ...\n\
                 \x20  [/STATISTICS [COUNT] [POPSIZE] [SE] [CV] [DEFF] [DEFFSQRT] [CIN[(95)]]]\n\
                 \x20  [/SUBPOP TABLE=var [BY var ...] [DISPLAY=LAYERED | SEPARATE]]\n\
                 \x20  [/MISSING [SCOPE=ANALYSIS | LISTWISE] [CLASSMISSING=EXCLUDE | INCLUDE]]\n\
                 \x20  [/JOINTPROB FILE='file' | dataset].\n\
                 The design comes from a plan written by CSPLAN; WEIGHT and SPLIT FILE do not apply. A case\n\
                 with a missing or non-positive weight, or a missing stratum, cluster, population size or\n\
                 inclusion probability, leaves the design. Totals are weighted sums; a mean is the total over\n\
                 the summed weights and a ratio is one total over another, and their variances come from\n\
                 Taylor linearization. The variance adds, stratum by stratum, the spread of the cluster\n\
                 totals, times the finite population correction under EQUAL_WOR, plus the variance of later\n\
                 stages. A stratum with one cluster adds nothing. Intervals and t tests use t with the number\n\
                 of first-stage clusters minus the number of first-stage strata as df. The design effect\n\
                 divides the variance by the one simple random sampling would give (see CSPLAN SRSESTIMATOR).\n\
                 Each RATIO crosses its numerators with its denominators. Subpopulations are domains: their\n\
                 estimates use the whole design, with cases outside counting as zero; a Total row follows\n\
                 them. Under SCOPE=ANALYSIS each statistic uses the cases valid on its own variables, under\n\
                 LISTWISE those valid on all of them; the others count as outside, so the design stays whole.\n\
                 Without STATISTICS: the estimate and its standard error. Under UNEQUAL_WOR a first-stage\n\
                 stratum adds, over each pair of its units, (pi_a pi_b / pi_ab - 1) times the squared\n\
                 difference of their totals (Yates and Grundy), with the inclusion and joint probabilities\n\
                 read from JOINTPROB FILE, or by default the file CSSELECT wrote next to the plan; a later\n\
                 stage counts times the unit's inclusion probability. A negative variance is left empty.",
        example: "CSDESCRIPTIVES /PLAN FILE='farms_plan.json' /SUMMARY VARIABLES=yield acres /MEAN /RATIO NUMERATOR=yield DENOMINATOR=acres /STATISTICS SE CIN DEFF /SUBPOP TABLE=organic.",
        parse: complex_samples::csdescriptives,
    },
    CommandSpec {
        name: "CSTABULATE",
        summary: "One-way and two-way tables of a complex sample with design-based errors and adjusted tests.",
        syntax: "CSTABULATE /PLAN FILE='file' /TABLES VARIABLES=vars [BY var]\n\
                 \x20  [/CELLS [POPSIZE] [ROWPCT] [COLPCT] [TABLEPCT]]\n\
                 \x20  [/STATISTICS [SE] [CV] [DEFF] [DEFFSQRT] [CIN[(95)]] [COUNT] [CUMULATIVE]\n\
                 \x20              [EXPECTED] [RESID] [ASRESID]]\n\
                 \x20  [/TEST [HOMOGENEITY] [INDEPENDENCE] [ODDSRATIO] [RELRISK] [RISKDIFF]]\n\
                 \x20  [/SUBPOP TABLE=var [BY var ...] [DISPLAY=LAYERED | SEPARATE]]\n\
                 \x20  [/MISSING [SCOPE=TABLE | LISTWISE] [CLASSMISSING=EXCLUDE | INCLUDE]]\n\
                 \x20  [/JOINTPROB FILE='file' | dataset].\n\
                 Each variable gets a one-way table, or with BY a two-way table against the BY variable, with\n\
                 estimated population sizes and percents for every cell and margin. The design comes from a\n\
                 CSPLAN plan, as in CSDESCRIPTIVES, and so do the standard errors, df and design effects.\n\
                 Percents are ratios of population sizes; their intervals are found on the logit scale.\n\
                 CUMULATIVE (one-way) adds running sums. EXPECTED, RESID and ASRESID (two-way, with\n\
                 TABLEPCT) show the table percent expected under independence, the difference from it and\n\
                 that difference over its design-based standard error. HOMOGENEITY tests equal shares of a\n\
                 one-way table, INDEPENDENCE the rows against the columns: the Pearson and likelihood ratio\n\
                 chi-squares are divided by the sum of the generalized design effects and referred to F\n\
                 with d and d times the design df, d being the squared sum over the sum of squares of those\n\
                 effects (a second-order adjustment). ODDSRATIO, RELRISK and RISKDIFF give the 2 x 2\n\
                 measures with intervals (log scale for the first two). Tests are for the whole population,\n\
                 and also for each subpopulation when SUBPOP names only first-stage strata. Without CELLS:\n\
                 population sizes; without STATISTICS: standard errors. JOINTPROB works as in\n\
                 CSDESCRIPTIVES.",
        example: "CSTABULATE /PLAN FILE='farms_plan.json' /TABLES VARIABLES=irrigated BY organic /CELLS POPSIZE ROWPCT /STATISTICS SE CIN DEFF /TEST INDEPENDENCE ODDSRATIO.",
        parse: complex_samples::cstabulate,
    },
    CommandSpec {
        name: "CSCOXREG",
        summary: "Cox proportional hazards regression on a complex sample, with design-based standard errors and tests.",
        syntax: "CSCOXREG [start] end [BY factors] [WITH covariates]\n\
                 \x20  /VARIABLES STATUS=var(values) [ID=var] [BASELINESTRATA=var] /PLAN FILE='file'\n\
                 \x20  [/MODEL effects]\n\
                 \x20  [/CUSTOM [LABEL='text'] LMATRIX=effect coefficients ... [; ...] [KMATRIX=k [; k ...]]]\n\
                 \x20  [/CRITERIA [CILEVEL=95] [DF=n] [LCONVERGE=[0 RELATIVE | ABSOLUTE]] [MXITER=100]\n\
                 \x20             [MXSTEP=5] [PCONVERGE=[1E-6 RELATIVE | ABSOLUTE]] [SINGULAR=1E-12]\n\
                 \x20             [TIES=EFRON | BRESLOW]]\n\
                 \x20  [/STATISTICS [PARAMETER] [EXP] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]\n\
                 \x20  [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]\n\
                 \x20         [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]\n\
                 \x20  [/TESTASSUMPTIONS [PROPHAZARD=KM | IDENTITY | LOG | RANK] [PARAMETER] [COVB]]\n\
                 \x20  [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [SAMPLEINFO] [EVENTINFO] [RISKINFO] [HISTORY[(n)]] [GEF] [LMATRIX] [COVB]\n\
                 \x20          [CORB] [BASELINE] [NONE]]\n\
                 \x20  [/SURVIVALMETHOD [BASELINE=EFRON | BRESLOW | PRODUCTLIMIT] [CI=ORIGINAL | LOG | LML]]\n\
                 \x20  [/SAVE keyword[(name)] ... | keyword[(root[:n])] ...]\n\
                 \x20  [/PLOT [SURVIVAL] [CUMHAZARD] [LML] [OMS] [CI=NO | YES]]\n\
                 \x20  [/PATTERN [end(times)] var(value or values) ... [BY factor]] [/PATTERN ...]\n\
                 \x20  [/OUTFILE [COVB='file' | CORB='file'] [MODEL='file'] [SURVIVAL='file']]\n\
                 \x20  [/JOINTPROB FILE='file' | dataset].\n\
                 A record is at risk after its start time up to and including its end time; with one time\n\
                 variable it is at risk from the beginning. A status among the event values means the\n\
                 event happened at the end time; any other valid status is censored. A negative time, or a\n\
                 start time not below the end time, leaves the record out. The hazard is h0(t) exp(x'b)\n\
                 without an intercept, which the baseline hazard h0 absorbs, so the last level of each\n\
                 factor is the reference. BASELINESTRATA gives each of its values its own baseline hazard\n\
                 while the coefficients are shared. The estimates maximize the weighted partial\n\
                 likelihood, tied event times treated by Efron's method (TIES=BRESLOW: Breslow's), by\n\
                 Newton-Raphson with step-halving from b = 0. The covariance is a sandwich: the inverse\n\
                 information on both sides of the design covariance of the total of the records' score\n\
                 residuals. Design, df, domains, TEST, CUSTOM and design effects work as in CSGLM. The\n\
                 default output is the sample design information with the event and censored counts, the\n\
                 events and censored records of each baseline stratum, the -2 log pseudo likelihood and\n\
                 the Tests of Model Effects; the parameter table needs STATISTICS. RISKINFO lists, per\n\
                 event time, the events and the records at risk. PRINT GEF shows the general\n\
                 estimable function: one column per nonredundant parameter.\n\
                 ID names the subject of each record. The records of a subject are summed before the\n\
                 design variance, and where the plan's last stage has no clusters the subjects are its\n\
                 units. With ID and one time variable, a subject's first record starts at the\n\
                 beginning and each later one where the previous record (in time) ended.\n\
                 Factors and covariates may come from a TIME PROGRAM: each record is then split at the\n\
                 event times of its baseline stratum and the program runs on every piece with T_ set to\n\
                 the piece's event time. With such covariates there are no baseline functions, plots,\n\
                 PATTERN or SURVIVAL file, and SAVE writes only SCHOENFELD, SCORE, DFBETA, their\n\
                 aggregated forms and XBETA. TESTASSUMPTIONS fits a larger model in which every\n\
                 predictor also enters multiplied by g(t), one minus the weighted Kaplan-Meier survival\n\
                 of all records (KM, the default), t, ln(t) or the rank of the event time plus one, and\n\
                 gives the Wald test (as TEST says) that those extra coefficients are 0; PARAMETER and\n\
                 COVB show the larger model's estimates, standard errors and covariances.\n\
                 Baseline functions are computed per baseline stratum at the reference values (each\n\
                 factor at its last level, each covariate at its weighted mean): the cumulative hazard\n\
                 by Efron's or Breslow's sum (as TIES unless SURVIVALMETHOD says otherwise) or minus the\n\
                 log of the product-limit survival. Their standard errors are design-based, from the\n\
                 design variance of the total of each record's influence on the cumulative hazard, the\n\
                 same for all three estimates. The survival interval uses the normal quantile on the\n\
                 scale CI names: the survival itself, its log (default) or the log of minus its log,\n\
                 cut to 0 and 1. PRINT BASELINE tabulates them; PLOT draws the step functions for the\n\
                 reference values and for each PATTERN line (BY gives one line per factor level;\n\
                 variables a PATTERN leaves out stay at the reference). SAVE writes, for the records\n\
                 analyzed: SCHOENFELD, SCORE and DFBETA (one variable per parameter, root_1, root_2 ...,\n\
                 25 at most unless :n says), MARTINGALE, DEVIANCE, COXSNELL, XBETA, and SURVIVAL,\n\
                 CUMHAZARD and their LCL_ and UCL_ limits at each record's own end time and values.\n\
                 Default names: Resid_Schoenfeld, Resid_Score, Resid_DFBETA, Resid_Martingale,\n\
                 Resid_Deviance, Resid_CoxSnell, XBETA, Survival, CumHazard, LCL_Survival and so on; a\n\
                 taken default name gets a suffix. AGGMARTINGALE, AGGDEVIANCE, AGGCOXSNELL, AGGSCORE\n\
                 and AGGDFBETA (default names AggResid_...) total a subject's records and are written on\n\
                 its last record; the deviance comes from the summed martingale residual and events.\n\
                 Without ID they are skipped with a warning. A PATTERN that gives the end time a list\n\
                 of ascending times is a path: each other variable takes one value, or one per time,\n\
                 held up to that time, and the curve stops at the last time. OUTFILE COVB or CORB writes\n\
                 the matrix file of CSGLM, MODEL a JSON file with the estimates, covariances, reference\n\
                 values and baseline functions, SURVIVAL a data file of the curves of PLOT at every event\n\
                 time. JOINTPROB works as in CSDESCRIPTIVES.",
        example: "CSCOXREG months BY clinic WITH dose /VARIABLES STATUS=relapse(1) /PLAN FILE='trial_plan.json' /STATISTICS PARAMETER EXP SE CINTERVAL /PRINT RISKINFO.",
        parse: complex_samples::cscoxreg,
    },
    CommandSpec {
        name: "CSGLM",
        summary: "Linear regression, ANOVA and ANCOVA on a complex sample, with design-based standard errors and tests.",
        syntax: "CSGLM dependent [BY factors] [WITH covariates] /PLAN FILE='file'\n\
                 \x20  [/MODEL effects] [/INTERCEPT [INCLUDE=YES | NO | ONLY] [SHOW=YES | NO]]\n\
                 \x20  [/CUSTOM [LABEL='text'] [LMATRIX=[c] effect values ... | ALL values; ...] [KMATRIX=k; ...]] ...\n\
                 \x20  [/EMMEANS [TABLES=factor[*factor ...]] [OTHER=[covariate(value | MEAN) ...]]\n\
                 \x20           [COMPARE=factor] [CONTRAST=SIMPLE[(value)] | DEVIATION[(value)] | DIFFERENCE |\n\
                 \x20                                   HELMERT | REPEATED | POLYNOMIAL[(metric)]]] ...\n\
                 \x20  [/CRITERIA [CILEVEL=95] [DF=n] [SINGULAR=1E-12]]\n\
                 \x20  [/STATISTICS [PARAMETER] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]\n\
                 \x20  [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]\n\
                 \x20         [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]\n\
                 \x20  [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [SAMPLEINFO] [VARIABLEINFO] [SUMMARY] [GEF] [LMATRIX] [COVB] [CORB] [NONE]]\n\
                 \x20  [/SAVE [PRED[(name)]] [RESID[(name)]]]\n\
                 \x20  [/OUTFILE [COVB='file' | CORB='file'] [MODEL='file' | PARAMETER='file']].\n\
                 The plan written by CSPLAN gives the weight and the design; WEIGHT and SPLIT FILE do not\n\
                 apply. The coefficients solve the weighted normal equations, with one parameter per level of\n\
                 each factor effect; a parameter that depends on earlier ones is redundant and set to 0. The\n\
                 covariance of the coefficients is a sandwich: the inverse of X'WX on both sides of the design\n\
                 covariance of the total of x times the residual, found as in CSDESCRIPTIVES. Every df is the\n\
                 number of first-stage clusters minus the number of first-stage strata, unless CRITERIA DF\n\
                 sets it. Without MODEL the model has the intercept, the factors and the covariates. Tests of\n\
                 Model Effects use Type III hypotheses: TYPE=F gives the Wald F, (df - r + 1) / (r df) times\n\
                 the Wald chi-square, on r and df - r + 1 df; ADJF divides the chi-square of the simple random\n\
                 sampling covariance by the sum of the generalized design effects, on d and d times df (d as\n\
                 in CSTABULATE); CHISQUARE and ADJCHISQUARE give the two chi-squares. CUSTOM tests L b = K\n\
                 row by row and as a whole; EMMEANS gives the means of the factor cells averaged with equal\n\
                 weights, covariates at their weighted means unless OTHER sets them, and COMPARE tests the\n\
                 contrasts of one factor. PADJUST adjusts the p values of the single rows. DOMAIN analyzes\n\
                 the cases with the value, keeping the whole design; cases with a missing value on a model\n\
                 variable count as outside it. R Square compares the weighted residual sum of squares with\n\
                 the weighted total sum of squares. Without STATISTICS the estimates come with SE, interval\n\
                 and t test. OUTFILE COVB writes, to a data file or a dataset made by DATASET DECLARE, the\n\
                 variables RowType_ and VarName_ and P1, P2, ... (one per row of Parameter Estimates,\n\
                 labelled with its name): rows EST, SE, SIG (of the t test) and DF, then a COV row per\n\
                 parameter; CORB puts CORR rows of correlations there instead. MODEL writes the estimates\n\
                 and their covariance matrix to a JSON file of Statscraft's own, PARAMETER the estimates\n\
                 alone. JOINTPROB works as in CSDESCRIPTIVES. PRINT GEF writes every estimable function\n\
                 as a combination of free coefficients: a row per parameter and a column per free\n\
                 coefficient L1, L2, ... (one per parameter that is not redundant); a redundant parameter's\n\
                 row says how its coefficient follows from the free ones. Any L that gives every parameter\n\
                 its row's combination of the free coefficients is estimable, and every estimable L has\n\
                 that form.",
        example: "CSGLM yield BY soil WITH rainfall /PLAN FILE='farms_plan.json' /STATISTICS PARAMETER SE CINTERVAL DEFF /EMMEANS TABLES=soil COMPARE=soil CONTRAST=REPEATED.",
        parse: complex_samples::csglm,
    },
    CommandSpec {
        name: "CSLOGISTIC",
        summary: "Binary and multinomial logistic regression on a complex sample, with design-based standard errors and tests.",
        syntax: "CSLOGISTIC dependent [(LOW | HIGH | value)] [BY factors] [WITH covariates] /PLAN FILE='file'\n\
                 \x20  [/MODEL effects] [/INTERCEPT [INCLUDE=YES | NO | ONLY] [SHOW=YES | NO]]\n\
                 \x20  [/CUSTOM [LABEL='text'] [LMATRIX=[c] effect values ... | ALL values; ...] [KMATRIX=k; ...]] ...\n\
                 \x20  [/ODDSRATIOS FACTOR=[factor[(LOW | HIGH | value)] ...] | COVARIATE=[covariate[(changes)] ...]\n\
                 \x20               [CONTROL=[factor(LOW | HIGH | value) covariate(MEAN | value) ...]]] ...\n\
                 \x20  [/CRITERIA [CHKSEP=20] [CILEVEL=95] [DF=n] [LCONVERGE=[0 RELATIVE | ABSOLUTE]] [MXITER=100]\n\
                 \x20             [MXSTEP=5] [PCONVERGE=[1E-6 RELATIVE | ABSOLUTE]] [SINGULAR=1E-12]]\n\
                 \x20  [/STATISTICS [PARAMETER] [EXP] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]\n\
                 \x20  [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]\n\
                 \x20         [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]\n\
                 \x20  [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [SAMPLEINFO] [VARIABLEINFO] [SUMMARY] [HISTORY[(n)]] [GEF] [LMATRIX] [COVB]\n\
                 \x20          [CORB] [CLASSTABLE] [NONE]]\n\
                 \x20  [/SAVE [PREDPROB[(root[:25])]] [PREDVAL[(name)]]]\n\
                 \x20  [/OUTFILE [COVB='file' | CORB='file'] [MODEL='file' | PARAMETER='file']].\n\
                 Each category of the dependent variable but the reference one (the highest unless the\n\
                 parentheses say otherwise) gets a logit against the reference with its own set of\n\
                 parameters. The estimates maximize the weighted log likelihood (pseudo-likelihood) by\n\
                 Newton-Raphson with step-halving; they start at 0 with the intercepts at the log ratio of\n\
                 the weighted category counts, and stop when the parameters (PCONVERGE) or the log\n\
                 likelihood (LCONVERGE) change little or after MXITER steps. The covariance is a sandwich:\n\
                 the inverse information on both sides of the design covariance of the score total. Design,\n\
                 df, domains, CUSTOM, TEST and PADJUST work as in CSGLM; a CUSTOM row without ALL is tested\n\
                 for every logit, a row with ALL lists the parameters of all logits in turn. The model\n\
                 summary gives the pseudo R squares of Cox and Snell, Nagelkerke and McFadden from the\n\
                 weighted log likelihoods of the model and of the intercept-only start, with the summed\n\
                 weights as N. ODDSRATIOS compares each level of a factor with its reference, or a covariate\n\
                 moved by each change, with the other variables at CONTROL (factors at their highest level,\n\
                 covariates at their weighted means by default). CLASSTABLE puts each case in its most\n\
                 probable category and sums the weights. Without STATISTICS no parameter table is shown.\n\
                 OUTFILE works as in CSGLM, the parameters of every logit in turn. JOINTPROB works as in\n\
                 CSDESCRIPTIVES. PRINT GEF works as in CSGLM, with a separate set of free coefficients for\n\
                 every logit.",
        example: "CSLOGISTIC irrigated (LOW) BY soil WITH rainfall /PLAN FILE='farms_plan.json' /STATISTICS PARAMETER EXP SE CINTERVAL /ODDSRATIOS FACTOR=[soil(LOW)] /PRINT SUMMARY CLASSTABLE.",
        parse: complex_samples::cslogistic,
    },
    CommandSpec {
        name: "CSORDINAL",
        summary: "Ordinal regression (cumulative link model) on a complex sample, with design-based standard errors and tests.",
        syntax: "CSORDINAL dependent [(ASCENDING | DESCENDING)] [BY factors] [WITH covariates] /PLAN FILE='file'\n\
                 \x20  [/MODEL effects] [/LINK FUNCTION=LOGIT | PROBIT | CLOGLOG | NLOGLOG | CAUCHIT]\n\
                 \x20  [/CRITERIA [CHKSEP=20] [CILEVEL=95] [DF=n] [LCONVERGE=[0 RELATIVE | ABSOLUTE]]\n\
                 \x20             [METHOD=NEWTON | FISHER[(n)]] [MXITER=100] [MXSTEP=5]\n\
                 \x20             [PCONVERGE=[1E-6 RELATIVE | ABSOLUTE]] [SINGULAR=1E-12]]\n\
                 \x20  [/STATISTICS [PARAMETER] [EXP] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]\n\
                 \x20  [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]\n\
                 \x20         [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]\n\
                 \x20  [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [SAMPLEINFO] [VARIABLEINFO] [SUMMARY] [HISTORY[(n)]] [GEF] [LMATRIX] [COVB]\n\
                 \x20          [CORB] [CLASSTABLE] [NONE]]\n\
                 \x20  [/CUSTOM [LABEL='text'] LMATRIX=[thresholds] effect coefficients ... [; ...]\n\
                 \x20           | ALL coefficients [; ...] [KMATRIX=value [; value ...]]] ...\n\
                 \x20  [/ODDSRATIOS FACTOR=[factor(LOW | HIGH | value) ...] | COVARIATE=[covariate(changes) ...]\n\
                 \x20               [CONTROL=[factor(LOW | HIGH | value) covariate(MEAN | value) ...]]] ...\n\
                 \x20  [/NONPARALLEL [TEST] [PARAMETER] [COVB]]\n\
                 \x20  [/SAVE [CUMPROB[(root[:n])]] [PREDPROB[(root[:n])]] [PREDVAL[(name)]]\n\
                 \x20         [PREDVALPROB[(name)]] [OBSVALPROB[(name)]]]\n\
                 \x20  [/OUTFILE [COVB='file' | CORB='file'] [MODEL='file' | PARAMETER='file']].\n\
                 The categories of the dependent variable, sorted up (or down with DESCENDING), are ordered\n\
                 1 to K. For each k below K the model says link(P(Y <= k)) = threshold_k - x'b: one\n\
                 threshold per cut point and one set of coefficients b for all of them, so a positive\n\
                 coefficient moves cases toward the higher categories. LOGIT is the default link. The\n\
                 estimates maximize the weighted log likelihood by Newton-Raphson with step-halving;\n\
                 METHOD=FISHER(n) takes n Fisher scoring steps first (FISHER alone: until convergence),\n\
                 then Newton-Raphson. The start is b = 0 with each threshold at the link of the weighted\n\
                 cumulative share of its category. The covariance is a sandwich: the inverse observed\n\
                 information on both sides of the design covariance of the score total. Design, df,\n\
                 domains and TEST work as in CSGLM; the Tests of Model Effects test the coefficients of\n\
                 each effect (and all of them in the corrected model). The model summary gives the pseudo\n\
                 R squares of Cox and Snell, Nagelkerke and McFadden against the thresholds-only fit,\n\
                 with the summed weights as N. EXP needs the logit link. CLASSTABLE puts each case in its\n\
                 most probable category and sums the weights. Without STATISTICS no parameter table is\n\
                 shown. A CUSTOM row may open with one coefficient per threshold before its effects; ALL\n\
                 takes the thresholds first, then every design column but the intercept. Moving all\n\
                 thresholds together is the same as moving the intercept, so a row is tested only when its\n\
                 coefficients pass as estimable with minus their threshold sum on the intercept: a lone\n\
                 threshold is not testable once there are factors. ODDSRATIOS (logit link only) gives one\n\
                 cumulative odds ratio per comparison, exp(-b'(x1 - x2)), x1 the compared level or the\n\
                 covariate raised by the change; other variables sit at their CONTROL values (factors at\n\
                 the highest level, covariates at the mean by default). NONPARALLEL refits the model with\n\
                 a set of coefficients per threshold, starting from the estimates; TEST is the Wald test\n\
                 that each coefficient is the same for every threshold, PARAMETER and COVB show that\n\
                 model. SAVE writes cumulative and category probabilities, the most probable category and\n\
                 its probability, and the probability of the observed category; a domain case whose\n\
                 dependent is missing still gets predictions when every predictor has a parameter. Default\n\
                 names: CumulativeProbability_k, PredictedProbability_k, PredictedValue,\n\
                 PredictedValueProbability, ObservedValueProbability, with a suffix when taken. OUTFILE\n\
                 works as in CSGLM, thresholds first; the model file also names the link. JOINTPROB works\n\
                 as in CSDESCRIPTIVES. PRINT GEF works as in CSGLM with a free coefficient per threshold\n\
                 first: since moving every threshold moves the intercept, each threshold's coefficient\n\
                 comes with minus the intercept's share of the redundant columns.",
        example: "CSORDINAL rating BY region WITH age /PLAN FILE='poll_plan.json' /LINK FUNCTION=PROBIT /STATISTICS PARAMETER SE CINTERVAL TTEST /PRINT SUMMARY HISTORY.",
        parse: complex_samples::csordinal,
    },
    CommandSpec {
        name: "REPORT",
        summary: "Case listings and summary statistics by break groups, laid out as a report.",
        syntax: "REPORT [FORMAT=[MANUAL | AUTOMATIC] [NOLIST | LIST[(n)]] [ALIGN(LEFT | CENTER | RIGHT)] [TSPACE(1)]\n\
                 \x20       [CHDSPACE(1)] [FTSPACE(1)] [SUMSPACE(1)] [COLSPACE(4)] [BRKSPACE(n | -1)] [LENGTH(t,b)]\n\
                 \x20       [MARGINS(l,r)] [CHALIGN(TOP | BOTTOM)] [UNDERSCORE(ON | OFF)] [PAGE1(1)] [MISSING '.']\n\
                 \x20       [ONEBREAKCOLUMN ON | OFF] [INDENT(2)] [CHWRAP ON | OFF] [PREVIEW ON | OFF]]\n\
                 \x20  [/OUTFILE='path'] [/STRING=name (var [(width)] [(BLANK)] 'literal' ...) ...]\n\
                 \x20  /VARIABLES=var [(VALUE | LABEL | DUMMY)] [+ var ...] ['heading' ...] [(width)]\n\
                 \x20     [(OFFSET(n | CENTER))] [(LEFT | CENTER | RIGHT)] ...\n\
                 \x20  [/MISSING=VAR | NONE | LIST[([varlist] [n])]] [/TITLE=[LEFT | CENTER | RIGHT] 'line' ...]\n\
                 \x20  [/FOOTNOTE=[LEFT | CENTER | RIGHT] 'line' ...]\n\
                 \x20  [/BREAK=varlist | (TOTAL) ['heading' ...] [(width)] [(VALUE | LABEL)] [(TOTAL | NOTOTAL)]\n\
                 \x20     [(SKIP(1))] [(PAGE)] [(OFFSET(n | CENTER))] [(UNDERSCORE[(varlist)])] [(LEFT | CENTER | RIGHT)]\n\
                 \x20     [(NAME | NONAME)] [/SUMMARY=function ... ['title'] [(break column)] [SKIP(n)] | PREVIOUS[(n)]] ...] ...\n\
                 functions: VALIDN SUM MIN MAX MEAN STDDEV (SD, STDEV) VARIANCE KURTOSIS SKEWNESS\n\
                 MEDIAN(min,max) MODE(min,max) PGT(n) PLT(n) PIN(min,max) FREQUENCY(min,max) PERCENT(min,max),\n\
                 each with an optional (var [(format | PLAIN)] [(decimals)] ...); composite functions\n\
                 DIVIDE(a b [factor]) MULTIPLY PCT(a b) SUBTRACT(a b) ADD GREAT LEAST AVERAGE of aggregates\n\
                 such as SUM(var) and numbers, with an optional (column [(format)] [(decimals)]).\n\
                 The report is a table: a column for each BREAK (with ONEBREAKCOLUMN ON one for all, indented\n\
                 by INDENT), then one for each report column, headed by its heading or the variable label or\n\
                 name. LIST lists the cases, each value in its print format or with LABEL as its value label;\n\
                 a stacked column lists each of its variables on a line of its own. A break group starts\n\
                 when a value of its BREAK or an earlier one changes (sort the cases first): its values head\n\
                 it, and its SUMMARY lines follow with their title (by default the function's) in the break\n\
                 column. The statistics use the valid values and the weight; MISSING=NONE counts user-missing\n\
                 values as valid, and MISSING=LIST leaves out the cases missing on n of the variables. MEDIAN,\n\
                 MODE, FREQUENCY and PERCENT count the whole numbers of the range, cutting decimals off.\n\
                 TOTAL on the first BREAK repeats its first SUMMARY for all cases at the end, under Grand\n\
                 Total. Formats follow the manual's defaults for each function. TITLE and FOOTNOTE lines (by\n\
                 default the text of the TITLE command) show above and below the report; )PAGE, )DATE and\n\
                 )var in them give the page number, the date and the value label of the first case. OUTFILE\n\
                 writes the report as text, with the column widths, spacing and alignment, instead of\n\
                 showing it; a later REPORT of the same session appends to the file. The report is one\n\
                 page, so LENGTH, PAGE and PREVIEW change nothing.",
        example: "SORT CASES BY region.\nREPORT FORMAT=LIST /VARIABLES=shop (LABEL) visitors 'Weekly' 'visitors'\n  /BREAK=region 'Region' (LABEL) /SUMMARY=VALIDN(shop) MEAN(visitors).",
        parse: report::report,
    },
    CommandSpec {
        name: "ADD FILES",
        summary: "Append the cases of several data files, matching variables by name.",
        syntax: "ADD FILES /FILE=* | 'path' | dataset [/RENAME=(old = new) ...] [/IN=var]\n\
                 \x20         /FILE=... [/RENAME=...] [/IN=var] [/BY varlist] [/FIRST=var] [/LAST=var]\n\
                 \x20         [/MAP] [/KEEP=varlist] [/DROP=varlist].\n\
                 FILE=* is the active dataset; a dataset name (DATASET NAME), quoted or not, is that open\n\
                 dataset and comes before a file of the same name. A variable missing from a file is blank in its cases.\n\
                 The first file that has a variable gives its format and measurement level; a later file\n\
                 fills in a label, value labels or missing values the earlier ones lack. A string variable\n\
                 takes its widest width; a variable that is numeric in one file and a string in another is\n\
                 an error. BY interleaves files sorted by the key variables.\n\
                 RENAME and IN follow their FILE; BY, FIRST, LAST, MAP, KEEP and DROP follow every FILE.\n\
                 IN adds a variable that is 1 for the cases from that file, else 0. FIRST and LAST (with\n\
                 BY) add one that is 1 for the first or the last case of each key. These are F1.0 and go\n\
                 at the end, IN first; KEEP and DROP cannot name them. MAP lists the variables of the\n\
                 result at that point with their names in each file.\n\
                 The result has the documents of every file. It stays weighted only when every file is\n\
                 weighted by the same variable; otherwise a warning says so.",
        example: "ADD FILES /FILE=* /FILE='wave2.sav' /RENAME=(q1_new = q1) /DROP=tmp.",
        parse: files::add_files,
    },
    CommandSpec {
        name: "MATCH FILES",
        summary: "Combine the variables of several data files case by case or by key.",
        syntax: "MATCH FILES /FILE=* | 'path' | dataset [/RENAME=(old = new) ...] [/IN=var]\n\
                 \x20           [/TABLE='path' | dataset | * [/RENAME=...] [/IN=var] ...]\n\
                 \x20           /FILE=... [/BY varlist] [/FIRST=var] [/LAST=var]\n\
                 \x20           [/MAP] [/KEEP=varlist] [/DROP=varlist].\n\
                 A dataset name (DATASET NAME), quoted or not, is that open dataset and comes before a file\n\
                 of the same name.\n\
                 With BY, cases with equal keys are joined; every file must be sorted by the keys (use\n\
                 SORT CASES). Without BY the n-th cases of the files are joined. A TABLE adds its variables\n\
                 to the cases whose key it holds and adds no cases; it may hold each key once. A variable in\n\
                 several files takes its value from the first file that has the case.\n\
                 RENAME and IN follow their FILE or TABLE; BY, FIRST, LAST, MAP, KEEP and DROP follow\n\
                 every FILE and TABLE. IN (with BY) adds a variable that is 1 for the cases that file or\n\
                 table has, else 0. FIRST and LAST (with BY) add one that is 1 for the first or the last\n\
                 case of each key. These are F1.0 and go at the end, IN first; KEEP and DROP cannot name\n\
                 them. MAP lists the variables of the result at that point with their names in each file.\n\
                 The result has the documents of every file and keeps the weight when every weighted file\n\
                 uses the same variable and KEEP or DROP leave it; otherwise a warning says so.",
        example: "MATCH FILES /FILE=* /TABLE='regions.sav' /BY region.",
        parse: files::match_files,
    },
    CommandSpec {
        name: "STAR JOIN",
        summary: "Add the fields of lookup files to every case of a data file, by keys, as an SQL left join.",
        syntax: "STAR JOIN /SELECT * | alias.field [AS name], alias.field [AS name], ...\n\
                 \x20         /FROM 'path' [PASSWORD='text'] | dataset | * AS alias\n\
                 \x20         /JOIN 'path' [PASSWORD='text'] | dataset | * AS alias\n\
                 \x20               ON alias.key = alias.key [AND alias.key = alias.key ...] [IN=name]\n\
                 \x20         [/JOIN ...] /OUTFILE FILE='path' | dataset | *\n\
                 \x20         [/PASSPROTECT PASSWORD='text' | NONE [ENCRYPTEDPW=NO | YES]].\n\
                 The subcommands come in this order and every keyword is written in full. A dataset\n\
                 name, quoted or not, is that open dataset and comes before a file of the same name.\n\
                 Every case of FROM is kept, in its order; a JOIN adds its fields to the cases whose keys\n\
                 it holds and leaves them blank (system-missing, empty) elsewhere. The files need no\n\
                 sorting, and each JOIN has its own keys. ON pairs a field of FROM with one of that JOIN,\n\
                 numeric with numeric, string with string (widths may differ; trailing blanks do not\n\
                 count). The keys of a JOIN must be unique there. A system-missing key matches nothing.\n\
                 IN=name adds a variable that is 1 where the JOIN had the case and 0 where not.\n\
                 The result holds the SELECT fields and every key: SELECT * is every field of every\n\
                 source, and a JOIN key of the same name as its FROM key appears once, from FROM. Keys\n\
                 not named on SELECT come first; the rest follow in SELECT order; IN variables last.\n\
                 A name that two sources bring gets its alias in front (t1.name, t2.name) unless AS\n\
                 names it. The dictionary comes from FROM (with its weight, filter and split); a key\n\
                 without labels or missing values takes them from the first JOIN that has them; the\n\
                 documents of every file are kept; the file label is dropped. Not after TEMPORARY.\n\
                 OUTFILE=* replaces the active dataset, a dataset name (DATASET DECLARE) fills that\n\
                 dataset, a path writes a data file. PASSWORD on FROM and JOIN is accepted for files that\n\
                 are not encrypted (encrypted ones cannot be read); PASSPROTECT with a password is\n\
                 refused for a file, since Statscraft writes no encrypted files, and ignored otherwise.",
        example: "STAR JOIN /SELECT t1.id, t1.region, t2.name /FROM * AS t1\n\
                  \x20 /JOIN 'regions.sav' AS t2 ON t1.region = t2.region /OUTFILE FILE=*.",
        parse: files::star_join,
    },
    CommandSpec {
        name: "UPDATE",
        summary: "Replace values of a primary file with the values of transaction files, by key.",
        syntax: "UPDATE FILE=* | 'path' | dataset [/RENAME=(old = new) ...] [/IN=var]\n\
                 \x20      /FILE=... [/RENAME=...] [/IN=var] [/FILE=...] /BY varlist\n\
                 \x20      [/MAP] [/KEEP=varlist] [/DROP=varlist].\n\
                 The first FILE is the primary file, which may hold each key once; the others are\n\
                 transactions. Every file must be sorted by the keys. A transaction value replaces the value\n\
                 of the case with its key unless it is system-missing, user-missing or blank; transactions\n\
                 apply in order, case by case and file by file. A key only a transaction has adds a case, a\n\
                 variable only a transaction has adds a variable. Dictionaries combine as in MATCH FILES.\n\
                 IN adds a variable at the end, 1 for the cases that file holds, else 0 (F1.0); KEEP and\n\
                 DROP cannot name it. MAP lists the variables at that point with their names in each file.\n\
                 There is no FIRST or LAST, since every key gives one case. The weight and the documents\n\
                 follow MATCH FILES. FILE=* cannot follow TEMPORARY.",
        example: "UPDATE FILE='customers.sav' /FILE='changes.sav' /IN=changed /BY id.",
        parse: files::update,
    },
    CommandSpec {
        name: "COMPARE DATASETS",
        summary: "Compare the values and dictionary of the active dataset with another dataset or .sav file.",
        syntax: "COMPARE DATASETS /COMPDATASET 'file.sav' [PASSWORD='text'] | dataset\n\
                 \x20 /VARIABLES varlist | ALL [/CASEID varlist]\n\
                 \x20 [/SAVE [FLAGMISMATCHES=YES | NO] [VARNAME=name]\n\
                 \x20        [MATCHDATASET=NO | YES] [MATCHNAME=dataset | 'file' [MATCHPASS='text' | NONE]]\n\
                 \x20        [MISMATCHDATASET=NO | YES] [MISMATCHNAME=dataset | 'file' [MISMATCHPASS=...]]\n\
                 \x20        [ENCRYPTEDPW=NO | YES]]\n\
                 \x20 [/OUTPUT [VARPROPERTIES=NONE | ALL | MEASURE LABEL VALUELABELS COLUMNS MISSING ALIGN ROLE\n\
                 \x20                         ATTRIBUTES WIDTH] [CASETABLE=YES | NO [TABLELIMIT=100 | n | NONE]]].\n\
                 Every subcommand and keyword is written in full. Without CASEID the first case of each file is\n\
                 paired with the first of the other, and so on; with CASEID cases pair by their key values and\n\
                 both files must be sorted by the keys in ascending order (a key held twice pairs in file order).\n\
                 Each listed variable that the other file also has, with the same type, is compared value by\n\
                 value: numbers exactly, system-missing equal only to system-missing, strings without trailing\n\
                 blanks. A variable the other file lacks or holds with the other type is listed, not compared.\n\
                 The tables: a summary of cases and variables, the variables not compared, the number of cases\n\
                 that differ per variable, and the case table (one row per differing value, the first 100\n\
                 cases by default). The flag variable, CasesCompare unless VARNAME names another, is added to\n\
                 the active dataset unless FLAGMISMATCHES=NO: 0 the paired case has the same values, 1 it\n\
                 differs, -1 the other file has no such case; an existing numeric variable of that name is\n\
                 overwritten. MATCHDATASET and MISMATCHDATASET write the active cases flagged 0, or flagged 1\n\
                 or -1, to a dataset declared before or a .sav file; encrypted files cannot be written.\n\
                 VARPROPERTIES lists every requested property that differs between the files. SPLIT FILE and\n\
                 FILTER are ignored.",
        example: "COMPARE DATASETS /COMPDATASET 'wave1_checked.sav' /VARIABLES q1 TO q12 /CASEID respid\n\
                  \x20 /SAVE VARNAME=changed MISMATCHDATASET=YES MISMATCHNAME=review\n\
                  \x20 /OUTPUT VARPROPERTIES=LABEL VALUELABELS TABLELIMIT=50.",
        parse: datacheck::compare_datasets,
    },
    CommandSpec {
        name: "DEFINE",
        summary: "Define a macro: a name that stands for syntax, with arguments.",
        syntax: "DEFINE name ([argument [/ argument ...]])\n\
                 body\n\
                 !ENDDEFINE.\n\
                 argument: !POSITIONAL or keyword =, then one of !TOKENS(n), !CHAREND('c'),\n\
                 !ENCLOSE('a', 'b'), !CMDEND, optionally with !DEFAULT(value) and !NOEXPAND (the value is\n\
                 not scanned for macro calls when the expansion is).\n\
                 DEFINE and !ENDDEFINE are written in full; the name may start with !.\n\
                 A call is the macro name followed by its arguments, anywhere in a command outside quotes:\n\
                 positional values first, in order, then keyword = value in any order. !TOKENS takes the\n\
                 next n tokens, !CHAREND the tokens up to the character, !ENCLOSE the tokens between the two\n\
                 characters, !CMDEND the rest of the command; the end of the command ends every argument,\n\
                 and an argument left out gets its default or nothing. Tokens no argument takes stay in place.\n\
                 In the body !1, !2 ... are the positional arguments, !* all of them, !name a keyword one.\n\
                 Outside quotes the body is read in upper case.\n\
                 Statements: !LET !var = expression; !IF (expression) !THEN ... [!ELSE ...] !IFEND;\n\
                 !DO !var = from !TO to [!BY step] ... !DOEND; !DO !var !IN (list) ... !DOEND; !BREAK;\n\
                 !OFFEXPAND and !ONEXPAND to stop and resume expanding macro calls.\n\
                 Functions: !LENGTH(s) !CONCAT(s, ...) !SUBSTR(s, from[, length]) !INDEX(s, part) !HEAD(s)\n\
                 !TAIL(s) !QUOTE(s) !UNQUOTE(s) !UPCASE(s) !BLANKS(n) !NULL !EVAL(s).\n\
                 Expressions: = <> ~= < > <= >= (or !EQ !NE !LT !GT !LE !GE), !AND !OR !NOT (or & | ~),\n\
                 parentheses. Operands are unquoted and compared as numbers when both are numbers, else as\n\
                 text with case; the result is 1 or 0, and 0 is false.\n\
                 The expansion is scanned again for macro calls, up to SET MNEST levels (50, at most 100); a\n\
                 !DO loop runs at most SET MITERATE times (1000). One expansion makes at most 100 000 calls\n\
                 and 1 MiB of text. SET MEXPAND=OFF stops expansion and\n\
                 SET MPRINT=YES shows every expanded command. A session (command line or window) keeps\n\
                 the macros and these settings from run to run; DISPLAY MACROS lists the macros.\n\
                 A body that ends with a period ends the command when the call is the last thing in it;\n\
                 in the middle of a command that period is dropped.",
        example: "DEFINE !freqs (vars = !CMDEND) FREQUENCIES VARIABLES=!vars. !ENDDEFINE.",
        parse: session_commands::define,
    },
    CommandSpec {
        name: "INSERT",
        summary: "Run the commands of another syntax file in place of this command.",
        syntax: "INSERT FILE='path' | handle [SYNTAX=INTERACTIVE | BATCH] [ERROR=CONTINUE | STOP] [CD=NO | YES]\n\
                 \x20      [ENCODING='name'] [PASSWORD='text'].\n\
                 A relative path starts from the folder of the file that holds the INSERT, or from the\n\
                 folder CD named. BATCH: a line that starts in the first column starts a new command, and\n\
                 + - or . there is ignored. CD=YES makes the folder of the file the folder for later\n\
                 relative paths. ENCODING names the encoding of the file, such as 'windows-1257' or '1252';\n\
                 by default UTF-8 when the file is valid UTF-8, else the system's legacy code page.\n\
                 Files nest up to five levels; a file may not insert itself. ERROR and PASSWORD are\n\
                 accepted and have no effect: a run stops at its first error.",
        example: "INSERT FILE='macros/tables.sps' CD=YES.",
        parse: session_commands::insert,
    },
    CommandSpec {
        name: "INCLUDE",
        summary: "Run the commands of another syntax file, read by the batch rules.",
        syntax: "INCLUDE [FILE=]'path' | handle [ENCODING='name'] [PASSWORD='text'].\n\
                 Like INSERT SYNTAX=BATCH: a line that starts in the first column starts a new command.",
        example: "INCLUDE FILE='setup.sps'.",
        parse: session_commands::include,
    },
    CommandSpec {
        name: "BEGIN PROGRAM",
        summary: "A block of code in another programming language; it is skipped with a warning.",
        syntax: "BEGIN PROGRAM [language].\n\
                 code\n\
                 END PROGRAM.\n\
                 Statscraft runs no Python or R code. The block is read up to END PROGRAM and not run;\n\
                 the run goes on with one warning per block. Files, variables or commands the block would\n\
                 make are missing, so later commands that need them fail.",
        example: "BEGIN PROGRAM PYTHON3.\nprint('not run')\nEND PROGRAM.",
        parse: session_commands::begin_program,
    },
    CommandSpec {
        name: "SET",
        summary: "Change settings of the session.",
        syntax: "SET subcommand[=]value [[/] subcommand[=]value ...].\n\
                 Followed: FORMAT=Fw.d (format of new numeric variables, default F8.2), DECIMAL=DOT | COMMA\n\
                 (decimal mark when reading text data), MPRINT, MEXPAND, MNEST, MITERATE (see DEFINE),\n\
                 LOCALE='name' (sets DECIMAL to the decimal mark of the locale: et_EE, de_DE give COMMA,\n\
                 en_US, en_GB give DOT; OSLOCALE gives DOT; a later DECIMAL overrides it; files are always\n\
                 Unicode, so a code page in the name changes nothing), UNICODE (Statscraft always works in\n\
                 Unicode; OFF has no effect), PRINTBACK, CCA to CCE='negative\n\
                 prefix,prefix,suffix,negative suffix' (custom currency formats; three commas, or three periods\n\
                 when the decimal mark is a comma; ' before a period or comma makes it text), RNG=MC | MT\n\
                 (the generator of random numbers), SEED=n | RANDOM (starts the MC generator again; a whole\n\
                 number from 1 to 1999999999, default 2000000), MTINDEX=x | RANDOM (starts the MT generator,\n\
                 the Mersenne Twister, again; any number, default 2000000). RANDOM takes the time of day.\n\
                 MXLOOPS=n: the iterations of a LOOP without an index, and of every loop of MATRIX\n\
                 (default 40). BLANKS=n | SYSMIS: the value of a completely blank numeric field read from\n\
                 text (DATA LIST, GET DATA, NUMBER); it is not made a missing value. EPOCH=year | AUTOMATIC:\n\
                 two-digit years in dates and the DATE. functions fall in the 100 years from that year\n\
                 (AUTOMATIC: from 69 years before the current year). FUZZBITS=n (0 to 20, default 6): the\n\
                 fuzzbits of RND and TRUNC without a third argument. SCALEMIN=n (default 24): a new numeric\n\
                 variable read from text with at least n different values is scale. TVARS=NAMES | LABELS |\n\
                 BOTH: variables in tables as names, labels (default; the name when there is no label) or\n\
                 both, name then label (\"q1 Satisfaction\"). TNUMBERS=VALUES | LABELS | BOTH: values in\n\
                 tables as values, value labels (default; the value when there is no label) or both, value\n\
                 then label (\"1 A great deal\"). OVARS and ONUMBERS do the same for the titles of tables.\n\
                 CTABLES /VLABELS and MULT RESPONSE /FORMAT=NOLABELS win over them. Settings for speed,\n\
                 memory and messages (CACHE, THREADS, WORKSPACE, MXWARNS ...) are accepted and have nothing\n\
                 to do. Settings that only change how output looks (TLOOK, WIDTH, LENGTH, DIGITGROUPING ...)\n\
                 are accepted with a warning. YES and ON, NO and OFF are the same.",
        example: "SET FORMAT=F10.3 MPRINT=YES /DECIMAL=DOT TVARS=BOTH TNUMBERS=BOTH.",
        parse: session_commands::set,
    },
    CommandSpec {
        name: "SHOW",
        summary: "Show the current settings.",
        syntax: "SHOW [ALL] [setting ...].\n\
                 Any SET subcommand, and N, WEIGHT, FILTER, SPLIT, DIRECTORY, SYSMIS, VERSION, HANDLES (the file\n\
                 handles), CC (CCA to CCE), ENVIRONMENT, LICENSE, $VARS, SCOMPRESSION, EXTPATHS, PLUGINS,\n\
                 TMSRECORDING, EXTENSIONS and MXMEMORY. SEED shows where the MC generator stands.\n\
                 A setting Statscraft does not follow shows what Statscraft uses, with a note in a Note column.\n\
                 Without names: the main settings.",
        example: "SHOW FORMAT DECIMAL LOCALE.",
        parse: session_commands::show,
    },
    CommandSpec {
        name: "PRESERVE",
        summary: "Store the current settings and folder, to bring them back with RESTORE.",
        syntax: "PRESERVE.\n\
                 Stores every setting of SET (CCA to CCE, DECIMAL, FORMAT, LOCALE, MPRINT, MEXPAND, MNEST,\n\
                 MITERATE and the rest) and the folder of CD. PRESERVE and RESTORE nest up to five levels.\n\
                 A SET subcommand name after PRESERVE (PRESERVE LOCALE, as survey tools write it) is\n\
                 accepted, since every setting is stored anyway; other words are ignored with a warning.",
        example: "PRESERVE.",
        parse: session_commands::preserve,
    },
    CommandSpec {
        name: "RESTORE",
        summary: "Bring back the settings and folder the last PRESERVE stored.",
        syntax: "RESTORE.\n\
                 Every setting and the folder come back as the last PRESERVE stored them; a RESTORE without\n\
                 PRESERVE does nothing, with a warning. After RESTORE LOCALE (or another SET subcommand name)\n\
                 every setting still comes back; a warning names the settings besides LOCALE and DECIMAL that\n\
                 changed after PRESERVE and so come back too. Other words are ignored with a warning.",
        example: "RESTORE.",
        parse: session_commands::restore,
    },
    CommandSpec {
        name: "CACHE",
        summary: "Accepted and has nothing to do: the data are always kept in memory.",
        syntax: "CACHE.",
        example: "CACHE.",
        parse: session_commands::cache,
    },
    CommandSpec {
        name: "PROCEDURE OUTPUT",
        summary: "Name the file the next procedures write their results to.",
        syntax: "PROCEDURE OUTPUT OUTFILE='file' | handle.\n\
                 CROSSTABS /WRITE puts its cell counts in this file. The file stays in effect, for every\n\
                 procedure that asks for it, until another PROCEDURE OUTPUT replaces it. A relative name starts\n\
                 from the current folder (CD).",
        example: "PROCEDURE OUTPUT OUTFILE='cells.txt'.",
        parse: session_commands::procedure_output,
    },
    CommandSpec {
        name: "CD",
        summary: "Change the folder relative file names start from.",
        syntax: "CD 'folder' | handle.\n\
                 A relative folder starts from the current one, which is the folder of the syntax file at\n\
                 first. When the last part names no existing folder it is taken as a file name and left out.",
        example: "CD 'data/2026'.",
        parse: session_commands::cd,
    },
    CommandSpec {
        name: "FILE HANDLE",
        summary: "Give a file or a folder a short name to use in place of its path.",
        syntax: "FILE HANDLE handle /NAME='path' [/MODE=CHARACTER] [/ENCODING='name'] [/RECFORM=...]\n\
                 \x20           [/LRECL=n].\n\
                 Every command that takes a file (GET, SAVE, DATA LIST, GET DATA, MATCH FILES, ADD FILES,\n\
                 APPLY DICTIONARY, AGGREGATE OUTFILE, INSERT, INCLUDE, CD) takes the handle in place of the\n\
                 path, quoted or not; 'handle/name.sav' starts from the path of a folder handle. A dataset\n\
                 of the same name comes before a handle. A relative NAME starts from the folder of the last\n\
                 CD. ENCODING is the encoding of a text file read through the handle, when the command gives\n\
                 none. RECFORM and LRECL are ignored with a warning; MODE other than CHARACTER (binary and\n\
                 EBCDIC files) is refused. Handles last for the session.",
        example: "FILE HANDLE wave2 /NAME='data/2026/wave2.sav'.\nGET FILE=wave2.",
        parse: session_commands::file_handle,
    },
    CommandSpec {
        name: "DATASET NAME",
        summary: "Give the active dataset a name, so that it stays open when another one becomes active.",
        syntax: "DATASET NAME name [WINDOW=ASIS | FRONT].\n\
                 A dataset that had the name before is closed. WINDOW is accepted and has no effect.",
        example: "DATASET NAME survey.",
        parse: session_commands::dataset_name_command,
    },
    CommandSpec {
        name: "DATASET ACTIVATE",
        summary: "Make a named dataset the active one.",
        syntax: "DATASET ACTIVATE name [WINDOW=ASIS | FRONT].\n\
                 The dataset that was active stays open when it has a name, else it is closed.",
        example: "DATASET ACTIVATE survey.",
        parse: session_commands::dataset_activate,
    },
    CommandSpec {
        name: "DATASET COPY",
        summary: "Copy the active dataset, as it is now, into a named dataset.",
        syntax: "DATASET COPY name [WINDOW=MINIMIZED | HIDDEN | FRONT].\n\
                 A dataset that had the name is replaced, with a warning. When the active dataset has the\n\
                 name, the copy takes it and the active dataset is left without a name.",
        example: "DATASET COPY men.",
        parse: session_commands::dataset_copy,
    },
    CommandSpec {
        name: "DATASET DECLARE",
        summary: "Reserve a dataset name for a dataset that a later command creates.",
        syntax: "DATASET DECLARE name [WINDOW=MINIMIZED | HIDDEN | FRONT].",
        example: "DATASET DECLARE results.",
        parse: session_commands::dataset_declare,
    },
    CommandSpec {
        name: "DATASET CLOSE",
        summary: "Close a named dataset, or take the name off the active one.",
        syntax: "DATASET CLOSE name | * | ALL.\n\
                 The active dataset stays open: closing it, or *, only takes its name off. ALL closes every\n\
                 other dataset.",
        example: "DATASET CLOSE men.",
        parse: session_commands::dataset_close,
    },
    CommandSpec {
        name: "DATASET DISPLAY",
        summary: "List the open datasets.",
        syntax: "DATASET DISPLAY.",
        example: "DATASET DISPLAY.",
        parse: session_commands::dataset_display,
    },
    CommandSpec {
        name: "OUTPUT NEW",
        summary: "Start a new output document, which gets the tables from now on.",
        syntax: "OUTPUT NEW [NAME=name] [TYPE=VIEWER].\n\
                 The tables of the commands that follow go to the new document until another one is\n\
                 started or activated. Without NAME it is called Document1, Document2 and so on; a name\n\
                 that another document had passes to the new one, and the other gets a new name. TYPE is\n\
                 ignored. The window shows the tables of every document in its Output view; the documents\n\
                 decide what OUTPUT EXPORT writes. The window session keeps its documents from run to run; a\n\
                 command line run starts without one.",
        example: "OUTPUT NEW NAME=women.",
        parse: output_docs::output_new,
    },
    CommandSpec {
        name: "OUTPUT NAME",
        summary: "Name the output document that gets the tables.",
        syntax: "OUTPUT NAME [NAME=]name.\n\
                 The designated document (the one that gets the tables) takes the name, and a document that\n\
                 had it gets a new one. Without any document yet, an empty one is started.",
        example: "OUTPUT NAME men.",
        parse: output_docs::output_name,
    },
    CommandSpec {
        name: "OUTPUT ACTIVATE",
        summary: "Send the tables from now on to an open output document.",
        syntax: "OUTPUT ACTIVATE [NAME=]name.\n\
                 A name that no open document has gives a warning, and the tables keep going where they went.",
        example: "OUTPUT ACTIVATE men.",
        parse: output_docs::output_activate,
    },
    CommandSpec {
        name: "OUTPUT CLOSE",
        summary: "Close an output document, or all of them.",
        syntax: "OUTPUT CLOSE [NAME=]name | * | ALL.\n\
                 * closes the designated document. Nothing is saved; the next table starts a new document.\n\
                 A name that no open document has is ignored with a warning.",
        example: "OUTPUT CLOSE *.",
        parse: output_docs::output_close,
    },
    CommandSpec {
        name: "OUTPUT DISPLAY",
        summary: "List the open output documents.",
        syntax: "OUTPUT DISPLAY.\n\
                 One row per document with its number of tables; the designated one is marked.",
        example: "OUTPUT DISPLAY.",
        parse: output_docs::output_display,
    },
    CommandSpec {
        name: "OUTPUT EXPORT",
        summary: "Write the tables and charts of an output document to a workbook, document, presentation, PDF, web page, text or CSV file.",
        syntax: "OUTPUT EXPORT [NAME=* | name] [/CONTENTS [EXPORT=ALL | VISIBLE] [LAYERS=...] [MODELVIEWS=...]]\n\
                 \x20  [/XLSX DOCUMENTFILE='path' [OPERATION=CREATEFILE | CREATESHEET | MODIFYSHEET] [SHEET='name']\n\
                 \x20         [LOCATION=LASTCOLUMN | LASTROW | STARTCELL('B2')] [NOTESCAPTIONS=YES | NO]]\n\
                 \x20  [/DOC DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO] [WIDETABLES=...] [PAGESIZE=...]\n\
                 \x20         [TOPMARGIN=...] [BOTTOMMARGIN=...] [LEFTMARGIN=...] [RIGHTMARGIN=...] [PAGEBREAKS=...]]\n\
                 \x20  [/PPT DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO] [SLIDETITLES=YES | NO] [WIDETABLES=...]\n\
                 \x20         [SLIDESIZE=...] [TOPMARGIN=...] [BOTTOMMARGIN=...] [LEFTMARGIN=...] [RIGHTMARGIN=...]]\n\
                 \x20  [/PDF DOCUMENTFILE='path' [EMBEDBOOKMARKS=YES | NO] [EMBEDFONTS=YES | NO]]\n\
                 \x20  [/HTML DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO] [STYLING=YES | NO]\n\
                 \x20         [IMAGEFORMAT=JPG | BMP | PNG] [INTERACTIVELAYERS=NO | YES]]\n\
                 \x20  [/TEXT DOCUMENTFILE='path' [ENCODING=LOCALE | UTF8 | UTF16] [NOTESCAPTIONS=YES | NO]\n\
                 \x20         [COLUMNWIDTH=AUTOFIT | n] [SEPARATOR=SPACE | TAB] [ROWBORDER='-'] [COLUMNBORDER='|']\n\
                 \x20         [IMAGEFORMAT=JPG | BMP | EMF | EPS | PNG | TIF]]\n\
                 \x20  [/CSV DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO]]\n\
                 \x20  [/PNG [IMAGEROOT='root'] [PERCENTSIZE=n] [DEPTH=SCREEN | BLACKWHITE | GRAYS256 | ...]]\n\
                 \x20  [/JPG [IMAGEROOT='root'] [PERCENTSIZE=n] [GRAYSCALE=NO | YES]]\n\
                 \x20  [/BMP [IMAGEROOT='root'] [PERCENTSIZE=n] [COMPRESS=NO | YES]]\n\
                 \x20  [/TIF [IMAGEROOT='root'] [PERCENTSIZE=n]] [/EMF ...] [/EPS ...].\n\
                 Writes the tables and charts that the designated document (NAME=*) or the named one holds so\n\
                 far, in order; an existing file is replaced. Subcommands and keywords are spelled in full.\n\
                 CONTENTS EXPORT=VISIBLE leaves out what OUTPUT MODIFY hid; every table has one layer, so\n\
                 LAYERS and MODELVIEWS change nothing. NOTESCAPTIONS=NO leaves out\n\
                 the notes under the tables. XLSX puts the tables one below the other on one sheet, named by\n\
                 SHEET or Tables, laid out as the Excel export of the window. CREATESHEET and MODIFYSHEET add\n\
                 to a workbook that OUTPUT EXPORT wrote earlier in the session and that has not changed since;\n\
                 another workbook is refused, so its formulas and formatting cannot be lost. CREATESHEET\n\
                 replaces or adds the sheet; MODIFYSHEET places the tables after the last column or row of\n\
                 the sheet, one column or row apart, or from STARTCELL. A bar, pie, line or area chart goes to\n\
                 the sheet as its data table with an editable chart beside it, drawn in the theme's colors.\n\
                 DOC writes a Word document in the .docx format, whatever the file is called: each table\n\
                 under its title, each chart as an editable chart whose numbers Edit Data opens. PPT writes a\n\
                 presentation in the .pptx format with one slide per table or chart; SLIDETITLES=NO leaves the\n\
                 slide titles out, and a chart then shows its own. Page and slide sizes, margins, WIDETABLES\n\
                 and PAGEBREAKS are accepted and not applied. PDF lays the output on A4 portrait pages with\n\
                 2 cm margins: each command's heading, then its tables as text you can select, header rows\n\
                 repeated when a table goes on over the next page, the caption below. A table too wide for\n\
                 the page is set smaller, to 70 percent at most; if it still does not fit, its columns come\n\
                 in parts, one below the other, each with the row labels again. Charts are vector drawings.\n\
                 The TITLE text heads every page and the page number stands at the foot. EMBEDBOOKMARKS=NO\n\
                 leaves out the outline of commands and items; EMBEDFONTS=NO names the font instead of\n\
                 embedding the glyphs used, so a reader without it uses one of its own, and characters\n\
                 outside Windows Latin show as question marks. In DOC, PPT and XLSX, the other charts, such as a histogram or a\n\
                 boxplot, go in as a picture; on a sheet the picture stands beside the data table. HTML\n\
                 writes one web page with every chart in it as a picture, JPG unless IMAGEFORMAT names\n\
                 another; STYLING=NO\n\
                 leaves out the style sheet. TEXT lines the columns up between the borders, or with\n\
                 SEPARATOR=TAB separates the cells by tabs; COLUMNWIDTH=n wraps longer cells; LOCALE and UTF8\n\
                 write UTF-8, UTF16 writes UTF-16 with a byte order mark. CSV, a Statscraft addition, writes\n\
                 each table as comma-separated values after its title, with a blank line between tables.\n\
                 TEXT saves each chart as a picture file in the folder of the text, named after the text\n\
                 file with a number from 0 (out.txt gives out0.jpg, out1.jpg, ...), and writes the chart's\n\
                 title and that file name where the chart stands. CSV shows a chart as its data table.\n\
                 With no document subcommand, each of /PNG, /JPG, /BMP and /TIF saves every chart in its\n\
                 format: IMAGEROOT='charts/june' gives charts/june0.png, charts/june1.png and so on. With a\n\
                 document subcommand IMAGEROOT is not used, and the subcommand of the format in use sets\n\
                 the pictures' size and colors. PERCENTSIZE (above 0, at most 200) scales a picture: 100\n\
                 is 1280 pixels wide, the PNG of --charts. GRAYSCALE=YES, DEPTH=GRAYS256 and\n\
                 DEPTH=BLACKWHITE drop the color; the other depths and COMPRESS change nothing. EMF and EPS\n\
                 are not written yet: a warning, and their charts stay data tables, as does a kind that is\n\
                 not drawn yet, such as a stem-and-leaf display. /XLS and /XLSM are refused.",
        example: "OUTPUT EXPORT /XLSX DOCUMENTFILE='results.xlsx' SHEET='Region 1'.",
        parse: output_docs::output_export,
    },
    CommandSpec {
        name: "OUTPUT SAVE",
        summary: "Save an output document as a Statscraft output file (.scout).",
        syntax: "OUTPUT SAVE [NAME=* | name] OUTFILE='path' [TYPE=SPV] [LOCK=NO | YES] [STOREMODEL=YES | NO]\n\
                 \x20  [/PASSPROTECT PASSWORD=NONE [ENCRYPTEDPW=NO | YES]].\n\
                 Writes the designated document (NAME=*), or the named one, with every command's syntax,\n\
                 warnings, tables and charts, as JSON that File > Open, OUTPUT OPEN and any JSON tool read.\n\
                 A file that exists is replaced; the document stays open and keeps getting tables. An\n\
                 unknown NAME is an error. A path ending in .spv, .spo or .spw (another program's output\n\
                 files) is written with .scout in its place and a warning, so such a file is never\n\
                 overwritten. TYPE, LOCK and STOREMODEL change nothing. A password on /PASSPROTECT is an\n\
                 error and nothing is saved: Statscraft output files are never encrypted.",
        example: "OUTPUT SAVE OUTFILE='wave2-results.scout'.",
        parse: output_docs::output_save,
    },
    CommandSpec {
        name: "OUTPUT OPEN",
        summary: "Open a Statscraft output file (.scout) as a new output document.",
        syntax: "OUTPUT OPEN FILE='path' [NAME=name] [LOCK=YES | NO] [PASSWORD='text'].\n\
                 Reads the file into a new document, named by NAME or Document1, Document2 and so on, and\n\
                 sends the tables of the commands that follow to it; OUTPUT SAVE then writes old and new\n\
                 together, and OUTPUT EXPORT converts the file to a workbook, document or web page. A name\n\
                 another document has passes to the new one. A file that is missing, is no Statscraft\n\
                 output file or comes from a newer Statscraft gives a warning, and the tables keep going\n\
                 where they went. For a path ending in .spv, .spo or .spw the .scout file of that name is\n\
                 read. LOCK and PASSWORD change nothing.",
        example: "OUTPUT OPEN FILE='wave2-results.scout' NAME=wave2.",
        parse: output_docs::output_open,
    },
    CommandSpec {
        name: "OUTPUT MODIFY",
        summary: "Delete, hide, rename, sort, transpose or reformat items of an output document.",
        syntax: "OUTPUT MODIFY [NAME=* | ALL | name] [/REPORT PRINTREPORT=NO | YES]\n\
                 \x20  /SELECT kinds | ALL [EXCEPT (kinds)]\n\
                 \x20  [/IF [COMMANDS=['name' 'name(n)' 'name(LAST)' LAST]] [SUBTYPES=['title' ...]]\n\
                 \x20       [LABELS=[EXACT('t') BEGINS('t') ENDS('t') CONTAINS('t')]] [INSTANCES=[n ... LAST]]\n\
                 \x20       [VISIBLEONLY=NO | YES]]\n\
                 \x20  [/DELETEOBJECT DELETE=NO | YES]\n\
                 \x20  [/INDEXING [INDEXTYPE=NUMBER | LETTERLOW | LETTERCAP | ROMANLOW | ROMANCAP] [INDEXSTART=n | 'a']]\n\
                 \x20  [/OBJECTPROPERTIES [VISIBLE=ASIS | YES | NO] [OUTLINELABEL='text'] [SIZE=...]]\n\
                 \x20  [/TABLE [TABLETITLE='text'] [TRANSPOSE=NO | YES] [SORT=COLLABEL('label') | COLPOSITION(n)]\n\
                 \x20       [SORTDIRECTION=ASCENDING | DESCENDING] [TLOOK=...] [SELECTEDLAYER=[...]] [TABLESUMMARY='text'] [PIVOT=[...]]]\n\
                 \x20  [/TABLECELLS SELECT=['label' POSITION(n) BODY HEADERS TITLE CAPTION FOOTNOTES COUNT MEAN ...]\n\
                 \x20       [SELECTDIMENSION=BOTH | COLUMNS | ROWS] [SELECTCONDITION=[ALL 'x<0.05' TOP(n) BOTTOM(n)]]\n\
                 \x20       [APPLYTO=CELL | COLUMN | ROW | COLUMNDATA | ROWDATA | COLUMNHEADER | ROWHEADER]\n\
                 \x20       [FORMAT=F8.2] [REPLACE='text' | number] [HIDE=NO | YES] [style keywords]]\n\
                 \x20  [/GRAPHS ...] [/TEXTS [TEXT='text'] ...]\n\
                 \x20  [/SELECT ... another block].\n\
                 Works on the items the designated document (or NAME, or ALL documents) holds when it runs.\n\
                 Each block begins with /SELECT and runs on what the blocks before it left. Kinds: TABLES,\n\
                 CHARTS, HEADINGS and OUTLINEHEADERS (the heading over each command's items), PAGETITLE (the\n\
                 output of TITLE and SUBTITLE) and UNKNOWN; LOGS, TEXTS, WARNINGS, NOTES, MODELS, TREES and\n\
                 OTHER pick nothing, as Statscraft documents have no such items. /IF keeps the items that meet\n\
                 every condition: COMMANDS names commands as written in the outline, such as 'Means', with (n)\n\
                 for the nth run or (LAST); LAST alone is the document's last command. SUBTYPES match table\n\
                 titles, any case. LABELS test the outline text, case counts. INSTANCES pick the nth item\n\
                 within each command. DELETE=YES removes the items; a heading goes with its command's items.\n\
                 VISIBLE=NO hides items: the window and OUTPUT EXPORT with /CONTENTS EXPORT=VISIBLE leave them\n\
                 out, the document and its file keep them. On HEADINGS it hides the heading alone and the\n\
                 tables under it stay; with OUTLINEHEADERS picked as well the command's items go too. OUTLINELABEL, TABLETITLE and TEXT take )LABEL (the\n\
                 text so far), )INDEX (from INDEXING, else 1, 2, 3), )DATE, )ADATE, )SDATE, )EDATE, )TIME and\n\
                 )ETIME. SORT orders the rows by a column; empty cells go last, rows labelled Total stay, and a\n\
                 table with nested row labels stays as it is. TABLECELLS picks rows or columns by label (any\n\
                 header level) or position, keeps the cells that meet every comparison on x (ABS(x), SYSMIS,\n\
                 numbers or quoted text) and then TOP or BOTTOM, and changes what APPLYTO names: FORMAT\n\
                 changes how numbers show, REPLACE puts in a value, HIDE takes the picked rows or columns out.\n\
                 Keywords may be shortened to three letters. Fonts, colors, styles, sizes, column widths,\n\
                 table looks, layers, PIVOT, UNGROUP and chart templates change nothing and say so in a\n\
                 warning. PARENT and CHILD conditions are not supported. PRINTREPORT=YES adds a table of\n\
                 what changed.",
        example: "OUTPUT MODIFY /SELECT TABLES /IF COMMANDS=['Means'] /TABLE SORT=COLLABEL('Mean')\n\
                 \x20 SORTDIRECTION=DESCENDING /TABLECELLS SELECT=['Mean'] SELECTDIMENSION=COLUMNS FORMAT=F8.1.",
        parse: output_modify::output_modify,
    },
    CommandSpec {
        name: "OMS",
        summary: "Send the tables, charts and warnings of the commands that follow to a file, or keep them out of the results.",
        syntax: "OMS [/SELECT kinds | ALL [EXCEPT=[kinds]]]\n\
                 \x20  [/IF [COMMANDS=['name' ...]] [SUBTYPES=['title' ...]] [LABELS=['text' ...]] [INSTANCES=[n ... LAST]]]\n\
                 \x20  [/EXCEPTIF [COMMANDS=[...]] [SUBTYPES=[...]] [LABELS=[...]]]\n\
                 \x20  /DESTINATION [FORMAT=SAV | HTML | TEXT | TABTEXT | XLSX | DOC | PDF OUTFILE='file' | dataset]\n\
                 \x20       [VIEWER=YES | NO] [NUMBERED[='name']] [IMAGES=YES | NO] [IMAGEFORMAT=PNG | JPG | BMP] [CHARTSIZE=n]\n\
                 \x20  [/COLUMNS DIMNAMES=['name' ...] | SEQUENCE=[R1 R2 ... RALL C1 ... CALL]]\n\
                 \x20  [/TAG='name'] [/NOWARN].\n\
                 Kinds: TABLES, CHARTS, WARNINGS (the warnings of a command, as one item), TEXTS (the output\n\
                 of ECHO, TITLE and SUBTITLE); HEADINGS, LOGS, MODELS and TREES choose nothing, as Statscraft\n\
                 makes no such items (HTML and PDF show each command's heading anyway). COMMANDS take names\n\
                 such as 'Frequencies' or 'T-Test', any case; SUBTYPES take table titles, any case, and the\n\
                 tables of single variables from FREQUENCIES also answer to 'Frequencies'; LABELS take the\n\
                 outline text, any case. A trailing * matches every text that starts with what comes before\n\
                 it. INSTANCES keep the nth of the matching items of each command. An item must meet every\n\
                 list of /IF, and is left out when it meets every list of /EXCEPTIF. The request lasts until\n\
                 OMSEND, and its file is\n\
                 written again after each command that adds to it. FORMAT=SAV makes a data file of the\n\
                 tables: string variables Command_, Subtype_ and Label_, Var1, Var2, ... for the row labels,\n\
                 and one variable per column, named after its header (levels joined by _); a column of\n\
                 numbers is numeric. A table whose number of row labels differs from the earlier tables is\n\
                 left out with a warning. OUTFILE may name a dataset that DATASET DECLARE made. HTML, TEXT,\n\
                 TABTEXT, XLSX, DOC and PDF write as OUTPUT EXPORT does. VIEWER=NO keeps the chosen items\n\
                 out of the results; when several requests choose an item, the latest decides. COLUMNS puts\n\
                 the named row labels (DIMNAMES, by the header of their column) or positions (SEQUENCE) in\n\
                 the columns and the rest in the rows, in the files only. OXML, SPV, XLS, the report formats,\n\
                 XMLWORKSPACE and OUTPUTSET are not written: a warning, and the request only lists in\n\
                 OMSINFO. Keywords that change nothing here, such as IMAGEMAP, give a warning unless NOWARN.",
        example: "OMS /SELECT TABLES /IF COMMANDS=['Descriptives'] /DESTINATION FORMAT=SAV OUTFILE='stats.sav'\n\
                 \x20 VIEWER=NO /TAG='desc'.",
        parse: oms::oms,
    },
    CommandSpec {
        name: "OMSEND",
        summary: "End OMS requests and the OMS log.",
        syntax: "OMSEND [TAG=['name' ...] | TAG=ALL] [FILE=['file' ...]] [LOG].\n\
                 TAG ends the requests of those names, FILE those that write those files; a name or file\n\
                 that no request has gives a warning. LOG ends the log of OMSLOG. With nothing given, every\n\
                 request and the log end.",
        example: "OMSEND TAG=['desc'].",
        parse: oms::omsend,
    },
    CommandSpec {
        name: "OMSINFO",
        summary: "List the OMS requests in force.",
        syntax: "OMSINFO.\n\
                 A table with a row per request: its tag, the kinds it chooses, its format, its file and\n\
                 whether the chosen items still show in the results.",
        example: "OMSINFO.",
        parse: oms::omsinfo,
    },
    CommandSpec {
        name: "OMSLOG",
        summary: "Keep a log of when OMS requests start and end and which files they write.",
        syntax: "OMSLOG FILE='file' [/APPEND=NO | YES] [/FORMAT=XML | TEXT].\n\
                 Each event gets a line (TEXT) or an element (XML) with its name, the tag, the file and the\n\
                 time. A later OMSLOG starts a new log; APPEND=YES with TEXT adds to a file that exists.\n\
                 OMSEND LOG ends the log.",
        example: "OMSLOG FILE='oms_log.txt' /FORMAT=TEXT.",
        parse: oms::omslog,
    },
    CommandSpec {
        name: "BEGIN CHART",
        summary: "A Statscraft extension: a chart given as JSON goes to the output like a procedure's chart.",
        syntax: "BEGIN CHART\n\
                 JSON lines\n\
                 END CHART.\n\
                 Not a command of the language: Statscraft adds it so that an agent or a script can put a\n\
                 chart that no procedure makes into the designated output document, where it is drawn,\n\
                 saved by OUTPUT SAVE and written by OUTPUT EXPORT like any other. BEGIN CHART and\n\
                 END CHART each stand alone on their line; every line between is JSON, read as it is.\n\
                 The JSON is either a chart as `statscraft run --json` writes it (a member of charts),\n\
                 optionally with \"version\": 1, or a Vega-Lite specification of one view with inline\n\
                 data.values and mark bar, line, area, point or arc, encoded by x, y, color, theta and\n\
                 xOffset or yOffset, without aggregate, bin, timeUnit or transform. Values are taken as\n\
                 they are: a category and series given twice is an error. Fields Statscraft does not\n\
                 know are an error. At most 100 series, 3000 categories, 100000 values and strings of\n\
                 1000 characters. The MCP tool add_chart and `statscraft chart` write this command.",
        example: "BEGIN CHART\n{\"mark\": \"bar\", \"data\": {\"values\": [{\"region\": \"North\", \"sales\": 12}, {\"region\": \"South\", \"sales\": 9}]},\n \"encoding\": {\"x\": {\"field\": \"region\", \"type\": \"nominal\"}, \"y\": {\"field\": \"sales\", \"type\": \"quantitative\"}}}\nEND CHART.",
        parse: output_docs::begin_chart,
    },
    CommandSpec {
        name: "USE",
        summary: "Set the historical period of the time series procedures, or use all cases.",
        syntax: "USE ALL.\n\
                 USE [start] [THRU [end]] [/PERMANENT].\n\
                 A start or end is a case number or a date written with DATE keywords and values\n\
                 (YEAR 2019 MONTH 3; the highest date variable must be among them); FIRST and LAST mean\n\
                 the ends of the file. The two kinds do not mix. The range acts as a filter for the\n\
                 procedures that follow, while CREATE ignores it and EXSMOOTH and ARIMA estimate on it\n\
                 and forecast after it (see PREDICT). A date start is the first case of that date, a date\n\
                 end the last. ALL or THRU alone uses every case again. USE turns FILTER off; FILTER,\n\
                 SORT CASES and DATE turn USE off. /PERMANENT deletes the cases outside the range.",
        example: "USE YEAR 2016 THRU YEAR 2021.",
        parse: ts_models::use_command,
    },
    CommandSpec {
        name: "DISPLAY",
        summary: "Show dictionary information of the active dataset.",
        syntax: "DISPLAY [SORTED] [NAMES | INDEX | VARIABLES | LABELS | DICTIONARY | ATTRIBUTES | @ATTRIBUTES\n\
                 \x20        | SCRATCH | VECTOR | MACROS | DOCUMENTS | FILE LABEL] [/VARIABLES=varlist].\n\
                 NAMES (the default): names. INDEX: names and positions. VARIABLES: also formats and missing\n\
                 values. LABELS: positions and labels. DICTIONARY: everything, value labels included.\n\
                 ATTRIBUTES: custom attributes except those starting with @ or $@; @ATTRIBUTES all of them.\n\
                 MACROS: the names of the macros defined in this run and the runs of the session before,\n\
                 sorted. SORTED sorts by name; /VARIABLES limits the list.",
        example: "DISPLAY SORTED DICTIONARY /VARIABLES=age TO q5.",
        parse: session_commands::display,
    },
    CommandSpec {
        name: "CODEBOOK",
        summary: "Dictionary entries and summary statistics of variables and multiple response sets.",
        syntax: "CODEBOOK [varlist] [$set ...] [$ALL]\n\
                 \x20        [/VARINFO=POSITION LABEL TYPE FORMAT MEASURE ROLE ATTRIBUTES VALUELABELS MISSING\n\
                 \x20                  RESERVEDATTRIBUTES]\n\
                 \x20        [/FILEINFO=NAME LOCATION CASECOUNT LABEL ATTRIBUTES DOCUMENTS WEIGHT RESERVEDATTRIBUTES]\n\
                 \x20        [/STATISTICS=COUNT PERCENT MEAN STDDEV QUARTILES | NONE]\n\
                 \x20        [/OPTIONS [MAXCATS=200] [VARORDER=FILE | ALPHA | VARLIST | MEASURE | ATTRIBUTE(name)]\n\
                 \x20                  [SORT=ASCENDING | DESCENDING]].\n\
                 One table per variable and set; without a list every variable in file order, then every set.\n\
                 [N], [O] or [S] right after a name (after a TO list: its last variable) sets the level that\n\
                 decides its statistics, not the level shown; strings and sets cannot be [S]. Nominal and\n\
                 ordinal variables and sets get the count and percent of every value (each labeled value and\n\
                 each value found), scale variables the mean, standard deviation and quartiles plus counts of\n\
                 their labeled values. An unknown level is guessed from the values. Percents are of every case\n\
                 the filter keeps, missing ones included; a set's can add up to more than 100. Above MAXCATS\n\
                 values (scale: labeled values) the values, labels and counts are left out. VARINFO left out\n\
                 means all but RESERVEDATTRIBUTES; ATTRIBUTES shows custom attributes, RESERVEDATTRIBUTES those\n\
                 starting with $ (both skip names starting with @ or $@). FILEINFO adds a table about the file;\n\
                 NAME and LOCATION come from the GET FILE or SAVE that last read or wrote it in this run.\n\
                 VARORDER: ALPHA by name; MEASURE nominal, ordinal, scale, unknown; ATTRIBUTE(name) by the\n\
                 attribute's value, those without it first, then those with an empty one. VARLIST is the default\n\
                 with a list. Every variable or set may be named once. SPLIT FILE is ignored; FILTER and the\n\
                 weight apply.",
        example: "CODEBOOK region income [O] $media /VARINFO=LABEL MEASURE VALUELABELS\n\
                  \x20 /STATISTICS=COUNT MEAN /OPTIONS MAXCATS=12 VARORDER=ALPHA.",
        parse: codebook::codebook,
    },
    CommandSpec {
        name: "VALIDATEDATA",
        summary: "Find suspicious variables, incomplete or duplicate IDs, empty cases and rule violations.",
        syntax: "VALIDATEDATA [VARIABLES=varlist] [ID=varlist] [CROSSVARRULES=$VD.CRule[n] ... | n ... | ALL]\n\
                 \x20 [/VARCHECKS [STATUS=ON | OFF] [PCTMISSING=70] [PCTEQUAL=95] [PCTUNEQUAL=90] [CV=0.001]\n\
                 \x20             [STDDEV=0]]\n\
                 \x20 [/IDCHECKS [INCOMPLETE] [DUPLICATE] | NONE]\n\
                 \x20 [/CASECHECKS [REPORTEMPTY=YES | NO] [SCOPE=ANALYSISVARS | ALLVARS]]\n\
                 \x20 [/RULESUMMARIES [BYVARIABLE] [BYRULE] | NONE]\n\
                 \x20 [/CASEREPORT [DISPLAY=YES | NO] [MINVIOLATIONS=1] [CASELIMIT=FIRSTN(500) | NONE]]\n\
                 \x20 [/SAVE [EMPTYCASE[(name)]] [INCOMPLETEID[(name)]] [DUPLICATEID[(name)]]\n\
                 \x20        [RULEVIOLATIONS[(name)]]].\n\
                 Give at least one of VARIABLES (analysis variables), ID (the variables that together identify\n\
                 a case) and CROSSVARRULES. Keywords are written in full; a subcommand may come once and may not\n\
                 be empty. VARCHECKS lists analysis variables with more missing values than PCTMISSING percent\n\
                 of the cases; nominal and ordinal ones (strings too) whose largest category holds more than\n\
                 PCTEQUAL percent of the valid values, or whose categories have one case each in more than\n\
                 PCTUNEQUAL percent of them; scale ones whose absolute coefficient of variation is below CV (0\n\
                 turns it off; skipped when the mean is 0) or whose standard deviation is below STDDEV. A\n\
                 variable of unknown level gets one from its values. IDCHECKS lists the cases whose ID has a\n\
                 missing or blank part and the groups of cases that share a complete ID. CASECHECKS lists empty\n\
                 cases: every analysis variable (SCOPE=ALLVARS: every variable but the ID, split and rule\n\
                 outcome variables) missing or blank.\n\
                 Rules live in the dictionary. A single-variable rule is the data file attribute $VD.SRule[n],\n\
                 a text of Name='value' properties separated by commas, among them Label and Type (Numeric,\n\
                 String or Date); an analysis variable links to it with the variable attribute $VD.SRuleRef[n]\n\
                 holding Rule='$VD.SRule[n]' and OutcomeVar='name'. A cross-variable rule is $VD.CRule[n] with\n\
                 Label, Expression and OutcomeVar. The outcome variable, made beforehand with COMPUTE, is 1 for\n\
                 a case that breaks the rule; mark it with the variable attribute $VD.RuleOutcomeVar('Yes') so\n\
                 the variable lists skip it. Broken or unknown rules and links are skipped with a warning; one\n\
                 outcome variable used by two rules is an error. RULESUMMARIES counts single-variable\n\
                 violations by variable (default) or by rule; cross-variable rules are always counted per\n\
                 rule. CASEREPORT lists the cases with at least MINVIOLATIONS violations, the first 500 by\n\
                 default. SAVE adds EmptyCase, IncompleteID (1 or 0), DuplicateIDGroup (the same number for\n\
                 cases sharing an ID, else 0) and ValidationRuleViolations (the count per case); a default name\n\
                 in use gets _1, _2 and so on, a given name in use is an error. Missing means user- or\n\
                 system-missing. Works within SPLIT FILE groups and on the cases FILTER keeps; the weight is\n\
                 ignored with a warning.",
        example: "VALIDATEDATA VARIABLES=age income visits ID=household member\n\
                  \x20 /VARCHECKS PCTMISSING=40 CV=0.01 /SAVE DUPLICATEID(dupgroup) EMPTYCASE.",
        parse: datacheck::validatedata,
    },
    CommandSpec {
        name: "LIST",
        summary: "List the values of cases.",
        syntax: "LIST [[VARIABLES=]varlist] [/CASES=[FROM n] [TO n] [BY n]] [/FORMAT=[NUMBERED | UNNUMBERED]\n\
                 \x20    [WRAP | SINGLE]].\n\
                 Values show in their print format; system-missing is a period. NUMBERED adds the case\n\
                 number. Follows FILTER and SPLIT FILE.",
        example: "LIST VARIABLES=id age /CASES=FROM 1 TO 10 /FORMAT=NUMBERED.",
        parse: session_commands::list,
    },
    CommandSpec {
        name: "ADD VALUE LABELS",
        summary: "Add or change value labels, keeping the other labels of the variables.",
        syntax: "ADD VALUE LABELS varlist value 'label' [value 'label' ...] [/ varlist ...].\n\
                 Values are numbers or quoted strings; a label may be joined from pieces with +.",
        example: "ADD VALUE LABELS q1 TO q5 9 'No answer' / plan 'x' 'Unknown'.",
        parse: dictionary::add_value_labels,
    },
    CommandSpec {
        name: "FILE LABEL",
        summary: "Set the label of the data file.",
        syntax: "FILE LABEL text.\n\
                 The text needs no quotes; a label that starts with a quote is read as a quoted string.",
        example: "FILE LABEL Survey wave 2, cleaned.",
        parse: dictionary::file_label,
    },
    CommandSpec {
        name: "DOCUMENT",
        summary: "Add free text to the documents of the data file.",
        syntax: "DOCUMENT text.\n\
                 The text may run over several lines and hold blank lines; a line ending in a period\n\
                 ends it. Every line becomes a line of the documents.",
        example: "DOCUMENT Weights were trimmed at 5.",
        parse: dictionary::document,
    },
    CommandSpec {
        name: "ADD DOCUMENT",
        summary: "Add quoted lines to the documents of the data file.",
        syntax: "ADD DOCUMENT 'line' ['line' ...].\n\
                 Each string is one line; an empty string gives a blank line.",
        example: "ADD DOCUMENT 'Cleaned on 10 November.' 'Outliers removed.'",
        parse: dictionary::add_document,
    },
    CommandSpec {
        name: "DROP DOCUMENTS",
        summary: "Remove the documents of the data file.",
        syntax: "DROP DOCUMENTS.",
        example: "DROP DOCUMENTS.",
        parse: dictionary::drop_documents,
    },
    CommandSpec {
        name: "VARIABLE ROLE",
        summary: "Set the role of variables for dialogs.",
        syntax: "VARIABLE ROLE /INPUT varlist /TARGET varlist /BOTH varlist /NONE varlist\n\
                 \x20             /PARTITION varlist /SPLIT varlist.\n\
                 At least one subcommand; a variable named twice keeps its last role.",
        example: "VARIABLE ROLE /INPUT age income /TARGET satisfaction /NONE id.",
        parse: dictionary::variable_role,
    },
    CommandSpec {
        name: "VARIABLE ALIGNMENT",
        summary: "Set the alignment of variables in the data grid.",
        syntax: "VARIABLE ALIGNMENT varlist (LEFT | RIGHT | CENTER) [[/] varlist (...) ...].",
        example: "VARIABLE ALIGNMENT id (LEFT) / q1 TO q5 (CENTER).",
        parse: dictionary::variable_alignment,
    },
    CommandSpec {
        name: "VARIABLE ATTRIBUTE",
        summary: "Set or delete custom attributes of variables.",
        syntax: "VARIABLE ATTRIBUTE VARIABLES=varlist ATTRIBUTE=name('value') [name[n]('value') ...]\n\
                 \x20                  [DELETE=name [name[n] ...]] [/VARIABLES=...].\n\
                 name[n] is element n of an attribute array (from 1); DELETE=name[n] removes it and moves\n\
                 the later elements up, DELETE=name removes the whole attribute. Values are always quoted.",
        example: "VARIABLE ATTRIBUTE VARIABLES=q1 q2 ATTRIBUTE=Source('wave 2') Note[2]('checked').",
        parse: dictionary::variable_attribute,
    },
    CommandSpec {
        name: "DATAFILE ATTRIBUTE",
        summary: "Set or delete custom attributes of the data file.",
        syntax: "DATAFILE ATTRIBUTE ATTRIBUTE=name('value') [name[n]('value') ...] [DELETE=name ...].\n\
                 As VARIABLE ATTRIBUTE, for the file.",
        example: "DATAFILE ATTRIBUTE ATTRIBUTE=Version('2') Revised[1]('2026-11-10').",
        parse: dictionary::datafile_attribute,
    },
    CommandSpec {
        name: "ALTER TYPE",
        summary: "Change the type, string width or format of variables.",
        syntax: "ALTER TYPE varlist ([input =] format | AMIN [+ n[%]]) [varlist (...) ...] [/PRINT ...].\n\
                 A string variable may get another width (A20); values longer than it are cut. A numeric\n\
                 variable may get another numeric format, or become a string holding its formatted\n\
                 values; a string may become numeric with an F, COMMA or DOT format, a value that is no\n\
                 number becoming system-missing. A type change drops value labels and missing values.\n\
                 input = (A=, F=, A10=) limits the change to variables of that type and width.\n\
                 AMIN gives a string variable the width of its longest value, plus n bytes or n percent of\n\
                 that width (rounded).\n\
                 /PRINT is accepted; no report is shown.",
        example: "ALTER TYPE comment (A500) code (A=AMIN).",
        parse: dictionary::alter_type,
    },
    CommandSpec {
        name: "APPLY DICTIONARY",
        summary: "Copy dictionary information from a data file or dataset to the active dataset.",
        syntax: "APPLY DICTIONARY FROM 'file' | dataset | * [/SOURCE VARIABLES=varlist]\n\
                 \x20                [/TARGET VARIABLES=varlist] [/NEWVARS]\n\
                 \x20                [/FILEINFO [ATTRIBUTES[=REPLACE | MERGE]] [DOCUMENTS[=...]] [FILELABEL]\n\
                 \x20                           [MRSETS[=...]] [VARSETS[=...]] [WEIGHT] [ALL]]\n\
                 \x20                [/VARINFO [ALIGNMENT] [ATTRIBUTES[=...]] [FORMATS] [LEVEL] [MISSING]\n\
                 \x20                          [ROLE] [VALLABELS[=REPLACE | MERGE]] [VARLABEL] [WIDTH] [ALL]].\n\
                 Variables match by name and type; with a different type only the label is copied.\n\
                 Without VARINFO everything but attributes is copied, value labels replacing; without\n\
                 FILEINFO only the weight. SOURCE with one variable and TARGET copies that variable's\n\
                 information to every target of the same type. Empty information in the source does\n\
                 not overwrite the active dataset's. NEWVARS adds the source variables the active dataset\n\
                 lacks, with their dictionary information and missing values; without an active dataset it\n\
                 makes one with the source's variables and no cases.",
        example: "APPLY DICTIONARY FROM 'template.sav' /FILEINFO VARSETS=MERGE MRSETS.",
        parse: dictionary::apply_dictionary,
    },
    CommandSpec {
        name: "GET DATA",
        summary: "Read a text data file or a sheet of a workbook as the active dataset.",
        syntax: "GET DATA /TYPE=TXT /FILE='path' [/ENCODING='name'] [/ARRANGEMENT=DELIMITED | FIXED]\n\
                 \x20        [/FIRSTCASE=n] [/DELCASE=LINE | VARIABLES n] [/FIXCASE=n] [/DELIMITERS=\"chars\"]\n\
                 \x20        [/QUALIFIER=\"c\"] [/IMPORTCASE=ALL | FIRST n] [/MULTIPLESPACES IGNORE=NO | YES]\n\
                 \x20        [/LEADINGSPACES IGNORE=NO | YES]\n\
                 \x20        /VARIABLES=name format [name format ...] [/MAP].\n\
                 GET DATA /TYPE=XLSX | XLSM | XLS /FILE='path' [/SHEET=INDEX n | NAME 'name']\n\
                 \x20        [/CELLRANGE=FULL | RANGE 'A1:K14'] [/READNAMES=ON | OFF] [/ASSUMEDSTRWIDTH=n]\n\
                 \x20        [/HIDDEN IGNORE=NO | YES] [/LEADINGSPACES IGNORE=NO | YES]\n\
                 \x20        [/TRAILINGSPACES IGNORE=NO | YES] [/DATATYPEMIN PERCENTAGE=n].\n\
                 Text files. FIXED: /VARIABLES=[/record] name first-last format ..., columns counted from 0.\n\
                 format: F, A, DATE, ADATE, SDATE, DATETIME, DOLLAR, COMMA and the other input formats, with\n\
                 or without a width; AUTO reads a number when every value is one, else a string as wide as the\n\
                 longest value; X skips the field. DELIMITERS: \\t is a tab, \\\\ a backslash; two delimiters\n\
                 in a row leave a value empty. Default delimiters: tab and comma. A value between\n\
                 qualifiers may hold delimiters and line breaks; a doubled qualifier is one. SET DECIMAL=COMMA\n\
                 reads a comma as the decimal mark. ENCODING as for INSERT; a byte order mark wins. MAP, the\n\
                 only subcommand allowed after VARIABLES, is accepted and shows nothing.\n\
                 Workbooks (XLSX and XLSM from Excel 2007 on, XLS from Excel 95 to 2003). The first sheet by\n\
                 default; FULL reads from the first to the last cell with a value. READNAMES=ON (default)\n\
                 takes the first row as names, made valid: other characters become _, a name that does not\n\
                 start with a letter gets a V in front, a repeated one _2; OFF and an empty heading give V1,\n\
                 V2, ... by the column in the range. A column is numeric (F, 0 or 2 decimals) when all its\n\
                 values are numbers (TRUE and FALSE are 1 and 0), a date when all are dates or times (DATE11,\n\
                 TIME8 or DATETIME20), else a string as wide as its longest value. DATATYPEMIN PERCENTAGE=n\n\
                 (over 50) lets the kind of n percent of the values decide; the others become system-missing.\n\
                 Empty rows and empty columns without a name are left out; error cells are empty. HIDDEN\n\
                 IGNORE=YES leaves out hidden rows and columns (XLSX only). ASSUMEDSTRWIDTH cuts longer\n\
                 strings (XLS: 255 by default). LEADINGSPACES IGNORE=YES removes leading blanks of strings;\n\
                 TRAILINGSPACES IGNORE=YES leaves trailing blanks out of the width.\n\
                 ODBC and OLEDB are not supported yet.",
        example: "GET DATA /TYPE=TXT /FILE='wave2.csv' /DELIMITERS=\",\" /QUALIFIER='\"' /FIRSTCASE=2\n\
                  \x20 /VARIABLES=id F5 name A20 born SDATE10.\n\
                  GET DATA /TYPE=XLSX /FILE='wave3.xlsx' /SHEET=NAME 'Answers' /CELLRANGE=RANGE 'A1:F200'.",
        parse: get_data::get_data,
    },
    CommandSpec {
        name: "SAVE TRANSLATE",
        summary: "Write the active dataset to a CSV or tab-delimited text file, an Excel workbook or a Stata file.",
        syntax: "SAVE TRANSLATE OUTFILE='path' | handle /TYPE=CSV | TAB | XLS /VERSION=12 | STATA [/REPLACE]\n\
                 \x20   [/FIELDNAMES [VALUE=NAMES | LABELS]] [/CELLS=VALUES | LABELS] [/MISSING=IGNORE | RECODE]\n\
                 \x20   [/TEXTOPTIONS [DELIMITER='c'] [QUALIFIER='c'] [DECIMAL=DOT | COMMA] [FORMAT=PLAIN | VARIABLE]]\n\
                 \x20   [/EXCELOPTIONS SHEET='name'] [/ENCODING='name' [BOM=YES | NO]] [/UNSELECTED=RETAIN | DELETE]\n\
                 \x20   [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...] [/MAP]\n\
                 \x20   [/VERSION=8 to 14] [/EDITION=INTERCOOLED | SE].\n\
                 TYPE may be left out for a .csv, .xlsx or .dta file; TYPE=XLSX is short for XLS /VERSION=12. An\n\
                 existing file is overwritten only with REPLACE. FIELDNAMES writes a first row of names\n\
                 (VALUE=LABELS: the variable labels, else the names). CELLS=LABELS writes the value label where\n\
                 a value has one. MISSING=RECODE writes user-missing values as empty. UNSELECTED=DELETE leaves\n\
                 out the cases FILTER leaves out. KEEP, DROP and RENAME shape the file only, in the order\n\
                 written; KEEP does not change the order of the variables. Each MAP lists the names in the\n\
                 file next to the names in the dataset, as the subcommands before it leave them.\n\
                 Text: system-missing is empty. FORMAT=PLAIN (default) writes numbers with the decimals they\n\
                 need, dates as mm/dd/yyyy and times as hh:mm:ss; VARIABLE writes the display formats. The\n\
                 decimal mark is DECIMAL's, else SET DECIMAL's. CSV separates values with a comma, or with a\n\
                 semicolon when the decimal mark is a comma, and puts the qualifier (default \") around a value\n\
                 that holds the delimiter, the qualifier (doubled inside) or a line break. TAB never qualifies.\n\
                 ENCODING as for INSERT; the default is UTF-8 with a byte order mark, which BOM=NO leaves out.\n\
                 Workbook: numbers keep their display format and dates and times are Excel dates;\n\
                 system-missing is #NULL!. EXCELOPTIONS names the sheet (default Sheet1); more cases than a\n\
                 sheet holds continue on further sheets.\n\
                 Stata: VERSION is the Stata release that reads the file (default 14; 8 and 9 give format\n\
                 113, 10 and 11 format 114, 12 format 115, 13 format 117, 14 and later format 118).\n\
                 INTERCOOLED (the default) keeps up to 2047 variables and 80 bytes of each string; SE keeps\n\
                 32767 variables and 244 bytes before Stata 13, whole strings from 13 on (over 2045 bytes as\n\
                 strL). Names keep their case: characters other than letters, digits and _ become _, and\n\
                 names are cut to 32 bytes and made unique; before Stata 14 a name that is not ASCII becomes\n\
                 v and its position. Variable labels and the file label keep 80 bytes. Value labels go with\n\
                 numeric variables, for whole values up to 2147483620 (80 bytes each for Stata 8). Dates\n\
                 become %td days since 1960, dates with a time %tc milliseconds; times and other numbers\n\
                 are plain. User-missing values stay valid values unless MISSING=RECODE. Text is UTF-8 from\n\
                 Stata 14 on, else in ENCODING (default: the system's code page).\n\
                 Other types (SAS, dBASE, ODBC, 1-2-3, SYLK) and APPEND are not supported yet.",
        example: "SAVE TRANSLATE OUTFILE='wave2.xlsx' /TYPE=XLS /VERSION=12 /FIELDNAMES /CELLS=LABELS /REPLACE.\n\
                  SAVE TRANSLATE OUTFILE='wave2.csv' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DECIMAL=COMMA /REPLACE.\n\
                  SAVE TRANSLATE OUTFILE='clinic.dta' /TYPE=STATA /EDITION=SE /DROP=nurse_note /MAP /REPLACE.",
        parse: save_translate::save_translate,
    },
    CommandSpec {
        name: "EXPORT",
        summary: "Write the active dataset to a portable (.por) file.",
        syntax: "EXPORT OUTFILE='path' | handle [/TYPE=COMM | TAPE] [/UNSELECTED=RETAIN | DELETE]\n\
                 \x20      [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...] [/MAP] [/DIGITS=n].\n\
                 KEEP, DROP and RENAME shape the file only, in the order written; KEEP sets the order of\n\
                 the variables. Each MAP shows the variables as the subcommands before it leave them.\n\
                 UNSELECTED=DELETE leaves out the cases FILTER leaves out. DIGITS rounds values that are\n\
                 not whole to n significant digits; without it every value is written exactly.\n\
                 The file holds names (upper case, at most 8 bytes: longer ones are shortened and made\n\
                 unique, as MYLONGRO and MYLONG_2), variable and value labels, missing values, formats,\n\
                 the weight variable and documents. Strings hold at most 255 bytes; text is UTF-8.\n\
                 TYPE is accepted and changes nothing; control characters in text are written as 0.",
        example: "EXPORT OUTFILE='wave2.por' /RENAME=(v1 TO v3 = id sex age) /MAP.",
        parse: portable::export,
    },
    CommandSpec {
        name: "IMPORT",
        summary: "Read a portable (.por) file and make it the active dataset.",
        syntax: "IMPORT FILE='path' | handle [/TYPE=COMM | TAPE] [/KEEP=varlist] [/DROP=varlist]\n\
                 \x20      [/RENAME=(old = new) ...] [/MAP].\n\
                 KEEP, DROP and RENAME work in the order written, as in GET; KEEP sets the order of the\n\
                 variables. Each MAP shows the variables as the subcommands before it leave them. The\n\
                 file's weight variable weights the cases. TYPE is accepted and changes nothing.",
        example: "IMPORT FILE='wave2.por' /DROP=dept TO division /MAP.",
        parse: portable::import,
    },
    CommandSpec {
        name: "GET TRANSLATE",
        summary: "Read a tab-delimited text file, a dBASE file or an old Excel workbook.",
        syntax: "GET TRANSLATE FILE='path' | handle [/TYPE=TAB | DBF | XLS] [/FIELDNAMES]\n\
                 \x20             [/RANGE=A1..K14 | 'A1:K14'] [/KEEP=varlist] [/DROP=varlist]\n\
                 \x20             [/RENAME=(old = new) ...] [/MAP].\n\
                 TYPE may be left out for a .tab, .dbf or .xls file. KEEP and DROP do not change the order\n\
                 of the variables. FIELDNAMES takes the names from the first row (of the range); they are\n\
                 made valid: other characters become _, a reserved word gets $ (GE$), a repeated name a\n\
                 number, and a column without a name is left out. RANGE limits the rows and columns.\n\
                 TAB: rows end with a line break and values with a tab. Names are VAR1, VAR2, ...\n\
                 without FIELDNAMES. The first value of a column decides its type: a number gives F8.2\n\
                 or wider, any other text a string of at least 8 bytes, as wide as that value; longer\n\
                 values are cut, and text in a numeric column is system-missing.\n\
                 DBF (dBASE II, III and IV): every record, also those marked deleted, which get * in an\n\
                 added string D_R. Character and logical fields are strings, numeric and float fields\n\
                 numbers (F with the field's width and decimals), date fields dates (DATE11); memo fields\n\
                 are left out. FIELDNAMES is ignored.\n\
                 XLS (Excel 95 to 2003, read as GET DATA reads it): the first sheet; names are the\n\
                 column letters without FIELDNAMES.\n\
                 1-2-3, Symphony and SYLK files (WK, WK1, WKS, WR1, WRK, SYM, SLK) are not supported.",
        example: "GET TRANSLATE FILE='address.dbf' /DROP=phoneno entry /MAP.\n\
                  GET TRANSLATE FILE='wave3.txt' /TYPE=TAB /FIELDNAMES.",
        parse: portable::get_translate,
    },
    CommandSpec {
        name: "GET STATA",
        summary: "Read a Stata data file (.dta) and make it the active dataset.",
        syntax: "GET STATA FILE='path' | handle [/ENCODING='name'].\n\
                 Reads formats 113 to 119 (Stata 8 to 19), in either byte order. ENCODING names the code\n\
                 page of text in files older than Stata 14 (LOCALE and SYSTEM: the system's); without it\n\
                 such text is UTF-8 when it reads as UTF-8, else in the system's code page. Newer files\n\
                 are always UTF-8.\n\
                 Names keep their case; one that differs from an earlier one only in case gets _A, _B, ...,\n\
                 one that starts with _ gets V in front and a reserved word gets $. Variable labels, value\n\
                 labels and the file label (cut to 64 bytes) come along. Every Stata missing value, . and\n\
                 .a to .z, is system-missing, and labels of .a to .z are dropped. %td dates become DATE11\n\
                 and %tc and %tC datetimes DATETIME20; weekly, monthly, quarterly, half-yearly and yearly\n\
                 values stay counts since 1960 in F format. Long strings (strL) become strings of up to\n\
                 32767 bytes; one holding binary data is blank.",
        example: "GET STATA FILE='clinic.dta'.\n\
                  GET STATA FILE='panel2009.dta' /ENCODING='windows-1257'.",
        parse: portable::get_stata,
    },
    CommandSpec {
        name: "TITLE",
        summary: "Put a title in the output, above the output that follows.",
        syntax: "TITLE [']text['].\n\
                 Quotes are optional; with them the text may hold the other kind of quote. The title\n\
                 is cut to 60 bytes and shows as a heading where the command stands.",
        example: "TITLE 'Wave 2: cleaned data'.",
        parse: output::title,
    },
    CommandSpec {
        name: "SUBTITLE",
        summary: "Put a subtitle in the output, above the output that follows.",
        syntax: "SUBTITLE [']text['].\n\
                 As TITLE, for a second heading. A blank subtitle (' ') shows nothing.",
        example: "SUBTITLE \"Children's shoes only\".",
        parse: output::subtitle,
    },
    CommandSpec {
        name: "ECHO",
        summary: "Show a text in the output.",
        syntax: "ECHO 'text'.\n\
                 Pieces joined with + show as one line.",
        example: "ECHO 'Checks done'.",
        parse: output::echo,
    },
    CommandSpec {
        name: "NEW FILE",
        summary: "Start a new, empty active dataset.",
        syntax: "NEW FILE.\n\
                 A named active dataset stays open under its name; an unnamed one is gone.",
        example: "NEW FILE.",
        parse: output::new_file,
    },
    CommandSpec {
        name: "CLEAR TRANSFORMATIONS",
        summary: "Undo the transformations and dictionary changes since the last data pass.",
        syntax: "CLEAR TRANSFORMATIONS.\n\
                 The dataset goes back to how it was after the last command that read the data\n\
                 (a procedure, EXECUTE, SAVE, GET and the like) or the start of the run. Pending\n\
                 PRINT, WRITE and XSAVE never run. Settings, titles and files stay as they are.",
        example: "CLEAR TRANSFORMATIONS.",
        parse: output::clear_transformations,
    },
    CommandSpec {
        name: "ERASE",
        summary: "Delete a file.",
        syntax: "ERASE FILE [=] 'path'.\n\
                 One file, named in full; no wildcards. A missing file is an error.",
        example: "ERASE FILE='old_extract.txt'.",
        parse: output::erase,
    },
    CommandSpec {
        name: "SYSFILE INFO",
        summary: "Show the dictionary of a data file without opening it as the active dataset.",
        syntax: "SYSFILE INFO [FILE=] 'path'.\n\
                 A table of the file (label, cases, variables, documents) and the tables of\n\
                 DISPLAY DICTIONARY. PASSWORD (encrypted files) is not supported yet.",
        example: "SYSFILE INFO FILE='survey.sav'.",
        parse: output::sysfile_info,
    },
    CommandSpec {
        name: "PERMISSIONS",
        summary: "Make a file read-only, or writeable for its owner.",
        syntax: "PERMISSIONS FILE='path' /PERMISSIONS READONLY | WRITEABLE.\n\
                 READONLY takes the write permission from everyone; WRITEABLE gives it to the owner.\n\
                 On a file system without such permissions READONLY sets the read-only flag.",
        example: "PERMISSIONS FILE='final.sav' /PERMISSIONS READONLY.",
        parse: output::permissions,
    },
    CommandSpec {
        name: "HOST",
        summary: "Refused: Statscraft never runs operating system commands from syntax.",
        syntax: "HOST COMMAND=['command' ...] [TIMELIMIT=n].\n\
                 Read and checked, then refused with an error: a command run from a syntax file\n\
                 could do anything the user can, which makes shared syntax a security risk. Run the\n\
                 commands outside Statscraft, before or after the syntax.",
        example: "HOST COMMAND=['dir'].",
        parse: output::host,
    },
    CommandSpec {
        name: "FINISH",
        summary: "End the job: the commands after it are not read.",
        syntax: "FINISH.\n\
                 The run ends here, as at the end of the syntax. Nothing after FINISH is read, so it causes\n\
                 no error. FINISH in an inserted file ends the whole run; when a range of lines runs, only a\n\
                 FINISH inside it counts. The window and its session stay open.",
        example: "FREQUENCIES VARIABLES=region.\nFINISH.",
        parse: output::finish,
    },
    CommandSpec {
        name: "DATE",
        summary: "Add date variables that count the cases of a time series.",
        syntax: "DATE keyword [start [period]] [keyword ...] [BY n].\n\
                 keyword: YEAR QUARTER MONTH (long periods), WEEK DAY HOUR MINUTE SECOND (short periods),\n\
                 CYCLE, OBS; any start of the word, MINUTE from MI. Long and short do not mix.\n\
                 Each keyword makes a numeric variable named keyword_; DATE_ (a string) shows them all,\n\
                 and CYCLE_ is added when the highest keyword has a period. Defaults: start 1 (HOUR,\n\
                 MINUTE, SECOND 0), period QUARTER 4, MONTH 12, DAY 7, HOUR 24, MINUTE and SECOND 60,\n\
                 none for the highest keyword except QUARTER. OBS needs a start and a period; CYCLE takes\n\
                 no period. QUARTER and MONTH give each other their start; with MONTH the quarter\n\
                 follows the month. BY n steps the lowest keyword and must divide its period. Earlier\n\
                 DATE variables are replaced. A table lists the new variables and their labels.",
        example: "DATE Y 1990 M 7.",
        parse: output::date,
    },
    CommandSpec {
        name: "TSET",
        summary: "Settings shared by the time series procedures.",
        syntax: "TSET [PRINT=BRIEF | DEFAULT | DETAILED] [NEWVAR=CURRENT | NONE | ALL] [MXAUTO=16]\n\
                 \x20    [MXCROSS=7] [MXNEWVARS=60] [MXPREDICT=60] [MISSING=EXCLUDE | INCLUDE] [CIN=95]\n\
                 \x20    [TOLER=0.0001] [CNVERGE=0.001] [ACFSE=IND | MA] [PERIOD=n] [ID=variable]\n\
                 \x20    [CONSTANT | NOCONSTANT] [DEFAULT].\n\
                 The values shown are the starting ones. Slashes and equals signs are optional, and DEFAULT\n\
                 as a value puts that one setting back; DEFAULT alone puts back all of them. A setting holds\n\
                 for the rest of the session, until TSET changes it; a subcommand of a procedure that does\n\
                 the same job wins for that procedure. Who reads what: ACF, PACF and CCF take MXAUTO,\n\
                 MXCROSS, ACFSE, PERIOD and MISSING; SEASON takes PERIOD, PRINT, NEWVAR, MXNEWVARS and\n\
                 MISSING; PPLOT takes PERIOD and MISSING; CREATE takes PERIOD (for SDIFF) and MISSING;\n\
                 CURVEFIT takes NEWVAR, MXNEWVARS, CIN, TOLER, CONSTANT, ID and MISSING; WLS takes NEWVAR,\n\
                 MXNEWVARS, PRINT and CONSTANT. NEWVAR: CURRENT saves the new variables of a procedure\n\
                 under names numbered from 1, replacing variables of those names; ALL saves them under the\n\
                 next free numbers; NONE saves nothing unless the procedure's SAVE asks. A procedure that\n\
                 would make more than MXNEWVARS variables saves none, with a warning. PERIOD wins over the\n\
                 period that DATE variables give. MXPREDICT and CNVERGE are kept and shown by TSHOW; no\n\
                 procedure that reads them is supported yet. NEWVARS is taken as NEWVAR.",
        example: "TSET PERIOD=4 MXAUTO=12 NEWVAR=ALL.",
        parse: timeseries::tset,
    },
    CommandSpec {
        name: "TSHOW",
        summary: "List the time series settings.",
        syntax: "TSHOW.\n\
                 A table with every TSET setting, the period the DATE variables of the active dataset give\n\
                 (if any), and the period the procedures use when they are not told one.",
        example: "TSHOW.",
        parse: timeseries::tshow,
    },
    CommandSpec {
        name: "CREATE",
        summary: "New series from functions of existing series: lags, differences, smoothers, FFT.",
        syntax: "CREATE new names = function(series [, numbers]) [/new names = function(...) ...].\n\
                 function: CSUM(series) running total; DIFF(series, order) differences; SDIFF(series, order\n\
                 [, period]) differences over a period; LAG(series, order [, order]) and LEAD(series, order\n\
                 [, order]) shifted copies, one per order of the range; MA(series, span [, minimum]) centered\n\
                 moving average; RMED(series, span [, minimum]) centered running median; PMA(series, span)\n\
                 mean of the span cases before; T4253H(series) compound smoother; FFT(series) real and\n\
                 imaginary parts; IFFT(real, imaginary) the series back.\n\
                 Commas or blanks separate the items in parentheses; v1 TO v4 names numbered new series. A\n\
                 function makes one series per source (LAG and LEAD: per source and order, sources outer;\n\
                 FFT two per source; IFFT one per pair), and the names on the left must match that count.\n\
                 An existing variable of the same name is replaced; a later equation may use a series an\n\
                 earlier one made. A value missing (user-missing too, unless TSET MISSING=INCLUDE) gives a\n\
                 missing result where it is used; MA, RMED and PMA restart after it. An even span is\n\
                 centered by averaging two neighbouring spans. Near the ends MA and RMED shrink the span\n\
                 symmetrically down to the minimum (of the same parity, raised by 1 otherwise; 1 is always\n\
                 allowed). SDIFF without a period uses TSET PERIOD, then the DATE period. T4253H, FFT and\n\
                 IFFT need a series without gaps inside it; FFT pads an odd length with a 0 and gives n/2\n\
                 cases (the first case's imaginary part holds the highest frequency's cosine term), IFFT\n\
                 twice as many as it reads. With SPLIT FILE each group is a series of its own; cases a\n\
                 filter leaves out keep their value (system-missing in a new series). A table lists every\n\
                 new series with its first and last valid case, its number of valid values and its function.",
        example: "CREATE sales_lag1 TO sales_lag3 = LAG(sales, 1, 3) /trend = MA(sales, 6, 2).",
        parse: timeseries::create,
    },
    CommandSpec {
        name: "PRINT",
        summary: "Show the values of variables for every case, as lines of text.",
        syntax: "PRINT [OUTFILE='path'] [ENCODING='name'] [RECORDS=n] [TABLE | NOTABLE]\n\
                 \x20     /[line] item [item ...] [/[line] ...].\n\
                 item: varlist, varlist first-last [(type[,decimals])], varlist (format list), 'text'\n\
                 [column], or * to end the variables a format applies to. Format list: formats such as\n\
                 F8.2, n(...) repeats, Tn goes to column n, nX skips n columns.\n\
                 A transformation: the lines appear when the data are next read, case by case, one line\n\
                 per slash. A variable in its own print format is followed by a blank; one with a given\n\
                 format or columns is not. System-missing shows as a period. OUTFILE writes the lines to a\n\
                 file, each starting with a blank (the carriage control), replaced on each data pass.\n\
                 TABLE shows where each variable goes. Formats P, PK, IB, PIB, PIBHEX, RB, RBHEX and Z are\n\
                 not supported yet.",
        example: "PRINT /id name (A10) score (F6.2).",
        parse: output::print,
    },
    CommandSpec {
        name: "PRINT EJECT",
        summary: "Like PRINT, starting a new page each time it runs.",
        syntax: "PRINT EJECT [OUTFILE='path'] [ENCODING='name'] [RECORDS=n] [TABLE | NOTABLE] /item ....\n\
                 As PRINT. In the output each new page is a new table; in a file its first line starts\n\
                 with 1 in place of the blank. Usually inside DO IF, for headings.",
        example: "DO IF $CASENUM = 1.\nPRINT EJECT /'Name' 1 'Score' 12.\nEND IF.",
        parse: output::print_eject,
    },
    CommandSpec {
        name: "PRINT SPACE",
        summary: "Add blank lines to the PRINT output, for every case it runs on.",
        syntax: "PRINT SPACE [OUTFILE='path'] [ENCODING='name'] [expression].\n\
                 The expression gives the number of lines (default 1); a fraction is cut off; a missing\n\
                 or negative number gives one line and a warning. Usually inside DO IF.",
        example: "PRINT SPACE.",
        parse: output::print_space,
    },
    CommandSpec {
        name: "WRITE",
        summary: "Write the values of variables for every case to a text file.",
        syntax: "WRITE [OUTFILE='path'] [ENCODING='name'] [BOM=YES | NO] [RECORDS=n] [TABLE | NOTABLE]\n\
                 \x20     /[line] item [item ...] [/[line] ...].\n\
                 Items as on PRINT, in the write formats, with no blank between values and no carriage\n\
                 control. System-missing is written as blanks. Without OUTFILE the lines go to the output.\n\
                 The file is replaced on each data pass; commands that name the same file in one pass\n\
                 write to it in case order. ENCODING: UTF-8 (default, no byte order mark), UTF-16,\n\
                 UTF-16LE, UTF-16BE or a code page such as windows-1252; a UTF encoding given by name\n\
                 gets a byte order mark unless BOM=NO.",
        example: "WRITE OUTFILE='scores.txt' /id (F4) score (F6.2).",
        parse: output::write,
    },
    CommandSpec {
        name: "PRINT FORMATS",
        summary: "Set the print format of variables: how procedures and PRINT show them.",
        syntax: "PRINT FORMATS varlist (format) [[/] varlist (format) ...].\n\
                 As FORMATS, but the write format stays as it is. A string variable keeps its width.",
        example: "PRINT FORMATS salary (DOLLAR8) / rate (PCT5.1).",
        parse: output::print_formats,
    },
    CommandSpec {
        name: "WRITE FORMATS",
        summary: "Set the write format of variables: how WRITE writes them.",
        syntax: "WRITE FORMATS varlist (format) [[/] varlist (format) ...].\n\
                 As FORMATS, but the print format stays as it is. Saved with the data file.",
        example: "WRITE FORMATS salary (F10.2).",
        parse: output::write_formats,
    },
    CommandSpec {
        name: "XSAVE",
        summary: "Write the cases to a data file as the next data pass reads them.",
        syntax: "XSAVE OUTFILE='path' | handle [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...]\n\
                 \x20     [/MAP] [/COMPRESSED | /UNCOMPRESSED | /ZCOMPRESSED] [/PERMISSIONS=READONLY | WRITEABLE].\n\
                 A transformation: the file holds the cases as they reach XSAVE in the next procedure or\n\
                 EXECUTE (or the end of the run), with the variables of that moment, scratch variables\n\
                 left out. KEEP, DROP and RENAME as on SAVE. MAP lists the saved variables. Two XSAVE\n\
                 commands cannot write the same file in one pass. Compression as on SAVE.",
        example: "XSAVE OUTFILE='wave2.sav' /DROP=tmp1 TO tmp5.",
        parse: output::xsave,
    },
];

/// Commands of the language that have no parser yet. They get "not supported yet" instead of
/// "unknown command", and their bodies are not tokenised.
static UNSUPPORTED: &[&str] = &[];

/// The ends of blocks whose start reads them, such as END DATA after BEGIN DATA. Standing
/// alone they are an error at their own line.
static STRAY: &[&str] = &[
    "END DATA",
    "END CHART",
    "END GPL",
    "END MATRIX",
    "END REPEAT",
];

/// Command names the manual wants written in full, never shortened.
static IN_FULL: &[&str] = &["SELECTPRED", "RESPONSE RATE"];

fn names() -> impl Iterator<Item = (&'static str, Option<&'static CommandSpec>)> {
    let supported = COMMANDS.iter().map(|spec| (spec.name, Some(spec)));
    let recognised = UNSUPPORTED.iter().chain(STRAY);
    supported.chain(recognised.map(|&name| (name, None)))
}

/// True when `word` is `keyword` or an abbreviation of it with at least three letters.
pub(crate) fn matches_keyword(word: &str, keyword: &str) -> bool {
    let (word, keyword) = (word.as_bytes(), keyword.as_bytes());
    let abbreviated = word.len() >= 3 && word.len() <= keyword.len();
    word.eq_ignore_ascii_case(keyword)
        || (abbreviated && keyword[..word.len()].eq_ignore_ascii_case(word))
}

/// The usize is the number of tokens (for `resolve`: words) that form the command name.
pub(crate) enum Resolution {
    Supported(&'static CommandSpec, usize),
    Unsupported(&'static str, usize),
    Ambiguous(Vec<&'static str>),
    Unknown,
}

/// Finds the command whose name words match the leading `words`. The longest name wins, and a
/// name written in full wins over an abbreviation.
pub(crate) fn resolve(words: &[&str]) -> Resolution {
    let mut best = Vec::new();
    let mut best_rank = (0, false);
    for (name, spec) in names() {
        let parts: Vec<&str> = name.split(' ').collect();
        let pairs = || parts.iter().zip(words);
        if parts.len() > words.len() || !pairs().all(|(part, word)| matches_keyword(word, part)) {
            continue;
        }
        let exact = pairs().all(|(part, word)| word.eq_ignore_ascii_case(part));
        if !exact && IN_FULL.contains(&name) {
            continue;
        }
        let rank = (parts.len(), exact);
        if rank > best_rank {
            best.clear();
            best_rank = rank;
        }
        if rank == best_rank {
            best.push((name, spec));
        }
    }
    match best.as_slice() {
        [] => Resolution::Unknown,
        [(_, Some(spec))] => Resolution::Supported(spec, best_rank.0),
        [(name, None)] => Resolution::Unsupported(name, best_rank.0),
        many => Resolution::Ambiguous(many.iter().map(|(name, _)| *name).collect()),
    }
}

/// The leading words of a command, each with the number of tokens up to and including it.
/// `T-TEST` and `K-M` arrive as three tokens and are joined into one word here.
pub(crate) fn name_words(tokens: &[Token]) -> Vec<(String, usize)> {
    let word = |i: usize| match tokens.get(i).map(|t| &t.kind) {
        Some(TokenKind::Word(w)) => Some(w),
        _ => None,
    };
    let mut words = Vec::new();
    let mut i = 0;
    while let Some(w) = word(i) {
        let minus = tokens.get(i + 1).map(|t| &t.kind) == Some(&TokenKind::Punct(Punct::Minus));
        if let Some(test) = word(i + 2)
            && minus
            && ((w.eq_ignore_ascii_case("T") && test.eq_ignore_ascii_case("TEST"))
                || (w.eq_ignore_ascii_case("K") && test.eq_ignore_ascii_case("M")))
        {
            i += 3;
            words.push((format!("{}-{}", w.to_uppercase(), test.to_uppercase()), i));
        } else {
            i += 1;
            words.push((w.clone(), i));
        }
    }
    words
}

pub(crate) fn resolve_tokens(tokens: &[Token]) -> Resolution {
    let words = name_words(tokens);
    let strs: Vec<&str> = words.iter().map(|(w, _)| w.as_str()).collect();
    match resolve(&strs) {
        Resolution::Supported(spec, n) => Resolution::Supported(spec, words[n - 1].1),
        Resolution::Unsupported(name, n) => Resolution::Unsupported(name, words[n - 1].1),
        other => other,
    }
}

fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut diagonal = row[0];
        row[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let substitute = diagonal + usize::from(ca != *cb);
            diagonal = row[j + 1];
            row[j + 1] = substitute.min(diagonal + 1).min(row[j] + 1);
        }
    }
    row[b.len()]
}

/// Command names close to the unknown name that starts with `words` (not empty).
pub(crate) fn suggest(words: &[&str]) -> Vec<&'static str> {
    let upper: Vec<String> = words.iter().map(|w| w.to_uppercase()).collect();
    let typed =
        |from: usize, to: usize| upper[from.min(upper.len())..to.min(upper.len())].join(" ");
    let close = |typed: String, target: &str| {
        let distance = levenshtein(&typed, target);
        (distance <= (target.len() / 3).max(1)).then_some(distance)
    };
    // multi-word names that share the typed first word: the mistake is in a later word
    let family: Vec<(&str, &str)> = names()
        .filter_map(|(name, _)| {
            name.split_once(' ')
                .map(|(first, rest)| (name, first, rest))
        })
        .filter(|(_, first, _)| matches_keyword(words[0], first))
        .map(|(name, _, rest)| (name, rest))
        .collect();
    let mut scored: Vec<(usize, &str)> = if family.is_empty() {
        names()
            .filter_map(|(name, _)| {
                close(typed(0, name.split(' ').count()), name).map(|d| (d, name))
            })
            .collect()
    } else {
        family
            .iter()
            .filter_map(|(name, rest)| {
                close(typed(1, name.split(' ').count()), rest).map(|d| (d, *name))
            })
            .collect()
    };
    scored.sort();
    let best = scored.first().map(|(d, _)| *d);
    let near: Vec<&str> = scored
        .into_iter()
        .take_while(|(d, _)| Some(*d) == best)
        .map(|(_, name)| name)
        .take(3)
        .collect();
    if near.is_empty() {
        family.into_iter().map(|(name, _)| name).take(6).collect()
    } else {
        near
    }
}

/// The names of the commands that can be parsed, in help order.
pub fn command_names() -> Vec<&'static str> {
    COMMANDS.iter().map(|spec| spec.name).collect()
}

/// The command whose name the leading `words` spell, abbreviations allowed, supported or not,
/// with the number of words its name takes. None when no single command matches.
pub fn command_name(words: &[&str]) -> Option<(&'static str, usize)> {
    match resolve(words) {
        Resolution::Supported(spec, used) => Some((spec.name, used)),
        Resolution::Unsupported(name, used) => Some((name, used)),
        _ => None,
    }
}

/// Every supported command in help order, for tools that render the help, such as the docs.
pub fn command_specs() -> &'static [CommandSpec] {
    COMMANDS
}

/// The commands of the language that are recognised but not supported yet.
pub fn unsupported_commands() -> &'static [&'static str] {
    UNSUPPORTED
}

/// Help text, generated from the table the parser uses. `help(None)` lists the supported
/// commands with one-line summaries and the general rules of the language. A command name may
/// be abbreviated like in syntax. None for a name that is no command of the language.
pub fn help(command: Option<&str>) -> Option<String> {
    let Some(command) = command else {
        let mut text = String::from("Supported commands:\n");
        // the longest name plus two spaces, so a long name never runs into its summary
        let width = COMMANDS.iter().map(|s| s.name.len()).max().unwrap_or(0) + 2;
        for spec in COMMANDS {
            text.push_str(&format!("  {:<width$}{}\n", spec.name, spec.summary));
        }
        text.push_str(GENERAL_HELP);
        return Some(text);
    };
    let words: Vec<&str> = command.split_whitespace().collect();
    match resolve(&words) {
        Resolution::Supported(spec, used) if used == words.len() => {
            let syntax = spec.syntax.replace('\n', "\n  ");
            Some(format!(
                "{}: {}\n\nSyntax:\n  {}\n\nExample:\n  {}\n",
                spec.name, spec.summary, syntax, spec.example
            ))
        }
        Resolution::Unsupported(name, used) if used == words.len() => Some(match name {
            "END DATA" => "END DATA ends the inline data that BEGIN DATA starts.\n".into(),
            "END CHART" => "END CHART ends the chart that BEGIN CHART starts.\n".into(),
            "END GPL" => "END GPL ends the chart program that BEGIN GPL starts.\n".into(),
            "END REPEAT" => "END REPEAT ends the commands that DO REPEAT repeats.\n".into(),
            "END MATRIX" => "END MATRIX ends the program that MATRIX starts.\n".into(),
            _ => format!("{name} is a command of the language, but it is not supported yet.\n"),
        }),
        _ => None,
    }
}

const GENERAL_HELP: &str = "
General rules:
  A command ends at a line whose last character is a period, at a blank line, or at the end of input.
  Command names and keywords are case-insensitive and may be shortened to three or more letters.
  varlist: variable names, a range written as first TO last, or ALL.
  Strings use single or double quotes; double the quote character to include it. A string may
  run on over line breaks, which it keeps; 'a' + 'b' joins two strings into one.
  Comments: a command starting with * or COMMENT, or /* text */ inside a line.
  Expressions: + - * / **, = <> < > <= >= (or EQ NE LT GT LE GE), AND OR NOT (or & | ~),
  functions such as MEAN(a, b) or SUM(q1 TO q5); LAG(var [, n]) is the value n cases back.
  A function name may be shortened to three or more letters that start no other function.
  Scratch variables start with #: they begin at 0 or blank, keep their value from case to case,
  are never saved and are gone when the next procedure starts. $CASENUM is the case number.

Functions of expressions (COMPUTE, IF, SELECT IF, DO IF, ELSE IF, LOOP IF, END LOOP IF):
  Arithmetic: ABS RND TRUNC MOD SQRT EXP LN LG10 LNGAMMA SIN COS TAN ARSIN ARCOS ARTAN.
  Statistical, with .n for the valid values needed (MEAN.2): SUM MEAN MEDIAN MIN MAX SD VARIANCE
  CFVAR. Missing values: MISSING SYSMIS VALUE NMISS NVALID. Logical: ANY RANGE. Also LAG and
  VALUELABEL.
  Strings: CONCAT LENGTH CHAR.LENGTH UPCASE LOWER LTRIM RTRIM NTRIM SUBSTR CHAR.SUBSTR INDEX
  CHAR.INDEX RINDEX CHAR.RINDEX LPAD CHAR.LPAD RPAD CHAR.RPAD REPLACE STRUNC CHAR.MBLEN
  MBLEN.BYTE NORMALIZE NUMBER STRING.
  Dates and times: DATE.DMY DATE.MDY DATE.MOYR DATE.QYR DATE.WKYR DATE.YRDAY YRMODA TIME.DAYS
  TIME.HMS CTIME.DAYS CTIME.HOURS CTIME.MINUTES CTIME.SECONDS XDATE.DATE XDATE.HOUR XDATE.JDAY
  XDATE.MDAY XDATE.MINUTE XDATE.MONTH XDATE.QUARTER XDATE.SECOND XDATE.TDAY XDATE.TIME
  XDATE.WEEK XDATE.WKDAY XDATE.YEAR DATEDIFF DATESUM (units years quarters months weeks days
  hours minutes seconds; DATESUM methods 'closest', the default, and 'rollover').
  Distributions: CDFNORM PROBIT, and CDF.d IDF.d PDF.d NCDF.d NPDF.d SIG.d RV.d where the
  distribution d has them: BERNOULLI BETA BINOM BVNOR CAUCHY CHISQ EXP F GAMMA GEOM HALFNRM HYPER
  IGAUSS LAPLACE LNORMAL LOGISTIC NEGBIN NORMAL PARETO POISSON SMOD SRANGE T UNIFORM WEIBULL
  (NCDF and NPDF: BETA CHISQ F T; SIG: CHISQ F). UNIFORM(max), NORMAL(sd) and the RV functions
  draw random numbers. An argument out of range gives system-missing.
";
