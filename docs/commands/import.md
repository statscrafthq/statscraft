<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# IMPORT

Read a portable (.por) file and make it the active dataset.

## Syntax

```text
IMPORT FILE='path' | handle [/TYPE=COMM | TAPE] [/KEEP=varlist] [/DROP=varlist]
       [/RENAME=(old = new) ...] [/MAP].
```

KEEP, DROP and RENAME work in the order written, as in GET; KEEP sets the order of the variables. Each MAP shows the variables as the subcommands before it leave them. The file's weight variable weights the cases. TYPE is accepted and changes nothing.

## Example

```text
IMPORT FILE='wave2.por' /DROP=dept TO division /MAP.
```

`statscraft help IMPORT` prints the same text in the terminal. [All commands](README.md)
