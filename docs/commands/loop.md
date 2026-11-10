<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# LOOP

Repeat transformations for each case, with an index, a condition or both.

## Syntax

```text
LOOP [var = first TO last [BY step]] [IF condition].
  transformation commands
```

END LOOP \[IF condition\].

The index runs from first to last by step (1 by default); the expressions may differ per case, a missing one skips the loop. IF on LOOP is checked before each iteration, IF on END LOOP after it. A loop without an index stops after 40 iterations. BREAK leaves the innermost loop.

## Example

```text
LOOP #i = 1 TO 5.
COMPUTE total = total + #i.
END LOOP.
```

`statscraft help LOOP` prints the same text in the terminal. [All commands](README.md)
