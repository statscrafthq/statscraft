<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FILTER

Leave out of procedures the cases whose filter value is 0 or missing.

## Syntax

```text
FILTER BY var.
FILTER OFF.
```

The cases stay in the dataset. The filter is kept with the dataset, follows a rename and ends when the variable is deleted or the dataset replaced.

## Example

```text
FILTER BY adult.
```

`statscraft help FILTER` prints the same text in the terminal. [All commands](README.md)
