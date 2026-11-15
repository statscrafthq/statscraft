<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER PROPORTIONS RELATED

Power or number of pairs of McNemar's test of two paired proportions.

## Syntax

```text
POWER PROPORTIONS RELATED /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                          NPAIRS=n | POWER=powers  VALUES=PROPORTIONS(a b) | COUNTS(a b)
                          [MARGINAL=FALSE | TRUE] [CORRELATION=0.5] [ESTIMATE=NORMAL | BINOMIAL] [TIMER=minutes]
                          [/PLOT [NPAIRS[(low high)]] [RISK_DIFF] [RISK_RATIO[(low high)]] [ODDS_RATIO[(low high)]]
                                 [CORRELATION] [DISCORDANT] [MARGINAL] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. A pair is a success or failure on a first and a second occasion. With MARGINAL=FALSE, a is the share of pairs with success first and failure second and b the reverse; with TRUE, a and b are the success rates of the first and second occasion, turned into those two shares through their CORRELATION (between -1 and 1). COUNTS give the numbers of such pairs and need NPAIRS. The risk difference is b - a in both readings, the risk and odds ratios compare the second discordant share with the first. NORMAL uses the normal approximation; BINOMIAL averages the exact conditional binomial test over the number of discordant pairs, up to 500 pairs. PLOT: NPAIRS (within 2 and 2500), RISK\_DIFF, RISK\_RATIO and ODDS\_RATIO by moving b with a kept, CORRELATION (MARGINAL=TRUE only), and surfaces over the two discordant shares (DISCORDANT) or the two success rates (MARGINAL, with TRUE).

## Example

```text
POWER PROPORTIONS RELATED /PARAMETERS VALUES=PROPORTIONS(0.6 0.45) MARGINAL=TRUE
  CORRELATION=0.3 POWER=0.9 /PLOT CORRELATION.
```

`statscraft help POWER PROPORTIONS RELATED` prints the same text in the terminal. [All commands](README.md)
