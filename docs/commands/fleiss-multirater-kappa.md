<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FLEISS MULTIRATER KAPPA

Fleiss' kappa: how well several raters agree when they sort subjects into categories.

## Syntax

```text
FLEISS MULTIRATER KAPPA [VARIABLES=]varlist
                        [/MISSING CLASSMISSING=EXCLUDE | INCLUDE]
                        [/CRITERIA [ASYMPTOTICCILEVEL=95] [IGNORECASE=FALSE | TRUE]]
                        [/PRINT [CATEGORY] [CASE(UPPER | LOWER)]].
```

Every variable is a rater and every case a subject; at least two raters, all numeric or all string, each named once. A case missing on any rater is dropped; INCLUDE keeps user-missing values as categories. Numeric ratings lose their fractions. The overall table gives the kappa, its asymptotic standard error under chance agreement, z with its two-sided significance and the confidence interval at ASYMPTOTICCILEVEL percent. CATEGORY adds a row per category: the conditional probability that a second rating of a subject falls in it when one did, its kappa and the same test. Case weights are rounded to whole numbers (0.5 or less drops the case). At least two subjects and two categories are needed. IGNORECASE and CASE work as on WEIGHTED KAPPA.

## Example

```text
FLEISS MULTIRATER KAPPA judge1 TO judge4 /PRINT CATEGORY /CRITERIA ASYMPTOTICCILEVEL=99.
```

`statscraft help FLEISS MULTIRATER KAPPA` prints the same text in the terminal. [All commands](README.md)
