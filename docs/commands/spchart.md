<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SPCHART

Control charts (X-bar with R or s, individuals, p, np, c, u) and the process capability chart.

## Syntax

```text
SPCHART [/TITLE='line' ['line']] [/SUBTITLE='line'] [/FOOTNOTE='line' ['line']]
   /XR | /XS = var BY group | var var [var ...] [BY label] [(XBARONLY)]
   | /IR | /I = var [BY label]
   | /P | /NP = status BY group | COUNT(var) N(var | n) [BY label]
   | /C | /U = defects BY group | COUNT(var) N(var | n) [BY label]
   | /CPCHART = var BY group | var var [var ...] | var
   [/STATISTICS=keywords] [/RULES=[ALL] [UCL] [R2UPPER] [R4UPPER] [R8UPPER] [R8LOWER]
             [R4LOWER] [R2LOWER] [LCL] [TRUP] [TRDOWN] [ALTERNATING]] [/ID=var]
   [/CAPSIGMA=RBAR | SBAR | MRBAR | WITHIN] [/SPAN=2] [/CONFORM | /NONCONFORM=value]
   [/SIGMAS=3] [/MINSAMPLE=2] [/LSL=value] [/USL=value] [/TARGET=value]
   [/MISSING=[NOREPORT | REPORT] [EXCLUDE | INCLUDE]]
   [/NORMAL=CAPABILITY | PERFORMANCE | BOTH | NONE] [/REFERENCE=[SL] [SIGMAS] | NONE].
```

One chart subcommand; XR may be left out. Each control chart is a line chart of the process with its center line and the control limits at SIGMAS sigmas (lower limits of ranges, standard deviations and counts stop at 0, a p chart's upper one at 1, an np chart's at the subgroup size); a series that is the same everywhere carries its value in its name. LSL and USL add fixed limit lines to the first chart. var BY group: cases are units, gathered into subgroups by the group variable in ascending order; var var ...: each case is a subgroup, one unit per variable. XR and XS plot subgroup means with sigma from the mean of R/d2(n) or S/c4(n), then the ranges or standard deviations (left out with XBARONLY); subgroups below MINSAMPLE units are dropped. IR and I plot each value with sigma from the mean moving range over SPAN values over d2(SPAN); IR adds the moving ranges, missing for the first SPAN-1 points. P plots the share nonconforming against the pooled share, NP the number; CONFORM names the one good value of the status (any other value is a defect), NONCONFORM the one bad value. C plots defects per subgroup, U defects per unit. COUNT and N read counts and sizes from each case; a count above its size drops a p or np subgroup. Unequal sizes give each subgroup its own limits, with a warning for np and c. BY label only names the points, else they are numbered by case.

RULES (ALL when empty) checks the first chart: beyond a control limit, 2 of 3 past 2 sigma, 4 of 5 past 1 sigma, 8 on one side of the center, 6 rising or falling, 14 going up and down in turn; only the last point of a pattern counts. The Rule Violations table names the points by ID, else by the BY value. CPCHART draws a histogram with the normal curve of the mean and the capability sigma (NORMAL: PERFORMANCE uses the standard deviation, NONE none, BOTH draws the capability curve); LSL and USL are required, REFERENCE values go to the footnote.

STATISTICS (XR, XS, IR, I and CPCHART): CP CPL CPU K CPK CR CPM CZL CZU CZMIN CZMAX CZOUT CZLOUT CZUOUT with the capability sigma, PP PPL PPU PPK PR PPM PZL PZU PZMIN PZMAX PZOUT PZLOUT PZUOUT with the standard deviation of all values, N MEAN STDDEV CAPSIGMA LSL USL TARGET, and AZOUT AZLOUT AZUOUT, the observed percentages at or past the limits. Keywords that need LSL and USL, or TARGET between them, are left out with a warning. CAPSIGMA: RBAR (XR), SBAR (XS) or WITHIN for subgroups, MRBAR for single values. CPCHART without STATISTICS lists its indices in a table. MISSING: REPORT keeps subgroups for missing BY values, INCLUDE counts user-missing process values. Weights count only for units, whole positive ones; each unit of an individuals chart is repeated by its weight. A chart has 2 to 3000 points.

## Example

```text
SPCHART /XR=fill_ml BY batch /RULES=ALL /STATISTICS=CP CPK PP PPK /LSL=495 /USL=505.
```

`statscraft help SPCHART` prints the same text in the terminal. [All commands](README.md)
