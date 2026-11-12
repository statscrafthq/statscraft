<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# SET

Change settings of the session.

## Syntax

```text
SET subcommand[=]value [[/] subcommand[=]value ...].
```

Followed: FORMAT=Fw.d (format of new numeric variables, default F8.2), DECIMAL=DOT \| COMMA (decimal mark when reading text data), MPRINT, MEXPAND, MNEST, MITERATE (see DEFINE), LOCALE='name' (sets DECIMAL to the decimal mark of the locale: et\_EE, de\_DE give COMMA, en\_US, en\_GB give DOT; OSLOCALE gives DOT; a later DECIMAL overrides it; files are always Unicode, so a code page in the name changes nothing), UNICODE (Statscraft always works in Unicode; OFF has no effect), PRINTBACK, CCA to CCE='negative prefix,prefix,suffix,negative suffix' (custom currency formats; three commas, or three periods when the decimal mark is a comma; ' before a period or comma makes it text), RNG=MC \| MT (the generator of random numbers), SEED=n \| RANDOM (starts the MC generator again; a whole number from 1 to 1999999999, default 2000000), MTINDEX=x \| RANDOM (starts the MT generator, the Mersenne Twister, again; any number, default 2000000). RANDOM takes the time of day.

MXLOOPS=n: the iterations of a LOOP without an index, and of every loop of MATRIX (default 40). BLANKS=n \| SYSMIS: the value of a completely blank numeric field read from text (DATA LIST, GET DATA, NUMBER); it is not made a missing value. EPOCH=year \| AUTOMATIC: two-digit years in dates and the DATE. functions fall in the 100 years from that year (AUTOMATIC: from 69 years before the current year). FUZZBITS=n (0 to 20, default 6): the fuzzbits of RND and TRUNC without a third argument. SCALEMIN=n (default 24): a new numeric variable read from text with at least n different values is scale. TVARS=NAMES \| LABELS \| BOTH: variables in tables as names, labels (default; the name when there is no label) or both, name then label ("q1 Satisfaction"). TNUMBERS=VALUES \| LABELS \| BOTH: values in tables as values, value labels (default; the value when there is no label) or both, value then label ("1 A great deal"). OVARS and ONUMBERS do the same for the titles of tables.

CTABLES /VLABELS and MULT RESPONSE /FORMAT=NOLABELS win over them. Settings for speed, memory and messages (CACHE, THREADS, WORKSPACE, MXWARNS ...) are accepted and have nothing to do. Settings that only change how output looks (TLOOK, WIDTH, LENGTH, DIGITGROUPING ...) are accepted with a warning. YES and ON, NO and OFF are the same.

## Example

```text
SET FORMAT=F10.3 MPRINT=YES /DECIMAL=DOT TVARS=BOTH TNUMBERS=BOTH.
```

`statscraft help SET` prints the same text in the terminal. [All commands](README.md)
