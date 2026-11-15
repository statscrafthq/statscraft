<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER PARTIALCORR

Power or sample size of a test of one partial correlation against a hypothesized value.

## Syntax

```text
POWER PARTIALCORR /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                  PARTIALOUT=k  N=n | POWER=powers  [NULL=0]  ALTERNATIVE=r
                  [/PLOT [N[(low high)]] [PARTIALOUT] [NULL] [ALTERNATIVE] [DIFF]
                         [N_XAXIS[(low high)] DIFF_YAXIS] [N_YAXIS[(low high)] DIFF_XAXIS]
                         [NULL_XAXIS ALTERNATIVE_YAXIS] [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. As POWER PEARSON ONESAMPLE with the bias adjustment, for the correlation of two variables with PARTIALOUT others (a whole number, 0 or more) held fixed: each of them costs one degree of freedom, so the standard error of z is 1 / sqrt(N - 3 - k) and N must exceed k + 3.

PLOT adds PARTIALOUT, power over the number held fixed (from 0 to N - 4).

## Example

```text
POWER PARTIALCORR /PARAMETERS PARTIALOUT=4 ALTERNATIVE=0.3 NULL=0.05 N=120
  /PLOT PARTIALOUT N_XAXIS DIFF_YAXIS.
```

`statscraft help POWER PARTIALCORR` prints the same text in the terminal. [All commands](README.md)
