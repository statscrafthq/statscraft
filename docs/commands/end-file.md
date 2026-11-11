<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# END FILE

In an input program or FILE TYPE: stop reading; the case being built is dropped.

## Syntax

```text
END FILE.
```

Usually inside DO IF. Put END CASE before it to keep the case being built.

## Example

```text
DO IF year >= 1881.
END FILE.
END IF.
```

`statscraft help END FILE` prints the same text in the terminal. [All commands](README.md)
