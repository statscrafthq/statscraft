<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CTABLES

Custom tables: counts, percentages and statistics by categories.

## Syntax

```text
CTABLES [/FORMAT EMPTY=ZERO|BLANK|'text' MISSING='text']
        [/VLABELS VARIABLES=varlist DISPLAY=DEFAULT|NAME|LABEL|BOTH|NONE]
        [/MRSETS COUNTDUPLICATES=NO|YES]
        [/HIDESMALLCOUNTS [COUNT=5]] [/WEIGHT VARIABLE=varname] [/SMISSING VARIABLE|LISTWISE]
        [/PCOMPUTE &name=EXPR(expression)]
        [/PPROPERTIES &name ... [LABEL='text'] [FORMAT=function format ...] [HIDESOURCECATS=NO|YES]]
        /TABLE [rows] [BY [columns] [BY layers]]
        [/SLABELS POSITION=COLUMN|ROW|LAYER VISIBLE=YES|NO]
        [/CLABELS AUTO|ROWLABELS=OPPOSITE|LAYER|COLLABELS=OPPOSITE|LAYER] [/CRITERIA CILEVEL=95]
        [/CATEGORIES VARIABLES=varlist [ORDER=A|D] [KEY=VALUE|LABEL|function[(var[, n])]]
                     [MISSING=EXCLUDE|INCLUDE]
                     [TOTAL=NO|YES] [LABEL='text'] [POSITION=AFTER|BEFORE] [EMPTY=INCLUDE|EXCLUDE]]
        [/CATEGORIES VARIABLES=varlist [value, low THRU high, OTHERNM, MISSING, &name,
                     SUBTOTAL='label', HSUBTOTAL='label', ...] [TOTAL=YES ...]]
        [/TITLES TITLE='text' ['text' ...] CAPTION='text' ... CORNER='text' ...]
        [/SIGTEST TYPE=CHISQUARE [ALPHA=0.05] [INCLUDEMRSETS=YES|NO] [CATEGORIES=ALLVISIBLE|SUBTOTALS]]
        [/COMPARETEST TYPE=PROP|MEAN [ALPHA=0.05 [0.01]] [ADJUST=BONFERRONI|BH|NONE]
                      [MEANSVARIANCE=ALLCATS|TESTEDCATS] [SHOWSIG=NO|YES] [INCLUDEMRSETS=YES|NO]
                      [CATEGORIES=ALLVISIBLE|SUBTOTALS] [MERGE=NO|YES [STYLE=APA|SIMPLE]]
                      [ORIGIN=COLUMN]]
        [/TABLE ... another table with its own SLABELS, CATEGORIES, TITLES and tests].
```

The command builds tables of counts, percentages and statistics. Each /TABLE is rows BY columns BY layers. On an axis, + stacks variables, \> nests them (a \> b: b within each category of a), and parentheses group. A \[C\] or \[S\] after a variable makes it categorical or scale. Otherwise level SCALE is scale and other levels categorical; a numeric variable of unknown level is scale with a currency, date or time format, a fraction, a negative value, 24 or more distinct values, or no value below 10. /TABLE BY columns gives one unlabeled row. SLABELS, CLABELS, CATEGORIES, TITLES, CRITERIA and the tests belong to the /TABLE before them.

The default summary is COUNT, or MEAN for a scale variable. A summary list follows a scale variable or the innermost categorical variable of an axis: var \[function \['label'\] \[format\], ...\]; a function given twice is two columns. In (a + b \> c) \[COUNT\] the summaries go to a and c, before their own. Categorical functions: COUNT, ROWPCT, COLPCT, TABLEPCT, SUBTABLEPCT, LAYERPCT, LAYERROWPCT, LAYERCOLPCT; a percentage takes the base .COUNT (default), .VALIDN or .TOTALN. Scale variables add MEAN MEDIAN MODE MINIMUM MAXIMUM RANGE SUM STDDEV VARIANCE SEMEAN PTILE n VALIDN TOTALN MISSING and the percentages with .SUM. A U prefix (UCOUNT) ignores the weight. TOTALS \[function ...\] in a categorical variable's list gives its totals and subtotals their own summaries (the columns hold both lists); a scale function there summarizes the variable's own values: var \[COUNT TOTALS \[MEAN\]\].

Formats: F, PCT, COMMA, DOT (period and comma swapped), DOLLAR, N (leading zeros), E, CCA to CCE, date and time formats, and NEGPAREN (negatives in parentheses), NEQUAL (N= before), PAREN and PCTPAREN, which show F in the width with their marks outside it. A scale summary has the variable's format, but for dates STDDEV, SEMEAN and RANGE show DTIME, and SUM and VARIANCE (also of times) plain numbers.

CATEGORIES VARIABLES=varlist sorts by ORDER=A (default) or D and KEY=VALUE (default), LABEL or a summary of each category over all its cases (COUNT, MEAN(age), PTILE(age, 75)); ties keep value order. TOTAL=YES adds a total named by LABEL, placed by POSITION=AFTER (default) or BEFORE; its percentage base takes in every category totaled. EMPTY=EXCLUDE hides categories without cases. User-missing categories show only with MISSING=INCLUDE, system-missing ones never. An empty count shows 0, or blank or text with FORMAT EMPTY; FORMAT MISSING='text' fills cells that cannot be computed (else blank).

An explicit list fixes the categories and their order: VARIABLES=q1 \[3, 1, 4 THRU 6, OTHERNM, MISSING\]. OTHERNM is every valid value not named, ascending; MISSING every user-missing value; a value named twice takes its last place; KEY does not apply. A case whose valid value is not listed is not in the table. SUBTOTAL='label' sums the categories before it back to the previous subtotal (with POSITION=BEFORE, those after it up to the next); HSUBTOTAL also hides them. A subtotal counts a case once and has every summary a total has.

Layers (rows BY columns BY layers, or rows BY BY layers) give a table per layer category, titled by it: every combination of nested layer variables and every category of stacked ones. Percentages stay within the layer (LAYERPCT: all its cells; LAYERROWPCT, LAYERCOLPCT: across its subtables), except TABLEPCT, which spans all layers. Tests run in every layer.

SLABELS POSITION puts summary labels in the columns (default), rows or layers (each summary a layer); VISIBLE=NO hides them. CLABELS ROWLABELS=OPPOSITE moves the innermost row categories to the columns, inside the column categories and outside the summary labels; COLLABELS=OPPOSITE does the reverse. The moved variable is then the innermost of its new axis, also for percentages. ROWLABELS=LAYER or COLLABELS=LAYER makes each category a layer and the variable a layer variable. Moved stacked variables need the same categories and labels, and tests are then unavailable. AUTO (default) moves nothing.

TITLES takes TITLE, CAPTION and CORNER; several quoted texts are several lines. )DATE is today's date (YYYY-MM-DD), )TIME the time (HH:MM:SS), both local, and )TABLE the table expression with labels, without levels and summaries: Gender \> Age BY Region. VLABELS sets the headings of the named variables: DEFAULT, NAME, LABEL, BOTH or NONE.

A case whose missing value is left out still counts in TOTALN and the .TOTALN percentages. SMISSING LISTWISE (default VARIABLE) makes a case missing on one scale variable missing on all scale variables of the table (the sections with the same categorical variables); it still counts in COUNT and TOTALN. The weight variable applies unrounded; a case with zero, negative or missing weight is left out, also from U functions. MEDIAN and PTILE use the averaged empirical distribution. HIDESMALLCOUNTS COUNT=n (whole, from 2, default 5) shows every COUNT, VALIDN, TOTALN or RESPONSES below n, weighted or with U, as \<n in all tables.

Tests run in every innermost subtable (innermost row by innermost column variable, per category of the outer ones), each in a table after the main one. They compare the shown categories, an HSUBTOTAL standing for those it hides; CATEGORIES=SUBTOTALS puts every subtotal in place of its categories. With a weight variable they round the weighted cell counts, column totals or category weights. SIGTEST TYPE=CHISQUARE (required) gives the Pearson chi-square, without empty rows and columns.

COMPARETEST TYPE=PROP gives two-sided pairwise z-tests of column proportions, TYPE=MEAN t-tests of column means (a scale variable with MEAN in the rows), in every row of a subtable; a table may have one of each. The letter of the smaller column appears in the larger one: uppercase below the smaller ALPHA, lowercase below the larger (one or two levels, default 0.05). A period marks a column left out (a proportion of 0 or 1, or under 2 cases). ADJUST is BONFERRONI (default), BH or NONE; MEANSVARIANCE ALLCATS (default) or TESTEDCATS. ORIGIN=COLUMN changes nothing. MERGE=YES puts the letters in the main table, in a column after each compared category (in the row of MEAN, COLPCT or COUNT when summaries are in rows). STYLE=APA (default) gives two columns a shared letter unless they differ at the smaller ALPHA; SIMPLE gives the separate table's keys. SHOWSIG=YES keeps the separate table.

A multiple response set (\$name, see MRSETS) stands where a categorical variable may. A case counts once in each category it responds in; the set's total counts cases with any response; a case with none counts only in TOTALN and the .TOTALN percentages. A dichotomy set lists its variables in set order, also by name in CATEGORIES: \[q5\_3, q5\_1\]. A set may carry the summaries with categorical variables nested in it; of two nested sets only the inner one may. Sets add RESPONSES and, for every percentage base, base.RESPONSES (responses over the area's responses), base.RESPONSES.COUNT (responses over its count) and base.COUNT.RESPONSES (count over its responses). MRSETS COUNTDUPLICATES=YES counts a value given in two variables of a category set twice in the responses, never in counts.

Tests include sets (INCLUDEMRSETS=YES, default), counting each case once in the margins and allowing for cases two categories share; the means test then pools the variance of the two categories compared. INCLUDEMRSETS=NO, or COUNTDUPLICATES=YES for a category set, leaves sets out.

PCOMPUTE &name=EXPR(expression) defines a computed category for an explicit category list. For every summary it combines the same summary of \[value\], \[low THRU high\] (the sum of the listed categories in range), TOTAL (needs TOTAL=YES), SUBTOTAL (the only one) or SUBTOTAL\[n\], \[OTHERNM\] and \[MISSING\], with numbers, + - \* / \*\* and parentheses. Every category it names must be listed. PPROPERTIES sets its LABEL (default the expression; )LABEL\[n\] is the label of the nth list item as written, a range as 'low THRU high'), a FORMAT per summary, and HIDESOURCECATS=YES to hide its source categories (not with tests). Totals and tests leave computed categories out.

A .LCL, .UCL or .SE suffix gives confidence limits or a standard error (COUNT.LCL, COLPCT.COUNT.UCL, MEAN.SE): limits for COUNT, the percentages of COUNT, VALIDN or TOTALN, MEAN, MEDIAN, PTILE and SUM; .SE for COUNT, those percentages, MEAN and SUM; neither for sets or U functions. CRITERIA CILEVEL (default 95) sets the level of its table. Proportions and counts use the Jeffreys interval (weights rounded), means and sums the t interval.

/WEIGHT VARIABLE=var (before the first /TABLE) weights all tables by an effective base weight instead of the WEIGHT command's variable. ECOUNT (also for categorical variables and sets), EVALIDN and ETOTALN then give the effective bases, (sum of weights)^2 / (sum of squared weights), of COUNT, VALIDN and TOTALN, which they equal without /WEIGHT. SEMEAN uses the variance adjusted to the effective base. The tests use the effective bases and round nothing; the column t-tests compare two categories when both have a case and one has two.

DMTABLES is another name for the command.

## Example

```text
CTABLES /TABLE q1 [COUNT, COLPCT.COUNT PCT8.1] BY gender /CATEGORIES VARIABLES=q1 TOTAL=YES.
```

`statscraft help CTABLES` prints the same text in the terminal. [All commands](README.md)
