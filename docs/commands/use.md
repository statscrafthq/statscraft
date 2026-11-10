<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# USE

Use all cases, or a range of cases, in the commands that follow.

## Syntax

```text
USE ALL.
USE [first] [THRU [last]] [/PERMANENT].
USE ALL (or THRU alone) turns FILTER off. A range of case numbers is supported with
```

/PERMANENT, which deletes the cases outside it; dates are not supported yet.

## Example

```text
USE ALL.
```

`statscraft help USE` prints the same text in the terminal. [All commands](README.md)
