<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# VARIABLE ATTRIBUTE

Set or delete custom attributes of variables.

## Syntax

```text
VARIABLE ATTRIBUTE VARIABLES=varlist ATTRIBUTE=name('value') [name[n]('value') ...]
                   [DELETE=name [name[n] ...]] [/VARIABLES=...].
```

name\[n\] is element n of an attribute array (from 1); DELETE=name\[n\] removes it and moves the later elements up, DELETE=name removes the whole attribute. Values are always quoted.

## Example

```text
VARIABLE ATTRIBUTE VARIABLES=q1 q2 ATTRIBUTE=Source('wave 2') Note[2]('checked').
```

`statscraft help VARIABLE ATTRIBUTE` prints the same text in the terminal. [All commands](README.md)
