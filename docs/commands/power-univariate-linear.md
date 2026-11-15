<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# POWER UNIVARIATE LINEAR

Power or sample size of the F test of a set of predictors in a linear regression.

## Syntax

```text
POWER UNIVARIATE LINEAR /PARAMETERS [SIGNIFICANCE=0.05] [MODEL=FIXED | RANDOM]  N=n | POWER=powers
                        TOTAL_PREDICTORS=p  TEST_PREDICTORS=q
                        PARTIAL_CORR=r | FULL_MODEL=r2 NESTED_MODEL=r2  [INTERCEPT=TRUE | FALSE]
                        [/PLOT [N[(low high)]] [PARTIAL_CORR] [N_XAXIS[(low high)] PARTIAL_CORR_YAXIS]
                               [N_YAXIS[(low high)] PARTIAL_CORR_XAXIS] [VROTATE(10)] [HROTATE(325)]].
```

Needs no data. A regression with p predictors (besides the intercept) tests whether q of them add nothing to the other p - q. The effect is the multiple partial correlation of the outcome with the q, or comes from the squared multiple correlations of the full and the nested model as (full - nested) / (1 - nested). The F test has q and N - p - 1 df (N - p without the intercept). MODEL=FIXED takes the predictor values as fixed: noncentral F with noncentrality N r^2 / (1 - r^2). RANDOM takes outcome and predictors as jointly normal and uses the exact distribution of the sample squared multiple partial correlation. q is from 1 to p (below p with FULL\_MODEL); PARTIAL\_CORR lies strictly between -1 and 1 and is not 0 with POWER; the squared correlations lie strictly between 0 and 1, full not below nested (above it with POWER). N is at least p + 2 (p + 1 without the intercept). PLOT charts power over N and over the partial correlation (default 0 to 0.99), and a surface over both.

## Example

```text
POWER UNIVARIATE LINEAR /PARAMETERS TOTAL_PREDICTORS=7 TEST_PREDICTORS=3 FULL_MODEL=0.31
  NESTED_MODEL=0.24 MODEL=RANDOM POWER=0.8 /PLOT N.
```

`statscraft help POWER UNIVARIATE LINEAR` prints the same text in the terminal. [All commands](README.md)
