<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# KM

Kaplan-Meier survival estimates by factor level and stratum, with tests that compare the levels.

## Syntax

```text
KM time [BY factor] /STATUS=var [EVENT](values) [LOST(values)]
   [/STRATA=var] [/ID=var] [/PLOT=[SURVIVAL] [LOGSURV] [HAZARD] [OMS]]
   [/PRINT=[TABLE] [MEAN] [NONE]] [/PERCENTILES=[(]values[)]]
   [/TEST=[LOGRANK] [BRESLOW] [TARONE]] [/COMPARE=[OVERALL | PAIRWISE] [POOLED | STRATA]]
   [/TREND[=(weights)]] [/SAVE=[SURVIVAL[(name)]] [HAZARD[(name)]] [SE[(name)]] [CUMEVENT[(name)]]].
```

K-M is another name. A case whose status is one of the EVENT values (single values or a THRU b, quoted for a string status) had the event at its time; any other status counts as censored, and LOST values are counted apart in the tables though they are censored too. Cases with a missing time, status, factor or stratum, or a negative time, are left out; weights count cases. Each factor level within each stratum gets its own curve.

Tables: Case Processing Summary; a Survival Table per curve, a row per case with the estimate and its Greenwood standard error at each event time, the events so far and the cases still at risk (ID labels the rows); Means and Medians, the mean being the area under the curve up to the longest time; Percentiles (25, 50 and 75 when no values are given), where the p% value is the first event time at which the estimate is p/100 or lower. PRINT names what to show: TABLE, MEAN or NONE.

With a factor, the log rank test (or TEST: Breslow, weighted by the cases at risk, and Tarone-Ware, by their square root) compares the levels: all at once or each pair (PAIRWISE), pooled over the strata or within each (STRATA). TREND tests a linear trend over the levels in ascending order with one degree of freedom, with the given weights or evenly spaced ones centered on 0. PLOT draws, for each stratum, the step curves of the levels with censored times marked: SURVIVAL, LOGSURV (its log), HAZARD (minus its log, the cumulative hazard) and OMS (one minus it). SAVE adds the estimate, the cumulative hazard, the standard error and the events up to each case's time; the default names are SUR\_n, HAZ\_n, SE\_n and CUM\_n.

## Example

```text
KM weeks BY plan /STATUS=outcome EVENT(1) LOST(2) /STRATA=region /TEST=LOGRANK BRESLOW /COMPARE=PAIRWISE /PLOT=SURVIVAL.
```

`statscraft help KM` prints the same text in the terminal. [All commands](README.md)
