<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# WEIGHT

Weight the cases by a variable, or turn weighting off.

## Syntax

```text
WEIGHT BY var.
WEIGHT OFF.
```

Fractional weights are used as they are in counts, percentages and statistics; most significance tests, confidence intervals and standard errors round them. A case whose weight is zero, negative or missing is left out.

## Example

```text
WEIGHT BY weight.
```

`statscraft help WEIGHT` prints the same text in the terminal. [All commands](README.md)
