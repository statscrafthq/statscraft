<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# HOST

Refused: Statscraft never runs operating system commands from syntax.

## Syntax

```text
HOST COMMAND=['command' ...] [TIMELIMIT=n].
```

Read and checked, then refused with an error: a command run from a syntax file could do anything the user can, which makes shared syntax a security risk. Run the commands outside Statscraft, before or after the syntax.

## Example

```text
HOST COMMAND=['dir'].
```

`statscraft help HOST` prints the same text in the terminal. [All commands](README.md)
