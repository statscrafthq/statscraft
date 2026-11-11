<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# LEAVE

Keep the values of new variables from case to case instead of resetting them.

## Syntax

```text
LEAVE varlist.
```

The variables must be new: made by the transformations since the data were last read, before LEAVE, and not scratch variables. A numeric one starts at 0 for the first case, a string one blank; each later case starts with the value the case before ended with.

In an input program the variables are not reset before each case.

## Example

```text
COMPUTE total = total + salary.
LEAVE total.
```

`statscraft help LEAVE` prints the same text in the terminal. [All commands](README.md)
