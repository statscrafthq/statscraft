<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SEASON

Seasonal decomposition by ratios (or differences) to a moving average.

## Syntax

```text
SEASON [VARIABLES=] series [/MODEL=MULTIPLICATIVE | ADDITIVE] [/MA=EQUAL | CENTERED]
       [/PERIOD=n] [/APPLY].
```

The period comes from PERIOD, else TSET PERIOD, else the DATE variables; the series needs at least four full periods and no missing value. A moving average over one period gives the trend: for an even period CENTERED (the default) spans period + 1 cases with half weight at both ends; EQUAL weighs the period cases equally, from period/2 cases before a case to period/2 - 1 after it. An odd period always uses EQUAL. The multiplicative model divides the series by it (times 100), the additive one subtracts it. For each position in the period the mean of these values without the largest and the smallest is the raw factor; the factors are scaled to average 100 (multiplicative) or centered on 0 (additive). The adjusted series SAS is the series divided by (or less) the factor, the trend-cycle STC a 1-2-3-2-1 weighted average of SAS (3-term at the second and next-to-last case, extrapolated at the ends) and the irregular part ERR SAS over (or less) STC. Tables: the model, the cases with every component (left out under TSET PRINT=BRIEF) and the seasonal factors. TSET NEWVAR saves ERR\_n, SAS\_n, SAF\_n and STC\_n per series; saved multiplicative factors are ratios, so SAS = series / SAF.

APPLY repeats the previous SEASON, or the model named in quotes (MOD\_1, MOD\_2, ... in the order the session made them).

## Example

```text
SEASON VARIABLES=heating_kwh /MODEL=ADDITIVE /PERIOD=12.
```

`statscraft help SEASON` prints the same text in the terminal. [All commands](README.md)
