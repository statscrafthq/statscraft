<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# EXECUTE

Run the pending transformations.

## Syntax

```text
EXECUTE.
```

The command reads the data once so that the pending transformations (COMPUTE, IF, RECODE, SELECT IF and the like) take effect. A procedure does the same, so EXECUTE is needed only before a command that does not read the data, or to see the new values in the data window.

## Example

```text
EXECUTE.
```

`statscraft help EXECUTE` prints the same text in the terminal. [All commands](README.md)
