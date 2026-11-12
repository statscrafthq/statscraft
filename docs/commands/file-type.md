<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FILE TYPE

Start the definition of a file with several kinds of records.

## Syntax

```text
FILE TYPE MIXED | GROUPED | NESTED [FILE='path' | handle] [ENCODING='UTF-8']
    RECORD=[var] columns [(format)] [CASE=[var] columns [(format)]]
    [WILD=WARN | NOWARN] [DUPLICATE=WARN | NOWARN | CASE] [MISSING=WARN | NOWARN]
    [ORDERED=YES | NO].
```

RECORD TYPE value ... / DATA LIST ... pairs follow, then END FILE TYPE.

MIXED: each record of a listed type is a case; others are skipped. GROUPED: the records with one CASE value make one case (CASE required). NESTED: the RECORD TYPE commands go from the highest level to the lowest; each lowest-level record is a case with the values of the records above it. Defaults: WILD=NOWARN (WARN for grouped), DUPLICATE and MISSING WARN for grouped and NOWARN for nested, ORDERED=YES (grouped only); DUPLICATE=CASE (nested) makes a case of a record that has no records below it.

## Example

```text
FILE TYPE GROUPED RECORD=#kind 6 CASE=runner 1-4.
RECORD TYPE 1.
DATA LIST /club 8-9 (A).
RECORD TYPE 2.
DATA LIST /sprint 8-10.
END FILE TYPE.
```

`statscraft help FILE TYPE` prints the same text in the terminal. [All commands](README.md)
