<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# WEIGHT

Weight the cases by a variable, or turn weighting off.

## Syntax

```text
WEIGHT BY var.
WEIGHT OFF.
```

The command makes each case count as many times as its value of var, a numeric variable: a case with weight 2.5 counts as two and a half cases. Weighting stays on until WEIGHT OFF, until the variable is deleted, or until another data file is read, and SAVE stores it with the data. Fractional weights are used as they are in counts, percentages and statistics; most significance tests, confidence intervals and standard errors round them. A case whose weight is zero, negative or missing is left out. SHOW WEIGHT tells which variable weights the data.

## Example

```text
WEIGHT BY weight.
```

`statscraft help WEIGHT` prints the same text in the terminal. [All commands](README.md)
