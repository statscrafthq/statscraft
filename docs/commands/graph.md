<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GRAPH

One chart: summaries by category as bars, lines, areas, a pie, points with error bars, a Pareto chart or ranges, a histogram, a scatterplot or a scatterplot matrix.

## Syntax

```text
GRAPH [/TITLE='line' ['line']] [/SUBTITLE='line'] [/FOOTNOTE='line' ['line']]
      /BAR[(SIMPLE | GROUPED | STACKED | RANGE)]=spec | /PIE=spec
      | /LINE[(SIMPLE | MULTIPLE | AREA | DROP | DIFFERENCE)]=spec | /HILO[(SIMPLE | GROUPED)]=spec
      | /PARETO[(CUM | NOCUM)][(SIMPLE | STACKED)]=spec
      | /ERRORBAR[(CI [95] | STERROR [2] | STDDEV [2])]=varlist [BY var] | var BY var BY var
      | /HISTOGRAM[(NORMAL)]=var
      | /SCATTERPLOT[(BIVARIATE | OVERLAY | MATRIX | XYZ)]=x WITH y | xlist WITH ylist [(PAIR)]
          | varlist | x WITH y WITH z, then [BY group] [BY id (NAME | IDENTIFY)]
      [/INTERVAL CI[(95)] | SE[(2)] | STDDEV[(2)]]
      [/PANEL [COLVAR=varlist [COLOP=CROSS | NEST]] [ROWVAR=varlist [ROWOP=CROSS | NEST]]]
      [/MISSING=[LISTWISE | VARIABLE] [NOREPORT | REPORT] [EXCLUDE | INCLUDE]].
```

The command draws one chart. The first subcommand needs no slash, and keywords may be cut to three letters (MIN, MAX, ERR, STE). A spec is COUNT, PCT, CUFREQ or CUPCT BY var \[BY var\]; a bare variable (COUNT BY var); func(varlist) \[BY var \[BY var\]\]; or VALUE(varlist) \[BY var\]. The functions are MEAN, MEDIAN, GMEDIAN, MODE, SUM, CUSUM, N, MINIMUM, MAXIMUM, STDDEV, VARIANCE, PTILE(varlist, p), PLT, PGT, NLT, NGT(varlist, x), PIN and NIN(varlist, low, high). SUM(a b) means SUM(a) SUM(b).

The spec sets the layout. One function by one variable gives one series; by two variables, a series per value of the second. Several functions give a series each by one variable, or one bar or slice each without BY. SIMPLE, GROUPED and MULTIPLE change nothing; STACKED stacks the series and AREA fills and stacks them. A pie takes one series.

COUNT and N add up the case weights; PCT is a share of all cases in the chart. CUFREQ, CUSUM and CUPCT add up along each series (CUPCT ends at 100). PLT, PGT and PIN give the share of valid values below x, above x or from low to high; NLT, NGT and NIN the weighted count. MEDIAN and PTILE work as in FREQUENCIES, MODE takes the smallest tied value, and GMEDIAN treats values as group midpoints. VALUE draws an unweighted bar per case and a series per variable; a missing value leaves a gap and the BY variable names the cases.

Categories ascend by value. A case missing a category value is left out unless MISSING=REPORT, which adds missing categories after the valid ones. LISTWISE drops a case missing on any function variable; VARIABLE drops it only from that variable's functions. INCLUDE makes user-missing values valid. Categories empty in every series are left out; at most 3000 categories. Cases with a zero or missing weight are left out. SPLIT FILE gives a chart per group.

ERRORBAR shows each mean as a point with a bar, laid out as MEAN(varlist). CI (the default, level 95, from 50 to 99.9) uses Student's t with W - 1 degrees of freedom; STERROR (or SE) and STDDEV reach n (default 2) standard errors or deviations each way. A mean whose cases weigh 1 or less gets no bar. INTERVAL adds such bars to a simple or clustered bar or line chart of MEAN, or of MEDIAN with CI only. A median's interval comes from interpolated order statistics, need not be symmetric and needs whole case weights.

PARETO counts or sums (COUNT, a bare variable, SUM or VALUE), largest category first, several series stacked. CUM, the default, adds the running total as a percentage line on a right axis from 0 to 100; NOCUM leaves it off. BAR(RANGE) floats a bar between two series (two functions, or a second BY variable with two values). LINE(DROP) joins the lowest and highest series of each category, LINE(DIFFERENCE) shades between two series, and HILO takes high, low and an optional close as two or three series.

HISTOGRAM bins the valid values as FREQUENCIES does; NORMAL adds the normal curve. SCATTERPLOT colors points by the BY group variable; BY id (NAME) labels them with the id values, (IDENTIFY) does not. Lists around WITH put every pair into one frame (with PAIR, first with first and so on, and no BY group), the variable before WITH across. A list without WITH draws a matrix with the names on the diagonal. x WITH y WITH z has a third axis and shows as a table of its points. LISTWISE needs a case valid on all variables; VARIABLE needs it valid on the pair.

PANEL repeats the chart for each combination of ROWVAR and COLVAR categories, rows first, each named in its title. Empty combinations get no chart, so COLOP and ROWOP change nothing. A case missing on a panel variable is left out unless REPORT. TITLE names the chart (by default after its content), SUBTITLE goes under it and FOOTNOTE below. TITLE and FOOTNOTE take up to two lines, SUBTITLE one, each at most 72 characters.

## Example

```text
GRAPH /TITLE='Visits per store' /BAR(STACKED)=COUNT BY store BY daypart.
```

`statscraft help GRAPH` prints the same text in the terminal. [All commands](README.md)
