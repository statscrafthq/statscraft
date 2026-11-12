<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# REREAD

In an input program: let the next DATA LIST read the last record again.

## Syntax

```text
REREAD [FILE='path' | handle] [COLUMN = expression].
```

COLUMN makes that column of the record column 1 for the DATA LIST (default 1). FILE names the data of the DATA LIST to reread (default: the last DATA LIST's); a DATA LIST that reads other data ignores the REREAD. Of several REREAD commands in a row the last counts.

## Example

```text
DATA LIST /kind 10-14 (A).
DO IF kind = 'BELT'.
REREAD.
DATA LIST /cost 15-18 (DOLLAR,2).
END CASE.
END IF.
```

`statscraft help REREAD` prints the same text in the terminal. [All commands](README.md)
