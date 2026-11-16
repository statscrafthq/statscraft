<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BAYES CORRELATION

Bayesian inference on the Pearson correlation of every pair of variables.

## Syntax

```text
BAYES CORRELATION /INFERENCE VARIABLES=varlist [ANALYSIS=POSTERIOR | BAYESFACTOR | BOTH]
                             [CVALUE=0] [MAXPLOTS=10]
   [/ESTBF [TYPE=JZS | FRACTION[(0.5)]] [NULLVALUE=0]]
   [/CRITERIA [CILEVEL=95] [SEED=RANDOM | n] [TOL=0.0001] [MAXITER=2000] [MCSAMPLES=10000]
              [POSTSAMPLES=10000]] [/MISSING SCOPE=PAIRWISE | LISTWISE].
```

Every variable is paired with every later one. JZS gives BF01 of a zero correlation against the default prior of the linear model; FRACTION(b) the fractional Bayes factor BF01 of NULLVALUE, with the fraction b of the data training the prior (b times N must exceed 1). A correlation of 1 or -1 gets no Bayes factor. The posterior of rho is proportional to (1 - rho^2)^CVALUE times the approximate likelihood; its mode, mean, variance and highest-density interval are computed by quadrature, so SEED and POSTSAMPLES do not change them. Unless ANALYSIS=BAYESFACTOR, the first MAXPLOTS pairs get a chart of the prior, likelihood and posterior densities of rho as chart data. MCSAMPLES is accepted and not used. Weights as on BAYES ONESAMPLE.

## Example

```text
BAYES CORRELATION /INFERENCE VARIABLES=sleep_hours focus_score mood ANALYSIS=BOTH
  /ESTBF TYPE=FRACTION(0.4).
```

`statscraft help BAYES CORRELATION` prints the same text in the terminal. [All commands](README.md)
