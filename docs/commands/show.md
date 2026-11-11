<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SHOW

Show the current settings.

## Syntax

```text
SHOW [ALL] [setting ...].
```

Any SET subcommand, and N, WEIGHT, FILTER, SPLIT, DIRECTORY, SYSMIS, VERSION, HANDLES (the file handles), CC (CCA to CCE), ENVIRONMENT, LICENSE, \$VARS, SCOMPRESSION, EXTPATHS, PLUGINS, TMSRECORDING, EXTENSIONS and MXMEMORY. SEED shows where the MC generator stands.

A setting Statscraft does not follow shows what Statscraft uses, with a note in a Note column.

Without names: the main settings.

## Example

```text
SHOW FORMAT DECIMAL LOCALE.
```

`statscraft help SHOW` prints the same text in the terminal. [All commands](README.md)
