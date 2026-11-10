<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DOCUMENT

Add free text to the documents of the data file.

## Syntax

```text
DOCUMENT text.
```

The text may run over several lines and hold blank lines; a period at the end of a line ends it. Every line becomes a line of the documents.

## Example

```text
DOCUMENT Weights were trimmed at 5.
```

`statscraft help DOCUMENT` prints the same text in the terminal. [All commands](README.md)
