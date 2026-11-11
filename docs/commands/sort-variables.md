<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SORT VARIABLES

Put the variables in order by a property of their dictionary entries.

## Syntax

```text
SORT VARIABLES [BY] NAME | TYPE | FORMAT | LABEL | VALUES | MISSING | MEASURE | ROLE | COLUMNS
               | ALIGNMENT | ATTRIBUTE name [(A | D)].
```

A or UP is ascending (the default), D or DOWN reverses the whole order.

NAME: letters without regard to case, then a number at the end by its value (v2 before v10).

TYPE: numeric, then strings from narrow to wide. FORMAT: by format type in the order of the data file codes (A, COMMA, DOLLAR, F, ... DATE ...), then width and decimals.

LABEL, VALUES, MISSING, ATTRIBUTE: variables without one come first; then labels and attribute values without regard to case; value labels and missing values so that equal definitions sit together. MEASURE: nominal, ordinal, scale. ROLE: input, target, both, none, partition, split. COLUMNS: display width (default 8 for numbers, the width for strings).

ALIGNMENT: left, right, center. Variables that tie keep their order.

## Example

```text
SORT VARIABLES BY NAME (A).
```

`statscraft help SORT VARIABLES` prints the same text in the terminal. [All commands](README.md)
