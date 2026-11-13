<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSTABULATE

One-way and two-way tables of a complex sample with design-based errors and adjusted tests.

## Syntax

```text
CSTABULATE /PLAN FILE='file' /TABLES VARIABLES=vars [BY var]
   [/CELLS [POPSIZE] [ROWPCT] [COLPCT] [TABLEPCT]]
   [/STATISTICS [SE] [CV] [DEFF] [DEFFSQRT] [CIN[(95)]] [COUNT] [CUMULATIVE]
               [EXPECTED] [RESID] [ASRESID]]
   [/TEST [HOMOGENEITY] [INDEPENDENCE] [ODDSRATIO] [RELRISK] [RISKDIFF]]
   [/SUBPOP TABLE=var [BY var ...] [DISPLAY=LAYERED | SEPARATE]]
   [/MISSING [SCOPE=TABLE | LISTWISE] [CLASSMISSING=EXCLUDE | INCLUDE]].
```

Each variable gets a one-way table, or with BY a two-way table against the BY variable, with estimated population sizes and percents for every cell and margin. The design comes from a CSPLAN plan, as in CSDESCRIPTIVES, and so do the standard errors, df and design effects.

Percents are ratios of population sizes; their intervals are found on the logit scale.

CUMULATIVE (one-way) adds running sums. EXPECTED, RESID and ASRESID (two-way, with TABLEPCT) show the table percent expected under independence, the difference from it and that difference over its design-based standard error. HOMOGENEITY tests equal shares of a one-way table, INDEPENDENCE the rows against the columns: the Pearson and likelihood ratio chi-squares are divided by the sum of the generalized design effects and referred to F with d and d times the design df, d being the squared sum over the sum of squares of those effects (a second-order adjustment). ODDSRATIO, RELRISK and RISKDIFF give the 2 x 2 measures with intervals (log scale for the first two). Tests are for the whole population, and also for each subpopulation when SUBPOP names only first-stage strata. Without CELLS: population sizes; without STATISTICS: standard errors. Not supported yet: JOINTPROB.

## Example

```text
CSTABULATE /PLAN FILE='farms_plan.json' /TABLES VARIABLES=irrigated BY organic /CELLS POPSIZE ROWPCT /STATISTICS SE CIN DEFF /TEST INDEPENDENCE ODDSRATIO.
```

`statscraft help CSTABULATE` prints the same text in the terminal. [All commands](README.md)
