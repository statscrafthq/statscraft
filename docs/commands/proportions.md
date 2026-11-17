<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PROPORTIONS

Tests and confidence intervals for a proportion or a difference of two proportions.

## Syntax

```text
PROPORTIONS /ONESAMPLE varlist [TESTVAL=0.5] [TESTTYPES=type ...] [CITYPES=type ...]
PROPORTIONS /PAIREDSAMPLES varlist [WITH varlist [(PAIRED)]] [TESTTYPES=...] [CITYPES=...]
PROPORTIONS /INDEPENDENTSAMPLES varlist BY var [SELECT=LEVEL(1, 2) | MIDPOINT | CUTPOINT(value)]
            [TESTTYPES=...] [CITYPES=...]
            [/SUCCESSVALUE=LAST | FIRST | LEVEL(values) | MIDPOINT | CUTPOINT(value)]
            [/MISSING [SCOPE=ANALYSIS | LISTWISE] [USERMISSING=EXCLUDE | INCLUDE]] [/CRITERIA CILEVEL=95].
```

One analysis per command. Tests, defaults marked \*: one sample EXACT MIDP\* SCORE\* SCORECC WALD WALDCC; paired EXACT MIDP\* MCNEMAR\* MCNEMARCC WALD WALDCC; independent HAUCK\_ANDERSON WALD WALDCC WALDH0\* WALDH0CC. Intervals: one sample AGRESTI\_COULL\* ANSCOMBE CLOPPER\_PEARSON JEFFREYS\* LOGIT WALD WALDCC WILSON\_SCORE\* WILSON\_SCORECC; paired AGRESTI\_MIN BONETT\_PRICE\* NEWCOMBE\* WALD WALDCC; independent AGRESTI\_CAFFO\* BROWN\_LI\_JEFFREYS HAUCK\_ANDERSON NEWCOMBE\* NEWCOMBECC WALD WALDCC. NONE turns tests or intervals off. TESTVAL, SELECT, TESTTYPES and CITYPES may also be written as subcommands. A case is a success when its value is the highest (LAST) or lowest (FIRST) valid value the analysis sees, one of the LEVEL values, no less than (lowest + highest) / 2 (MIDPOINT) or no less than the cut point; every other valid value is a failure. Groups: the two SELECT values, or the cases below the middle of the grouping variable's range or below the cut point as group 1 and the rest as group 2; without SELECT the grouping variable is numeric with groups 1 and 2. A paired list without WITH pairs every variable with every later one; WITH pairs every left with every right variable, (PAIRED) the n-th with the n-th. ANALYSIS drops a case per analysis, LISTWISE drops it from all when any variable named is missing. Weights count as frequencies; exact and mid-p probabilities use the counts rounded to whole numbers. Bounds are kept within 0 to 1 (-1 to 1 for differences).

Two-sided p values are twice the smaller tail, at most 1. CILEVEL is a percent; a value below 1 is read as a fraction.

## Example

```text
PROPORTIONS /INDEPENDENTSAMPLES passed BY school SELECT=LEVEL('north', 'south')
  TESTTYPES=WALDH0 HAUCK_ANDERSON CITYPES=NEWCOMBE WALD /CRITERIA CILEVEL=90.
```

`statscraft help PROPORTIONS` prints the same text in the terminal. [All commands](README.md)
