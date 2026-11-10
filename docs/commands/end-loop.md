<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# END LOOP

End a LOOP, optionally when a condition holds.

## Syntax

```text
END LOOP [IF condition].
```

## Example

```text
LOOP.
COMPUTE x = x + 1.
END LOOP IF x >= 10.
```

`statscraft help END LOOP` prints the same text in the terminal. [All commands](README.md)
