<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# INSERT

Run the commands of another syntax file in place of this command.

## Syntax

```text
INSERT FILE='path' | handle [SYNTAX=INTERACTIVE | BATCH] [ERROR=CONTINUE | STOP] [CD=NO | YES]
       [ENCODING='name'] [PASSWORD='text'].
```

A relative path starts from the folder of the file that holds the INSERT, or from the folder CD named. BATCH: a line that starts in the first column starts a new command, and + - or . there is ignored. CD=YES makes the folder of the file the folder for later relative paths. ENCODING names the encoding of the file, such as 'windows-1257' or '1252'; by default UTF-8 when the file is valid UTF-8, else the system's legacy code page.

Files nest up to five levels; a file may not insert itself. ERROR and PASSWORD are accepted and have no effect: a run stops at its first error.

## Example

```text
INSERT FILE='macros/tables.sps' CD=YES.
```

`statscraft help INSERT` prints the same text in the terminal. [All commands](README.md)
