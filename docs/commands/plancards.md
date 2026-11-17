<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# PLANCARDS

Shows the profiles of a conjoint plan as a list or as one card per profile.

## Syntax

```text
PLANCARDS [/FACTORS=varlist] [/FORMAT=LIST | CARD | BOTH] [/TITLE='line' ...] [/FOOTER='line' ...]
   [/OUTFILE='file'].
```

Shows the profiles of the active dataset, a plan from ORTHOPLAN or typed in, with the factor and value labels: FACTORS picks the factors and their order (default every variable but STATUS\_ and CARD\_; string variables are allowed). LIST (default) is one table per split group, holdouts marked and simulation profiles in a table of their own; CARD is a table per experimental or holdout profile, without split groups; BOTH (or ALL) gives both. TITLE and FOOTER take one or more strings, a line each, and repeat to add lines; on cards )CARD becomes the profile's number. OUTFILE writes the cards to a text file instead of showing them (also with BOTH; ignored with LIST).

## Example

```text
PLANCARDS /FORMAT=CARD /TITLE='Profile )CARD' /FOOTER='Your score from 0 to 10:'.
```

`statscraft help PLANCARDS` prints the same text in the terminal. [All commands](README.md)
