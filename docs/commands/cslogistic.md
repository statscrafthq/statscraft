<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CSLOGISTIC

Binary and multinomial logistic regression on a complex sample, with design-based standard errors and tests.

## Syntax

```text
CSLOGISTIC dependent [(LOW | HIGH | value)] [BY factors] [WITH covariates] /PLAN FILE='file'
   [/MODEL effects] [/INTERCEPT [INCLUDE=YES | NO | ONLY] [SHOW=YES | NO]]
   [/CUSTOM [LABEL='text'] [LMATRIX=[c] effect values ... | ALL values; ...] [KMATRIX=k; ...]] ...
   [/ODDSRATIOS FACTOR=[factor[(LOW | HIGH | value)] ...] | COVARIATE=[covariate[(changes)] ...]
                [CONTROL=[factor(LOW | HIGH | value) covariate(MEAN | value) ...]]] ...
   [/CRITERIA [CHKSEP=20] [CILEVEL=95] [DF=n] [LCONVERGE=[0 RELATIVE | ABSOLUTE]] [MXITER=100]
              [MXSTEP=5] [PCONVERGE=[1E-6 RELATIVE | ABSOLUTE]] [SINGULAR=1E-12]]
   [/STATISTICS [PARAMETER] [EXP] [SE] [TTEST] [CINTERVAL] [DEFF] [DEFFSQRT]]
   [/TEST [TYPE=F | ADJF | CHISQUARE | ADJCHISQUARE]
          [PADJUST=LSD | BONFERRONI | SEQBONFERRONI | SIDAK | SEQSIDAK]]
   [/DOMAIN VARIABLE=var(value)] [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
   [/PRINT [SAMPLEINFO] [VARIABLEINFO] [SUMMARY] [HISTORY[(n)]] [LMATRIX] [COVB] [CORB]
           [CLASSTABLE] [NONE]]
   [/SAVE [PREDPROB[(root[:25])]] [PREDVAL[(name)]]].
```

Each category of the dependent variable but the reference one (the highest unless the parentheses say otherwise) gets a logit against the reference with its own set of parameters. The estimates maximize the weighted log likelihood (pseudo-likelihood) by Newton-Raphson with step-halving; they start at 0 with the intercepts at the log ratio of the weighted category counts, and stop when the parameters (PCONVERGE) or the log likelihood (LCONVERGE) change little or after MXITER steps. The covariance is a sandwich: the inverse information on both sides of the design covariance of the score total. Design, df, domains, CUSTOM, TEST and PADJUST work as in CSGLM; a CUSTOM row without ALL is tested for every logit, a row with ALL lists the parameters of all logits in turn. The model summary gives the pseudo R squares of Cox and Snell, Nagelkerke and McFadden from the weighted log likelihoods of the model and of the intercept-only start, with the summed weights as N. ODDSRATIOS compares each level of a factor with its reference, or a covariate moved by each change, with the other variables at CONTROL (factors at their highest level, covariates at their weighted means by default). CLASSTABLE puts each case in its most probable category and sums the weights. Without STATISTICS no parameter table is shown.

Not supported yet: JOINTPROB, OUTFILE, PRINT GEF and the UNEQUAL\_WOR estimator.

## Example

```text
CSLOGISTIC irrigated (LOW) BY soil WITH rainfall /PLAN FILE='farms_plan.json' /STATISTICS PARAMETER EXP SE CINTERVAL /ODDSRATIOS FACTOR=[soil(LOW)] /PRINT SUMMARY CLASSTABLE.
```

`statscraft help CSLOGISTIC` prints the same text in the terminal. [All commands](README.md)
