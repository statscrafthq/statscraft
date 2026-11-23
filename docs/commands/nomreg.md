<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NOMREG

Multinomial logistic regression: a nominal dependent variable on factors and covariates.

## Syntax

```text
NOMREG dependent [(BASE=FIRST | LAST | value  ORDER=ASCENDING | DATA | DESCENDING)]
   [BY factors] [WITH covariates]
   [/MODEL=[effects] [| BACKWARD | FORWARD | BSTEP | FSTEP = effects] | /FULLFACTORIAL]
   [/STEPWISE=[RULE(SINGLE | SFACTOR | CONTAINMENT | NONE)] [MINEFFECT(0)] [MAXEFFECT(n)]
              [PIN(0.05)] [POUT(0.1)] [ENTRYMETHOD(LR | SCORE)] [REMOVALMETHOD(LR | WALD)]]
   [/INTERCEPT=INCLUDE | EXCLUDE]
   [/CRITERIA=[CIN(95)] [DELTA(0)] [MXITER(100)] [MXSTEP(5)] [LCONVERGE(0)] [PCONVERGE(1E-6)]
              [SINGULAR(1E-8)] [BIAS(0)] [CHKSEP(20)]]
   [/PRINT=[CPS] [MFI] [PARAMETER] [LRT] [FIT] [SUMMARY] [CLASSTABLE] [IC] [KERNEL] [COVB] [CORB]
          [STEP] [HISTORY[(n)]] [ASSOCIATION] [CELLPROB] [NONE]]
   [/SAVE=[ESTPROB[(root[:n])]] [PREDCAT[(name)]] [PCPROB[(name)]] [ACPROB[(name)]]]
   [/SCALE=1 | n | DEVIANCE | PEARSON] [/SUBPOP=varlist] [/MISSING=EXCLUDE | INCLUDE]
   [/TEST[(constants)]=['label'] effect values ... | ALL values [; ...]] [/TEST ...].
```

The command fits a multinomial logistic regression: one logit for each category of a nominal dependent against a reference category. The dependent and the factors may be numeric or string; covariates are numeric. ORDER sorts the categories (ASCENDING by default; DATA keeps the order of first occurrence). BASE names the reference category (LAST by default), which has no parameters.

Without MODEL the model is the intercept, the covariates and the main effects of the factors. FULLFACTORIAL adds every interaction of the factors and cannot be given with MODEL. An effect is a factor or covariate, a\*b (a BY b), a(b) (a WITHIN b) or x\*x. INTERCEPT=EXCLUDE drops the intercept. A factor gets one parameter per level, an interaction one per level combination that occurs. A parameter that repeats others (by SINGULAR) is redundant and set to 0, so a factor's last level is its reference.

A case missing on any variable, or with a weight of 0 or less, is left out; weights are not rounded. MISSING=INCLUDE treats user-missing values as valid. Cases form subpopulations by the factors and covariates, or by the SUBPOP variables, which must include every model variable. DELTA (0 to below 1) is added to a subpopulation's empty cells and BIAS (same range) to every cell. The fit is Newton-Raphson from the intercept-only fit, with up to MXSTEP step halvings. It stops when the log likelihood changes by less than LCONVERGE, no parameter changes by PCONVERGE or more, or after MXITER iterations.

Without PRINT the output is CPS, MFI and STEP; a PRINT list replaces them, and NONE turns all off. MFI tests against the intercept-only model (without the intercept, the model of equal probabilities); IC adds AIC and BIC. SUMMARY gives the Cox and Snell, Nagelkerke and McFadden pseudo R-squares, FIT the Pearson and deviance chi-squares, LRT a likelihood ratio test of each effect, PARAMETER the estimates with Wald tests and the CIN interval of Exp(B), CLASSTABLE the classification table. The -2 log likelihoods include the multinomial constant; KERNEL leaves it out. SCALE multiplies the covariance matrix by the value, or by the Pearson or deviance chi-square over its df.

SAVE writes, for every case with valid factors and covariates, the probability of each category (ESTPROB: EST1\_1, EST2\_1 and so on, or root1, root2 and so on; the first n, 25 by default), the predicted category (PREDCAT: PRE\_1), its probability (PCPROB: PCP\_1) and the probability of the observed category (ACPROB: ACP\_1).

Stepwise: the intercept and the effects before \| always stay in; the method chooses among those after it. FORWARD adds, BACKWARD starts with all and drops, FSTEP adds one and then drops while it can, BSTEP starts with all, drops while it can, then adds one. The effect with the smallest p below PIN enters (likelihood ratio, or score with ENTRYMETHOD(SCORE)); the one with the largest p above POUT leaves (likelihood ratio, or Wald with REMOVALMETHOD(WALD)). A tie goes to the effect listed first. Stepping stops when nothing qualifies or a model would repeat. MAXEFFECT caps adding under FORWARD and FSTEP, MINEFFECT dropping under BACKWARD and BSTEP, not counting the intercept.

RULE(SINGLE), the default, holds an effect back until the listed effects made of part of its variables are in; SFACTOR does that only for effects of factors alone; CONTAINMENT waits for the effects it contains (same covariates, fewer factors); NONE never waits. An effect that another in the model needs is not dropped. The Step Summary lists each step with its -2 log likelihood and deciding test; the other tables describe the final model, which without the intercept can be the model of equal probabilities.

HISTORY(n) shows the -2 log likelihood and parameters of the final model at iteration 0, every n-th iteration and the last. ASSOCIATION, for a two-category response, gives Somers' D, gamma, tau-a and C over the pairs of cases with different observed categories, compared by the predicted probability of the non-base category (rounded down to a multiple of 1/500). CELLPROB gives observed and predicted counts and proportions, with raw and standardized residuals, per subpopulation and category.

TEST tests linear combinations of the parameters. A row gives an effect and one value per parameter of it (redundant ones included, one for INTERCEPT), or ALL and a value per parameter; the two forms do not mix in one TEST. Semicolons separate rows. Constants default to 0, and a value may be a fraction. It gives a Wald chi-square per logit (df the rank of the rows) and one for all logits. TEST may be repeated.

## Example

```text
NOMREG commute (BASE=FIRST) BY district WITH distance /PRINT=PARAMETER LRT FIT SUMMARY CLASSTABLE /SAVE=ESTPROB PREDCAT.
```

`statscraft help NOMREG` prints the same text in the terminal. [All commands](README.md)
