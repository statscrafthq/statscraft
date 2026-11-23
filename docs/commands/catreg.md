<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CATREG

Categorical regression: quantify a response and its predictors by optimal scaling (spline, ordinal, nominal or numeric) so that a linear model fits best, with ridge, lasso and elastic net penalties.

## Syntax

```text
CATREG VARIABLES=varlist
   /ANALYSIS=response [([LEVEL=SPORD | SPNOM | ORDI | NOMI | NUME] [DEGREE=2 | n] [INKNOT=2 | n])]
             WITH varlist [(...)] [varlist [(...)] ...]
   [/DISCRETIZATION=[varlist [([GROUPING] [NCAT=7 | n] [DISTR=NORMAL | UNIFORM] | [EQINTV=width]
                               | RANKING | MULTIPLYING)] ...]]
   [/MISSING=[varlist | ALL [(LISTWISE | MODEIMPU | EXTRACAT)] ...]]
   [/SUPPLEMENTARY=OBJECT(case numbers)]
   [/INITIAL=NUMERICAL | RANDOM | MULTISTART[(50 | threshold | ALL)]('file' | dataset)
             | FIXSIGNS(row)('file' | dataset)]
   [/MAXITER=100 | n] [/CRITITER=.00001 | value]
   [/REGULARIZATION=NONE | RIDGE[(start, stop, step)][('file')] | LASSO[(start, stop, step)][('file')]
                    | ENET[(start, stop, step)(start, stop, step)][('file')]]
   [/RESAMPLE=NONE | CROSSVAL[(10 | folds)] | BOOTSTRAP[(50 | samples)]]
   [/PRINT=[R] [COEFF] [DESCRIP[(varlist)]] [HISTORY] [ANOVA] [CORR] [OCORR] [QUANT[(varlist)]]
          [REGU] [NONE]]
   [/PLOT=[TRANS(varlist)[(len)]] [RESID(varlist)[(len)]] [REGU[(values | ALL)]]]
   [/SAVE=[TRDATA[(root)]] [PRED[(root)]] [RES[(root)]]]
   [/OUTFILE=[TRDATA('file' | dataset)] [DISCRDATA('file' | dataset)]].
```

The command gives each category a value (its quantification) and each predictor a coefficient so that the quantified response is predicted with the least weighted squared error; every quantified variable has mean 0 and variance 1. VARIABLES comes first and ANALYSIS is required: one response, WITH, then the predictors. Variables are coded as for CATPCA: whole codes from 1, lower codes and missing values missing, strings coded in sorted order, fractions cut into 7 groups unless DISCRETIZATION says otherwise. SPLIT FILE does not apply.

LEVEL says what the values must respect. NUME keeps equal steps (all numeric gives plain regression on standardized variables), ORDI the order of the codes and NOMI only the grouping. SPORD (the default) and SPNOM fit a spline of DEGREE (2) joined at INKNOT (2) interior knots; SPORD keeps it rising. MISSING drops cases with a gap (LISTWISE, the default) or fills the gap first with the most frequent code (MODEIMPU) or with a new code above all others (EXTRACAT, free of the level's restriction).

Iterations stop when the error drops less than CRITITER, or after MAXITER. INITIAL=NUMERICAL (the default) first fits every variable as numeric (iteration 0 of the history); RANDOM starts at once from the standardized codes. MULTISTART tries sign patterns for the coefficients of ordinal and spline ordinal predictors: ALL of them, or those a stepwise search allows with the threshold percent (50 by default). It keeps the best and writes each pattern with its R square to the file. FIXSIGNS takes the signs from a row of such a file.

REGULARIZATION shrinks the coefficients: RIDGE divides them by 1 plus the penalty, LASSO moves them half the penalty towards 0 (stopping at 0), and ENET does both. Each penalty from start to stop by step gives a model; the default range is (0, 1, 0.02), and (0, 1, 0.1) for the ENET ridge part. With several models and neither RESAMPLE nor supplementary cases, only the REGU table and chart are shown; otherwise the model with the least prediction error is reported. RESAMPLE estimates the prediction error by K-fold cross validation (10 folds) or by the .632 bootstrap (50 samples). Supplementary cases give a test error. They get the quantifications of the active cases; a code only they have is interpolated for numeric and spline levels (and extrapolated for numeric ones), otherwise the case gets no prediction.

The Case Processing Summary is always shown. PRINT adds: DESCRIP frequencies, missing count and mode; HISTORY R square and apparent prediction error by iteration; R the Model Summary (multiple R, R square, two adjusted R squares and the prediction error); ANOVA two tables, by predictor count and by optimal scaling degrees of freedom; COEFF the coefficients with bootstrap standard errors, F tests, correlations, Pratt's importance and tolerance; CORR and OCORR the correlations of the quantified and of the original predictors with eigenvalues; QUANT the quantification of each category; REGU per model the penalties, R square, prediction error and coefficients, also written to the REGULARIZATION file. Without PRINT, or with PRINT alone: R COEFF DESCRIP ANOVA.

Charts (none by default): TRANS the quantification by code; RESID per predictor its coefficient times quantification and the partial residuals, by code; REGU the coefficient paths against the standardized sum of coefficients (one chart per listed ridge penalty for ENET). A label length of 0 shows codes instead of labels; above 60 it is 60.

SAVE adds the quantified variables rootK\_S (root TRA, K the place on ANALYSIS with the response first, S the first free set number), the predictions root\_S (PRE) and the residuals root\_S (RES); excluded cases are system-missing. OUTFILE writes the quantified variables TRAK (TRDATA, also a file with no keyword) and the coded variables under their names (DISCRDATA).

## Example

```text
CATREG VARIABLES=rating taste price area /ANALYSIS=rating(LEVEL=ORDI) WITH taste price(LEVEL=NUME) area(LEVEL=NOMI) /MISSING=price(MODEIMPU) /PRINT=R COEFF ANOVA QUANT(area) /PLOT=TRANS(area) /SAVE=PRED RES.
```

`statscraft help CATREG` prints the same text in the terminal. [All commands](README.md)
