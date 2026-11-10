<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ADD DOCUMENT

Add quoted lines to the documents of the data file.

## Syntax

```text
ADD DOCUMENT 'line' ['line' ...].
```

Each string is one line; an empty string gives a blank line.

## Example

```text
ADD DOCUMENT 'Cleaned on 10 November.' 'Outliers removed.'
```

`statscraft help ADD DOCUMENT` prints the same text in the terminal. [All commands](README.md)
