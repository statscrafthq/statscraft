<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CLEAR TIME PROGRAM

Drop the time-dependent covariates of the last TIME PROGRAM.

## Syntax

```text
CLEAR TIME PROGRAM.
```

The transformations of the last TIME PROGRAM are dropped, so a new TIME PROGRAM can define the time-dependent covariates afresh. COXREG uses a program up when it runs, and so does any other command that reads the data.

## Example

```text
CLEAR TIME PROGRAM.
```

`statscraft help CLEAR TIME PROGRAM` prints the same text in the terminal. [All commands](README.md)
