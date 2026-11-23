<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ELSE IF

Start the next conditional branch of a DO IF block.

## Syntax

```text
ELSE IF (condition).
```

Inside a DO IF block: the commands that follow run for the cases where no earlier condition of the block was true and this one is. A case whose earlier condition was missing runs no later branch.

## Example

```text
ELSE IF (age < 65).
```

`statscraft help ELSE IF` prints the same text in the terminal. [All commands](README.md)
