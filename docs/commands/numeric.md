<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# NUMERIC

Declare new numeric variables, filled with system-missing.

## Syntax

```text
NUMERIC names [(format)] [/ names [(format)] ...].
```

Without a format the variables get F8.2. Each name must be new, and every case starts as system-missing. COMPUTE and IF make numeric variables too; NUMERIC is needed when a variable must exist first, for example inside a DO IF block or with a given format.

## Example

```text
NUMERIC score (F5.1) / flag.
```

`statscraft help NUMERIC` prints the same text in the terminal. [All commands](README.md)
