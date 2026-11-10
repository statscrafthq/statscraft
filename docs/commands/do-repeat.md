<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DO REPEAT

Repeat transformations with stand-in names replaced by variables or values.

## Syntax

```text
DO REPEAT standin = varlist | values [/ standin = varlist | values ...].
  transformation commands
```

END REPEAT \[PRINT\].

Every list must have as many items; the commands run once per item, each stand-in replaced by its item. A list holds existing variables (TO in file order, ALL) or new names (v1 TO v5), or numbers and quoted strings (1 TO 5 counts up). PRINT shows the generated commands.

## Example

```text
DO REPEAT r = region1 TO region5 / v = 1 TO 5.
COMPUTE r = (region = v).
END REPEAT.
```

`statscraft help DO REPEAT` prints the same text in the terminal. [All commands](README.md)
