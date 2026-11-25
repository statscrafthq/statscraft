<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DERIVATIVES

Start the derivatives program of the model program.

## Syntax

```text
DERIVATIVES.
```

The transformations that follow, up to the next command of another kind, compute derivatives of the predicted value of the model for NLR. Name the derivative of a parameter D.parameter, for example D.b1 for b1. The commands may use the parameters, the variables of the data, the variables of the model program and the derivatives computed before. Give the derivatives of some of the parameters or of all; NLR estimates the others numerically and, at the start, compares the derivatives given with numerical ones (CKDER of CRITERIA).

## Example

```text
DERIVATIVES.
COMPUTE D.b0 = EXP(b1 * dose).
COMPUTE D.b1 = b0 * dose * D.b0.
```

`statscraft help DERIVATIVES` prints the same text in the terminal. [All commands](README.md)
