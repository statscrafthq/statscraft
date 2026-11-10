<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SET

Change settings of the session.

## Syntax

```text
SET subcommand[=]value [[/] subcommand[=]value ...].
```

Followed: FORMAT=Fw.d (format of new numeric variables, default F8.2), DECIMAL=DOT \| COMMA (decimal mark when reading text data), MPRINT, MEXPAND, MNEST, MITERATE (see DEFINE), LOCALE, UNICODE (StatsCraft always works in Unicode), PRINTBACK.

Settings for speed, memory and messages (CACHE, THREADS, WORKSPACE, MXWARNS ...) are accepted and have nothing to do. Settings that only change how output looks (TNUMBERS, TVARS, ONUMBERS, OVARS, TLOOK, WIDTH, LENGTH, DIGITGROUPING ...) are accepted with a warning. BLANKS, EPOCH, FUZZBITS, MXLOOPS, RNG, SEED, MTINDEX and SCALEMIN are accepted with a warning that results do not follow them yet. YES and ON, NO and OFF are the same.

## Example

```text
SET FORMAT=F10.3 MPRINT=YES /DECIMAL=DOT.
```

`statscraft help SET` prints the same text in the terminal. [All commands](README.md)
