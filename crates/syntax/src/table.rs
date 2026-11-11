//! The command table: the single source for command name matching, parsing and help.

use crate::ast::Command;
use crate::commands;
use crate::ctables;
use crate::data_commands;
use crate::data_list;
use crate::dictionary;
use crate::examine;
use crate::files;
use crate::frequencies;
use crate::get_data;
use crate::lexer::{Punct, Token, TokenKind};
use crate::loops;
use crate::mrsets;
use crate::mult_response;
use crate::parser::{PResult, Parser};
use crate::procedures;
use crate::rank;
use crate::restructure;
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

pub(crate) static COMMANDS: &[CommandSpec] = &[
    CommandSpec {
        name: "DATA LIST",
        summary: "Define text data, inline or in a file, and read it as the new active dataset.",
        syntax: "DATA LIST [FILE='path'] [FIXED | FREE [(delimiters)] | LIST [(delimiters)]] [RECORDS=n] [SKIP=n]\n\
                 \x20         /[record] var [TO var] columns [(type[,decimals])] | (formats) [/ ...].\n\
                 FIXED (the default): columns such as 1-3, or formats such as (F2.0, 1X, A10) with Tn and nX.\n\
                 FREE and LIST: var [(format)], values separated by blanks and commas or by the delimiters\n\
                 given as quoted characters or TAB; LIST reads one case per line. * ends a run of F8.2 variables.\n\
                 Without FILE the data follow between BEGIN DATA and END DATA.\n\
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
                 \x20           [/BARCHART=...] [/PIECHART=...] [/HISTOGRAM=...].\n\
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
                 n - 1 points that cut the cases into n equal groups; several PERCENTILES and NTILES add up.\n\
                 GROUPED treats the values as groups for the median and percentiles: alone the values are group\n\
                 midpoints, (width) groups that wide around each value, (boundaries) the given groups; a value\n\
                 outside the boundaries falls in an open group, where percentiles are undefined (shown empty).\n\
                 INCLUDE makes user-missing values valid. Weighted when a weight variable is set; a case with a\n\
                 missing, zero or negative weight is left out. String variables get the table and the counts only.\n\
                 BARCHART, PIECHART and HISTOGRAM are accepted (BARCHART and HISTOGRAM not together) and not\n\
                 drawn yet, with a warning.",
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
        [removed]
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
        summary: "Recode values into consecutive integers in new variables, labelled with the old values.",
        syntax: "AUTORECODE [VARIABLES=] varlist /INTO newlist [/BLANK=VALID | MISSING] [/GROUP]\n\
                 \x20          [/DESCENDING] [/PRINT].\n\
                 Valid values get 1, 2, ... in ascending order (DESCENDING: descending), user-missing values\n\
                 the codes after them, still user-missing; system-missing stays missing. Each code is\n\
                 labelled with the value label or the value. GROUP uses one scheme for all variables,\n\
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
                 IF on END LOOP after it. A loop without an index stops after 40 iterations. BREAK\n\
                 leaves the innermost loop.",
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
                 MDGROUP (dichotomies): every variable is one category, labelled with its variable label or name; a\n\
                 case has a response in it when the variable holds VALUE, a whole number or, for strings, a quoted\n\
                 text no wider than the narrowest variable. CATEGORYLABELS=COUNTEDVALUES labels each category with\n\
                 the value label of VALUE in its variable instead; every variable needs one and they must differ.\n\
                 LABELSOURCE=VARLABEL (only with COUNTEDVALUES, instead of LABEL) takes the set label from the first\n\
                 variable of the set that has a variable label, else the name of the first variable.\n\
                 MCGROUP (categories): the values of the variables are\n\
                 the categories, labelled with the value labels of the first variable that has one.\n\
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
                 ZZZZ01 to ZZZZ09, ZQZQ01 to ZQZQ09 when that name is taken), labelled Zscore: plus the label.",
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
                 \x20         [/COUNT=[ASIS | CASE | CELL] [ROUND | TRUNCATE]].\n\
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
                 user-missing values as categories. Fisher's test takes whole counts: rounded, or truncated with TRUNCATE.",
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
                 \x20      [/MISSING=ANALYSIS | LISTWISE [INCLUDE | EXCLUDE]] [/CRITERIA=CILEVEL(0.95)].\n\
                 test: LSD BONFERRONI SIDAK TUKEY SCHEFFE GT2 GABRIEL GH T2 T3 C (pairwise comparisons),\n\
                 DUNNETT DUNNETTL DUNNETTR [(refcat)] (each group against the reference group, by default the last;\n\
                 L: smaller, R: larger), SNK BTUKEY DUNCAN FREGW QREGW (homogeneous subsets only); TUKEY SCHEFFE\n\
                 GT2 GABRIEL give both. DUNCAN and the Dunnett keywords are written in full. Every value of the\n\
                 factor is a group. CONTRAST gives one coefficient per group in ascending order (n*c repeats c n\n\
                 times); it is tested with pooled and separate variances. HOMOGENEITY is Levene's test based on\n\
                 the mean. Multiple Comparisons show every ordered pair of groups (Dunnett C has no significance).\n\
                 Homogeneous subsets use the harmonic mean of the group sizes (the R-E-G-W tests the sizes\n\
                 themselves); Tukey B shows no significance. Weighted when a weight variable is set.\n\
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
        name: "EXAMINE",
        summary: "Descriptives, percentiles, M-estimators, extreme values and normality tests, by factor cells.",
        syntax: "EXAMINE [VARIABLES=]deps [BY factor [BY factor] [factor ...]] [/TOTAL | NOTOTAL] [/ID=var]\n\
                 \x20       [/PERCENTILES[(list)][=HAVERAGE | WAVERAGE | ROUND | EMPIRICAL | AEMPIRICAL | NONE]]\n\
                 \x20       [/STATISTICS=DESCRIPTIVES EXTREME[(n)] ALL NONE] [/CINTERVAL=95]\n\
                 \x20       [/MESTIMATORS[=HUBER[(c)] TUKEY[(c)] HAMPEL[(a,b,c)] ANDREW[(c)] ALL NONE]]\n\
                 \x20       [/PLOT=[BOXPLOT] [STEMLEAF] [HISTOGRAM] [NPPLOT] | ALL | NONE] [/COMPARE=GROUPS | VARIABLES]\n\
                 \x20       [/MISSING=LISTWISE | PAIRWISE  EXCLUDE | INCLUDE  NOREPORT | REPORT].\n\
                 Each dependent is analysed over all cases (unless NOTOTAL) and in every cell of each factor term,\n\
                 two factors crossed with BY. Descriptives (the default): mean with its confidence interval (CINTERVAL,\n\
                 50 to 99.99), 5% trimmed mean, median, variance, standard deviation, minimum, maximum, range,\n\
                 interquartile range, skewness and kurtosis with standard errors. PERCENTILES alone gives 5 10 25 50\n\
                 75 90 95 by HAVERAGE, with Tukey's hinges when 25, 50 or 75 is listed; the method also gives the median\n\
                 and the interquartile range. MESTIMATORS alone gives all four (Huber 1.339, Tukey 4.685, Hampel\n\
                 1.7 3.4 8.5, Andrews 1.34 times pi), each started at the median and scaled by the median absolute\n\
                 deviation. EXTREME lists the n highest and lowest values by case number or by the ID variable.\n\
                 NPPLOT gives the Tests of Normality table (Kolmogorov-Smirnov with the Lilliefors significance and\n\
                 Shapiro-Wilk by Royston's algorithm, 3 to 5000 cases); the plots themselves are not drawn.\n\
                 LISTWISE (default) leaves out a case missing on any dependent or factor, PAIRWISE only from the\n\
                 analyses of that variable. INCLUDE makes user-missing values of the dependents valid, REPORT makes\n\
                 missing values of the factors categories. Weighted when a weight variable is set (Shapiro-Wilk rounds\n\
                 the weights and takes 3 to 50 cases with non-integer weights). COMPARE changes nothing.\n\
                 BOXPLOT, STEMLEAF and HISTOGRAM are accepted and not drawn yet, with a warning; the\n\
                 statistics are shown. ALL is the three and NPPLOT. Not supported yet: SPREADLEVEL.",
        example: "EXAMINE VARIABLES=income BY region /PERCENTILES(25,50,75)=HAVERAGE /STATISTICS=ALL /MESTIMATORS /PLOT=NPPLOT.",
        parse: examine::examine,
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
                 \x20    [/MISSING NOMINALMISSING=MISSING | VALID].\n\
                 Levels: [S] scale, [O] ordinal, [N] or [C] nominal; without one the dictionary's level holds,\n\
                 and an unknown level comes from the data as in a data pass. The dependent is nominal\n\
                 (Pearson or likelihood-ratio chi-square) or ordinal (likelihood ratio against Goodman's row\n\
                 effects model, scored 1, 2, ... or by SCORES). Scale predictors are cut into INTERVALS groups of\n\
                 about equal size. Predictor categories that do not differ at ALPHAMERGE are merged (ordinal ones\n\
                 only when adjacent); a missing value is a category of its own that may merge with any. A category\n\
                 smaller than MINCHILDSIZE is merged with its most similar neighbour. The predictor with the\n\
                 smallest Bonferroni-adjusted significance splits the node when that is at most ALPHASPLIT. A node\n\
                 is not split when it is pure, smaller than MINPARENTSIZE or at MAXDEPTH (AUTO is 3). Exhaustive\n\
                 CHAID merges down to two categories and keeps the most significant set. The tree is shown as a\n\
                 table with one row per node, indented by depth; charts are not drawn yet. TREETABLE lists the nodes\n\
                 in order with their parents. Gain tables (one per TARGET category) show terminal nodes by index.\n\
                 SAVE adds the terminal node, the predicted category and one probability per category for the\n\
                 cases in the analysis (default names NodeID, PredictedValue, PredictedProbability_1, ...).\n\
                 A weight counts each case as often as its rounded value. SPLIT FILE is ignored.\n\
                 PLOT, PRIORS, CRT, QUEST, the settings of CRT and QUEST and PRINT IMPORTANCE, SURROGATES and\n\
                 CATEGORYSPECS are accepted and ignored with a warning. Not supported\n\
                 yet: METHOD TYPE=CRT and QUEST, a scale dependent, VALIDATION SPLITSAMPLE and CROSSVALIDATION,\n\
                 SPLITMERGED=YES, COSTS CUSTOM, GAIN TYPE PTILE, RULES, PROFITS, INFLUENCE, OUTFILE, TARGETRESPONSE.",
        example: "TREE churn [n] BY region [n] age_group [o] income [s] /GROWTHLIMIT MAXDEPTH=2 /SAVE NODEID PREDVAL.",
        parse: tree::tree,
    },
    CommandSpec {
        name: "ADD FILES",
        summary: "Append the cases of several data files, matching variables by name.",
        syntax: "ADD FILES /FILE=* | 'path' | dataset [/RENAME=(old = new) ...] /FILE=... [/BY varlist]\n\
                 \x20         [/KEEP=varlist] [/DROP=varlist].\n\
                 FILE=* is the active dataset; a dataset name (DATASET NAME), quoted or not, is that open\n\
                 dataset and comes before a file of the same name. A variable missing from a file is blank in its cases.\n\
                 The first file that has a variable gives its format and measurement level; a later file\n\
                 fills in a label, value labels or missing values the earlier ones lack. A string variable\n\
                 takes its widest width; a variable that is numeric in one file and a string in another is\n\
                 an error. BY interleaves files sorted by the key variables. The result stays weighted only when\n\
                 every file is weighted by the same variable.",
        example: "ADD FILES /FILE=* /FILE='wave2.sav' /RENAME=(q1_new = q1) /DROP=tmp.",
        parse: files::add_files,
    },
    CommandSpec {
        name: "MATCH FILES",
        summary: "Combine the variables of several data files case by case or by key.",
        syntax: "MATCH FILES /FILE=* | 'path' | dataset [/RENAME=(old = new) ...] [/TABLE='path' | dataset ...]\n\
                 \x20           /FILE=... [/BY varlist] [/KEEP=varlist] [/DROP=varlist].\n\
                 A dataset name (DATASET NAME), quoted or not, is that open dataset and comes before a file\n\
                 of the same name.\n\
                 With BY, cases with equal keys are joined; every file must be sorted by the keys (use\n\
                 SORT CASES). Without BY the n-th cases of the files are joined. A TABLE adds its variables\n\
                 to the cases whose key it holds and adds no cases; it may hold each key once. A variable in\n\
                 several files takes its value from the first file that has the case. The result has no\n\
                 weight variable.",
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
                 The result keeps the primary file's weight. FILE=* cannot follow TEMPORARY.",
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
                 The expansion is scanned again for macro calls, up to SET MNEST levels (50); a !DO loop\n\
                 runs at most SET MITERATE times (1000). SET MEXPAND=OFF stops expansion and\n\
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
                 LOCALE, UNICODE (Statscraft always works in Unicode), PRINTBACK, CCA to CCE='negative\n\
                 prefix,prefix,suffix,negative suffix' (custom currency formats; three commas, or three periods\n\
                 when the decimal mark is a comma; ' before a period or comma makes it text), RNG=MC | MT\n\
                 (the generator of random numbers), SEED=n | RANDOM (starts the MC generator again; a whole\n\
                 number from 1 to 1999999999, default 2000000), MTINDEX=x | RANDOM (starts the MT generator,\n\
                 the Mersenne Twister, again; any number, default 2000000). RANDOM takes the time of day.\n\
                 Settings for speed, memory and messages (CACHE, THREADS, WORKSPACE, MXWARNS ...) are\n\
                 accepted and have nothing to do. Settings that only change how output looks (TNUMBERS,\n\
                 TVARS, ONUMBERS, OVARS, TLOOK, WIDTH, LENGTH, DIGITGROUPING ...) are accepted with a\n\
                 warning. BLANKS, EPOCH, FUZZBITS, MXLOOPS and SCALEMIN are accepted with a warning that\n\
                 results do not follow them yet. YES and ON, NO and OFF are the same.",
        example: "SET FORMAT=F10.3 MPRINT=YES /DECIMAL=DOT.",
        parse: session_commands::set,
    },
    CommandSpec {
        name: "SHOW",
        summary: "Show the current settings.",
        syntax: "SHOW [ALL] [setting ...].\n\
                 Any SET subcommand, and N, WEIGHT, FILTER, SPLIT, DIRECTORY, SYSMIS, VERSION, HANDLES (the file\n\
                 handles). SEED shows where the MC generator stands.\n\
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
                 Words after PRESERVE (PRESERVE LOCALE, as survey tools write it) are ignored with a\n\
                 warning: every setting is stored anyway.",
        example: "PRESERVE.",
        parse: session_commands::preserve,
    },
    CommandSpec {
        name: "RESTORE",
        summary: "Bring back the settings and folder the last PRESERVE stored.",
        syntax: "RESTORE.\n\
                 Every setting and the folder come back as the last PRESERVE stored them; a RESTORE without\n\
                 PRESERVE does nothing, with a warning. Words after RESTORE (RESTORE LOCALE) are ignored with\n\
                 a warning: every setting comes back.",
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
        summary: "Read a text data file, delimited or in fixed columns, as the active dataset.",
        syntax: "GET DATA /TYPE=TXT /FILE='path' [/ENCODING='name'] [/ARRANGEMENT=DELIMITED | FIXED]\n\
                 \x20        [/FIRSTCASE=n] [/DELCASE=LINE | VARIABLES n] [/FIXCASE=n] [/DELIMITERS=\"chars\"]\n\
                 \x20        [/QUALIFIER=\"c\"] [/IMPORTCASE=ALL | FIRST n] [/MULTIPLESPACES IGNORE=NO | YES]\n\
                 \x20        [/LEADINGSPACES IGNORE=NO | YES]\n\
                 \x20        /VARIABLES=name format [name format ...].\n\
                 FIXED: /VARIABLES=[/record] name first-last format ..., columns counted from 0.\n\
                 format: F, A, DATE, ADATE, SDATE, DATETIME, DOLLAR, COMMA and the other input formats, with\n\
                 or without a width; AUTO reads a number when every value is one, else a string as wide as the\n\
                 longest value; X skips the field. DELIMITERS: \\t is a tab, \\\\ a backslash; two delimiters\n\
                 in a row leave a value empty. Default delimiters: tab and comma. A value between\n\
                 qualifiers may hold delimiters and line breaks; a doubled qualifier is one. SET DECIMAL=COMMA\n\
                 reads a comma as the decimal mark. ENCODING as for INSERT; a byte order mark wins.\n\
                 Other types (XLS, XLSX, ODBC) are not supported yet.",
        example: "GET DATA /TYPE=TXT /FILE='wave2.csv' /DELIMITERS=\",\" /QUALIFIER='\"' /FIRSTCASE=2\n\
                  \x20 /VARIABLES=id F5 name A20 born SDATE10.",
        parse: get_data::get_data,
    },
];

/// Commands of the language that have no parser yet. They get "not supported yet" instead of
/// "unknown command", and their bodies are not tokenised.
static UNSUPPORTED: &[&str] = &[
    "ANOVA",
    "CLEAR TRANSFORMATIONS",
    "CLUSTER",
    "DATE",
    "DISCRIMINANT",
    "ECHO",
    "END CASE",
    "END DATA",
    "END FILE",
    "END FILE TYPE",
    "END INPUT PROGRAM",
    "END MATRIX",
    "END REPEAT",
    "ERASE",
    "EXPORT",
    "FACTOR",
    "FILE TYPE",
    "FINISH",
    "GET TRANSLATE",
    "GLM",
    "GRAPH",
    "HOST",
    "IMPORT",
    "INPUT PROGRAM",
    "LEAVE",
    "LOGISTIC REGRESSION",
    "MATRIX",
    "MATRIX DATA",
    "NEW FILE",
    "NPAR TESTS",
    "OUTPUT CLOSE",
    "OUTPUT EXPORT",
    "OUTPUT NEW",
    "OUTPUT SAVE",
    "PARTIAL CORR",
    "PERMISSIONS",
    "PRINT",
    "PRINT EJECT",
    "PRINT FORMATS",
    "PRINT SPACE",
    "QUICK CLUSTER",
    "RECORD TYPE",
    "REGRESSION",
    "RELIABILITY",
    "REPEATING DATA",
    "REPORT",
    "REREAD",
    "SAVE TRANSLATE",
    "SUBTITLE",
    "SYSFILE INFO",
    "TITLE",
    "UNIANOVA",
    "WRITE",
    "WRITE FORMATS",
    "XSAVE",
];

fn names() -> impl Iterator<Item = (&'static str, Option<&'static CommandSpec>)> {
    let supported = COMMANDS.iter().map(|spec| (spec.name, Some(spec)));
    supported.chain(UNSUPPORTED.iter().map(|&name| (name, None)))
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
        for spec in COMMANDS {
            text.push_str(&format!("  {:<18}{}\n", spec.name, spec.summary));
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
        Resolution::Unsupported(name, used) if used == words.len() => Some(format!(
            "{name} is a command of the language, but it is not supported yet.\n"
        )),
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
";
