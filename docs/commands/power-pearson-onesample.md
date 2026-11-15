<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER PEARSON ONESAMPLE

Power or sample size of a test of one Pearson correlation against a hypothesized value.

## Syntax

```text
POWER PEARSON ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                        N=n | POWER=powers  [NULL=0]  ALTERNATIVE=r  [ADJUST_BIAS=TRUE | FALSE]
                        [/PLOT [N[(low high)]] [NULL] [ALTERNATIVE] [DIFF] [N_XAXIS[(low high)] DIFF_YAXIS]
                               [N_YAXIS[(low high)] DIFF_XAXIS] [NULL_XAXIS ALTERNATIVE_YAXIS]
                               [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. The test compares Fisher's z of the sample correlation with that of NULL; its standard error is 1 / sqrt(N - 3) and the power comes from the normal approximation. N is a whole number above 3. NULL and ALTERNATIVE lie from -1 to 1; with POWER they lie strictly inside and differ. ADJUST\_BIAS=TRUE (the default) adds the small-sample corrections to the mean and spread of z; FALSE leaves them out. A one-sided test looks in the direction of ALTERNATIVE - NULL. PLOT charts power over N, NULL, ALTERNATIVE or DIFF = ALTERNATIVE - NULL (NULL kept), and surfaces over N and DIFF or NULL and ALTERNATIVE; correlation axes run from -0.99 to 0.99.

## Example

```text
POWER PEARSON ONESAMPLE /PARAMETERS NULL=0.15 ALTERNATIVE=0.4 POWER=0.8 0.9
  /PLOT N.
```

`statscraft help POWER PEARSON ONESAMPLE` prints the same text in the terminal. [All commands](README.md)
