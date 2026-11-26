<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# ADD CASE

A Statscraft extension: one blank case after the last case of the active dataset.

## Syntax

```text
ADD CASE.
```

Not a command of the language: Statscraft adds it so that a value typed under the last case in the window's Data tab has a case to go to. The new case is system-missing in every numeric variable and blank in every string; the transformations waiting for the data run first. A value goes in with IF (\$CASENUM = n), as an edit of any other cell does.

## Example

```text
ADD CASE.
IF ($CASENUM = 4) height = 172.
```

`statscraft help ADD CASE` prints the same text in the terminal. [All commands](README.md)
