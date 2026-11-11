<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# RESTORE

Bring back the settings and folder the last PRESERVE stored.

## Syntax

```text
RESTORE.
```

Every setting and the folder come back as the last PRESERVE stored them; a RESTORE without PRESERVE does nothing, with a warning. Words after RESTORE (RESTORE LOCALE) are ignored with a warning: every setting comes back.

## Example

```text
RESTORE.
```

`statscraft help RESTORE` prints the same text in the terminal. [All commands](README.md)
