<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# IGRAPH

The older chart command: bars, lines, areas, pies, boxes, error bars, histograms and scatterplots built from variables placed on axes and legends.

## Syntax

```text
IGRAPH [/Y=VAR(var) | $COUNT | $PCT [TYPE=SCALE [(MIN=n MAX=n)] | CATEGORICAL] [TITLE='text']]
       [/X1=VAR(var) | $CASE [TYPE=...] [TITLE='text']] [/X2=VAR(var) [TYPE=...] [TITLE='text']]
       [/COLOR=VAR(var) [TYPE=...] [LEGEND=ON | OFF] [TITLE='text'] [CLUSTER | STACK]]
       [/STYLE=VAR(var) [LEGEND=ON | OFF] [TITLE='text'] [CLUSTER | STACK]] [/SIZE=VAR(var) ...]
       [/SUMMARYVAR=VAR(var) | $COUNT | $PCT] [/STACK=VAR(var)] [/PANEL VAR(var) [VAR(var) ...]]
       [/CATORDER VAR(var) ([ASCENDING | DESCENDING] [VALUE | LABEL | COUNT | OCCURRENCE]
           [OMITEMPTY | SHOWEMPTY])] [/POINTLABEL=VAR(var) [ALL | NONE]] [/CASELABEL=VAR(var)]
       [/COORDINATE=VERTICAL | HORIZONTAL | THREE] [/NORMALIZE] [/TITLE='text'] [/SUBTITLE='text']
       [/CAPTION='text'] [/VIEWNAME='text'] [/MISSING=[LISTWISE | VARIABLEWISE] [NOREPORT | REPORT]
           [EXCLUDE | INCLUDE]]
       /BAR [(function)] | /LINE [(function)] | /AREA [(function)] | /PIE [(function)]
       | /BOX [OUTLIERS=ON | OFF] [EXTREME=ON | OFF] | /ERRORBAR CI(level) | SD(n) | SE(n)
       | /HISTOGRAM [CUM] [CURVE=ON | OFF] | /SCATTER [/FITLINE METHOD=REGRESSION LINEAR
           | ORIGIN LINEAR | MEAN | NONE [LINE=TOTAL MEFFECT]].
```

function: MEAN MEDIAN MODE MINIMUM MAXIMUM SUM SUMAV SUMSQ STDDEV VARIANCE SEMEAN SKEW KURTOSIS SESKEW SEKURT FIRST LAST, PTILE(p), NLT NLE NEQ NGE NGT(x), PLT PLE PEQ PGE PGT(x), NIN PIN(low, high). A variable may also be written without VAR( ).

Each chart is drawn as GRAPH would draw it. BAR, LINE and AREA put the categories of X1 along the axis and a summary of Y on the other: the function given (MEAN when none is) of a Y variable, or the count or percentage of cases with Y=\$COUNT or \$PCT. A categorical COLOR or STYLE variable gives a series per category, clustered unless STACK is given; NORMALIZE then stacks to 100%.

X1=\$CASE draws a bar per case, named by the CASELABEL variable. A Y variable of TYPE=CATEGORICAL takes MODE only. ERRORBAR alone draws means of Y with bars of a confidence interval or of n standard deviations or errors; given with BAR or LINE it adds those bars to the means.

PIE takes its slices from the COLOR (or STYLE) categories, sized by SUMMARYVAR: \$COUNT (the default), \$PCT, or a variable with a function such as PIE (SUM). X1, Y and STACK variables give a separate pie per category, as PANEL does.

BOX draws Tukey boxes of Y per category of X1, side by side per COLOR category: hinges, median, whiskers to the furthest values within 1.5 box lengths, outliers beyond them and extremes beyond 3 box lengths, each marked by case number. OUTLIERS=OFF and EXTREME=OFF leave those marks out. HISTOGRAM bins X1 with automatic widths as GRAPH does; Y=\$PCT shows percentages, CUM running totals and CURVE=ON the normal curve.

SCATTER plots Y against X1, a group per COLOR or STYLE category; a scale X2 adds a third axis.

POINTLABEL with ALL labels the points. FITLINE adds a least squares line, a line through the origin, or the mean of Y, over all points (LINE=TOTAL, the default) or per group (MEFFECT); the footnote gives each line's intercept and slope. The points and lines are unweighted.

A variable takes its TYPE from its measurement level when none is given. A scale COLOR variable only colors the elements, so it is ignored. CATORDER sorts the categories of a variable by value (the default), label, weighted count or first case. PANEL repeats the chart for each combination of the panel categories. TITLE, SUBTITLE and CAPTION go above and below the chart (a \\n inside becomes a space); VIEWNAME names it in the outline. X1 and Y TITLE name the axes, MAX sets the top of the axis. A case missing on a variable of the chart is left out; MISSING works as in GRAPH. With SPLIT FILE each group gets a chart.

Keywords that only change the look (CHARTLOOK, the LENGTH subcommands, EFFECT, SIZE, KEY=OFF, LEGEND=OFF, MIN, label, baseline, whisker, cap and line style settings, JITTER, SHOWEMPTY, X1INTERVAL and X1START, FORMAT) are accepted and named in one warning. REFLINE, SPIKE, FITLINE METHOD=LLR, FITLINE INTERVAL and SPIKE=ON cannot be drawn and give an error, as do X2 outside a scatterplot and a mix of elements other than those above.

## Example

```text
IGRAPH /X1=VAR(store) TYPE=CATEGORICAL /Y=VAR(basket) TYPE=SCALE /COLOR=VAR(daypart) /BAR (MEAN).
```

`statscraft help IGRAPH` prints the same text in the terminal. [All commands](README.md)
