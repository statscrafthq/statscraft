<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SET

Change settings of the session.

## Syntax

```text
SET subcommand[=]value [[/] subcommand[=]value ...].
```

Followed: FORMAT=Fw.d (format of new numeric variables, default F8.2), DECIMAL=DOT \| COMMA (decimal mark when reading text data), MPRINT, MEXPAND, MNEST, MITERATE (see DEFINE), LOCALE='name' (sets DECIMAL to the decimal mark of the locale: et\_EE, de\_DE give COMMA, en\_US, en\_UK give DOT; OSLOCALE gives DOT; a later DECIMAL overrides it; files are always Unicode, so a code page in the name changes nothing), UNICODE (Statscraft always works in Unicode; OFF has no effect), PRINTBACK, CCA to CCE='negative prefix,prefix,suffix,negative suffix' (custom currency formats; three commas, or three periods when the decimal mark is a comma; ' before a period or comma makes it text), RNG=MC \| MT (the generator of random numbers), SEED=n \| RANDOM (starts the MC generator again; a whole number from 1 to 1999999999, default 2000000), MTINDEX=x \| RANDOM (starts the MT generator, the Mersenne Twister, again; any number, default 2000000). RANDOM takes the time of day.

Settings for speed, memory and messages (CACHE, THREADS, WORKSPACE, MXWARNS ...) are accepted and have nothing to do. Settings that only change how output looks (TNUMBERS, TVARS, ONUMBERS, OVARS, TLOOK, WIDTH, LENGTH, DIGITGROUPING ...) are accepted with a warning. BLANKS, EPOCH, FUZZBITS, MXLOOPS and SCALEMIN are accepted with a warning that results do not follow them yet. YES and ON, NO and OFF are the same.

## Example

```text
SET FORMAT=F10.3 MPRINT=YES /DECIMAL=DOT.
```

`statscraft help SET` prints the same text in the terminal. [All commands](README.md)
