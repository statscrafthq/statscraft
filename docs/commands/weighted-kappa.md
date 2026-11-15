<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# WEIGHTED KAPPA

Cohen's weighted kappa: how well two raters agree on an ordered scale.

## Syntax

```text
WEIGHTED KAPPA varlist | rowlist BY columnlist
               [/MISSING CLASSMISSING=TABLE | INCLUDE]
               [/CRITERIA [ASYMPTOTICCILEVEL=95] [WEIGHTING=LINEAR | QUADRATIC] [IGNORECASE=FALSE | TRUE]]
               [/PRINT [TABULATION(ALL | rowlist BY columnlist)] [CASE(UPPER | LOWER)]
                       [ORDER(ASCENDING | DESCENDING)]].
```

One list pairs each variable with every later one, the earlier as the row; two lists pair every row variable with every column variable, never a variable with itself. Both variables of a pair must be numeric, or both string. The k categories of a pair are the values either rater used, numbers by size and strings alphabetically, and two categories d places apart get the agreement weight 1 - d/(k - 1) (LINEAR) or 1 - d^2/(k - 1)^2 (QUADRATIC). Each pair drops the cases missing on either of its variables; INCLUDE keeps user-missing values, each as a category of its own.

The kappa table has a row per pair: N, the kappa, its asymptotic standard error, z with its two-sided significance (from the standard error under chance agreement) and the confidence interval at ASYMPTOTICCILEVEL percent. IGNORECASE=TRUE compares strings regardless of case and shows them in the case CASE names. TABULATION adds the table of counts of every pair, or of the pairs the two subsets make, its categories in ORDER. Case weights are rounded to whole numbers (0.5 or less drops the case). A pair with fewer than two cases or only one category is skipped with a warning. KEYWORD(value) works as KEYWORD=value; ASYMPTOTIC\_CILEVEL and IGNORE\_CASE are other spellings.

## Example

```text
WEIGHTED KAPPA nurse_a nurse_b BY doctor /CRITERIA WEIGHTING=QUADRATIC ASYMPTOTICCILEVEL=90
  /PRINT TABULATION(ALL).
```

`statscraft help WEIGHTED KAPPA` prints the same text in the terminal. [All commands](README.md)
