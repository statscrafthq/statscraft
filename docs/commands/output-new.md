<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OUTPUT NEW

Start a new output document, which gets the tables from now on.

## Syntax

```text
OUTPUT NEW [NAME=name] [TYPE=VIEWER].
```

The tables of the commands that follow go to the new document until another one is started or activated. Without NAME it is called Document1, Document2 and so on; a name that another document had passes to the new one, and the other gets a new name. TYPE is ignored. The window shows the tables of every document in its Output view; the documents decide what OUTPUT EXPORT writes. The window session keeps its documents from run to run; a command line run starts without one.

## Example

```text
OUTPUT NEW NAME=women.
```

`statscraft help OUTPUT NEW` prints the same text in the terminal. [All commands](README.md)
