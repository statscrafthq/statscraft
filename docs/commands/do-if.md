<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# DO IF

Start a block of transformations that run only where a condition holds.

## Syntax

```text
DO IF (condition).
```

Followed by COMPUTE, IF, RECODE or nested DO IF commands, then optional ELSE IF (condition). and ELSE. branches, then END IF.

For each case the first branch whose condition is true runs. After a missing condition no later branch runs for that case.

## Example

```text
DO IF (age < 18).
```

`statscraft help DO IF` prints the same text in the terminal. [All commands](README.md)
