<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RELIABILITY

Reliability of a scale: Cronbach's alpha, split-half and other models, item statistics and ICC.

## Syntax

```text
RELIABILITY /VARIABLES=varlist [/SCALE(label)=varlist | ALL]
            [/MODEL=ALPHA | SPLIT[(n)] | GUTTMAN | PARALLEL | STRICTPARALLEL]
            [/STATISTICS=DESCRIPTIVE SCALE CORR COV ANOVA [FRIEDMAN | COCHRAN] TUKEY HOTELLING ALL]
            [/SUMMARY=MEANS VARIANCE COV CORR TOTAL ALL]
            [/ICC=[MODEL(MIXED | RANDOM | ONEWAY)] [TYPE(CONSISTENCY | ABSOLUTE)] [CIN=95] [TESTVAL=0]]
            [/METHOD=COVARIANCE] [/MISSING=EXCLUDE | INCLUDE].
```

A case missing on any variable of VARIABLES is left out. ALPHA (default) gives Cronbach's alpha and the alpha of the standardized items; SPLIT puts n items (default half, the odd one in the first part) in the second part and gives the alpha of each part, the correlation between them, Spearman-Brown and Guttman's split-half; GUTTMAN gives lambda 1 to 6; PARALLEL and STRICTPARALLEL give the maximum likelihood estimates and a chi-square test of the model. SUMMARY TOTAL is the Item-Total Statistics: scale mean, variance and alpha if the item is deleted and the corrected item-total correlation. ANOVA is the repeated measures table; FRIEDMAN (with Kendall's W) and COCHRAN give their chi-square in place of F, computed from the values as they are (give ranks or 0/1 items); TUKEY adds the test for nonadditivity; HOTELLING tests that the item means are equal. ICC gives single and average measure coefficients with confidence intervals and F tests.

Items without variance are dropped, with a warning, when the model or a statistic needs the covariance matrix (all but ALPHA and SPLIT with DESCRIPTIVE, SCALE, ANOVA and TOTAL).

Weighted when a weight variable is set. Not supported yet: MODEL=OMEGA, MATRIX.

## Example

```text
RELIABILITY /VARIABLES=q1 TO q10 /SCALE('Satisfaction') ALL /MODEL=ALPHA /STATISTICS=DESCRIPTIVE SCALE /SUMMARY=TOTAL.
```

`statscraft help RELIABILITY` prints the same text in the terminal. [All commands](README.md)
