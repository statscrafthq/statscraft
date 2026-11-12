<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GRAPH

One chart: summaries by category as bars, lines, areas or a pie, a histogram or a scatterplot.

## Syntax

```text
GRAPH [/TITLE='line' ['line']] [/SUBTITLE='line'] [/FOOTNOTE='line' ['line']]
      /BAR[(SIMPLE | GROUPED | STACKED)]=spec | /LINE[(SIMPLE | MULTIPLE | AREA)]=spec | /PIE=spec
      | /HISTOGRAM[(NORMAL)]=var | /SCATTERPLOT[(BIVARIATE)]=x WITH y [BY group] [BY id (NAME | IDENTIFY)]
      [/MISSING=[LISTWISE | VARIABLE] [NOREPORT | REPORT] [EXCLUDE | INCLUDE]].
```

spec: COUNT \| PCT \| CUFREQ \| CUPCT BY var \[BY var\]; var alone (COUNT BY var); func(varlist) ...

\[BY var \[BY var\]\]; VALUE(varlist) \[BY var\]. func: MEAN MEDIAN GMEDIAN MODE SUM CUSUM N MINIMUM MAXIMUM STDDEV VARIANCE, PTILE(varlist, p), PLT PGT NLT NGT(varlist, x), PIN NIN(varlist, low, high).

Keywords may be cut to three letters (MIN, MAX); the first subcommand needs no slash.

The shape of the spec sets the layout: one function by one variable gives one series; one function by two variables gives a series per value of the second; several functions by one variable give a series per function; several functions without BY give one bar or slice each. A list such as SUM(a b) is SUM(a) SUM(b). SIMPLE, GROUPED and MULTIPLE change nothing; STACKED stacks the series and AREA fills under the lines and stacks them. A pie takes a single series.

COUNT and N add up the case weights; PCT is a share of all cases in the chart; CUFREQ, CUSUM and CUPCT add up along the categories of each series, CUPCT ending at 100 per series. PLT, PGT and PIN give the share of a category's valid values below x, above x or from low to high; NLT, NGT and NIN the weighted count. MEDIAN and PTILE use (W + 1) p as FREQUENCIES does; MODE is the smallest of tied values; GMEDIAN takes the values as group midpoints. VALUE draws a bar per case and a series per variable, unweighted; missing values leave a gap and the BY variable names the cases.

Categories ascend by value. A case missing a category value is left out unless REPORT, which adds categories for missing values after the valid ones. LISTWISE drops a case missing on any function variable of the chart, VARIABLE only from that variable's functions; INCLUDE makes user-missing values valid. A category empty in every series is left out; at most 3000 categories.

HISTOGRAM bins the valid values as FREQUENCIES does; NORMAL adds the normal curve. SCATTERPLOT draws each case valid on both variables once, colored by the BY group variable; BY id (NAME) labels the points with that variable's values, (IDENTIFY) leaves them off.

TITLE names the chart (otherwise it is named after what it shows); SUBTITLE goes under the title and FOOTNOTE below the chart. Lines have at most 72 characters. With SPLIT FILE each group gets a chart. Weighted when a weight variable is set; cases with a zero or missing weight are left out.

Not supported yet: ERRORBAR, PARETO, HILO, PANEL, INTERVAL, TEMPLATE, BAR(RANGE), LINE(DROP) and LINE(DIFFERENCE), and OVERLAY, MATRIX and XYZ scatterplots; each gives an error.

## Example

```text
GRAPH /TITLE='Visits per store' /BAR(STACKED)=COUNT BY store BY daypart.
```

`statscraft help GRAPH` prints the same text in the terminal. [All commands](README.md)
