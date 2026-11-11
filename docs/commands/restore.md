<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RESTORE

Bring back the settings and folder the last PRESERVE stored.

## Syntax

```text
RESTORE.
```

Every setting and the folder come back as the last PRESERVE stored them; a RESTORE without PRESERVE does nothing, with a warning. After RESTORE LOCALE (or another SET subcommand name) every setting still comes back; a warning names the settings besides LOCALE and DECIMAL that changed after PRESERVE and so come back too. Other words are ignored with a warning.

## Example

```text
RESTORE.
```

`statscraft help RESTORE` prints the same text in the terminal. [All commands](README.md)
