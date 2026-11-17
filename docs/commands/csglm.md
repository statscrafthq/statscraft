<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSGLM

Linear regression, ANOVA and ANCOVA on a complex sample, with design-based standard errors and tests.

## Syntax

```text
CSGLM dependent [BY factors] [WITH covariates] /PLAN FILE='file'
   [/MODEL effects] [/INTERCEPT [INCLUDE=YES | NO | ONLY] [SHOW=YES | NO]]
   [/CUSTOM [LABEL='text'] [LMATRIX=[c] effect values ... | ALL values; ...] [KMATRIX=k; ...]] ...
   [/EMMEANS [TABLES=factor[*factor ...]] [OTHER=[covariate(value | MEAN) ...]]
            [COMPARE=factor] [CONTRAST=SIMPLE[(value)] | DEVIATION[(value)] | DIFFERENCE |
                                    HELMERT | REPEATED | POLYNOMIAL[(metric)]]] ...
   [/CRITERIA [CILEVEL=95] [DF=n] [SINGULAR=1E-12]]
   [/STATISTICS [PARAMETER] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]
   [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]
          [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]
   [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
   [/PRINT [SAMPLEINFO] [VARIABLEINFO] [SUMMARY] [GEF] [LMATRIX] [COVB] [CORB] [NONE]]
   [/SAVE [PRED[(name)]] [RESID[(name)]]]
   [/OUTFILE [COVB='file' | CORB='file'] [MODEL='file' | PARAMETER='file']].
```

The plan written by CSPLAN gives the weight and the design; WEIGHT and SPLIT FILE do not apply. The coefficients solve the weighted normal equations, with one parameter per level of each factor effect; a parameter that depends on earlier ones is redundant and set to 0. The covariance of the coefficients is a sandwich: the inverse of X'WX on both sides of the design covariance of the total of x times the residual, found as in CSDESCRIPTIVES. Every df is the number of first-stage clusters minus the number of first-stage strata, unless CRITERIA DF sets it. Without MODEL the model has the intercept, the factors and the covariates. Tests of Model Effects use Type III hypotheses: TYPE=F gives the Wald F, (df - r + 1) / (r df) times the Wald chi-square, on r and df - r + 1 df; ADJF divides the chi-square of the simple random sampling covariance by the sum of the generalized design effects, on d and d times df (d as in CSTABULATE); CHISQUARE and ADJCHISQUARE give the two chi-squares. CUSTOM tests L b = K row by row and as a whole; EMMEANS gives the means of the factor cells averaged with equal weights, covariates at their weighted means unless OTHER sets them, and COMPARE tests the contrasts of one factor. PADJUST adjusts the p values of the single rows. DOMAIN analyzes the cases with the value, keeping the whole design; cases with a missing value on a model variable count as outside it. R Square compares the weighted residual sum of squares with the weighted total sum of squares. Without STATISTICS the estimates come with SE, interval and t test. OUTFILE COVB writes, to a data file or a dataset made by DATASET DECLARE, the variables RowType\_ and VarName\_ and P1, P2, ... (one per row of Parameter Estimates, labelled with its name): rows EST, SE, SIG (of the t test) and DF, then a COV row per parameter; CORB puts CORR rows of correlations there instead. MODEL writes the estimates and their covariance matrix to a JSON file of Statscraft's own, PARAMETER the estimates alone. JOINTPROB works as in CSDESCRIPTIVES. PRINT GEF writes every estimable function as a combination of free coefficients: a row per parameter and a column per free coefficient L1, L2, ... (one per parameter that is not redundant); a redundant parameter's row says how its coefficient follows from the free ones. Any L that gives every parameter its row's combination of the free coefficients is estimable, and every estimable L has that form.

## Example

```text
CSGLM yield BY soil WITH rainfall /PLAN FILE='farms_plan.json' /STATISTICS PARAMETER SE CINTERVAL DEFF /EMMEANS TABLES=soil COMPARE=soil CONTRAST=REPEATED.
```

`statscraft help CSGLM` prints the same text in the terminal. [All commands](README.md)
