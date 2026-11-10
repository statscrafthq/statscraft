<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SAMPLE

Keep a random sample of the cases.

## Syntax

```text
SAMPLE proportion.
SAMPLE n FROM m.
```

A proportion between 0 and 1 keeps each case with that chance; n FROM m keeps exactly n of the first m cases (fewer when there are fewer than m). Inside DO IF only the cases of the branch are sampled. The random numbers start from the same seed in every session.

## Example

```text
SAMPLE 500 FROM 3420.
```

`statscraft help SAMPLE` prints the same text in the terminal. [All commands](README.md)
