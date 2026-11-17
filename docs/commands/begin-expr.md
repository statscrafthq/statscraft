<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BEGIN EXPR

Starts a block of expressions that END EXPR saves as a model for SIMPLAN.

## Syntax

```text
BEGIN EXPR /OUTFILE PREPXML='file'.
```

NUMERIC and STRING, VARIABLE LEVEL, VALUE LABELS and COMPUTE commands, then END EXPR.

Nothing in the block touches the active dataset. Each COMPUTE gives a target, in order, and may use the targets before it; a variable is computed once. NUMERIC names the inputs the active dataset lacks, and only those. Targets and declared inputs are scale unless VARIABLE LEVEL says otherwise. The file is Statscraft's own JSON (not XML); a name starting with \*\\ is kept in memory for the session.

## Example

```text
BEGIN EXPR /OUTFILE PREPXML='margin.json'.
NUMERIC units unit_price.
COMPUTE margin = units * (unit_price - unit_cost).
END EXPR.
```

`statscraft help BEGIN EXPR` prints the same text in the terminal. [All commands](README.md)
