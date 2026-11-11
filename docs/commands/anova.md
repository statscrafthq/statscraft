<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ANOVA

Analysis of variance for factorial designs with covariates, the older form with MCA tables.

## Syntax

```text
ANOVA [VARIABLES=] dependents BY factors(min,max) [factors(min,max) ...] [WITH covariates] [/next list ...]
      [/COVARIATES=FIRST | WITH | AFTER] [/MAXORDERS=ALL | n | NONE]
      [/METHOD=UNIQUE | EXPERIMENTAL | HIERARCHICAL] [/STATISTICS=[MCA] [REG] [MEAN] [ALL] [NONE]]
      [/MISSING=EXCLUDE | INCLUDE].
```

A range follows the factors it applies to; factor values are truncated to whole numbers and cases outside a range are left out, as is a case missing on any variable of its list. The model has the covariates, the main effects and the interactions up to MAXORDERS (five by default); an order of interactions with empty cells is dropped with a warning. UNIQUE (default) adjusts every effect for all others (Type III); EXPERIMENTAL assesses covariates, main effects and each order of interactions in turn, each adjusted for its own kind and the kinds before it; HIERARCHICAL adjusts each covariate and factor only for those before it. COVARIATES sets where the covariates enter. Tables: ANOVA (with REG the coefficients of the covariates where they enter), and, not with UNIQUE, Cell Means (MEAN), MCA, Factor Summary and Model Goodness of Fit (MCA). STATISTICS without keywords is MEAN and REG. Weighted when a weight variable is set.

## Example

```text
ANOVA VARIABLES=score BY region(1,4) sex(1,2) WITH age /METHOD=EXPERIMENTAL /STATISTICS=MEAN MCA.
```

`statscraft help ANOVA` prints the same text in the terminal. [All commands](README.md)
