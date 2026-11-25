<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# GGRAPH

A chart from a graph dataset and a chart program in BEGIN GPL: bars (clustered, stacked), lines, areas, points, a pie or a histogram.

## Syntax

```text
GGRAPH /GRAPHDATASET NAME="name" VARIABLES=item ... [TRANSFORM=NO | VARSTOCASES(SUMMARY="n" INDEX="n")]
              [MISSING=LISTWISE | VARIABLEWISE] [REPORTMISSING=NO | YES] [CASELIMIT=n]
       /GRAPHSPEC SOURCE=INLINE [LABEL="text"].
```

BEGIN GPL chart program END GPL.

item: a variable (ALL and TO allowed) with an optional \[NAME="new" LEVEL=SCALE \| NOMINAL \| ORDINAL\], or a function with an optional \[NAME="new"\]: COUNT(), MINIMUM MAXIMUM VALIDN SUM MEAN STDDEV VARIANCE MEDIAN GMEDIAN MODE (var), PTILE GPTILE PLT PGT NLT NGT NLE PLE NEQ PEQ NGE PGE (var, x), PIN NIN (var, low, high), CASEVALUE(var). Keywords are written in full. A function makes one row per combination of the other variables' values; its column is called by its name and arguments joined with underscores (MEAN\_price, PTILE\_price\_90) unless NAME renames it. Without a function every case with a positive weight is a row, and \$CASENUM numbers them. Weights count in every function.

GMEDIAN and GPTILE take each value as the midpoint of its interval and interpolate on the cumulative weights; a percentile below the first midpoint or above the last is missing.

LISTWISE drops a case missing on any listed variable; VARIABLEWISE drops it only from the functions of the variable it misses. REPORTMISSING=YES turns user-missing values of variables that are not at the scale level into categories of their own. VARSTOCASES makes a row of each function, numbered in the INDEX column with the value in the SUMMARY column. CASELIMIT keeps the first n rows.

Each GRAPHSPEC takes the next BEGIN GPL block; both stand right after the command. In the chart program: SOURCE: s=userSource(id("name")); DATA: x=col(source(s), name("column") \[, unit.category()\]); GUIDE: axis(dim(1 \| 2), label("text") \| null()), legend(aesthetic(...), label("text")), text.title, text.subtitle, text.footnote(label("text")); SCALE: linear(dim(2), include(0), max(n)), cat(dim(n) \| aesthetic(...), include("value", ...)), which adds categories; COORD: rect(dim(1,2), cluster(3)) clusters, transpose() lays bars across, polar.theta() makes a pie; ELEMENT: interval, interval.dodge, interval.stack, line, area, area.stack or point with position(x\*y) or position(x\*y\*z) with cluster, and color(variable) for a series per category or color("name") to name the series. Several rows on one point need summary.count, mean, sum, median, mode, min, max or percent around the position; position(summary.count(bin.rect(x))) is a histogram and line(position(density.normal(x))) adds its normal curve. point over two scale variables is a scatterplot. Other statements and settings give a warning and are skipped.

## Example

```text
GGRAPH /GRAPHDATASET NAME="sales" VARIABLES=store daypart MEAN(basket)
  /GRAPHSPEC SOURCE=INLINE.
BEGIN GPL
SOURCE: s=userSource(id("sales"))
DATA: store=col(source(s), name("store"), unit.category())
DATA: daypart=col(source(s), name("daypart"), unit.category())
DATA: basket=col(source(s), name("MEAN_basket"))
GUIDE: axis(dim(2), label("Average basket"))
ELEMENT: interval.stack(position(store*basket), color(daypart))
END GPL.
```

`statscraft help GGRAPH` prints the same text in the terminal. [All commands](README.md)
