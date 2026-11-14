<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SURVIVAL

Actuarial life tables over fixed time intervals, with the Wilcoxon (Gehan) comparison of groups.

## Syntax

```text
SURVIVAL [TABLES=] times [BY controls(min, max) ...] [BY controls(min, max) ...]
   /INTERVAL=THRU end BY width [THRU end BY width ...]
   /STATUS=var(min[, max]) [FOR times] [/STATUS=...]
   [/PLOTS[(ALL | SURVIVAL | LOGSURV | HAZARD | DENSITY | OMS ...)][=times BY controls BY controls]]
   [/PRINT=TABLE | NOTABLE] [/COMPARE[=times BY controls BY controls]]
   [/CALCULATE=[EXACT | CONDITIONAL | APPROXIMATE] [PAIRWISE] [COMPARE]]
   [/MISSING=[GROUPWISE | LISTWISE] [INCLUDE]] [/WRITE=NONE].
```

Intervals start at 0; each THRU ... BY period continues where the last one ended, and a period that the width does not divide evenly is stretched to the next whole interval.

One more interval, open at the top, holds the longer times. A time falls into the interval whose start it reaches. A case whose status lies in the range had the event; other cases are censored and count as half exposed in their interval. STATUS without FOR serves every time variable that no other STATUS names. Each control's values are truncated to whole numbers and kept inside its range; first order controls each split the cases on their own, a second BY splits every one of those again.

The life table shows, per interval with cases entering: those entering, withdrawing, exposed to risk and having the event, the proportions ending and surviving, the cumulative survival at the interval's end with its standard error, the density and the hazard with theirs (none for the open interval). Median Survival Times interpolates inside the interval where the cumulative survival passes 0.5, or shows the start of the open interval with a plus sign. Weights count cases, so counts per interval may be entered as weighted records. Negative times are missing.

COMPARE tests whether the groups of each first order control share one survival curve, with Gehan's scores (a tie between an event and a censored time ranks the event first); CALCULATE PAIRWISE adds each pair of values, APPROXIMATE scores the interval of each time instead of the time, COMPARE gives the comparisons only. MISSING=GROUPWISE leaves a case out only of the tables whose variables it misses; LISTWISE leaves out a case missing on any TABLES variable (a missing status only affects its own time variable); INCLUDE keeps user-missing values. PLOTS draws, per time variable, first order control and second order value, the survival function, its log, the hazard, the density or one minus survival, the groups as lines. Not supported yet: WRITE=TABLES and BOTH, which need PROCEDURE OUTPUT.

## Example

```text
SURVIVAL TABLES=weeks BY plan(1,3) /INTERVAL=THRU 30 BY 5 /STATUS=outcome(1) /COMPARE /CALCULATE=PAIRWISE /PLOTS(SURVIVAL,HAZARD).
```

`statscraft help SURVIVAL` prints the same text in the terminal. [All commands](README.md)
