<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# END CASE

In an input program: add the case built so far to the dataset and go on.

## Syntax

```text
END CASE.
```

The program goes on with the command after END CASE, with the variables blank or system-missing again except scratch and LEAVE variables. With END CASE in a program no case is built at the end of a run.

## Example

```text
LOOP #i = 1 TO 3.
COMPUTE x = v(#i).
END CASE.
END LOOP.
```

`statscraft help END CASE` prints the same text in the terminal. [All commands](README.md)
