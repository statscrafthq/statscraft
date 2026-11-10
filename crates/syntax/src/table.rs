//! The command table: the single source for command name matching, parsing and help.

use crate::ast::Command;
use crate::commands;
use crate::ctables;
use crate::data_commands;
use crate::data_list;
use crate::examine;
use crate::files;
use crate::lexer::{Punct, Token, TokenKind};
use crate::loops;
use crate::mrsets;
use crate::parser::{PResult, Parser};
use crate::procedures;
use crate::selection;
use crate::session_commands;
use crate::stats;

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
        syntax: "GET FILE='path'.",
        example: "GET FILE='survey.sav'.",
        parse: commands::get,
    },
    CommandSpec {
        name: "SAVE",
        summary: "Write the active dataset to a data file.",
        syntax: "SAVE OUTFILE='path' [/COMPRESSED | /UNCOMPRESSED | /ZCOMPRESSED].",
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
                 format: type, width and optional decimals, for example F8.2, A20, DATE11.",
        example: "FORMATS income (COMMA10.2) / age (F3).",
        parse: commands::formats,
    },
    CommandSpec {
        name: "FREQUENCIES",
        summary: "Frequency tables.",
        syntax: "FREQUENCIES [/]VARIABLES=varlist.\n\
                 Other subcommands are not supported yet.",
        example: "FREQUENCIES VARIABLES=gender region.",
        parse: commands::frequencies,
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
        syntax: "AGGREGATE [/OUTFILE=* | 'path' [MODE=ADDVARIABLES | REPLACE]] [/BREAK=varlist [(A | D)] ...]\n\
                 \x20         /target ['label'] [target ...] = function(varlist) [/target ...].\n\
                 OUTFILE=* replaces the active dataset with one case per break group (break variables, then\n\
                 targets); with MODE=ADDVARIABLES it keeps the cases and adds the targets to each of them.\n\
                 OUTFILE='path' writes the aggregated dataset to a file and leaves the active dataset alone.\n\
                 Without OUTFILE the targets are added to every case, as with OUTFILE=* MODE=ADDVARIABLES.\n\
                 functions: SUM MEAN SD MEDIAN MIN MAX FIRST LAST N NU NMISS NUMISS; N and NU also without (varlist).\n\
                 Missing values are skipped. With a weight variable SUM, MEAN, SD, MEDIAN, N and NMISS are weighted.",
        example: "AGGREGATE /OUTFILE=* MODE=ADDVARIABLES /BREAK=region /region_mean = MEAN(income) /region_n = N.",
        parse: data_commands::aggregate,
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
                 the branch are sampled. The random numbers start from the same seed in every session.",
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
                 \x20       [/TITLES TITLE='text' CAPTION='text' CORNER='text']\n\
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
                 var [function ['label'] [format], ...], format F, PCT, COMMA or DOLLAR, such as PCT8.1.\n\
                 After a parenthesis, (a + b > c) [COUNT], the summaries go to a and c, before their own ones.\n\
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
                 \x20         [/STATISTICS=CHISQ] [/MISSING=TABLE | INCLUDE] [/FORMAT=AVALUE | DVALUE].\n\
                 One table per pair of row and column variable; layer variables split it into blocks.\n\
                 CELLS defaults to COUNT; CELLS alone is COUNT ROW COLUMN TOTAL. Percentages are 0 to 100.\n\
                 STATISTICS alone is CHISQ. DVALUE orders the rows from the highest value; columns stay ascending.\n\
                 CHISQ: Pearson, likelihood ratio, linear-by-linear (numeric variables), and for a 2 x 2\n\
                 table Fisher's exact test and the continuity correction.\n\
                 MISSING=TABLE leaves out a case missing on any variable of the table; INCLUDE keeps\n\
                 user-missing values as categories. Weighted counts are not rounded (Fisher's test rounds them).",
        example: "CROSSTABS /TABLES=region BY gender /CELLS=COUNT ROW /STATISTICS=CHISQ.",
        parse: procedures::crosstabs,
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
                 \x20       [/PLOT=NPPLOT | NONE] [/COMPARE=GROUPS | VARIABLES]\n\
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
                 Not supported yet: STEMLEAF, BOXPLOT, HISTOGRAM, SPREADLEVEL.",
        example: "EXAMINE VARIABLES=income BY region /PERCENTILES(25,50,75)=HAVERAGE /STATISTICS=ALL /MESTIMATORS /PLOT=NPPLOT.",
        parse: examine::examine,
    },
    CommandSpec {
        name: "ADD FILES",
        summary: "Append the cases of several data files, matching variables by name.",
        syntax: "ADD FILES /FILE=* | 'path' [/RENAME=(old = new) ...] /FILE=... [/BY varlist]\n\
                 \x20         [/KEEP=varlist] [/DROP=varlist].\n\
                 FILE=* is the active dataset. A variable missing from a file is blank in its cases.\n\
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
        syntax: "MATCH FILES /FILE=* | 'path' [/RENAME=(old = new) ...] [/TABLE='path' ...] /FILE=...\n\
                 \x20           [/BY varlist] [/KEEP=varlist] [/DROP=varlist].\n\
                 With BY, cases with equal keys are joined; every file must be sorted by the keys (use\n\
                 SORT CASES). Without BY the n-th cases of the files are joined. A TABLE adds its variables\n\
                 to the cases whose key it holds and adds no cases; it may hold each key once. A variable in\n\
                 several files takes its value from the first file that has the case. The result has no\n\
                 weight variable.",
        example: "MATCH FILES /FILE=* /TABLE='regions.sav' /BY region.",
        parse: files::match_files,
    },
    CommandSpec {
        name: "DEFINE",
        summary: "Define a macro: a name that stands for syntax, with arguments.",
        syntax: "DEFINE name ([argument [/ argument ...]])\n\
                 body\n\
                 !ENDDEFINE.\n\
                 argument: !POSITIONAL or keyword =, then one of !TOKENS(n), !CHAREND('c'),\n\
                 !ENCLOSE('a', 'b'), !CMDEND, optionally with !DEFAULT(value) and !NOEXPAND.\n\
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
                 SET MPRINT=YES shows every expanded command.\n\
                 A body that ends with a period ends the command when the call is the last thing in it;\n\
                 in the middle of a command that period is dropped.",
        example: "DEFINE !freqs (vars = !CMDEND) FREQUENCIES VARIABLES=!vars. !ENDDEFINE.",
        parse: session_commands::define,
    },
    CommandSpec {
        name: "INSERT",
        summary: "Run the commands of another syntax file in place of this command.",
        syntax: "INSERT FILE='path' [SYNTAX=INTERACTIVE | BATCH] [ERROR=CONTINUE | STOP] [CD=NO | YES]\n\
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
        syntax: "INCLUDE [FILE=]'path' [ENCODING='name'] [PASSWORD='text'].\n\
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
                 StatsCraft runs no programs in other languages. The block is read up to END PROGRAM and\n\
                 not run; the run goes on with a warning.",
        example: "BEGIN PROGRAM PYTHON3.\nprint('not run')\nEND PROGRAM.",
        parse: session_commands::begin_program,
    },
];

/// Commands of the language that have no parser yet. They get "not supported yet" instead of
/// "unknown command", and their bodies are not tokenised.
static UNSUPPORTED: &[&str] = &[
    "ADD DOCUMENT",
    "ADD VALUE LABELS",
    "ALTER TYPE",
    "ANOVA",
    "APPLY DICTIONARY",
    "CACHE",
    "CASESTOVARS",
    "CD",
    "CLEAR TRANSFORMATIONS",
    "CLUSTER",
    "DATASET ACTIVATE",
    "DATASET CLOSE",
    "DATASET COPY",
    "DATASET DECLARE",
    "DATASET DISPLAY",
    "DATASET NAME",
    "DATE",
    "DISCRIMINANT",
    "DISPLAY",
    "DOCUMENT",
    "DROP DOCUMENTS",
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
    "FILE HANDLE",
    "FILE LABEL",
    "FILE TYPE",
    "FINISH",
    "FLIP",
    "GET DATA",
    "GET TRANSLATE",
    "GLM",
    "GRAPH",
    "HOST",
    "IMPORT",
    "INPUT PROGRAM",
    "LEAVE",
    "LIST",
    "LOGISTIC REGRESSION",
    "MATRIX",
    "MATRIX DATA",
    "MULT RESPONSE",
    "NEW FILE",
    "NPAR TESTS",
    "OUTPUT CLOSE",
    "OUTPUT EXPORT",
    "OUTPUT NEW",
    "OUTPUT SAVE",
    "PARTIAL CORR",
    "PERMISSIONS",
    "PRESERVE",
    "PRINT",
    "PRINT EJECT",
    "PRINT FORMATS",
    "PRINT SPACE",
    "QUICK CLUSTER",
    "RANK",
    "RECORD TYPE",
    "REGRESSION",
    "RELIABILITY",
    "REPEATING DATA",
    "REPORT",
    "REREAD",
    "RESTORE",
    "SAVE TRANSLATE",
    "SET",
    "SHOW",
    "SORT VARIABLES",
    "SUBTITLE",
    "SYSFILE INFO",
    "TITLE",
    "UNIANOVA",
    "UPDATE",
    "USE",
    "VARIABLE ALIGNMENT",
    "VARIABLE ATTRIBUTE",
    "VARIABLE ROLE",
    "VARSTOCASES",
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
  Scratch variables start with #: they begin at 0 or blank, keep their value from case to case,
  are never saved and are gone when the next procedure starts. $CASENUM is the case number.
";
