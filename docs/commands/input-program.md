<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# INPUT PROGRAM

Start an input program: commands that build the cases of a new active dataset.

## Syntax

```text
INPUT PROGRAM.
  DATA LIST, REREAD, REPEATING DATA, END CASE, END FILE and transformations
```

END INPUT PROGRAM.

The commands run again and again, each time from the top, until a DATA LIST runs out of data or END FILE runs. Each run builds a case at its end, or at each END CASE when the program has one. Before each case the variables are blank or system-missing again, except scratch variables and those named on LEAVE. A run that reads no record and does not end the file is an error, since it would repeat forever. SELECT IF, SAMPLE, FILTER, TEMPORARY and procedures cannot stand inside. The program runs right away, or at BEGIN DATA when it reads inline data.

## Example

```text
INPUT PROGRAM.
DATA LIST /#n 1 head 3-4 (A).
LEAVE head.
LOOP #i = 1 TO #n.
DATA LIST /item 1-2.
END CASE.
END LOOP.
END INPUT PROGRAM.
```

`statscraft help INPUT PROGRAM` prints the same text in the terminal. [All commands](README.md)
