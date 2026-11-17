<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# QUANTILE REGRESSION

Linear regression of a chosen quantile of the dependent variable, fitted by linear programming.

## Syntax

```text
QUANTILE REGRESSION dependent [BY factors] [WITH covariates]
   [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
   [/CRITERIA [QUANTILE=values [v1 TO v2 BY step]] [IID=TRUE | FALSE]
              [BANDWIDTH=BOFINGER | HALL_SHEATHER] [CILEVEL=95]
              [METHOD=AUTO | SIMPLEX | INTERIOR_POINT] [TOL=1E-12] [CONV=1E-6] [MAXITER=2000]]
   [/MODEL [effects] [REGWGT=var] [INTERCEPT=TRUE | FALSE]]
   [/SAVE [PRED[(name)]] [RESID[(name)]] [CIPREDL[(name)]] [CIPREDU[(name)]]]
   [/PREDICT_EFFECTS NUM_TOP_EFFECTS=3] [/PREDICTION_TABLES effects] [/PREDICTION_LINES effects]
   [/PRINT [PARAMETER] [COVB] [CORB]]
   [/PLOT [MAX_CATEGORIES=10] [PREDICTED_BY_OBSERVED=FALSE | TRUE] [PARAM_EST=50 | ALL]]
   [/OUTFILE [COVB=file] [CORB=file] [FILE_SEPARATE=FALSE | TRUE] [MODEL=file | PARAMETER=file]]
   [/EXTERNAL].
```

For each quantile p (0.5 unless QUANTILE lists others; a grid v1 TO v2 BY step may join them, a repeated value is fitted once) the coefficients make the weighted sum of p times each positive residual and 1-p times the size of each negative one as small as it can be.

The weight is the case weight, rounded, times REGWGT; a case with a weight below 0.5 or a REGWGT of 0 or less is left out, as is a case missing on any variable (CLASSMISSING=INCLUDE keeps user-missing factor values). MODEL lists effects as in UNIANOVA; without them the model has the intercept and every covariate and factor as a main effect. A parameter that depends on earlier ones is redundant and 0. SIMPLEX walks from vertex to vertex of the linear program to the exact solution; INTERIOR\_POINT runs Frisch-Newton steps (TOL for its matrices, CONV for the duality gap, MAXITER steps) and finishes with the simplex from there; AUTO takes the simplex for up to 1000 cases. With IID=TRUE the standard errors come from the spread of the residuals around p over a bandwidth (BOFINGER or HALL\_SHEATHER); with FALSE the model is refitted at p plus and minus the bandwidth and each case gets its own density.

t tests and CILEVEL intervals use N minus the number of estimated parameters as df.

Tables: Case Processing Summary, Model Quality (pseudo R squared against the intercept-only fit, mean absolute error), Parameter Estimates (PARAMETER, shown unless PRINT leaves it out), Parameter Covariance (COVB) and Parameter Correlation (CORB). Predicted values over the cells of effects with one or two factors are tables, along one covariate (alone or with factors) line charts, other covariates at their means and other factors averaged: for the effects of PREDICTION\_TABLES and PREDICTION\_LINES, or else the first NUM\_TOP\_EFFECTS such effects of the model. PLOT PREDICTED\_BY\_OBSERVED adds a scatter of each case per quantile, PARAM\_EST a chart of each of the first n estimated parameters across the quantiles (with two or more).

SAVE adds the predicted value, residual and interval bounds (named PRED, RESID, CIPREDL and CIPREDU unless a name is given; with several quantiles the name gets \_ and the quantile's digits, as in PRED\_25). OUTFILE COVB or CORB writes RowType\_, VarName\_ and P1, P2, ... with rows EST, SE, SIG, DF and then COV or CORR to a data file or declared dataset (a Quantile column joins several quantiles; FILE\_SEPARATE=TRUE writes one file each, the quantile's digits added to the name). MODEL or PARAMETER writes the estimates (MODEL with their covariances) to a JSON file of Statscraft's own, one per quantile. EXTERNAL changes nothing.

## Example

```text
QUANTILE REGRESSION rent BY district WITH floor_area /CRITERIA QUANTILE=0.25 0.5 0.75 IID=FALSE /PRINT PARAMETER COVB /SAVE PRED(fit) RESID.
```

`statscraft help QUANTILE REGRESSION` prints the same text in the terminal. [All commands](README.md)
