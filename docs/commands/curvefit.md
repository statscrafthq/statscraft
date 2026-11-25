<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CURVEFIT

Fit up to eleven curves of one independent variable, or of time, by least squares.

## Syntax

```text
CURVEFIT [VARIABLES=] dependents [WITH independent]
   [/MODEL=[LINEAR] [LOGARITHMIC] [INVERSE] [QUADRATIC] [CUBIC] [COMPOUND] [POWER] [S]
          [GROWTH] [EXPONENTIAL] [LGSTIC] [ALL]]
   [/CONSTANT | /NOCONSTANT] [/UPPERBOUND=NO | n] [/CIN=95] [/PRINT=ANOVA]
   [/SAVE=[PRED] [RESID] [CIN]] [/PLOT=FIT | NONE] [/ID=variable] [/TEMPLATE='file']
   [/APPLY[='model name'] [SPECIFICATIONS | FIT]].
```

Model names may be cut to three letters. With x the independent (or 1, 2, 3, ... in case order when there is no WITH): LINEAR b0 + b1 x, LOGARITHMIC b0 + b1 ln x, INVERSE b0 + b1/x, QUADRATIC and CUBIC add x^2 and x^3, COMPOUND b0 b1^x, POWER b0 x^b1, S exp(b0 + b1/x), GROWTH exp(b0 + b1 x), EXPONENTIAL b0 exp(b1 x), LGSTIC 1 / (1/u + b0 b1^x) with u from UPPERBOUND (no bound: 1/u is 0). The last six are fitted on the log scale, so they need every dependent value above 0 (and below u); their b0 and multiplicative b1 come back from the logarithm, with standard errors scaled the same way. NOCONSTANT drops b0. LINEAR is the default. A case missing on any named variable is left out of every fit; user-missing values count as missing unless TSET MISSING=INCLUDE. Case weights apply only when there is an independent variable. CIN, CONSTANT or NOCONSTANT and ID default to their TSET settings, and TSET TOLER is the tolerance QUADRATIC and CUBIC need.

Each dependent gets a Model Summary and Parameter Estimates table: R square, F, df and its significance and the estimates of every model. PRINT=ANOVA adds per model the summary, the analysis of variance and the coefficients with t tests. SAVE adds per dependent and model FIT\_n (PRED), ERR\_n (RESID, in the original units) and LCL\_n and UCL\_n (CIN: a prediction interval at the CIN level), n the first number free for all of them; fits and intervals are also given for cases with only the dependent missing. Without SAVE, TSET NEWVAR decides: CURRENT (the default) saves all four, numbered from 1 in the order of dependents and models and replacing variables of those names; ALL saves them under free numbers; NONE saves none. PLOT=FIT (the default) gives a chart of the observed values with every fitted curve; ID labels its points. Another subcommand given twice keeps the last. TEMPLATE is ignored. APPLY runs the previous CURVEFIT's specifications again: the dependents and the independent of the new command when it names some, else the old ones, and any subcommand it gives replaces the old one (SAVE is not carried over); APPLY can also name the model named in quotes (MOD\_1, MOD\_2, ... in the order the session made them). APPLY FIT keeps the estimates of that earlier fit: the tables are the original ones, while the plot and the saved variables put its coefficients to use on the current data, with the original error variance and degrees of freedom for the limits; a model the earlier command did not fit is left out with a warning.

## Example

```text
CURVEFIT VARIABLES=visitors WITH week /MODEL=LINEAR QUADRATIC GROWTH /PRINT=ANOVA /SAVE=PRED CIN.
```

`statscraft help CURVEFIT` prints the same text in the terminal. [All commands](README.md)
