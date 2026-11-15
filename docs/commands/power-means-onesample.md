<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER MEANS ONESAMPLE

Power or sample size of a one-sample t-test of a mean.

## Syntax

```text
POWER MEANS ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                      N=n | POWER=powers  SD=sd  MEAN=mean  [NULL=0]
                      [/PLOT [N[(low high)]] [ES[(low high)]] [N_XAXIS[(low high)] ES_YAXIS[(low high)]]
                             [N_YAXIS[(low high)] ES_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].
```

powers: one or more values between 0 and 1, and at most one grid low TO high BY step.

Needs no data. With N (a whole number above 1) the table gives the power of the test of MEAN against NULL when the population standard deviation is SD; with POWER it gives, for each power, the smallest N that reaches it and the power that N actually has. The power comes from the noncentral t with N - 1 df and noncentrality (MEAN - NULL) / SD \* sqrt(N); the effect size (MEAN - NULL) / SD shows unsigned for a two-sided test. A one-sided test looks in the direction of the effect. PLOT gives power curves as chart data: N over sample sizes (default from 2 to twice the table's N), ES over effect sizes (default 0 to twice the effect, or from minus to plus that for a one-sided test; within -5 and 5), and a pair of \_XAXIS and \_YAXIS keywords a surface over both. With several powers only the N chart is drawn, over the sizes the table found. VROTATE and HROTATE are accepted; the chart data has no viewing angle.

## Example

```text
POWER MEANS ONESAMPLE /PARAMETERS SD=12 MEAN=104 NULL=100 POWER=0.8 0.9
  /PLOT N.
```

`statscraft help POWER MEANS ONESAMPLE` prints the same text in the terminal. [All commands](README.md)
