<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OMSLOG

Keep a log of when OMS requests start and end and which files they write.

## Syntax

```text
OMSLOG FILE='file' [/APPEND=NO | YES] [/FORMAT=XML | TEXT].
```

Each event gets a line (TEXT) or an element (XML) with its name, the tag, the file and the time. A later OMSLOG starts a new log; APPEND=YES with TEXT adds to a file that exists.

OMSEND LOG ends the log.

## Example

```text
OMSLOG FILE='oms_log.txt' /FORMAT=TEXT.
```

`statscraft help OMSLOG` prints the same text in the terminal. [All commands](README.md)
