<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MODEL PROGRAM

Name the parameters of a nonlinear model and give their starting values; the transformations after it define the model.

## Syntax

```text
MODEL PROGRAM parameter[=value] [[,] parameter[=value] ...].
```

The transformations that follow (COMPUTE, IF, RECODE, DO IF, LOOP and the like), up to the next command of another kind, are kept for the next NLR instead of changing the data. They compute the predicted value of the dependent variable from the parameters and the variables of the data, into the variable PRED unless the PRED subcommand of NLR names another. Every parameter is named once and needs a starting value, here or from the FILE of NLR; a value of zero says nothing about the scale of the parameter, so avoid it. VARIABLE LABELS and FORMATS for the predicted values may stand among the transformations. The model program changes neither the data nor the variables: its parameters and the variables it creates exist only while NLR runs.

DERIVATIVES. inside the program starts the derivatives program, whose COMPUTE commands give the derivative of the predicted value for a parameter in the variable D.parameter; a derivative it leaves out is estimated numerically. A model program stays until an NLR uses it, CLEAR MODEL PROGRAMS, NEW FILE, CLEAR TRANSFORMATIONS or the next MODEL PROGRAM.

## Example

```text
MODEL PROGRAM b0=2 b1=0.3.
COMPUTE pred = b0 * EXP(b1 * dose).
```

`statscraft help MODEL PROGRAM` prints the same text in the terminal. [All commands](README.md)
