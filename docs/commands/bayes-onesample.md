<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES ONESAMPLE

Bayesian inference on one mean, proportion or Poisson rate: Bayes factors and posteriors.

## Syntax

```text
BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL | BINOMIAL | POISSON VARIABLES=varlist
                           [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]
   [/DATA VARIABLES=sublist [VARIANCE=v] [NULLVALUE=0] [ALTERGVALUE=g]          (NORMAL)
         [SUCCESS=LAST | FIRST | MIDPOINT | CUTPOINT(v) | LEVEL(values)]    (BINOMIAL)
         [NULLPRIORPARAM=2 2] [ALTERPRIORPARAM=a b]] ...         (BINOMIAL, POISSON)
   [/PRIOR [VARDIST=DIFFUSE | INVCHISQ(df scale) | INVGAMMA(shape scale) | JEFFREYS2 | JEFFREYS4]
           [PRECDIST=GAMMA(shape rate) | CHISQ(df)] [MEANDIST=DIFFUSE | NORMAL(mean [variance])]
           [KAPPA=1] [TYPE=CONJUGATE(a b)]]
   [/CRITERIA [CILEVEL=95] [METHOD=AGL | MCA] [SEED=RANDOM | n] [TOL=0.000001] [MAXITER=2000]
              [MCSAMPLES=10000]] [/MISSING SCOPE=ANALYSIS | LISTWISE].
```

Each variable is analysed alone. A DATA subcommand sets its keywords for the variables of its sublist; the others keep the defaults. NORMAL: the Bayes factor BF01 weighs a mean of NULLVALUE against a normal spread of means around it. With a known VARIANCE the spread is g times that variance (g = ALTERGVALUE, default 1). Without it, a given g fixes the spread relative to the unknown variance, and no g puts a Cauchy prior on the effect size (the JZS factor), integrated by adaptive quadrature or, with METHOD=MCA, by MCSAMPLES draws. The posterior of the mean: from the variance prior with a flat mean prior (DIFFUSE, JEFFREYS2, JEFFREYS4), or a conjugate normal mean prior centered on the first value of MEANDIST=NORMAL with KAPPA prior observations (INVCHISQ, INVGAMMA, PRECDIST); a conjugate prior with MEANDIST=DIFFUSE falls back to VARDIST=DIFFUSE. With a known variance the mean prior is flat or NORMAL(mean variance). BINOMIAL: SUCCESS picks the values that count as a success (LAST and FIRST: the highest and lowest value present; MIDPOINT: from halfway between the smallest and largest value up; CUTPOINT: from the value up). BF01 compares a beta prior NULLPRIORPARAM with ALTERPRIORPARAM, and the posterior comes from the beta prior TYPE=CONJUGATE(a b) (both above 0, or 0 0). POISSON: the same with gamma priors of shape and rate; the values must be counts.

ALTERPRIORPARAM is needed for a Bayes factor and TYPE for a posterior of these two.

Credible intervals have equal tails. Case weights are rounded to whole numbers; a weight below 0.5 drops the case.

## Example

```text
BAYES ONESAMPLE /INFERENCE DISTRIBUTION=NORMAL VARIABLES=commute ANALYSIS=BOTH
  /DATA VARIABLES=commute NULLVALUE=30
  /PRIOR VARDIST=INVGAMMA(3 200) MEANDIST=NORMAL(28) KAPPA=4.
```

`statscraft help BAYES ONESAMPLE` prints the same text in the terminal. [All commands](README.md)
