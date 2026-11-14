<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TIME PROGRAM

Start the transformations that define time-dependent covariates for the next COXREG.

## Syntax

```text
TIME PROGRAM.
```

The transformations that follow (COMPUTE, IF, RECODE, DO IF, LOOP and the like), up to the next command of another kind, such as a label or format command, are kept for the next COXREG instead of changing the data. COXREG runs them for every case at each event time of its stratum up to the case's own time, with the variable T\_ holding that time, so a covariate they compute can change with time. The data may not have a variable T\_, and T\_ itself is not a covariate. A missing value from the program where the case is otherwise valid is an error, and so is a program that adds or drops cases, leaves a LOOP or DO IF open, or holds PRINT, WRITE or XSAVE; a COXREG that stops for a fault in the program drops it, one that stops for another error keeps it. Any other command that reads the data drops the program, as do CLEAR TIME PROGRAM and CLEAR TRANSFORMATIONS.

## Example

```text
TIME PROGRAM.
COMPUTE spend_t = spend * LN(T_ + 1).
```

`statscraft help TIME PROGRAM` prints the same text in the terminal. [All commands](README.md)
