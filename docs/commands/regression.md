<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# REGRESSION

Linear regression with forced entry or removal and stepwise selection, residuals and diagnostics.

## Syntax

```text
REGRESSION [MATRIX=[IN(file | dataset | *)] [OUT(file | dataset | *)]]
           [/VARIABLES=varlist | (COLLECT)] [/DESCRIPTIVES=[DEFAULTS MEAN STDDEV VARIANCE CORR SIG
           BADCORR COV XPROD N ALL NONE]] [/MISSING=LISTWISE | PAIRWISE | MEANSUBSTITUTION [INCLUDE]]
           [/SELECT=var EQ | NE | LT | LE | GT | GE value] [/REGWGT=var] [/STATISTICS=[DEFAULTS R ANOVA COEFF OUTS ZPP CHANGE CI[(95)] BCOV COLLIN TOL
           F SES XTX SELECTION LABEL ALL]] [/CRITERIA=[DEFAULTS PIN[(0.05)] | FIN[(3.84)] POUT[(0.10)] |
           FOUT[(2.71)] TOLERANCE[(0.0001)] MAXSTEPS(n) CIN[(95)]]] [/ORIGIN | NOORIGIN]
           /DEPENDENT=varlist /[METHOD=]ENTER [varlist] | REMOVE varlist | STEPWISE [varlist] |
           FORWARD [varlist] | BACKWARD [varlist] | TEST(varlist)(varlist)... [/METHOD=...]
           [/RESIDUALS=[DEFAULTS DURBIN OUTLIERS(tempvars) ID(var) HISTOGRAM NORMPROB]]
           [/CASEWISE=[DEFAULTS OUTLIERS(3) | ALL PLOT(tempvar) DEPENDENT tempvars]]
           [/SAVE=tempvar[(name)] ... [FITS]] [/SCATTERPLOT=(y, x) ...] [/PARTIALPLOT=[ALL | varlist]]
           [/OUTFILE=COVB('file' | dataset) | CORB('file' | dataset)].
```

tempvar: PRED RESID ZPRED ZRESID SRESID DRESID SDRESID ADJPRED SEPRED MAHAL COOK LEVER DFBETA SDBETA DFFIT SDFIT COVRATIO MCIN ICIN. One equation per dependent; each METHOD works on the equation the ones before it left, and each step is a model. ENTER takes the variables in order of tolerance; STEPWISE removes the weakest variable while its F-to-remove fails POUT, then enters the strongest that passes PIN; FORWARD only enters, BACKWARD only removes (entering its list first when the equation is empty). A variable enters only when it and the variables in the equation keep the TOLERANCE. Without keywords STATISTICS is R ANOVA COEFF OUTS and DESCRIPTIVES is MEAN STDDEV CORR.

The tables: Variables Entered/Removed, Model Summary (CHANGE, DURBIN, SELECTION add columns), ANOVA, Coefficients (CI, ZPP, TOL, COLLIN, SES, F add columns), Excluded Variables, Coefficient Correlations (BCOV), Collinearity Diagnostics (COLLIN), the swept correlation matrix (XTX), Residuals Statistics (with RESIDUALS, CASEWISE or a plot), Casewise Diagnostics and Outlier Statistics. LISTWISE (default) uses the cases valid on every variable; PAIRWISE builds the correlations pair by pair, with the smallest pair count as N; MEANSUBSTITUTION puts the mean in place of a missing value. SAVE adds variables named PRE\_1, RES\_1 and so on, or as given; DFBETA and SDBETA save one per coefficient (name0, name1, ...), MCIN and ICIN a lower and an upper bound.

Weighted when a weight variable is set; REGWGT gives weighted least squares. Charts come as data: HISTOGRAM(tempvars) a histogram with a normal curve; NORMPROB(tempvars) a normal P-P plot (PRED, RESID and DRESID are plotted standardized); RESIDUALS alone gives both of ZRESID. SCATTERPLOT plots each pair, the first on the vertical axis; a temporary variable takes a star (\*ZRESID).

PARTIALPLOT plots, for each independent (or those listed), the dependent's residuals against the independent's, both regressed on the other independents; it needs two in the equation. TEMPLATE is ignored with a warning. LABEL changes nothing. TEST enters every variable named in its sets together (Model Variables Entered/Removed says Test), then gives a Subset Tests table: for each set the change in R square when it leaves the equation, the change in the regression sum of squares, df, mean square and F (against the residual mean square of the full equation) with its significance, and a Residual row.

SELECT makes the correlation matrix, the equation and its tests from the cases where the condition holds (a missing value on the variable leaves the case out; the variable is not on VARIABLES, DEPENDENT or METHOD, and its value is quoted for a string variable). The other cases still get predictions, residuals, leverage, Mahalanobis and Cook's distance; the residual statistics, the outlier lists and the plots are of the selected and the unselected cases apart (SEPARATE, the default) or of all of them (POOLED), Casewise Diagnostics list both with a Selected Status, and SAVE fills every case.

MATRIX must come first. OUT writes the records MEAN, STDDEV, N (one record, or a record per variable when pairwise counts differ) and CORR (OCORR with ORIGIN) of every variable on VARIABLES, per split group; \* puts them in the active dataset. IN gives the equation from such records, which have no cases: SAVE, RESIDUALS, CASEWISE, the plots, SELECT and REGWGT are errors then.

OUTFILE (one dependent variable, not with MATRIX=OUT) writes a .sav file or a dataset made with DATASET DECLARE for the last equation: COVB the covariance matrix of the coefficients, CORB their correlation matrix, as RowType\_ (COV or CORR), VarName\_ and a column P1, P2, ... for the constant (first, unless ORIGIN) and each independent in the order of the Coefficients table, with a record per coefficient and then the records EST, SE, SIG (the significance of t) and DF (the residual degrees of freedom), named by the dependent variable, per split group.

## Example

```text
GET FILE='survey.sav'.
REGRESSION /DEPENDENT=satisfaction /METHOD=ENTER age spend contacted.
REGRESSION /STATISTICS=COEFF R ANOVA CI(95) /DEPENDENT=spend /METHOD=ENTER age.
REGRESSION /DEPENDENT=satisfaction /METHOD=STEPWISE age spend contacted switching
  /SAVE PRED RESID.
```

The example reads the sample data file [survey.sav](../sample/survey.sav), an invented customer survey; save it in the folder you run the syntax from.

## Example output

### Coefficients

| Model |  | Unstandardized Coefficients / B | Unstandardized Coefficients / Std. Error | Standardized Coefficients / Beta | t | Sig. |
| --- | --- | --- | --- | --- | --- | --- |
| 1 | (Constant) | 4.34 | 0.35 |  | 12.38 | 0.00 |
|  | Age in years | -0.01 | 0.01 | -0.16 | -2.63 | 0.01 |
|  | Monthly spend in euros | -0.02 | 0.01 | -0.18 | -2.90 | 0.00 |
|  | Contacted customer support in the last year | 0.11 | 0.14 | 0.05 | 0.82 | 0.41 |

Dependent variable: Overall satisfaction with the service.

`statscraft help REGRESSION` prints the same text in the terminal. [All commands](README.md)
