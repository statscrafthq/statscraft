<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER PROPORTIONS INDEPENDENT

Power or group sizes of a test comparing two independent proportions.

## Syntax

```text
POWER PROPORTIONS INDEPENDENT /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                              N=n1 n2 | POWER=powers [NRATIO=1]  PROPORTIONS=p1 p2
                              [METHOD=CHISQ | T | LRT | FISHER] [ESTIMATE=NORMAL | BINOMIAL]
                              [CONTINUITY=FALSE | TRUE] [POOLED=TRUE | FALSE] [TIMER=minutes]
                              [/PLOT [NRATIO[(low high)]] [RISK_DIFF] [RISK_RATIO[(low high)]] [ODDS_RATIO[(low high)]]
                                     [N1_XAXIS[(low high)] N2_YAXIS[(low high)]] [N1_YAXIS[(low high)] N2_XAXIS[(low high)]]
                                     [P1_XAXIS P2_YAXIS] [P1_YAXIS P2_XAXIS] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. The table shows the risk difference p1 - p2, the risk ratio p1 / p2 and the odds ratio. METHOD picks the test: CHISQ the z (chi-square) test, with a pooled or separate standard error (POOLED) and optionally a continuity correction; T a t-test of the 0/1 values (pooled or not); LRT the likelihood ratio test; FISHER Fisher's exact test, by the arcsine approximation (it needs at least 0.5 / p successes and failures in the right groups, else the power is missing). With ESTIMATE=BINOMIAL and at most 500 trials in all, the power sums the binomial probabilities of every pair of outcomes the test rejects (for FISHER, the exact conditional test); with POWER and FISHER the size search uses the chi-square test instead, with a warning. With POWER the table gives n1 and n2 = NRATIO \* n1 rounded up (at least 1); the directional chi-square sizes come from a closed formula. A one-sided test looks in the direction of p1 - p2. PLOT: NRATIO with n1 + n2 kept (default 0.2 to 5), RISK\_DIFF, RISK\_RATIO and ODDS\_RATIO by moving p1 with p2 kept, and surfaces over the two group sizes (default 1 to twice each size) or the two proportions.

## Example

```text
POWER PROPORTIONS INDEPENDENT /PARAMETERS PROPORTIONS=0.22 0.35 N=90 110
  METHOD=T POOLED=FALSE /PLOT RISK_RATIO.
```

`statscraft help POWER PROPORTIONS INDEPENDENT` prints the same text in the terminal. [All commands](README.md)
