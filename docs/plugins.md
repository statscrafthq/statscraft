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
  to take them along to other computers. The window also remembers them for the data file's
  path, so reading the same file again (GET FILE in a syntax, or opening it) before it was
  saved keeps them; attributes the data carries win over the remembered values. A setting
  without a value takes the plugin's default.
- **Presets**: a setting the plugin marks with `"presets": true` has a list of saved values
  beside its field, each shown as `name: value`. Choosing one fills the field; **Remove** takes
  the chosen one off the list; a name and **Save as preset** add the value in the field (a
  preset of the same name is replaced). Presets belong to the window, not to a data file, so
  they are there for every data file; they are kept in the window's settings file as lines
  `preset <attribute>`, a tab, the name, a tab and the value.

After every run that makes custom tables, the plugin runs on the new tables in the background,
and the output window shows their colours when it answers. Hovering over a cell with a small
red corner shows its comment. When the plugin or its settings change, all tables are decorated
again; turning the plugin off or removing it takes its colours and comments away. A plugin that
fails, gives a broken answer or takes longer than two minutes leaves the tables as they were,
and the status bar says why.

The decorations are saved with the output document (.scout), so the document shows them again
without the plugin. What each place shows:

- **Excel exports**: everything: fills, text colours, bold, italic, alignment, borders, header
  cells, the font and its size for every cell of the table (title, header, body and note), the
  plain look, stacked row labels, column widths and, unless turned off, the comments as Excel
  notes.
- **Web page exports** and copied tables: the same as styles on the cells, the font on the
  table, the plain look and stacked row labels, and the comment as the cell's tooltip. Column
  widths are left to the browser.
- **The output window**: fills, text colours, bold, italic, borders, header cells and the
  alignment of the numbers. It keeps its own font and size, its own lines and its own layout.

## Settings in the data file

Each setting is kept as a data file attribute named `plugin_<name>_<key>`, where `<name>` is the
plugin's name and `<key>` the setting's key, both in lower case with every character other than
a letter or digit written as `_`, cut to 64 characters. For the example plugin:

```
DATAFILE ATTRIBUTE ATTRIBUTE=plugin_example_threshold('0.1') plugin_example_higher('#a9ecf5').
```

A setting with presets also keeps the name of the preset its value came from, in the attribute
`plugin_<name>_<key>_preset` (the setting's attribute cut to 57 characters, then `_preset`), so
the next time the dialog shows that preset chosen. The name is empty when the value was typed
or changed after choosing. The plugin gets only the value.

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

`kind` is `number`, `text`, `colour` (written `#rrggbb`) or `bool`. A `number` or `text`
setting may add `"presets": true` for a list of saved values in the dialog (see above). A
plugin whose `protocol` is not 1 is refused.

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
      "header_rows": 3,
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
- `header_rows`: the number of column header lines above the body (see Header lines below).
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
      "font": "Cambria",
      "font_size": 9,
      "cells": [
        {"row": 0, "col": 3, "fill": "#c6efce", "comment": "+23.8 points from the total"},
        {"row": 0, "col": 4, "text": "#9c0006", "bold": true},
        {"row": 0, "col": 2, "comment": "42.9% (13.3% to 72.4%)"},
        {"row": 1, "col": 0, "italic": true, "border": {"bottom": "#2e75b6"}}
      ],
      "header_cells": [
        {"row": 2, "col": 2, "fill": "#ddebf7", "bold": true, "border": {"bottom": "#2e75b6"}}
      ]
    }
  ]
}
```

- `cells`: body cells, counted as in the request; the row label cells may be styled too.
- `header_cells`: cells of the column header lines (below), with the same keys as `cells` but
  no `comment`.
- Style keys of a cell, each optional: `fill` (background) and `text` (text colour) as
  `#rrggbb`; `bold` and `italic` as true; `border` with any of `top`, `bottom`, `left` and
  `right`, each a thin line in the colour `#rrggbb`; `align` as `left`, `center` or `right`
  (the text's place across the cell; another value is dropped).
- `font` and `font_size` (in points, 1 to 409): the font of every cell of the table in the
  exports, header and title included. A font name may have letters, digits, spaces and hyphens,
  at most 31 characters; another is dropped.
- `plain`, `row_labels` and `column_widths`: the layout of the exports, see below.

Every key but `id`, and of a cell but `row` and `col`, may be left out. Any cell may be
decorated, totals included. Cells outside the body or the header lines and colours not written
`#rrggbb` are dropped. A table left out of the answer keeps what it had; a table in the answer
gets exactly what the answer gives it. Messages on the standard error show in the status bar
when the plugin fails.

#### Header lines

The column header has `header_rows` lines, counted from 0 at the top. A table whose
`column_groups` are all empty has one line: line 0 holds the column titles (`columns`). A table
with column groups has one line per group level and then the line of column titles: with groups
of at most `k` levels, line `i` (from 0 to `k - 1`) holds entry `i` of each column's
`column_groups` (empty where a column has fewer), and line `k` holds `columns`. So
`header_rows` is `k + 1`. The title of the table is not a header line. In the request above,
line 0 holds `group`, line 1 `Total`, `North` and `South`, and line 2 `Column %`.

A header cell's `col` counts columns as the body does, the row label columns included.
Neighbouring columns with the same label under the same labels above it are shown as one
merged cell, and Excel also merges the header lines above each row label column into one cell.
A merged cell takes its fill, text colour, bold and italic from its top left place; in Excel and
the window each place keeps its own borders, and on a web page the top and left border come
from the top left place and the bottom and right border from the bottom right place. The same
holds for a row label merged over the rows that repeat it.

#### Layout of the exports

Three optional keys of a table in the answer change how the Excel and web page exports lay it
out. The output window ignores them.

```json
{
  "id": 12,
  "plain": true,
  "row_labels": "stacked",
  "column_widths": [{"col": 1, "width": 30}, {"col": 2, "width": 3}],
  "cells": [
    {"row": 0, "col": 3, "align": "center"},
    {"row": 0, "col": 0, "bold": true, "border": {"top": "#2e75b6"}}
  ]
}
```

- `plain: true`: Statscraft draws none of its own lines around the cells, gives the header no
  fill and does not make it bold. Number formats and merged cells stay; only the plugin's styles
  show.
- `row_labels: "stacked"`: a compact form of the row labels for tables with two row label
  columns or more. Column 0, the outer row label (in a custom table the variable label,
  repeated down its categories), leaves the table. Each run of rows that share the same text in
  column 0 gets a row of its own above it, with that text in the first column and nothing to its
  right, so a long label runs on across the empty cells beside it (it is not wrapped). The rows
  of the run follow, every column moved one to the left: the category labels are in the first
  column, under the label row. Excel freezes only the header lines then, not the label columns.
  Any other value is dropped, and a table with one row label column keeps its layout.
- `column_widths`: Excel column widths in characters, kept within 1 to 255. A width for a
  column off the table is dropped. When two tables share a sheet, the wider need of a column
  wins.

How cells map with stacked row labels: `col` always counts the columns of the request. The
style and comment of the first row of a run in column 0 go to that run's label row; column 0 of
the other rows of the run is not shown, and nor are its styles and comments. A cell or header
cell in column `c` from 1 on shows in column `c - 1`. A width given for column 0 is dropped;
one for column `c` sets the width of column `c - 1`. In the answer above, row 0 of column 0
styles the label row of the first run, and column 2 (a statistic label column) is 3 characters
wide in the second column of the sheet.

## The example plugin

`examples/plugin-example` is a small plugin in Rust that uses only `serde_json`. In every row it
compares each cell with the cell of the same statistic in the first total column: a cell above
the total by more than the threshold gets one fill, a cell below it by more than the threshold
another, both in bold, and every compared cell gets a comment with the difference. Build it
with:

```
cargo build --release --manifest-path examples/plugin-example/Cargo.toml
```

and choose `examples/plugin-example/target/release/statscraft-plugin-example` in Edit > Settings
> Plugins.
