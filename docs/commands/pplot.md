<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PPLOT

Probability plots: Q-Q or P-P plots of variables against a test distribution.

## Syntax

```text
PPLOT [VARIABLES=] varlist
   [/DISTRIBUTION=NORMAL[(a,b)] | EXPONENTIAL[(a)] | WEIBULL[(a,b)] | PARETO[(a,b)] |
    LNORMAL[(a,b)] | BETA[(a,b)] | GAMMA[(a,b)] | LOGISTIC[(a,b)] | LAPLACE[(a,b)] |
    UNIFORM[(a,b)] | HNORMAL[(a)] | CHI(df) | STUDENT(df)]
   [/FRACTION=BLOM | RANKIT | TUKEY | VW] [/TIES=MEAN | LOW | HIGH | BREAK]
   [/TYPE=Q-Q | P-P] [/PLOT=BOTH | NORMAL | DETRENDED] [/STANDARDIZE | /NOSTANDARDIZE]
   [/LN | /NOLOG] [/DIFF[=n]] [/SDIFF[=n]] [/PERIOD=n] [/APPLY].
```

Parameters: NORMAL, LOGISTIC, LAPLACE location and scale; EXPONENTIAL rate; WEIBULL, LNORMAL scale and shape; PARETO threshold and shape; BETA two shapes; GAMMA shape and rate; UNIFORM minimum and maximum; HNORMAL scale (location 0). Without them they are estimated from the plotted values (moments; Weibull by least squares on the ranks; Pareto from the minimum and the mean log; half-normal by maximum likelihood) and listed in a table. Each variable is a series in case order: missing values are left out, LN logs it first (0 and below become missing), DIFF and SDIFF (over PERIOD cases; without PERIOD SDIFF is left out) difference it, STANDARDIZE makes z-scores of the result. Ranks of tied values follow TIES (BREAK numbers the distinct values 1, 2, ...); FRACTION turns rank r of n into a proportion p: BLOM (r-3/8)/(n+1/4), RANKIT (r-1/2)/n, TUKEY (r-1/3)/(n+1/3), VW r/(n+1).

Q-Q (the default) plots each value against the distribution's quantile at p, P-P plots p against the distribution function at the value, both with the line y = x; the detrended plot shows observed minus expected, with the line y = 0. BOTH draws both plots. Values the distribution cannot take leave the variable out with a warning. Weights are not used.

APPLY repeats the previous PPLOT (its distribution without the parameters, and its variables unless new ones are given); the subcommands given with it override.

Not supported yet: APPLY of a named model, TEMPLATE.

## Example

```text
PPLOT VARIABLES=wait_min /DISTRIBUTION=GAMMA /TYPE=P-P /PLOT=NORMAL.
```

`statscraft help PPLOT` prints the same text in the terminal. [All commands](README.md)
