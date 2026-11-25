<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CONSTRAINED FUNCTIONS

Start the program that computes the constrained functions of the next CNLR.

## Syntax

```text
CONSTRAINED FUNCTIONS.
```

The transformations that follow, up to the next command of another kind, compute functions of the parameters of the MODEL PROGRAM, each into a variable, for the bounds of the next CNLR (a bound such as cf \<= 0 names the variable). The program belongs right before CNLR; NLR refuses it. A constrained function may depend on the parameters only: its value is taken from the first case that is used, and a function that differs between cases is an error.

## Example

```text
CONSTRAINED FUNCTIONS.
COMPUTE cf = b0 - EXP(b1).
```

`statscraft help CONSTRAINED FUNCTIONS` prints the same text in the terminal. [All commands](README.md)
