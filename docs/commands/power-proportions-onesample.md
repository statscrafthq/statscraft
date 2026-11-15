<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER PROPORTIONS ONESAMPLE

Power or sample size of a test of one proportion against a hypothesized value.

## Syntax

```text
POWER PROPORTIONS ONESAMPLE /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                            N=n | POWER=powers  [NULL=0.5]  ALTERNATIVE=p
                            [ESTIMATE=NORMAL | BINOMIAL] [CONTINUITY=FALSE | TRUE] [TIMER=minutes]
                            [/PLOT [N[(low high)]] [NULL] [ALTERNATIVE] [DIFF] [N_XAXIS[(low high)] DIFF_YAXIS]
                                   [N_YAXIS[(low high)] DIFF_XAXIS] [NULL_XAXIS ALTERNATIVE_YAXIS]
                                   [NULL_YAXIS ALTERNATIVE_XAXIS] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. NULL and ALTERNATIVE lie between 0 and 1, and differ when POWER is given.

NORMAL uses the normal approximation, with a continuity correction of 1 / (2 sqrt(n)) when CONTINUITY=TRUE. BINOMIAL counts the exact binomial rejection region at NULL and its probability at ALTERNATIVE, up to 500 trials; above that the normal approximation is used with a warning. Because exact power rises in steps, the sample size search checks smaller sizes and then counts upwards until the power is reached. TIMER stops that search after so many minutes, with no result. A one-sided test looks in the direction of ALTERNATIVE. PLOT charts power over N, over NULL or ALTERNATIVE (the other kept), over DIFF = ALTERNATIVE - NULL with NULL kept, and surfaces over N and DIFF or NULL and ALTERNATIVE; proportion axes run from 0.01 to 0.99.

## Example

```text
POWER PROPORTIONS ONESAMPLE /PARAMETERS NULL=0.3 ALTERNATIVE=0.42 POWER=0.8 0.9
  ESTIMATE=BINOMIAL.
```

`statscraft help POWER PROPORTIONS ONESAMPLE` prints the same text in the terminal. [All commands](README.md)
