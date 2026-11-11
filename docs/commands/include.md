<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# INCLUDE

Run the commands of another syntax file, read by the batch rules.

## Syntax

```text
INCLUDE [FILE=]'path' | handle [ENCODING='name'] [PASSWORD='text'].
```

Like INSERT SYNTAX=BATCH: a line that starts in the first column starts a new command.

## Example

```text
INCLUDE FILE='setup.sps'.
```

`statscraft help INCLUDE` prints the same text in the terminal. [All commands](README.md)
