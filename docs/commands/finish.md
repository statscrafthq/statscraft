<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FINISH

End the job: the commands after it are not read.

## Syntax

```text
FINISH.
```

The run ends here, as at the end of the syntax. Nothing after FINISH is read, so it causes no error. FINISH in an inserted file ends the whole run; when a range of lines runs, only a FINISH inside it counts. The window and its session stay open.

## Example

```text
FREQUENCIES VARIABLES=region.
FINISH.
```

`statscraft help FINISH` prints the same text in the terminal. [All commands](README.md)
