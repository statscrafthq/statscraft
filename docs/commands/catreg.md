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

Variables are coded as for CATPCA: whole codes from 1, lower codes and missing values missing, strings coded in sorted order, fractions cut into 7 groups unless DISCRETIZATION says otherwise. MISSING drops cases with a gap (LISTWISE, the default) or fills the gap first with the most frequent code (MODEIMPU) or with a new code above all others (EXTRACAT, kept out of the level's restriction). Each variable gets a value for each category, and each predictor a coefficient, so that the weighted squared error of the quantified response is least, every quantified variable having mean 0 and variance 1. LEVEL says what the values must respect: NUME equal steps (all numeric is plain regression on standardized variables), ORDI the order of the codes, NOMI nothing but the grouping, SPORD (the default) and SPNOM a spline of DEGREE joined at INKNOT interior knots (SPORD rising). The fit updates the response and then each predictor in turn until the error drops less than CRITITER, at most MAXITER times.

INITIAL=NUMERICAL first fits every variable as numeric (iteration 0 of the history); RANDOM starts the levels at once from the standardized codes with the correlations as coefficients.

MULTISTART tries sign patterns for the coefficients of the ordinal and spline ordinal predictors from a nominal start, all of them (ALL) or those a step-by-step search allows (a predictor may turn negative only when its order restriction costs more than the threshold percent of its variance), keeps the best and writes each pattern with its R square to the file; FIXSIGNS takes the signs from a row of such a file. REGULARIZATION shrinks the coefficients: RIDGE divides by 1 plus the penalty, LASSO moves them half the penalty towards 0 (to 0 when they would cross), ENET does both; each value from start to stop by step gives a model. With more than one model and neither RESAMPLE nor supplementary cases only the REGU table and chart are shown; with them the model with the least prediction error is reported.

RESAMPLE estimates the prediction error by K-fold cross validation or by the .632 bootstrap; supplementary cases give a test error. Supplementary cases get the quantifications of the active ones; a code only they have is interpolated between neighbouring codes for numeric and spline levels and extrapolated along the line for numeric ones, otherwise the case gets no prediction. SPLIT FILE does not apply.

Tables: Case Processing Summary always; DESCRIP frequencies, missing count and mode; HISTORY R square and apparent prediction error by iteration; R the Model Summary (multiple R, R square, adjusted R square with the predictor count and with the optimal scaling degrees of freedom, the apparent and, with resampling or test cases, the expected prediction error); ANOVA two tables, one with the predictor count and one with the optimal scaling degrees of freedom; COEFF the coefficients with a standard error from 1000 bootstrap samples, degrees of freedom, F and significance, and the zero-order, partial and part correlations, Pratt's importance and the tolerance after and before the quantification; CORR and OCORR the correlations of the quantified and of the original predictors with eigenvalues; QUANT the quantification of each category; REGU per model the penalties, R square, the prediction error when estimated, and the coefficients, also written to the REGULARIZATION file.

Without PRINT, or with PRINT alone: R COEFF DESCRIP ANOVA.

Charts (none by default): TRANS the quantification by code; RESID per predictor its coefficient times quantification and the residuals of the other predictors by code; REGU the coefficient paths against the standardized sum of coefficients (one chart per listed ridge penalty for ENET). A label length of 0 shows codes instead of labels; above 60 it is 60.

SAVE adds the quantified variables rootK\_S (TRA, K the place on ANALYSIS with the response first, S the first free set number), the predictions root\_S (PRE) and the residuals root\_S (RES); excluded cases are system-missing. OUTFILE writes the quantified variables TRAK (TRDATA, also a file with no keyword) and the coded variables under their names (DISCRDATA).

## Example

```text
CATREG VARIABLES=rating taste price area /ANALYSIS=rating(LEVEL=ORDI) WITH taste price(LEVEL=NUME) area(LEVEL=NOMI) /MISSING=price(MODEIMPU) /PRINT=R COEFF ANOVA QUANT(area) /PLOT=TRANS(area) /SAVE=PRED RES.
```

`statscraft help CATREG` prints the same text in the terminal. [All commands](README.md)
