<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BREAK

Leave the innermost LOOP, usually inside DO IF.

## Syntax

```text
BREAK.
```

## Example

```text
LOOP #i = 1 TO 10.
DO IF v(#i) = 0.
BREAK.
END IF.
END LOOP.
```

`statscraft help BREAK` prints the same text in the terminal. [All commands](README.md)
