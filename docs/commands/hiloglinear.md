<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# HILOGLINEAR

Hierarchical loglinear models of a multiway table of counts, with backward elimination.

## Syntax

```text
HILOGLINEAR varlist (min,max) [varlist (min,max) ...]
   [/METHOD[=BACKWARD]] [/MAXORDER=k]
   [/CRITERIA=[CONVERGE(n)] [ITERATE(20)] [P(0.05)] [DELTA(0.5)] [MAXSTEPS(10)] [DEFAULT]]
   [/CWEIGHT=variable | (w w n*w ...)]
   [/PRINT=[FREQ] [RESID] [ESTIM] [ASSOCIATION] | DEFAULT | ALL | NONE]
   [/PLOT=[RESID] [NORMPROB] | DEFAULT | ALL | NONE] [/MISSING=EXCLUDE | INCLUDE]
   [/DESIGN=term [term ...]] [/DESIGN ...].
```

terms: a variable or variables joined by \* (or BY). At least two and at most ten numeric variables, each group followed by its whole-number range; every integer of the range is a category, values are cut to integers, and a case with a value outside a range is counted as out of range and left out. A case missing on a variable is left out unless INCLUDE makes user-missing values valid; case weights are used as they are.

A DESIGN lists the generating class: the highest terms, each of which brings in all the terms made of part of its variables. Every DESIGN fits its own model with the CRITERIA, PRINT, PLOT, MAXORDER and CWEIGHT given earlier in the command (they carry over); METHOD applies to the next DESIGN only. A DESIGN with no terms, or the one added (with a warning) when the command does not end with DESIGN, is the saturated model; MAXORDER k cuts every term down to its k-variable parts. The model is fitted by iterative proportional fitting until no cell moves by CONVERGE (by default the larger of 0.25 and a thousandth of the largest weighted count) or ITERATE cycles have run. CWEIGHT gives a weight per cell, from a variable (its mean in the cell, 1 for an empty cell) or as a list in cell order with the last variable changing fastest; a weight of 0 makes a structural zero and weights are cut to the range 0 to 1.

Tables: Data Information, Convergence Information, Cell Counts and Residuals (FREQ: observed and expected counts; RESID: raw and standardized residuals) and the likelihood-ratio and Pearson Goodness-of-Fit Tests, whose df leaves out the cells fitted as 0. A saturated model is fitted to the counts plus DELTA and adds the tests that all effects of order k, or of order k and higher, are zero; ESTIM gives its effect-coded parameters with standard errors (when no cell is 0) and ASSOCIATION the partial chi-square of each lower-order term. By default an unsaturated model shows FREQ and RESID, a saturated one all four. METHOD=BACKWARD removes, one per step and at most MAXSTEPS times, the term of the generating class whose removal has the largest likelihood-ratio p, while that p is above P, and lists the steps.

PLOT (unsaturated models only) draws the standardized residuals against the observed and expected counts (RESID) and a normal Q-Q plot of the adjusted residuals (NORMPROB, also NORMPLOT); PLOT alone means both.

## Example

```text
HILOGLINEAR route(1,3) season(1,4) delay(0,1) /METHOD=BACKWARD /CRITERIA=P(0.01) /DESIGN.
```

`statscraft help HILOGLINEAR` prints the same text in the terminal. [All commands](README.md)
