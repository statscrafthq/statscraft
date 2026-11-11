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

rows, columns: variables joined by + (stack) and \> (nest), with parentheses. var \[C\] or var \[S\] makes a variable categorical or scale; otherwise SCALE is scale and the other levels categorical.

A numeric variable of unknown level is scale with a currency, date or time format, a fraction, a negative value, 24 or more distinct values, or no value below 10; else categorical.

/TABLE BY columns leaves the rows empty: one row without a label. Layers (rows BY columns BY layers, or rows BY BY layers) give a table for every layer category, for every combination of nested layer variables and every category of stacked ones; its title names the layer. Within a layer the percentages are of that layer (LAYERPCT: of all its cells; LAYERROWPCT and LAYERCOLPCT: across its subtables), except TABLEPCT, which spans all layers.

SLABELS POSITION=LAYER makes every summary a layer of its own. Tests run in every layer.

A summary list follows a scale variable or the innermost categorical variable of one axis: var \[function \['label'\] \[format\], ...\], such as PCT8.1. Formats: F, PCT, COMMA, DOT (period and comma swapped), DOLLAR, N (leading zeros), E (scientific), and NEGPAREN (negative numbers in parentheses), NEQUAL (N= before), PAREN (in parentheses), PCTPAREN (percent in parentheses); these four show the number as F in the width and add their marks outside it; CCA to CCE and the date and time formats. By default a summary of a scale variable has the variable's format, but for dates: STDDEV, SEMEAN and RANGE as days and time (DTIME), SUM and VARIANCE plain; and VARIANCE of a time variable plain. A function given twice in a list is two columns, each with its own format.

After a parenthesis, (a + b \> c) \[COUNT\], the summaries go to a and c, before their own ones.

TITLES: several quoted texts are several lines. )DATE in them is today's date (YYYY-MM-DD), )TIME the time (HH:MM:SS), both local; )TABLE the table expression without levels and summaries, with variable and set labels in place of names: Gender \> Age BY Region.

TOTALS \[function ...\] inside the list gives the totals and subtotals of that categorical variable their own summaries (the columns hold both lists); a function of scale variables there, such as MEAN or MEDIAN, summarizes the variable's own values: var \[COUNT TOTALS \[MEAN\]\].

CLABELS ROWLABELS=OPPOSITE moves the categories of the innermost row variables to the columns, inside the column categories and outside the summary labels, and COLLABELS=OPPOSITE the other way; the moved variable counts as the innermost one of its new axis, also for percentages.

ROWLABELS=LAYER or COLLABELS=LAYER makes a layer of every category instead; the moved variable then counts as a layer variable.

Stacked variables whose labels move need the same categories with the same labels; tests are not available then.

Default: COUNT for categorical tables, MEAN for a scale variable.

Categorical: COUNT, ROWPCT, COLPCT, TABLEPCT, SUBTABLEPCT, LAYERPCT, LAYERROWPCT, LAYERCOLPCT, each with .COUNT, .VALIDN or .TOTALN (COLPCT.COUNT and so on); COLPCT alone means COLPCT.COUNT.

Scale: also MEAN MEDIAN MODE MINIMUM MAXIMUM RANGE SUM STDDEV VARIANCE SEMEAN PTILE n VALIDN TOTALN MISSING and the percentages with .SUM. A U prefix (UCOUNT, UMEAN) ignores the weight.

SMISSING LISTWISE: a case missing on one scale variable of a table is missing on all of them (a table: the sections with the same categorical variables); it still counts in COUNT and TOTALN.

Confidence limits and standard errors: a .LCL, .UCL or .SE suffix (COUNT.LCL, COLPCT.COUNT.UCL, MEAN.SE); .LCL and .UCL for COUNT, the percentages of COUNT, VALIDN or TOTALN, MEAN, MEDIAN, PTILE and SUM, .SE for COUNT, those percentages, MEAN and SUM; not for sets or with a U prefix.

CRITERIA CILEVEL (after its /TABLE, default 95) sets the level. A proportion has the Jeffreys interval (Beta quantiles of w + 0.5 and W - w + 0.5, weights rounded); a count is the table's weight times the interval of its share of the table; a mean or sum the t interval; a percentile the percentiles at the limits of the share at or below it.

/WEIGHT VARIABLE=var (before the first /TABLE) weights every table by an effective base weight, in place of the WEIGHT command's variable. ECOUNT (also for categorical variables and sets), EVALIDN and ETOTALN are then the effective bases (sum of weights)^2 / (sum of squared weights) of COUNT, VALIDN and TOTALN, and SEMEAN uses the variance adjusted to the effective base; without /WEIGHT they equal COUNT, VALIDN and TOTALN. The tests then round nothing: the chi-square is multiplied by the subtable's weight over its sum of squared weights, and the column tests are t-tests on the effective bases, comparing two categories when both have a case and one of them two.

Weighted by the weight variable when one is set; fractional weights are not rounded, and a case whose weight is zero, negative or missing is left out (also from U functions). MEDIAN and PTILE use the averaged empirical distribution. A count without cases is 0 unless EMPTY says otherwise.

A total has percentages too: the base of a total row or column takes in every category totaled.

In an explicit list OTHERNM stands for every valid value the list does not name, in ascending order, and MISSING for every user-missing value; a value named twice takes its last place.

KEY=function sorts the categories of a list without explicit values by that summary of each, taken over all cases of the category whatever the other variables (COUNT, or MEAN(age), PTILE(age, 75) for a variable); ORDER=D sorts from the largest, and ties keep value order.

SUBTOTAL='label' in an explicit list sums the listed categories before it, back to the previous subtotal (with POSITION=BEFORE the ones after it, up to the next); HSUBTOTAL also hides them.

A subtotal counts a case once, like a total, and has every summary and percentage a total has.

A computed category &name stands in an explicit category list of a categorical variable. For every summary its value is EXPR over the same summary of other categories: \[value\], \[low THRU high\] (the sum of the listed categories in the range), TOTAL (needs TOTAL=YES), numbers, + - \* / \*\* and parentheses, SUBTOTAL (the only subtotal) or SUBTOTAL\[n\] (the nth), \[OTHERNM\] and \[MISSING\] (the sums of those categories). Every category it names must be listed. PPROPERTIES gives its label (else the expression; )LABEL\[n\] in it is the label of the nth item of the list as written, a range as 'low THRU high'), a format per summary, and HIDESOURCECATS=YES to hide the categories it uses (not with tests). Totals and tests leave computed categories out.

HIDESMALLCOUNTS COUNT=n (n a whole number from 2, default 5) shows every COUNT, VALIDN, TOTALN or RESPONSES below n, weighted or with U, as \<n in all tables of the command.

User-missing categories are left out unless MISSING=INCLUDE; system-missing ones always are.

A case whose missing value is left out still counts in TOTALN and the .TOTALN percentages; a case whose valid value an explicit category list leaves out is not in the table at all.

Tests run in every innermost subtable (innermost row by innermost column variable, for each category of the outer ones), each in a table after the main one. SIGTEST: Pearson chi-square; empty rows and columns are left out. Tests compare the shown categories, with an HSUBTOTAL in place of the categories it hides; CATEGORIES=SUBTOTALS puts every subtotal in place of its categories.

COMPARETEST: two-sided pairwise z-tests of column proportions (PROP) or t-tests of column means (MEAN: a scale variable with MEAN in the rows) in every row of a subtable.

The letter of the column with the smaller value appears in the column with the larger one: uppercase below the smaller ALPHA, lowercase below the larger; a period marks a column left out (a proportion of 0 or 1, or less than 2 cases). ADJUST defaults to BONFERRONI, MEANSVARIANCE to ALLCATS. With a weight variable the tests round the weighted cell counts, column totals or category weights.

MERGE=YES puts the letters in the main table instead, in a column after each compared category (in the row of MEAN, or of COLPCT or COUNT, when the summaries are in the rows). STYLE=APA (the default) gives letters that two columns share unless they differ significantly at the smaller ALPHA; STYLE=SIMPLE gives the keys of the separate table. SHOWSIG=YES keeps the separate table.

A multiple response set (\$name, see MRSETS) stands where a categorical variable may. A case counts once in every category it has a response in; the set's total counts the cases with any response.

A dichotomy set lists its variables in set order; /CATEGORIES may list them by name: \[q5\_3, q5\_1\].

A set may carry the summaries even with categorical variables nested inside it; of two nested sets only the inner one may. Sets also have RESPONSES and, for every percentage base above, base.RESPONSES (responses over the area's responses), base.RESPONSES.COUNT (responses over the area's count) and base.COUNT.RESPONSES (count over the area's responses). A case without any response counts only in TOTALN and the .TOTALN percentages. COUNTDUPLICATES=YES counts a value that a case gives in two variables of a category set twice in the responses; counts never do.

Tests on sets (INCLUDEMRSETS=YES, the default) count each case once in the margins and allow for the cases two categories of a set share; the means test then pools the variance from the two categories compared. INCLUDEMRSETS=NO leaves sets out of the tests, and so does COUNTDUPLICATES=YES for a category set.

## Example

```text
CTABLES /TABLE q1 [COUNT, COLPCT.COUNT PCT8.1] BY gender /CATEGORIES VARIABLES=q1 TOTAL=YES.
```

`statscraft help CTABLES` prints the same text in the terminal. [All commands](README.md)
