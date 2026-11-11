<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# LOGISTIC REGRESSION

Binary logistic regression with categorical covariates, stepwise selection and case diagnostics.

## Syntax

```text
LOGISTIC REGRESSION [VARIABLES=] dependent [WITH var | var BY var ...]
   [/CATEGORICAL=varlist] [/CONTRAST(var)=INDICATOR[(ref)] | DEVIATION[(ref)] | SIMPLE[(ref)] |
   DIFFERENCE | HELMERT | REPEATED | POLYNOMIAL[(metric)] | SPECIAL(matrix)]
   [/METHOD=ENTER | FSTEP[(COND | LR | WALD)] | BSTEP[(COND | LR | WALD)] [ALL | terms]] ...
   [/ORIGIN | NOORIGIN] [/ID=var] [/PRINT=[DEFAULT] [SUMMARY] [CORR] [ITER[(n)]] [GOODFIT] [CI(95)] [ALL]]
   [/CRITERIA=[BCON(0.001)] [ITERATE(20)] [LCON(0)] [PIN(0.05)] [POUT(0.10)] [EPS(1E-8)] [CUT(0.5)]]
   [/MISSING=EXCLUDE | INCLUDE] [/CASEWISE=[tempvars] [OUTLIER(2)]]
   [/SAVE=tempvar[(name)] ...] [/CLASSPLOT] [/EXTERNAL].
```

tempvar: PRED PGROUP RESID DEV LRESID SRESID ZRESID LEVER COOK DFBETA. The dependent variable has exactly two values among the cases used: the lower is coded 0 and the higher 1 (strings in alphabetical order). A case missing on any variable is left out; one missing only on the dependent still gets PRED and PGROUP. String covariates are categorical; a categorical variable with k values becomes k - 1 coded variables that enter and leave together (INDICATOR, last category as reference, by default). The fit is Newton-Raphson from all coefficients 0 and stops when no coefficient changes by BCON or more, the -2 log likelihood changes by less than LCON percent, or after ITERATE iterations. Each METHOD starts from the model the one before it left: ENTER enters its terms, FSTEP enters by the score test (PIN) and removes by the chosen statistic (POUT), BSTEP enters its terms first and then removes and enters the same way. Tables: Case Processing Summary, Dependent Variable Encoding, Categorical Variables Codings, then for block 0 and each block the Classification Table, Variables in the Equation, Variables not in the Equation, Omnibus Tests of Model Coefficients, Model Summary (-2LL, Cox and Snell and Nagelkerke R square), Model if Term Removed (LR), and with PRINT the Iteration History, Hosmer and Lemeshow Test with its contingency table, the Correlation Matrix and the intervals of Exp(B). SUMMARY shows the last step of each block only. CASEWISE lists every case, or those whose studentized residual is OUTLIER or more away from 0. SAVE adds PRE\_1, PGR\_1, RES\_1, DEV\_1, LRE\_1, SRE\_1, ZRE\_1, LEV\_1, COO\_1 and DFB0\_1, DFB1\_1, ... (one per coefficient), or the names given (DFBETA(name) gives name0, name1, ...).

Weighted when a weight variable is set. CLASSPLOT is accepted with a warning that charts are not drawn yet; EXTERNAL changes nothing. Not supported yet: SELECT, OUTFILE.

## Example

```text
LOGISTIC REGRESSION VARIABLES=bought WITH age income region /CATEGORICAL=region /METHOD=FSTEP(LR) /PRINT=GOODFIT CI(95) /SAVE=PRED.
```

`statscraft help LOGISTIC REGRESSION` prints the same text in the terminal. [All commands](README.md)
