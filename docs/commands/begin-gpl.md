<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BEGIN GPL

The chart program of the GGRAPH command right before it, up to END GPL.

## Syntax

```text
BEGIN GPL
```

chart program END GPL.

BEGIN GPL and END GPL each stand alone on their line. The block belongs to the GGRAPH before it (one block for each GRAPHSPEC with SOURCE=INLINE); see GGRAPH for the statements it reads.

## Example

```text
BEGIN GPL
SOURCE: s=userSource(id("sales"))
DATA: price=col(source(s), name("price"))
ELEMENT: interval(position(summary.count(bin.rect(price))))
END GPL.
```

`statscraft help BEGIN GPL` prints the same text in the terminal. [All commands](README.md)
