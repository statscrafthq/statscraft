<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DELETE VARIABLES

Remove variables from the dataset.

## Syntax

```text
DELETE VARIABLES varlist.
```

The command removes the variables and their values from the active dataset at once. At least one variable must remain. Deleting the weight variable turns weighting off.

## Example

```text
DELETE VARIABLES ch_web TO ch_email.
```

`statscraft help DELETE VARIABLES` prints the same text in the terminal. [All commands](README.md)
