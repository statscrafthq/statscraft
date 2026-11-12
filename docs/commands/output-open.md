<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OUTPUT OPEN

Open a Statscraft output file (.scout) as a new output document.

## Syntax

```text
OUTPUT OPEN FILE='path' [NAME=name] [LOCK=YES | NO] [PASSWORD='text'].
```

Reads the file into a new document, named by NAME or Document1, Document2 and so on, and sends the tables of the commands that follow to it; OUTPUT SAVE then writes old and new together, and OUTPUT EXPORT converts the file to a workbook, document or web page. A name another document has passes to the new one. A file that is missing, is no Statscraft output file or comes from a newer Statscraft gives a warning, and the tables keep going where they went. For a path ending in .spv, .spo or .spw the .scout file of that name is read. LOCK and PASSWORD change nothing.

## Example

```text
OUTPUT OPEN FILE='wave2-results.scout' NAME=wave2.
```

`statscraft help OUTPUT OPEN` prints the same text in the terminal. [All commands](README.md)
