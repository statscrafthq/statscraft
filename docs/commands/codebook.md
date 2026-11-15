<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# CODEBOOK

Dictionary entries and summary statistics of variables and multiple response sets.

## Syntax

```text
CODEBOOK [varlist] [$set ...] [$ALL]
         [/VARINFO=POSITION LABEL TYPE FORMAT MEASURE ROLE ATTRIBUTES VALUELABELS MISSING
                   RESERVEDATTRIBUTES]
         [/FILEINFO=NAME LOCATION CASECOUNT LABEL ATTRIBUTES DOCUMENTS WEIGHT RESERVEDATTRIBUTES]
         [/STATISTICS=COUNT PERCENT MEAN STDDEV QUARTILES | NONE]
         [/OPTIONS [MAXCATS=200] [VARORDER=FILE | ALPHA | VARLIST | MEASURE | ATTRIBUTE(name)]
                   [SORT=ASCENDING | DESCENDING]].
```

One table per variable and set; without a list every variable in file order, then every set.

\[N\], \[O\] or \[S\] right after a name (after a TO list: its last variable) sets the level that decides its statistics, not the level shown; strings and sets cannot be \[S\]. Nominal and ordinal variables and sets get the count and percent of every value (each labeled value and each value found), scale variables the mean, standard deviation and quartiles plus counts of their labeled values. An unknown level is guessed from the values. Percents are of every case the filter keeps, missing ones included; a set's can add up to more than 100. Above MAXCATS values (scale: labeled values) the values, labels and counts are left out. VARINFO left out means all but RESERVEDATTRIBUTES; ATTRIBUTES shows custom attributes, RESERVEDATTRIBUTES those starting with \$ (both skip names starting with @ or \$@). FILEINFO adds a table about the file; NAME and LOCATION come from the GET FILE or SAVE that last read or wrote it in this run.

VARORDER: ALPHA by name; MEASURE nominal, ordinal, scale, unknown; ATTRIBUTE(name) by the attribute's value, those without it first, then those with an empty one. VARLIST is the default with a list. Every variable or set may be named once. SPLIT FILE is ignored; FILTER and the weight apply.

## Example

```text
CODEBOOK region income [O] $media /VARINFO=LABEL MEASURE VALUELABELS
  /STATISTICS=COUNT MEAN /OPTIONS MAXCATS=12 VARORDER=ALPHA.
```

`statscraft help CODEBOOK` prints the same text in the terminal. [All commands](README.md)
