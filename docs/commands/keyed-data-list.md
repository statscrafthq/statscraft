<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# KEYED DATA LIST

Read variables from the record of a text file that a key variable picks, for each case.

## Syntax

```text
KEYED DATA LIST FILE=handle | 'path' KEY=var IN=var [TABLE | NOTABLE] [ENCODING='encoding']
    /var columns [(type[,decimals])] | (formats) ...
```

For every case the record that KEY names is read from the file and its variables are added to the case, as DATA LIST would read them from one line in fixed columns. A numeric KEY reads the record of that number (1 for the first line, a whole number up to the number of lines): the file is direct-access. A string KEY reads the first record that begins with the key, without its trailing blanks: the file is keyed. IN names a variable that is set to 1 when the record was read and to 0 when there is none (a key that is missing, no whole number in range, or matches no record); the variables read keep their value or stay missing then. A variable that exists is given the value of the record for the cases where one is found. The command works on the cases that are there, or inside an input program, where a run reads one record whatever the outcome. Scratch variables may serve as KEY and IN. TABLE and NOTABLE change nothing.

## Example

```text
KEYED DATA LIST FILE=courses KEY=#key IN=#found /period 13 credits 16.
```

`statscraft help KEYED DATA LIST` prints the same text in the terminal. [All commands](README.md)
