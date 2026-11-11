<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PRESERVE

Store the current settings and folder, to bring them back with RESTORE.

## Syntax

```text
PRESERVE.
```

Stores every setting of SET (CCA to CCE, DECIMAL, FORMAT, LOCALE, MPRINT, MEXPAND, MNEST, MITERATE and the rest) and the folder of CD. PRESERVE and RESTORE nest up to five levels.

Words after PRESERVE (PRESERVE LOCALE, as survey tools write it) are ignored with a warning: every setting is stored anyway.

## Example

```text
PRESERVE.
```

`statscraft help PRESERVE` prints the same text in the terminal. [All commands](README.md)
