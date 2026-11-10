<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VARIABLE ROLE

Set the role of variables for dialogs.

## Syntax

```text
VARIABLE ROLE /INPUT varlist /TARGET varlist /BOTH varlist /NONE varlist
              /PARTITION varlist /SPLIT varlist.
```

At least one subcommand; a variable named twice keeps its last role.

## Example

```text
VARIABLE ROLE /INPUT age income /TARGET satisfaction /NONE id.
```

`statscraft help VARIABLE ROLE` prints the same text in the terminal. [All commands](README.md)
