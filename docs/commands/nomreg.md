<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NOMREG

Multinomial logistic regression: a nominal dependent variable on factors and covariates.

## Syntax

```text
NOMREG dependent [(BASE=FIRST | LAST | value  ORDER=ASCENDING | DATA | DESCENDING)]
   [BY factors] [WITH covariates]
   [/MODEL=effects | /FULLFACTORIAL] [/INTERCEPT=INCLUDE | EXCLUDE]
   [/CRITERIA=[CIN(95)] [DELTA(0)] [MXITER(100)] [MXSTEP(5)] [LCONVERGE(0)] [PCONVERGE(1E-6)]
              [SINGULAR(1E-8)] [BIAS(0)] [CHKSEP(20)]]
   [/PRINT=[CPS] [MFI] [PARAMETER] [LRT] [FIT] [SUMMARY] [CLASSTABLE] [IC] [KERNEL] [COVB] [CORB]
          [STEP] [NONE]]
   [/SAVE=[ESTPROB[(root[:n])]] [PREDCAT[(name)]] [PCPROB[(name)]] [ACPROB[(name)]]]
   [/SCALE=1 | n | DEVIANCE | PEARSON] [/SUBPOP=varlist] [/MISSING=EXCLUDE | INCLUDE].
```

effects: a factor or covariate, a\*b or a BY b, a(b) or a WITHIN b, x\*x. The dependent and the factors may be numeric or string; covariates are numeric. The categories of the dependent are sorted by ORDER (DATA keeps the order in which they first occur); one of them, the last unless BASE says otherwise, is the reference and has no parameters. Without MODEL the model is the intercept, the covariates and the main effects of the factors; FULLFACTORIAL adds every interaction of the factors. A factor gets one parameter per level and an interaction one per level combination that occurs; a parameter that repeats others (SINGULAR) is redundant and set to 0, so the last level of a factor is the reference. A case missing on any variable is left out, as is one with a weight of 0 or less; weights are used as they are. The cases are grouped into subpopulations by the factors and covariates, or by the SUBPOP variables, which must include every variable of the model. The fit is Newton-Raphson with step-halving (MXSTEP), starting from the intercept-only fit; it stops when the log likelihood changes by less than LCONVERGE, no parameter changes by PCONVERGE or more, or after MXITER iterations. DELTA is added to empty cells of a subpopulation and BIAS to every cell. Tables: Case Processing Summary (CPS), Model Fitting Information against the intercept-only model, or the model of equal probabilities without intercept (MFI, with AIC and BIC under IC), Pseudo R-Square with Cox and Snell, Nagelkerke and McFadden (SUMMARY), Goodness-of-Fit with Pearson and deviance chi-square (FIT), Likelihood Ratio Tests of each effect against the model without it (LRT), Parameter Estimates with Wald tests and the CIN interval of Exp(B) (PARAMETER), the Classification table (CLASSTABLE) and the covariance or correlation matrix of the estimates (COVB, CORB). Without PRINT: CPS and MFI. -2 log likelihoods include the multinomial constant of the subpopulations; KERNEL leaves it out. SCALE multiplies the covariance matrix by the value, or by the Pearson or deviance chi-square over its df. SAVE adds the probability of each response category (EST1\_1, EST2\_1, ... or root1, root2, ...; the first n, 25 by default), the predicted category (PRE\_1), its probability (PCP\_1) and the probability of the observed category (ACP\_1), for every case with valid factors and covariates. Not supported yet: stepwise methods (MODEL with \|, STEPWISE), TEST, OUTFILE and PRINT CELLPROB, HISTORY and ASSOCIATION.

## Example

```text
NOMREG commute (BASE=FIRST) BY district WITH distance /PRINT=PARAMETER LRT FIT SUMMARY CLASSTABLE /SAVE=ESTPROB PREDCAT.
```

`statscraft help NOMREG` prints the same text in the terminal. [All commands](README.md)
