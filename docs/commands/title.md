<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# TITLE

Put a title in the output, above the output that follows.

## Syntax

```text
TITLE [']text['].
```

Quotes are optional; with them the text may hold the other kind of quote. The title is cut to 60 bytes and shows as a heading where the command stands.

## Example

```text
TITLE 'Wave 2: cleaned data'.
```

`statscraft help TITLE` prints the same text in the terminal. [All commands](README.md)
