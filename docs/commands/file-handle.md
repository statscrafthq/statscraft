<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# FILE HANDLE

Give a file or a folder a short name to use in place of its path.

## Syntax

```text
FILE HANDLE handle /NAME='path' [/MODE=CHARACTER] [/ENCODING='name'] [/RECFORM=...]
            [/LRECL=n].
```

Every command that takes a file (GET, SAVE, DATA LIST, GET DATA, MATCH FILES, ADD FILES, APPLY DICTIONARY, AGGREGATE OUTFILE, INSERT, INCLUDE, CD) takes the handle in place of the path, quoted or not; 'handle/name.sav' starts from the path of a folder handle. A dataset of the same name comes before a handle. A relative NAME starts from the folder of the last CD. ENCODING is the encoding of a text file read through the handle, when the command gives none. RECFORM and LRECL are ignored with a warning; MODE other than CHARACTER (binary and EBCDIC files) is refused. Handles last for the session.

## Example

```text
FILE HANDLE wave2 /NAME='data/2026/wave2.sav'.
GET FILE=wave2.
```

`statscraft help FILE HANDLE` prints the same text in the terminal. [All commands](README.md)
