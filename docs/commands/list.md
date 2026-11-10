<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# LIST

List the values of cases.

## Syntax

```text
LIST [[VARIABLES=]varlist] [/CASES=[FROM n] [TO n] [BY n]] [/FORMAT=[NUMBERED | UNNUMBERED]
     [WRAP | SINGLE]].
```

Values show in their print format; system-missing is a period. NUMBERED adds the case number. Follows FILTER and SPLIT FILE.

## Example

```text
LIST VARIABLES=id age /CASES=FROM 1 TO 10 /FORMAT=NUMBERED.
```

`statscraft help LIST` prints the same text in the terminal. [All commands](README.md)
