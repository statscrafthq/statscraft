<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# MISSING VALUES

Declare user-missing values.

## Syntax

```text
MISSING VALUES varlist (values) [[/] varlist (values) ...].
```

values: up to three values, or one range with at most one extra value.

range: low THRU high, LOWEST THRU high, low THRU HIGHEST (LO and HI also work).

Empty parentheses clear the missing values.

## Example

```text
MISSING VALUES q1 q2 (98, 99) / income (LOWEST THRU -1, 999999) / plan ().
```

`statscraft help MISSING VALUES` prints the same text in the terminal. [All commands](README.md)
