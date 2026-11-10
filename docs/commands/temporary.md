<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TEMPORARY

Make the changes that follow last only until the next procedure.

## Syntax

```text
TEMPORARY.
```

Transformations, labels, formats, SELECT IF, SAMPLE, N OF CASES, WEIGHT, FILTER and SPLIT FILE after it are undone once the next command that reads the data (a procedure, EXECUTE or SAVE) has run, or at the end of the run. Variables that procedure adds stay.

## Example

```text
TEMPORARY.
```

`statscraft help TEMPORARY` prints the same text in the terminal. [All commands](README.md)
