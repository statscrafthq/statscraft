<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# REGRESSION

Linear regression with forced entry or removal and stepwise selection, residuals and diagnostics.

## Syntax

```text
REGRESSION [/VARIABLES=varlist | (COLLECT)] [/DESCRIPTIVES=[DEFAULTS MEAN STDDEV VARIANCE CORR SIG
           BADCORR COV XPROD N ALL NONE]] [/MISSING=LISTWISE | PAIRWISE | MEANSUBSTITUTION [INCLUDE]]
           [/REGWGT=var] [/STATISTICS=[DEFAULTS R ANOVA COEFF OUTS ZPP CHANGE CI[(95)] BCOV COLLIN TOL
           F SES XTX SELECTION LABEL ALL]] [/CRITERIA=[DEFAULTS PIN[(0.05)] | FIN[(3.84)] POUT[(0.10)] |
           FOUT[(2.71)] TOLERANCE[(0.0001)] MAXSTEPS(n) CIN[(95)]]] [/ORIGIN | NOORIGIN]
           /DEPENDENT=varlist /[METHOD=]ENTER [varlist] | REMOVE varlist | STEPWISE [varlist] |
           FORWARD [varlist] | BACKWARD [varlist] [/METHOD=...]
           [/RESIDUALS=[DEFAULTS DURBIN OUTLIERS(tempvars) ID(var) HISTOGRAM NORMPROB]]
           [/CASEWISE=[DEFAULTS OUTLIERS(3) | ALL PLOT(tempvar) DEPENDENT tempvars]]
           [/SAVE=tempvar[(name)] ... [FITS]] [/SCATTERPLOT ...] [/PARTIALPLOT ...].
```

tempvar: PRED RESID ZPRED ZRESID SRESID DRESID SDRESID ADJPRED SEPRED MAHAL COOK LEVER DFBETA SDBETA DFFIT SDFIT COVRATIO MCIN ICIN. One equation per dependent; each METHOD works on the equation the ones before it left, and each step is a model. ENTER takes the variables in order of tolerance; STEPWISE removes the weakest variable while its F-to-remove fails POUT, then enters the strongest that passes PIN; FORWARD only enters, BACKWARD only removes (entering its list first when the equation is empty). A variable enters only when it and the variables in the equation keep the TOLERANCE. Without keywords STATISTICS is R ANOVA COEFF OUTS and DESCRIPTIVES is MEAN STDDEV CORR.

The tables: Variables Entered/Removed, Model Summary (CHANGE, DURBIN, SELECTION add columns), ANOVA, Coefficients (CI, ZPP, TOL, COLLIN, SES, F add columns), Excluded Variables, Coefficient Correlations (BCOV), Collinearity Diagnostics (COLLIN), the swept correlation matrix (XTX), Residuals Statistics (with RESIDUALS, CASEWISE or a plot), Casewise Diagnostics and Outlier Statistics. LISTWISE (default) uses the cases valid on every variable; PAIRWISE builds the correlations pair by pair, with the smallest pair count as N; MEANSUBSTITUTION puts the mean in place of a missing value. SAVE adds variables named PRE\_1, RES\_1 and so on, or as given; DFBETA and SDBETA save one per coefficient (name0, name1, ...), MCIN and ICIN a lower and an upper bound.

Weighted when a weight variable is set; REGWGT gives weighted least squares. HISTOGRAM, NORMPROB, SCATTERPLOT, PARTIALPLOT and TEMPLATE are accepted with a warning that charts are not drawn yet.

LABEL changes nothing. Not supported yet: METHOD=TEST, SELECT, MATRIX, OUTFILE.

## Example

```text
REGRESSION /STATISTICS=COEFF OUTS R ANOVA CHANGE /DEPENDENT=spend /METHOD=ENTER age /METHOD=STEPWISE q1 TO q5 /SAVE=PRED ZRESID.
```

`statscraft help REGRESSION` prints the same text in the terminal. [All commands](README.md)
