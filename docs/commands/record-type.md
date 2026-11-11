<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RECORD TYPE

Inside FILE TYPE: the record types the following DATA LIST reads.

## Syntax

```text
RECORD TYPE value [value ...] | OTHER [SKIP] [CASE=columns] [DUPLICATE=WARN | NOWARN]
    [MISSING=WARN | NOWARN] [SPREAD=YES | NO].
```

Values are numbers, or quoted strings for a string record identifier. OTHER stands for every type not named before and comes last. SKIP passes over the records. CASE gives other columns for the case identifier; DUPLICATE (grouped) and MISSING apply to these types only. SPREAD=NO (nested): the values go to the first case built under the record only.

## Example

```text
RECORD TYPE 21, 22, 23.
DATA LIST /sex 5 age 6-7.
```

`statscraft help RECORD TYPE` prints the same text in the terminal. [All commands](README.md)
