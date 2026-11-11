<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BEGIN PROGRAM

A block of code in another programming language; it is skipped with a warning.

## Syntax

```text
BEGIN PROGRAM [language].
```

code END PROGRAM.

Statscraft runs no Python or R code. The block is read up to END PROGRAM and not run; the run goes on with one warning per block. Files, variables or commands the block would make are missing, so later commands that need them fail.

## Example

```text
BEGIN PROGRAM PYTHON3.
print('not run')
END PROGRAM.
```

`statscraft help BEGIN PROGRAM` prints the same text in the terminal. [All commands](README.md)
