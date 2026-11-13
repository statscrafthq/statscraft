//! The command table: the single source for command name matching, parsing and help.

use crate::anova;
use crate::ast::Command;
use crate::cluster;
use crate::commands;
use crate::complex_samples;
use crate::correspondence;
use crate::ctables;
use crate::data_commands;
use crate::data_list;
use crate::dictionary;
use crate::discriminant;
use crate::examine;
use crate::factor;
use crate::files;
use crate::frequencies;
use crate::genlin;
use crate::get_data;
use crate::glm;
use crate::graph;
use crate::input_program;
use crate::lexer::{Punct, Token, TokenKind};
use crate::logistic;
use crate::loops;
use crate::matrix;
use crate::matrix_data;
use crate::mixed;
use crate::mrsets;
use crate::mult_response;
use crate::nomreg;
use crate::npar;
use crate::output;
use crate::output_docs;
use crate::output_modify;
use crate::parser::{PResult, Parser};
use crate::plum;
use crate::portable;
use crate::pplot;
use crate::procedures;
use crate::rank;
use crate::regression;
use crate::reliability;
use crate::report;
use crate::restructure;
use crate::save_translate;
use crate::selection;
use crate::session_commands;
use crate::stats;
use crate::tree;

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
                 \x20  [/PRINT=[DESCRIPTIVE] [HOMOGENEITY] [PARAMETER] [ETASQ] [OPOWER] [LOF]]\n\
                 \x20  [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]\n\
                 \x20            [COMPARE[(factor)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...\n\
                 \x20  [/POSTHOC=factor ... (tests)] ... [/CONTRAST(factor)=DEVIATION[(ref)] | SIMPLE[(ref)] |\n\
                 \x20  DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)]] ...\n\
                 \x20  [/LMATRIX=['label'] effect coefficients ... [; ...] | ALL coefficients] ... [/KMATRIX=values]\n\
                 \x20  [/SAVE=tempvar[(name)] ...] [/DESIGN=effects]\n\
                 \x20  [/PLOT=[SPREADLEVEL] [RESIDUALS] [PROFILE(factor[*factor[*factor]] ...) [WITH(covariate=value | MEAN ...)]\n\
                 \x20         [TYPE=LINE | BAR] [MEANREFERENCE=NO | YES] [ERRORBAR=NO | CI | SE[(2)]] [YAXIS=AUTO | 0]]] ...\n\
                 effects: INTERCEPT, a factor or covariate, a*b or a BY b, a(b) or a WITHIN b, a(b(c)), x*x, x*a.\n\
                 tempvar: PRED WPRED RESID WRESID DRESID ZRESID SRESID SEPRED COOK LEVER. tests: as in ONEWAY\n\
                 /POSTHOC (LSD BONFERRONI SIDAK TUKEY SCHEFFE GH T2 SNK BTUKEY DUNCAN GT2 GABRIEL FREGW QREGW T3 C\n\
                 DUNNETT DUNNETTL DUNNETTR). Without DESIGN the model is the intercept, the covariates and the full\n\
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
                 PLOT gives chart data. PROFILE draws the estimated marginal means as a line chart (bars with\n\
                 TYPE=BAR) over the levels of the first factor, one line per level of a second factor and one chart\n\
                 per level of a third; MEANREFERENCE adds the grand mean as a flat line. ERRORBAR=CI spans each\n\
                 mean's confidence interval at 1 minus CRITERIA ALPHA, ERRORBAR=SE(n) n standard errors on each\n\
                 side (2 by default); YAXIS is left to the drawing. SPREADLEVEL plots the mean of every\n\
                 cell with two cases or more against its standard deviation, then against its variance. RESIDUALS\n\
                 gives every case's predicted value, observed value and standardized residual. With WSFACTOR, PLOT\n\
                 is ignored with a warning. Not supported yet: GLM with several\n\
                 dependents without WSFACTOR (the multivariate model), RANDOM, TEST, ROBUST, OUTFILE, MMATRIX,\n\
                 MEASURE, POSTHOC VS, CONTRAST SPECIAL, PRINT GEF, TEST(LMATRIX), MBP, BP, F and WHITE; with\n\
                 WSFACTOR also EMMEANS (left out with a warning), POSTHOC, CONTRAST, LMATRIX, SAVE and PRINT\n\
                 PARAMETER, HOMOGENEITY and LOF.")
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
                 variable is a series in case order: missing values are left out, LN logs it first (0 and\n\
                 below become missing), DIFF and SDIFF (over PERIOD cases; without PERIOD SDIFF is left\n\
                 out) difference it, STANDARDIZE makes z-scores of the result. Ranks of tied values follow\n\
                 TIES (BREAK numbers the distinct values 1, 2, ...); FRACTION turns rank r of n into a\n\
                 proportion p: BLOM (r-3/8)/(n+1/4), RANKIT (r-1/2)/n, TUKEY (r-1/3)/(n+1/3), VW r/(n+1).\n\
                 Q-Q (the default) plots each value against the distribution's quantile at p, P-P plots p\n\
                 against the distribution function at the value, both with the line y = x; the detrended\n\
                 plot shows observed minus expected, with the line y = 0. BOTH draws both plots. Values the\n\
                 distribution cannot take leave the variable out with a warning. Weights are not used.\n\
                 APPLY repeats the previous PPLOT (its distribution without the parameters, and its\n\
                 variables unless new ones are given); the subcommands given with it override.\n\
                 Not supported yet: APPLY of a named model, TEMPLATE.";

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
                 Formats: F, N, E, COMMA, DOT, DOLLAR, PCT, A, DATE, ADATE, EDATE, SDATE.\n\
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
        syntax: "GET FILE='path' | handle [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...].\n\
                 KEEP, DROP and RENAME work in the order written, as in ADD FILES.",
        example: "GET FILE='survey.sav'.",
        parse: commands::get,
    },
    CommandSpec {
        name: "SAVE",
        summary: "Write the active dataset to a data file.",
        syntax: "SAVE OUTFILE='path' | handle [/COMPRESSED | /UNCOMPRESSED | /ZCOMPRESSED] [/KEEP=varlist] [/DROP=varlist]\n\
                 \x20    [/RENAME=(old = new) ...].\n\
                 KEEP, DROP and RENAME shape the saved file only; the active dataset stays as it is.",
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
        syntax: "AGGREGATE [/OUTFILE=* | 'path' | dataset [MODE=ADDVARIABLES | REPLACE]] [/BREAK=varlist [(A | D)] ...]\n\
                 \x20         /target ['label'] [target ...] = function(varlist) [/target ...].\n\
                 OUTFILE=* replaces the active dataset with one case per break group (break variables, then\n\
                 targets); with MODE=ADDVARIABLES it keeps the cases and adds the targets to each of them.\n\
                 OUTFILE='path' writes the aggregated dataset to a file and leaves the active dataset alone;\n\
                 OUTFILE=dataset puts it into a dataset that DATASET DECLARE or DATASET NAME defined.\n\
                 Without OUTFILE the targets are added to every case, as with OUTFILE=* MODE=ADDVARIABLES.\n\
                 functions: SUM MEAN SD MEDIAN MIN MAX FIRST LAST N NU NMISS NUMISS; N and NU also without (varlist).\n\
                 Missing values are skipped. With a weight variable SUM, MEAN, SD, MEDIAN, N and NMISS are weighted.",
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
                 SPLIT FILE after it are undone once the next command that reads the data (a procedure,\n\
                 EXECUTE or SAVE) has run, or at the end of the run. Variables that procedure adds stay.",
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
        syntax: "DESCRIPTIVES [/]VARIABLES=varlist [/STATISTICS=stat ...] [/MISSING=VARIABLE | LISTWISE [INCLUDE]] [/SAVE].\n\
                 stat: MEAN SEMEAN STDDEV VARIANCE KURTOSIS SKEWNESS RANGE MINIMUM MAXIMUM SUM DEFAULT ALL.\n\
                 DEFAULT is MEAN STDDEV MINIMUM MAXIMUM; N is always shown. Weighted when a weight variable is set.\n\
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
                 \x20         [/CELLS=COUNT ROW COLUMN TOTAL EXPECTED RESID SRESID ASRESID ALL NONE]\n\
                 \x20         [/STATISTICS=CHISQ] [/MISSING=TABLE | INCLUDE]\n\
                 \x20         [/FORMAT=AVALUE | DVALUE TABLES | NOTABLES]\n\
                 \x20         [/COUNT=[ASIS | CASE | CELL] [ROUND | TRUNCATE]] [/BARCHART].\n\
                 One table per pair of row and column variable; layer variables split it into blocks.\n\
                 NOTABLES leaves out the tables, as CELLS=NONE does, keeping the statistics.\n\
                 COUNT makes weighted counts whole: CASE every case's weight, CELL every cell's count, before\n\
                 anything is computed; ROUND or TRUNCATE. A keyword left out is CELL or ROUND; without COUNT,\n\
                 ASIS, weights stay as they are.\n\
                 CELLS defaults to COUNT; CELLS alone is COUNT ROW COLUMN TOTAL. Percentages are 0 to 100.\n\
                 STATISTICS alone is CHISQ. DVALUE orders the rows from the highest value; columns stay ascending.\n\
                 CHISQ: Pearson, likelihood ratio, linear-by-linear (numeric variables), and for a 2 x 2\n\
                 table Fisher's exact test and the continuity correction.\n\
                 MISSING=TABLE leaves out a case missing on any variable of the table; INCLUDE keeps\n\
                 user-missing values as categories. Fisher's test takes whole counts: rounded, or truncated with TRUNCATE.\n\
                 BARCHART adds a clustered bar chart per table: a cluster per row category, a bar per column\n\
                 category, the counts of the table summed over its layers.",
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
        syntax: "ONEWAY deps BY factor [/STATISTICS=DESCRIPTIVES HOMOGENEITY WELCH BROWNFORSYTHE ALL NONE]\n\
                 \x20      [/CONTRAST=coefficients] [/POSTHOC=test ... [ALPHA(0.05)]]\n\
                 \x20      [/MISSING=ANALYSIS | LISTWISE [INCLUDE | EXCLUDE]] [/CRITERIA=CILEVEL(0.95)] [/PLOT MEANS].\n\
                 test: LSD BONFERRONI SIDAK TUKEY SCHEFFE GT2 GABRIEL GH T2 T3 C (pairwise comparisons),\n\
                 DUNNETT DUNNETTL DUNNETTR [(refcat)] (each group against the reference group, by default the last;\n\
                 L: smaller, R: larger), SNK BTUKEY DUNCAN FREGW QREGW (homogeneous subsets only); TUKEY SCHEFFE\n\
                 GT2 GABRIEL give both. DUNCAN and the Dunnett keywords are written in full. Every value of the\n\
                 factor is a group. CONTRAST gives one coefficient per group in ascending order (n*c repeats c n\n\
                 times); it is tested with pooled and separate variances. HOMOGENEITY is Levene's test based on\n\
                 the mean. Multiple Comparisons show every ordered pair of groups (Dunnett C has no significance).\n\
                 Homogeneous subsets use the harmonic mean of the group sizes (the R-E-G-W tests the sizes\n\
                 themselves); Tukey B shows no significance. Weighted when a weight variable is set. PLOT MEANS\n\
                 gives each dependent a line chart through the mean of every group, as chart data.\n\
                 Not supported yet: POLYNOMIAL, RANGES, EFFECTS, WALLER.",
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
                 \x20          [/REGWGT=var] [/STATISTICS=[DEFAULTS R ANOVA COEFF OUTS ZPP CHANGE CI[(95)] BCOV COLLIN TOL\n\
                 \x20          F SES XTX SELECTION LABEL ALL]] [/CRITERIA=[DEFAULTS PIN[(0.05)] | FIN[(3.84)] POUT[(0.10)] |\n\
                 \x20          FOUT[(2.71)] TOLERANCE[(0.0001)] MAXSTEPS(n) CIN[(95)]]] [/ORIGIN | NOORIGIN]\n\
                 \x20          /DEPENDENT=varlist /[METHOD=]ENTER [varlist] | REMOVE varlist | STEPWISE [varlist] |\n\
                 \x20          FORWARD [varlist] | BACKWARD [varlist] [/METHOD=...]\n\
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
                 is ignored with a warning. LABEL changes nothing. Not supported yet: METHOD=TEST, SELECT, MATRIX, OUTFILE.",
        example: "REGRESSION /STATISTICS=COEFF OUTS R ANOVA CHANGE /DEPENDENT=spend /METHOD=ENTER age /METHOD=STEPWISE q1 TO q5 /SAVE=PRED ZRESID.",
        parse: regression::regression,
    },
    CommandSpec {
        name: "RELIABILITY",
        summary: "Reliability of a scale: Cronbach's alpha, split-half and other models, item statistics and ICC.",
        syntax: "RELIABILITY /VARIABLES=varlist [/SCALE(label)=varlist | ALL]\n\
                 \x20           [/MODEL=ALPHA | SPLIT[(n)] | GUTTMAN | PARALLEL | STRICTPARALLEL]\n\
                 \x20           [/STATISTICS=DESCRIPTIVE SCALE CORR COV ANOVA [FRIEDMAN | COCHRAN] TUKEY HOTELLING ALL]\n\
                 \x20           [/SUMMARY=MEANS VARIANCE COV CORR TOTAL ALL]\n\
                 \x20           [/ICC=[MODEL(MIXED | RANDOM | ONEWAY)] [TYPE(CONSISTENCY | ABSOLUTE)] [CIN=95] [TESTVAL=0]]\n\
                 \x20           [/METHOD=COVARIANCE] [/MISSING=EXCLUDE | INCLUDE].\n\
                 A case missing on any variable of VARIABLES is left out. ALPHA (default) gives Cronbach's alpha\n\
                 and the alpha of the standardized items; SPLIT puts n items (default half, the odd one in the\n\
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
                 Weighted when a weight variable is set. Not supported yet: MODEL=OMEGA, MATRIX.",
        example: "RELIABILITY /VARIABLES=q1 TO q10 /SCALE('Satisfaction') ALL /MODEL=ALPHA /STATISTICS=DESCRIPTIVE SCALE /SUMMARY=TOTAL.",
        parse: reliability::reliability,
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
        name: "LOGISTIC REGRESSION",
        summary: "Binary logistic regression with categorical covariates, stepwise selection and case diagnostics.",
        syntax: "LOGISTIC REGRESSION [VARIABLES=] dependent [WITH var | var BY var ...]\n\
                 \x20  [/CATEGORICAL=varlist] [/CONTRAST(var)=INDICATOR[(ref)] | DEVIATION[(ref)] | SIMPLE[(ref)] |\n\
                 \x20  DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)] | SPECIAL(matrix)]\n\
                 \x20  [/METHOD=ENTER | FSTEP[(COND | LR | WALD)] | BSTEP[(COND | LR | WALD)] [ALL | terms]] ...\n\
                 \x20  [/ORIGIN | NOORIGIN] [/ID=var] [/PRINT=[DEFAULT] [SUMMARY] [CORR] [ITER[(n)]] [GOODFIT] [CI(95)] [ALL]]\n\
                 \x20  [/CRITERIA=[BCON(0.001)] [ITERATE(20)] [LCON(0)] [PIN(0.05)] [POUT(0.10)] [EPS(1E-8)] [CUT(0.5)]]\n\
                 \x20  [/MISSING=EXCLUDE | INCLUDE] [/CASEWISE=[tempvars] [OUTLIER(2)]]\n\
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
                 by the observed category. EXTERNAL changes nothing. Not supported yet: SELECT, OUTFILE.",
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
                 \x20  [/SCALE=1 | n | DEVIANCE | PEARSON] [/SUBPOP=varlist] [/MISSING=EXCLUDE | INCLUDE].\n\
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
                 can be the null model of equal probabilities. Not supported yet: TEST, OUTFILE and PRINT\n\
                 CELLPROB, HISTORY and ASSOCIATION.",
        example: "NOMREG commute (BASE=FIRST) BY district WITH distance /PRINT=PARAMETER LRT FIT SUMMARY CLASSTABLE /SAVE=ESTPROB PREDCAT.",
        parse: nomreg::nomreg,
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
                 takes one value per threshold, ALL one per parameter. Each row is tested against its constant (0\n\
                 by default) on 1 df and the rows together on the rank of their covariance; a coefficient on a\n\
                 redundant parameter multiplies its 0.",
        example: "PLUM comfort BY seat_row WITH trip_hours /LINK=PROBIT /PRINT=FIT PARAMETER SUMMARY TPARALLEL /SAVE=ESTPROB PREDCAT.",
        parse: plum::plum,
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
                 \x20  [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/PRINT [CPS] [DESCRIPTIVES] [MODELINFO] [FIT] [SUMMARY] [SOLUTION[(EXPONENTIATED)]] [COVB] [CORB] | NONE]\n\
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
                 Not supported yet: REPEATED (generalized estimating equations), OUTFILE, TWEEDIE, INITIAL,\n\
                 CITYPE=PROFILE and PRINT GEF, HISTORY, LAGRANGE and LMATRIX.",
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
        summary: "General linear model: one dependent variable as in UNIANOVA, or repeated measures with WSFACTOR.",
        syntax: glm_syntax!("GLM"),
        example: "GLM t1 t2 t3 BY group /WSFACTOR=time 3 POLYNOMIAL /WSDESIGN=time /DESIGN=group.",
        parse: glm::glm,
    },
    CommandSpec {
        name: "MIXED",
        summary: "Linear mixed model: fixed effects with random effects of subjects and correlated repeated measures.",
        syntax: "MIXED dependent [BY factors] [WITH covariates]\n\
                 \x20  [/FIXED=[effects] [| [NOINT] [SSTYPE(3)]]] [/METHOD=REML | ML] [/MISSING=EXCLUDE | INCLUDE]\n\
                 \x20  [/RANDOM=effects [| [SUBJECT(var[*var...])] [COVTYPE(type)]]] ...\n\
                 \x20  [/REPEATED=var[*var...] | SUBJECT(var[*var...]) [COVTYPE(type)]] [/REGWGT=var]\n\
                 \x20  [/CRITERIA=[CIN(95)] [MXITER(100)] [MXSTEP(10)] [SCORING(1)] [SINGULAR(1E-12)]\n\
                 \x20            [PCONVERGE(1E-6[, ABSOLUTE | RELATIVE])] [LCONVERGE(0[, ...])] [HCONVERGE(0[, ...])]\n\
                 \x20            [DFMETHOD(SATTERTHWAITE | RESIDUAL)]]\n\
                 \x20  [/PRINT=[SOLUTION] [TESTCOV] [G] [R] [COVB] [CORB]] [/SAVE=tempvar[(name)] ...]\n\
                 \x20  [/EMMEANS=TABLES(OVERALL | factor[*factor...]) [WITH(covariate=value | MEAN ...)]\n\
                 \x20            [COMPARE[(factor)] [REFCAT(value | FIRST | LAST)] [ADJ(LSD | BONFERRONI | SIDAK)]]] ...\n\
                 \x20  [/TEST[(constants)]=['label'] effect values ... [DIVISOR=n]; ... | ALL values ...] ...\n\
                 effects: INTERCEPT (first), a factor or covariate, a*b or a BY b, a(b) or a WITHIN b.\n\
                 type: VC (the RANDOM default: one variance per effect), ID (one variance for the block), DIAG (the\n\
                 REPEATED default: one variance per level), UN (every variance and covariance), CS (a diagonal\n\
                 offset plus one common covariance), AR1 (a variance and a correlation that is raised to the\n\
                 power of the distance between levels). VC on REPEATED is DIAG. tempvar: FIXPRED (X b), PRED\n\
                 (X b plus the predicted random effects), RESID (dependent minus PRED), SEFIXP and DFFIXP (the\n\
                 standard error and degrees of freedom of FIXPRED).\n\
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
                 fixed parameters and only covariance parameters count), Type III Tests of Fixed Effects,\n\
                 Estimates of Fixed Effects (SOLUTION) and Estimates of Covariance Parameters (with TESTCOV the\n\
                 standard errors from the observed Hessian, Wald Z and intervals on the log scale for variances\n\
                 and the Fisher z scale for correlations). Degrees of freedom are Satterthwaite's, or the cases\n\
                 less the rank of the fixed design with DFMETHOD(RESIDUAL). EMMEANS averages the fitted cells\n\
                 equally over the factors left out, covariates at their means or the WITH values; each mean and\n\
                 each difference gets its own degrees of freedom. COMPARE lists every pair of levels, or each\n\
                 level against REFCAT, with Bonferroni or Sidak adjustment over that number of comparisons, and\n\
                 an F test of the compared factor at each level of the others. TEST takes coefficients for the\n\
                 parameters of named fixed effects (redundant ones included) or ALL of them, one row per\n\
                 combination; the constants in parentheses are the values tested against (0 by default) and\n\
                 DIVISOR divides a row. It gives each row's estimate with a t test and the joint F test; a row\n\
                 that is not estimable is dropped with a warning. PRINT G and R show the covariance of one\n\
                 subject's random effects and repeated levels, COVB and CORB the covariance and correlation of\n\
                 the fixed estimates. Not supported yet: TEST coefficients for random effects, SSTYPE(1),\n\
                 DFMETHOD(KENWARDROGER), COVTYPE AD1, ARH1, ARMA11, CSH, CSR, FA1, FAH1, HF, TP, TPH, UNR and the\n\
                 spatial and Kronecker types, KRONECKER, SPCOORDS, SOLUTION on RANDOM, PRINT CPS, DESCRIPTIVES,\n\
                 HISTORY and LMATRIX, SAVE SEPRED and DFPRED.",
        example: "MIXED yield BY plot_type WITH rain /FIXED=plot_type rain /RANDOM=INTERCEPT rain | SUBJECT(farm) COVTYPE(UN) /PRINT=SOLUTION TESTCOV.",
        parse: mixed::mixed,
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
        name: "CSPLAN",
        summary: "Write or show an analysis plan of a complex sample: strata, clusters, weight and estimator per stage.",
        syntax: "CSPLAN ANALYSIS /PLAN FILE='file' /PLANVARS ANALYSISWEIGHT=var [/SRSESTIMATOR TYPE=WOR | WR]\n\
                 \x20  [/PRINT [PLAN] [MATRIX]]\n\
                 \x20  /DESIGN [STAGELABEL='label'] [STRATA=vars] [CLUSTER=vars]\n\
                 \x20  /ESTIMATOR TYPE=EQUAL_WOR | UNEQUAL_WOR | WR\n\
                 \x20  [/POPSIZE VALUE=n | VARIABLE=var | MATRIX=vars; categories n; ...]\n\
                 \x20  [/INCLPROB VALUE=p | VARIABLE=var | MATRIX=vars; categories p; ...]\n\
                 \x20  [/DESIGN ... /ESTIMATOR ... (a second and a third stage)].\n\
                 CSPLAN VIEW /PLAN FILE='file' [/PRINT [PLAN] [MATRIX]].\n\
                 The plan is a small JSON file that CSDESCRIPTIVES and CSTABULATE read; CSPLAN does not read\n\
                 the data. Each DESIGN starts a stage; the ESTIMATOR, POPSIZE and INCLPROB after it belong\n\
                 to that stage. WR treats the units of the stage as drawn with replacement and ends the\n\
                 design: later stages add nothing to the variance. EQUAL_WOR treats them as drawn without\n\
                 replacement with equal chances and needs either the population size of each stratum\n\
                 (POPSIZE, whole numbers) or the share of units drawn (INCLPROB, above 0 and at most 1), as\n\
                 one value, a variable that holds it, or a matrix with one row per stratum; it brings the\n\
                 finite population correction. A stage followed by another needs CLUSTER. SRSESTIMATOR picks\n\
                 the simple random sampling variance that design effects compare with: WOR (the default)\n\
                 includes the finite population correction, WR does not. PRINT PLAN (the default) shows the\n\
                 plan, MATRIX the matrix rows. Not supported yet: CSPLAN SAMPLE and the UNEQUAL_WOR\n\
                 estimator in analysis.",
        example: "CSPLAN ANALYSIS /PLAN FILE='farms_plan.json' /PLANVARS ANALYSISWEIGHT=wt /DESIGN STRATA=county CLUSTER=parish /ESTIMATOR TYPE=WR.",
        parse: complex_samples::csplan,
    },
    CommandSpec {
        name: "CSDESCRIPTIVES",
        summary: "Means, sums and ratios of a complex sample with design-based standard errors and design effects.",
        syntax: "CSDESCRIPTIVES /PLAN FILE='file' [/SUMMARY VARIABLES=vars]\n\
                 \x20  [/MEAN [TTEST=value | values]] [/SUM [TTEST=value | values]]\n\
                 \x20  [/RATIO NUMERATOR=vars DENOMINATOR=vars [TTEST=value | values]] ...\n\
                 \x20  [/STATISTICS [COUNT] [POPSIZE] [SE] [CV] [DEFF] [DEFFSQRT] [CIN[(95)]]]\n\
                 \x20  [/SUBPOP TABLE=var [BY var ...] [DISPLAY=LAYERED | SEPARATE]]\n\
                 \x20  [/MISSING [SCOPE=ANALYSIS | LISTWISE] [CLASSMISSING=EXCLUDE | INCLUDE]].\n\
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
                 Without STATISTICS: the estimate and its standard error. Not supported yet: JOINTPROB.",
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
                 \x20  [/MISSING [SCOPE=TABLE | LISTWISE] [CLASSMISSING=EXCLUDE | INCLUDE]].\n\
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
                 population sizes; without STATISTICS: standard errors. Not supported yet: JOINTPROB.",
        example: "CSTABULATE /PLAN FILE='farms_plan.json' /TABLES VARIABLES=irrigated BY organic /CELLS POPSIZE ROWPCT /STATISTICS SE CIN DEFF /TEST INDEPENDENCE ODDSRATIO.",
        parse: complex_samples::cstabulate,
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
                 out, the document and its file keep them. OUTLINELABEL, TABLETITLE and TEXT take )LABEL (the\n\
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
        summary: "Use all cases, or a range of cases, in the commands that follow.",
        syntax: "USE ALL.\n\
                 USE [first] [THRU [last]] [/PERMANENT].\n\
                 USE ALL (or THRU alone) turns FILTER off. A range of case numbers is supported with\n\
                 /PERMANENT, which deletes the cases outside it; dates are not supported yet.",
        example: "USE ALL.",
        parse: session_commands::use_command,
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
                 The text may run over several lines and hold blank lines; a period at the end of a\n\
                 line ends it. Every line becomes a line of the documents.",
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
                 \x20        /VARIABLES=name format [name format ...].\n\
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
                 reads a comma as the decimal mark. ENCODING as for INSERT; a byte order mark wins.\n\
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
        summary: "Write the active dataset to a CSV or tab-delimited text file or an Excel workbook.",
        syntax: "SAVE TRANSLATE OUTFILE='path' | handle /TYPE=CSV | TAB | XLS /VERSION=12 [/REPLACE]\n\
                 \x20   [/FIELDNAMES [VALUE=NAMES | LABELS]] [/CELLS=VALUES | LABELS] [/MISSING=IGNORE | RECODE]\n\
                 \x20   [/TEXTOPTIONS [DELIMITER='c'] [QUALIFIER='c'] [DECIMAL=DOT | COMMA] [FORMAT=PLAIN | VARIABLE]]\n\
                 \x20   [/EXCELOPTIONS SHEET='name'] [/ENCODING='name' [BOM=YES | NO]] [/UNSELECTED=RETAIN | DELETE]\n\
                 \x20   [/KEEP=varlist] [/DROP=varlist] [/RENAME=(old = new) ...].\n\
                 TYPE may be left out for a .csv or .xlsx file; TYPE=XLSX is short for XLS /VERSION=12. An\n\
                 existing file is overwritten only with REPLACE. FIELDNAMES writes a first row of names\n\
                 (VALUE=LABELS: the variable labels, else the names). CELLS=LABELS writes the value label where\n\
                 a value has one. MISSING=RECODE writes user-missing values as empty. UNSELECTED=DELETE leaves\n\
                 out the cases FILTER leaves out. KEEP, DROP and RENAME shape the file only, in the order\n\
                 written; KEEP does not change the order of the variables.\n\
                 Text: system-missing is empty. FORMAT=PLAIN (default) writes numbers with the decimals they\n\
                 need, dates as mm/dd/yyyy and times as hh:mm:ss; VARIABLE writes the display formats. The\n\
                 decimal mark is DECIMAL's, else SET DECIMAL's. CSV separates values with a comma, or with a\n\
                 semicolon when the decimal mark is a comma, and puts the qualifier (default \") around a value\n\
                 that holds the delimiter, the qualifier (doubled inside) or a line break. TAB never qualifies.\n\
                 ENCODING as for INSERT; the default is UTF-8 with a byte order mark, which BOM=NO leaves out.\n\
                 Workbook: numbers keep their display format and dates and times are Excel dates;\n\
                 system-missing is #NULL!. EXCELOPTIONS names the sheet (default Sheet1); more cases than a\n\
                 sheet holds continue on further sheets.\n\
                 Other types (SAS, STATA, dBASE, ODBC, 1-2-3, SYLK) and APPEND are not supported yet.",
        example: "SAVE TRANSLATE OUTFILE='wave2.xlsx' /TYPE=XLS /VERSION=12 /FIELDNAMES /CELLS=LABELS /REPLACE.\n\
                  SAVE TRANSLATE OUTFILE='wave2.csv' /TYPE=CSV /FIELDNAMES /TEXTOPTIONS DECIMAL=COMMA /REPLACE.",
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
                 \x20     [/MAP] [/COMPRESSED | /ZCOMPRESSED] [/PERMISSIONS=READONLY | WRITEABLE].\n\
                 A transformation: the file holds the cases as they reach XSAVE in the next procedure or\n\
                 EXECUTE (or the end of the run), with the variables of that moment, scratch variables\n\
                 left out. KEEP, DROP and RENAME as on SAVE. MAP lists the saved variables. Two XSAVE\n\
                 commands cannot write the same file in one pass. /UNCOMPRESSED is not supported yet.",
        example: "XSAVE OUTFILE='wave2.sav' /DROP=tmp1 TO tmp5.",
        parse: output::xsave,
    },
];

/// Commands of the language that have no parser yet. They get "not supported yet" instead of
/// "unknown command", and their bodies are not tokenised.
static UNSUPPORTED: &[&str] = &[];

/// The ends of blocks whose start reads them, such as END DATA after BEGIN DATA. Standing
/// alone they are an error at their own line.
static STRAY: &[&str] = &["END DATA", "END CHART", "END MATRIX", "END REPEAT"];

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
/// `T-TEST` arrives as three tokens and is joined into one word here.
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
            && w.eq_ignore_ascii_case("T")
            && test.eq_ignore_ascii_case("TEST")
        {
            i += 3;
            words.push(("T-TEST".to_string(), i));
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
            "END REPEAT" => "END REPEAT ends the commands that DO REPEAT repeats.\n".into(),
            "END MATRIX" => "END MATRIX ends the program that MATRIX starts.\n".into(),
            _ => format!("{name} is a command of the language, but it is not supported yet.\n"),
        }),
        _ => None,
    }
}

const GENERAL_HELP: &str = "
General rules:
  A command ends with a period at the end of a line, at a blank line, or at the end of input.
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
