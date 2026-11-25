<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# XGRAPH

Charts from one expression: summaries as bars or lines, clustered, stacked and paneled, population pyramids, binned histograms, dot plots and boxes.

## Syntax

```text
XGRAPH [CHART=] (y [+ y ...]) [> var | 1] [BY x [> var] [BY z [> var]]]
   [/BIN START=AUTO | n SIZE=AUTO | WIDTH(n) | COUNT(n)] [/DISPLAY DOT=ASYMMETRIC | SYMMETRIC | FLAT]
   [/DISTRIBUTION TYPE=NORMAL] [/COORDINATE SPLIT=NO | YES] [/ERRORBAR CI(level) | STDDEV(n) | SE(n)]
   [/MISSING USE=LISTWISE | VARIABLEWISE REPORT=NO | YES]
   [/PANEL COLVAR=varlist [COLOP=CROSS | NEST] ROWVAR=varlist [ROWOP=CROSS | NEST]]
   [/TEMPLATE FILE='file'] [/TITLES TITLE='line' ['line'] SUBTITLE='line' FOOTNOTE='line' ['line']].
```

y: var \[function\] \[element\], (var + var ...) \[function\] \[element\], or \[count function\] \[element\].

x, z: var \[c\] or \[s\], \$CASENUM \[LABEL=var\], or 1, the stand-in for the y variables.

function: COUNT PCT CUPCT CUFREQ (no variable); VALUE; MINIMUM MAXIMUM VALIDN SUM CUSUM MEAN STDDEV VARIANCE MEDIAN GMEDIAN MODE, PTILE(p), GPTILE(p), PLT PGT NLT NGT(x), PIN NIN(low, high).

element: BAR (the default), LINE, HISTOBAR, POINT, BOX. Square brackets are typed as shown, one keyword in each; names and keywords are written in full, each subcommand at most once.

BAR and LINE draw the function of y for every category of x. A variable after \> on y stacks the bars; one after \> on x clusters them; z gives the series when neither is there. Every further dimension (a second cluster, or z next to a stack or cluster) repeats the chart for each of its categories, as PANEL does. Several y variables, joined with +, need the 1 where they form the categories, the series or the stack. \[VALUE\] draws a bar for every case, the cases numbered on the axis that holds \$CASENUM or named by its LABEL variable; a variable on the other axis gives a chart for each of its categories. \[COLOR\] and \[PATTERN\] after a stack or cluster variable are accepted and change nothing. GMEDIAN and GPTILE take each value as the midpoint of its interval and interpolate on the cumulative weights; a percentile below the first midpoint or above the last is missing.

COORDINATE SPLIT=YES turns a bar chart into a population pyramid: horizontal bars with a series for each category of z. HISTOBAR counts the cases in bins of a scale x and draws a histogram for each category of z, all with the same bins, which BIN sets (by default as FREQUENCIES makes them; COUNT(n) gives n bins from the start to the largest value).

DISTRIBUTION adds the normal curve of each histogram. POINT puts a dot for every case at its x value; DISPLAY stacks equal values up from 1 (ASYMMETRIC), around 0 (SYMMETRIC) or not at all (FLAT). BOX draws Tukey boxes of the y variable by x, side by side by the x cluster or z.

ERRORBAR adds bars to counts and percentages (CI: Jeffreys' interval; SE and STDDEV: n times the binomial spread of the count), means, and medians (CI only). MISSING USE=LISTWISE leaves out a case missing on any y variable, VARIABLEWISE only from that variable; REPORT=YES keeps categories of missing values. TITLES lines may hold )DATE, )TIME and )CHART. TEMPLATE is accepted and ignored with a warning. With SPLIT FILE each group gets its charts.

## Example

```text
XGRAPH CHART=(spend [MEAN] [BAR]) BY region[c] > channel[c] BY season[c]
  /ERRORBAR SE(1) /TITLES TITLE='Spend by region'.
```

`statscraft help XGRAPH` prints the same text in the terminal. [All commands](README.md)
