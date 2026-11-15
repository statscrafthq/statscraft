<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER MEANS INDEPENDENT

Power or group sizes of a t-test comparing the means of two independent groups.

## Syntax

```text
POWER MEANS INDEPENDENT /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                        N=n1 n2 | POWER=powers [NRATIO=1]  SD=sd | sd1 sd2  MEAN=difference | mean1 mean2
                        [/PLOT [NRATIO[(low high)]] [ES[(low high)]] [NRATIO_XAXIS[(low high)] ES_YAXIS[(low high)]]
                               [NRATIO_YAXIS[(low high)] ES_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. One SD assumes equal variances: df n1 + n2 - 2 and effect size difference / SD. Two different SDs give the separate-variances test with Welch's df, and the table shows the mean difference instead of an effect size. Two MEAN values are the group means, one is their difference. With POWER the table gives n1 (the smallest that reaches the power) and n2 = NRATIO \* n1 rounded up (at least 2), with the power they actually have. NRATIO takes 0.01 to 100. PLOT: NRATIO over n2 / n1 with n1 + n2 kept (default 0.2 to 5), ES over the effect size, or the mean difference with two SDs (default 0 to twice the table's value). The other rules are those of POWER MEANS ONESAMPLE.

## Example

```text
POWER MEANS INDEPENDENT /PARAMETERS SD=8 11 MEAN=31 25 POWER=0.85 NRATIO=2
  /PLOT ES(0 10).
```

`statscraft help POWER MEANS INDEPENDENT` prints the same text in the terminal. [All commands](README.md)
