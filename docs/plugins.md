# Plugins

A plugin colours the cells of custom tables and adds comments to them. It is a program of its
own, written in any language, that Statscraft starts and talks to in JSON. It is never built
into Statscraft: you choose its executable file in the window.

Typical use: mark the subgroup cells of a crosstab that are clearly above or below the total of
their row, and put a confidence interval in each cell's comment.

## Using a plugin in the window

- **Edit > Settings > Plugins**: choose the plugin's executable file with **Browse...**. The
  line under it shows the plugin's name and version, or why it cannot be used. The box
  **Colour and comment tables with the plugin** turns it on and off, and **Write plugin
  comments as Excel comments** decides whether Excel exports carry the comments as cell notes.
- **Edit > Plugin Settings...** shows the settings the plugin declares, with their values for
  the active data. OK runs `DATAFILE ATTRIBUTE` to keep them with the data; save the data file
  to keep them for the next time it is opened. A setting without a value takes the plugin's
  default.

After every run that makes custom tables, the plugin runs on the new tables in the background,
and the output window shows their colours when it answers. Hovering over a cell with a small
red corner shows its comment. When the plugin or its settings change, all tables are decorated
again. A plugin that fails, gives a broken answer or takes longer than two minutes leaves the
tables as they were, and the status bar says why.

The colours and comments are saved with the output document (.scout), so the document shows
them again without the plugin. Excel exports write the fills, the text colours and (unless
turned off) the comments as Excel notes; web page exports write the colours and the comment as
the cell's tooltip; copying tables keeps the colours.

## Settings in the data file

Each setting is kept as a data file attribute named `plugin_<name>_<key>`, where `<name>` is the
plugin's name and `<key>` the setting's key, both in lower case with every character other than
a letter or digit written as `_`, cut to 64 characters. For the example plugin:

```
DATAFILE ATTRIBUTE ATTRIBUTE=plugin_example_threshold('0.1') plugin_example_higher('#a9ecf5').
```

## The protocol (version 1)

### Describing itself

Statscraft runs `plugin --describe` and reads one JSON document from its standard output (the
example plugin's, shortened):

```json
{
  "name": "Example",
  "version": "0.1.0",
  "protocol": 1,
  "settings": [
    {"key": "threshold", "label": "Smallest difference marked", "kind": "number", "default": 0.05},
    {"key": "higher", "label": "Colour of a higher cell", "kind": "colour", "default": "#c6efce"},
    {"key": "comments", "label": "Write comments", "kind": "bool", "default": true}
  ]
}
```

`kind` is `number`, `text`, `colour` (written `#rrggbb`) or `bool`. A plugin whose `protocol`
is not 1 is refused.

### Decorating tables

Statscraft runs the plugin without arguments, writes one request to its standard input and
closes it. Every table with cell statistics of the batch is in one request. Shortened, for
`CTABLES /TABLE answer [C] [COLPCT] BY group [C] /CATEGORIES VARIABLES=group TOTAL=YES
POSITION=BEFORE` on seven made-up cases:

```json
{
  "protocol": 1,
  "settings": {"threshold": 0.05, "higher": "#c6efce", "lower": "#ffc7ce", "comments": true},
  "tables": [
    {
      "id": 12,
      "title": "Custom Tables",
      "row_headers": 2,
      "columns": ["", "", "Column %", "Column %", "Column %"],
      "column_groups": [["", ""], ["", ""], ["group", "Total"], ["group", "North"], ["group", "South"]],
      "rows": [
        ["answer", "Yes", "42.9%", "66.7%", "25.0%"],
        ["answer", "No", "57.1%", "33.3%", "75.0%"]
      ],
      "meta": {
        "total_rows": [],
        "total_columns": [2],
        "cells": [
          {"row": 0, "col": 2, "kind": "COLPCT.COUNT", "estimate": 0.42857142857142855,
           "base": 7.0, "weighted_base": 7.0, "effective_base": 7.0},
          {"row": 0, "col": 3, "kind": "COLPCT.COUNT", "estimate": 0.6666666666666667,
           "base": 3.0, "weighted_base": 3.0, "effective_base": 3.0}
        ]
      }
    }
  ]
}
```

- `settings`: every declared setting, from the data file attributes or the default. Numbers
  come as numbers, yes/no as true or false, text and colours as strings.
- `rows`: every cell as it is shown, row label columns first. Rows and columns are counted
  from 0, the row label columns included, in `rows`, `meta` and the answer alike.
- `meta.total_rows` and `meta.total_columns`: the rows and columns that hold a total or a
  subtotal. The total of a cell's row is the cell in that row in a total column.
- `meta.cells`: one entry per cell that shows a number with a statistic behind it:
  - `kind`: the summary function as written in the syntax, such as `COLPCT.COUNT`,
    `ROWPCT.COUNT`, `LAYERPCT.COUNT`, `COUNT` or `MEAN`.
  - `estimate`: a percentage as a proportion from 0 to 1; a count or a mean as it is.
  - `base`: the unweighted number of cases the estimate rests on. For a percentage that is the
    base of the percentage (for a column percentage the cases of the column); otherwise the
    cases of the cell.
  - `weighted_base`: the same base with the case weights.
  - `effective_base`: the same base as (sum of weights)^2 / (sum of squared weights). Without
    weights it equals `base`.
  - `sd`: the standard deviation, for a mean only.

  A cell whose count is hidden (HIDESMALLCOUNTS) is left out. The bases of a column percentage
  give the weighted and unweighted size of each column, for a plugin that shows them.

The plugin writes one answer to its standard output and exits with status 0:

```json
{
  "tables": [
    {
      "id": 12,
      "cells": [
        {"row": 0, "col": 3, "fill": "#c6efce", "comment": "+23.8 points from the total"},
        {"row": 0, "col": 4, "fill": "#ffc7ce", "text": "#9c0006"},
        {"row": 0, "col": 2, "comment": "42.9% (13.3% to 72.4%)"}
      ]
    }
  ]
}
```

Every key of a cell but `row` and `col` may be left out. Any cell may be decorated, totals
included. Cells outside the table and colours not written `#rrggbb` are dropped. A table left
out of the answer keeps what it had. Messages on the standard error show in the status bar when
the plugin fails.

## The example plugin

`examples/plugin-example` is a small plugin in Rust that uses only `serde_json`. In every row it
compares each cell with the cell of the same statistic in the first total column: a cell above
the total by more than the threshold gets one fill, a cell below it by more than the threshold
another, and every compared cell gets a comment with the difference. Build it with:

```
cargo build --release --manifest-path examples/plugin-example/Cargo.toml
```

and choose `examples/plugin-example/target/release/statscraft-plugin-example` in Edit > Settings
> Plugins.
