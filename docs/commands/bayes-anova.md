<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES ANOVA

Bayesian one-way analysis of variance: a Bayes factor for the groups and posteriors of their means.

## Syntax

```text
BAYES ANOVA dependent BY factor
   [/INFERENCE ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]
   [/ESTBF COMPUTATION=JZS | ZELLNER(g) | HYPER[(3)] | ROUDER[(1)]]
   [/PRIOR [TYPE=REFERENCE | CONJUGATE] [SHAPEPARAM=a0] [SCALEPARAM=b0]
           [MEANVECTOR=values] [VMATRIX=IDENTITY | row1; row2; ...]]
   [/PLOT [MEANS=values] [ERRORVAR=FALSE | TRUE]] [/DESIGN REGWGT=var]
   [/CRITERIA [CILEVEL=95] [TOL=0.000001] [MAXITER=2000]].
```

Each value of the factor forms a group. BF10 compares the model with one mean per group against one common mean; it depends on N, the number of groups and R square. ZELLNER fixes g of the g-prior; JZS mixes g over an inverse gamma (1/2, N/2); ROUDER(s) uses the scale s in that mixture; HYPER(a), a above 2, uses the hyper-g prior. The posterior table gives each group mean and the error variance. REFERENCE is the flat prior 1/variance. CONJUGATE needs SHAPEPARAM and SCALEPARAM of an inverse gamma on the error variance and MEANVECTOR, one prior mean per group in ascending order of the factor; VMATRIX is the lower triangle of the prior matrix of the means (times the variance), rows ending in ';'; one that is not positive definite becomes the identity with a warning. PLOT draws the prior, the likelihood and the posterior of the listed group means and of the error variance as chart data. REGWGT multiplies the case weights in the sums of squares; cases with a regression weight of 0 or below are left out.

## Example

```text
BAYES ANOVA reaction_ms BY font_size
  /INFERENCE ANALYSIS=BOTH /ESTBF COMPUTATION=ROUDER(0.7) /PLOT MEANS=10 14.
```

`statscraft help BAYES ANOVA` prints the same text in the terminal. [All commands](README.md)
