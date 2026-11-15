<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER SPEARMAN ONESAMPLE

Power or sample size of a test of one Spearman rank correlation against a hypothesized value.

## Syntax

```text
POWER SPEARMAN ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                         N=n | POWER=powers  [NULL=0]  ALTERNATIVE=r  [VARIANCE=BW | FHP | CC]
                         [/PLOT [N[(low high)]] [NULL] [ALTERNATIVE] [DIFF] [N_XAXIS[(low high)] DIFF_YAXIS]
                                [N_YAXIS[(low high)] DIFF_XAXIS] [NULL_XAXIS ALTERNATIVE_YAXIS]
                                [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. As POWER PEARSON ONESAMPLE, with Fisher's z of the rank correlation, whose variance VARIANCE chooses: BW (Bonett and Wright, the default) (1 + ALTERNATIVE^2 / 2) / (N - 3); FHP (Fieller, Hartley and Pearson) 1.06 / (N - 3); CC (Caruso and Cliff) 1 / (N - 2) + \|z of ALTERNATIVE\| / (6 N + 4 sqrt(N)). N is above 3, above 2 with CC. There is no bias adjustment.

## Example

```text
POWER SPEARMAN ONESAMPLE /PARAMETERS ALTERNATIVE=-0.35 N=45 VARIANCE=CC
  TEST=DIRECTIONAL /PLOT DIFF.
```

`statscraft help POWER SPEARMAN ONESAMPLE` prints the same text in the terminal. [All commands](README.md)
