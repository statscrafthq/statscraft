<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER MEANS RELATED

Power or number of pairs of a paired t-test.

## Syntax

```text
POWER MEANS RELATED /PARAMETERS [TEST=NONDIRECTIONAL | DIRECTIONAL] [SIGNIFICANCE=0.05]
                    NPAIRS=n | POWER=powers  SD=sd | sd1 sd2  [RHO=0.5]  MEAN=difference | mean1 mean2
                    [/PLOT [NPAIRS[(low high)]] [ES[(low high)]] [NPAIRS_XAXIS[(low high)] ES_YAXIS[(low high)]]
                           [NPAIRS_YAXIS[(low high)] ES_XAXIS[(low high)]] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. The test is the one-sample t-test of the differences within pairs. One SD is the standard deviation of those differences; two are those of the two measures, combined with their correlation RHO (-1 to 1, not 0) as sqrt(sd1^2 + sd2^2 - 2 RHO sd1 sd2). RHO is ignored with one SD. NPAIRS is a whole number above 1; the NPAIRS chart range lies within 2 and 5000. The other rules are those of POWER MEANS ONESAMPLE.

## Example

```text
POWER MEANS RELATED /PARAMETERS SD=6.5 7 RHO=0.7 MEAN=3 NPAIRS=40 TEST=DIRECTIONAL
  /PLOT NPAIRS(10 80).
```

`statscraft help POWER MEANS RELATED` prints the same text in the terminal. [All commands](README.md)
