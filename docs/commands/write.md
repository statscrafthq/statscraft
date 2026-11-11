<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# WRITE

Write the values of variables for every case to a text file.

## Syntax

```text
WRITE [OUTFILE='path'] [ENCODING='name'] [BOM=YES | NO] [RECORDS=n] [TABLE | NOTABLE]
      /[line] item [item ...] [/[line] ...].
```

Items as on PRINT, in the write formats, with no blank between values and no carriage control. System-missing is written as blanks. Without OUTFILE the lines go to the output.

The file is replaced on each data pass; commands that name the same file in one pass write to it in case order. ENCODING: UTF-8 (default, no byte order mark), UTF-16, UTF-16LE, UTF-16BE or a code page such as windows-1252; a UTF encoding given by name gets a byte order mark unless BOM=NO.

## Example

```text
WRITE OUTFILE='scores.txt' /id (F4) score (F6.2).
```

`statscraft help WRITE` prints the same text in the terminal. [All commands](README.md)
