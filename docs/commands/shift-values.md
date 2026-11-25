<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SHIFT VALUES

New variables that hold the value of the same variable in an earlier or later case.

## Syntax

```text
SHIFT VALUES VARIABLE=var RESULT=newvar {LEAD=n | LAG=n | SHIFT=n}
             [/VARIABLE=var RESULT=newvar {LEAD=n | LAG=n | SHIFT=n}] ...
```

LEAD=n takes the value from the case n after the current one, LAG=n from n before it, and SHIFT=n from n after it, or from -n before it when n is negative. n is a whole number and 0 copies the value in place. The result must be a name no variable has yet. The first or last n cases of the data (of each split group) get the system-missing value, or an empty string.

The result is of the same type as the variable and keeps its format, value labels, missing values and measurement level, but not its custom attributes; its label tells what shifted it. User-missing values are carried over as they are. A filter is ignored: every case counts.

## Example

```text
SHIFT VALUES VARIABLE=price RESULT=price_next LEAD=1 /VARIABLE=price RESULT=price_prev LAG=1.
```

`statscraft help SHIFT VALUES` prints the same text in the terminal. [All commands](README.md)
