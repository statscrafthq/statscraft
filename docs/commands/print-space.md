<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PRINT SPACE

Add blank lines to the PRINT output, for every case it runs on.

## Syntax

```text
PRINT SPACE [OUTFILE='path'] [ENCODING='name'] [expression].
```

The expression gives the number of lines (default 1); a fraction is cut off; a missing or negative number gives one line and a warning. Usually inside DO IF.

## Example

```text
PRINT SPACE.
```

`statscraft help PRINT SPACE` prints the same text in the terminal. [All commands](README.md)
