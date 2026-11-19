# Plugins

A plugin colours the cells of custom tables and adds comments to them. It is a program of its
own, written in any language, that Statscraft starts and talks to in JSON. It is never built
into Statscraft: you choose its executable file in the window.

A plugin gets each table with the statistics behind its cells and answers with styles,
comments and layout: shading cells as a heat map, flagging cells with a small base, adding a
note to a cell, or giving the exports a look of your own.

## Using plugins in the window

- **Edit > Settings > Plugins** lists the installed plugins in the order they run. Each row
  shows the plugin's name and version (or why it cannot be used) beside a box that turns it on
  and off, its full path, **Up** and **Down** to change the order, **Remove**, and
  **Settings...** for its own settings (below). **Add plugin...** adds an executable file to
  the end of the list. Whether a plugin writes comments, in the window and in Excel, is one of
  its own settings.
- **Several plugins**: after a run, every plugin that is on decorates the tables, one after
  the other in the order of the list, each with its own settings. Their answers are merged
  table by table: for a cell, a header cell, the caption and the title, each key the later
  plugin gives (a fill, bold, a text colour, one side of a border, a comment) wins over the
  same key of an earlier one, and the keys it leaves out stay. So a fill from one plugin and
  bold from another both show. The table's keys (font, size, plain look, row labels, frozen
  columns, rows above) go the same way, column widths column by column and header heights line
  by line. A plugin that fails does not stop the others: the status bar says why, and the
  others' answers still apply. On the command line, `statscraft run --xlsx FILE --plugin A
  --plugin B` runs and merges them the same way, but there a plugin that fails fails the run.
- **Settings...** of a plugin opens its Plugin Settings dialog: the settings the plugin
  declares, with their values for the active data. OK runs `DATAFILE ATTRIBUTE` to keep them
  with the data; save the data file
  to take them along to other computers. The window also remembers them for the data file's
  path, so reading the same file again (GET FILE in a syntax, or opening it) before it was
  saved keeps them; attributes the data carries win over the remembered values. A setting
  without a value takes the plugin's default.
- **Sections**: settings with a `group` show under that heading, groups in the order they
  first appear, after the settings without a group. On a wide screen the sections stand in two
  columns; a long list scrolls.
- **Styles**: a setting of kind `style` shows as one row with a sample of the style and
  **Define...**, which opens a small window for its font, size, bold, italic, text colour (or
  none), fill (or no fill) and alignment (or not set). OK there puts the style in the row;
  nothing is kept until OK in Plugin Settings.
- **Presets**: a setting the plugin marks with `"presets": true` has a list of saved values
  beside its field, each shown as `name: value`. Choosing one fills the field; **Remove** takes
  the chosen one off the list; a name and **Save as preset** add the value in the field (a
  preset of the same name is replaced). Presets belong to the window, not to a data file, so
  they are there for every data file; they are kept in the window's settings file as lines
  `preset <attribute>`, a tab, the name, a tab and the value.
- **Templates**: a design template belongs to one plugin, and each plugin has its own active
  template. It is a named set of values for every setting except those the plugin keeps per
  data file (`"per_file": true`, see the protocol below), for example one look
  per client. The **Template** row at the top of the dialog chooses one (or **(none)**), which
  fills the template's settings and leaves the per-file ones as they are; a name and **Save as
  template** keep the current values under that name (a template of the same name is
  replaced), and **Delete** removes the chosen one. OK makes the chosen template the active one
  and keeps any changes to its settings in it; Cancel changes nothing, the template list
  included. While a template is active, its values hold for every data file: they win over the
  data's attributes, and a setting the template lacks takes the data's value or the default.
  Per-file settings come from the data as before. OK still writes every value to the data
  file's attributes, so the data file and the command line see the same values. Templates are
  kept in the window's settings file as lines `plugin-template <plugin>`, a tab, the template's
  name, a tab, the attribute, a tab and the value, and each plugin's active one as
  `plugin-template-active <plugin>`, a tab and the name, where `<plugin>` is the name the
  plugin describes itself with. Lines `template` and `template-active` without the plugin's
  name, from before there were several plugins, are still read: such a template belongs to the
  plugin whose attributes it holds.

- **Updates**: a plugin that offers updates (`"updates": true`, see [Updates](#updates)
  below) has **Check now** in its row. While the Plugins page is open, Statscraft also asks
  each such plugin by itself, at most once a day per plugin, in the background. When a newer
  version exists, the row says "Version X is available" with the plugin's note and an
  **Update** button. Nothing is installed without that button. Update downloads the new
  program, checks it and puts it in the old one's place, asks the plugin for its version again
  ("Updated to X", or why it failed), and decorates every table again. Each check is kept in
  the window's settings file as a line `plugin-checked <seconds since 1970>`, a tab and the
  path, followed, when it found a newer version, by tabs and the version, address, SHA-256 and
  note.

The plugins are kept in the window's settings file in their order, one line each:
`plugin <path>` for one that is on and `plugin-off <path>` for one that is off.

After every run that makes custom tables, the plugins run on the new tables in the background,
and the output window shows their colours when they answer. Hovering over a cell with a small
red corner shows its comment. When a plugin, the order or their settings change, all tables
are decorated again; turning the last plugin off or removing it takes the colours and comments
away. A plugin that fails, gives a broken answer or takes longer than two minutes adds nothing,
and the status bar says why; when every plugin fails the tables stay as they were.

The decorations are saved with the output document (.scout), so the document shows them again
without the plugin. What each place shows:

- **Excel exports**: everything: fills, text colours, bold, italic, alignment, borders, header
  cells, the font and its size for every cell of the table (title, header, body and note) and
  for single cells, the plain look, stacked row labels, column widths, header line heights,
  blank rows above the table and, unless turned off, the comments as Excel notes.
- **Web page exports** and copied tables: the same as styles on the cells, the font on the
  table, the plain look and stacked row labels, and the comment as the cell's tooltip. Column
  widths, row heights and the rows above are left to the browser.
- **The output window**: fills, text colours, bold, italic, borders, header cells and the
  alignment of the numbers. It keeps its own font and size, its own lines and its own layout.

## Settings in the data file

Each setting is kept as a data file attribute named `plugin_<name>_<key>`, where `<name>` is the
plugin's name and `<key>` the setting's key, both in lower case with every character other than
a letter or digit written as `_`, cut to 64 characters. For the example plugin:

```
DATAFILE ATTRIBUTE ATTRIBUTE=plugin_example_comments('No') plugin_example_high('#ed7d31').
```

A style is kept as its JSON text on one line:

```
DATAFILE ATTRIBUTE ATTRIBUTE=plugin_example_largest('{"bold":true,"text":"#1f4e79"}').
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
    {"key": "comments", "label": "Show each cell's base in a comment", "kind": "bool",
     "default": true},
    {"key": "high", "label": "Colour of the largest value", "kind": "colour", "default": "#5b9bd5",
     "group": "Look"},
    {"key": "largest", "label": "Largest value of a row", "kind": "style",
     "default": {"bold": true}, "group": "Look"}
  ]
}
```

`kind` is `number`, `text`, `colour` (written `#rrggbb`), `bool` or `style`. A `number` or
`text` setting may add `"presets": true` for a list of saved values in the dialog (see above).
Any setting may add `"per_file": true` when its value belongs to one data file rather than to a
look, such as the size of the population the sample was drawn from: design templates leave it
out, and it always comes from the data file. Any setting may add `"group"` with the heading
of its section in the dialog. A plugin whose `protocol` is not 1 is refused. A plugin that
answers `--update-check` adds `"updates": true` (see [Updates](#updates)); without it,
Statscraft never asks.

A `style` is a JSON object with these keys, each optional:

```json
{"font": "Verdana", "size": 8, "bold": true, "italic": false, "text": "#1f4e79",
 "fill": "#ddebf7", "align": "center"}
```

`font` is a font name, `size` in points, `text` the text colour and `fill` the background as
`#rrggbb`, `align` is `left`, `center` or `right`. A key left out means the style does not set
it. The keys are those of a decorated cell (below), so a plugin can copy a style into a cell as
it is.

### Decorating tables

Statscraft runs the plugin without arguments, writes one request to its standard input and
closes it. Every table with cell statistics of the batch is in one request. Shortened, for
`CTABLES /TABLE answer [C] [COLPCT] BY group [C] /CATEGORIES VARIABLES=group TOTAL=YES
POSITION=BEFORE` on seven made-up cases:

```json
{
  "protocol": 1,
  "settings": {"comments": true, "low": "#ffffff", "high": "#5b9bd5",
               "largest": {"bold": true}},
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
  come as numbers, yes/no as true or false, text and colours as strings, a style as its object
  (the keys the user set, nothing filled in from the default). A style whose attribute is
  missing or is not a JSON object comes as the default.
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
        {"row": 0, "col": 3, "fill": "#76acdc", "comment": "n = 3"},
        {"row": 0, "col": 4, "text": "#9c0006", "bold": true},
        {"row": 0, "col": 2, "comment": "n = 7"},
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
  (the text's place across the cell; another value is dropped); `font` and `size` for that cell
  alone, over the table's `font` and `font_size`, with the same rules (the output window keeps
  its own).
- `font` and `font_size` (in points, 1 to 409): the font of every cell of the table in the
  exports, header and title included. A font name may have letters, digits, spaces and hyphens,
  at most 31 characters; another is dropped.
- `caption`: the look of the caption under the table in the exports (such as the filter
  note), with the style keys of a cell and no `row`, `col` or `comment`, as in
  `"caption": {"text": "#094e56", "italic": true}`. Without it the caption is plain italic.
- `title`: the look of the table's title above it in the exports, the same way as `caption`.
  Without it the title is bold.
- `plain`, `row_labels`, `column_widths`, `freeze_columns`, `header_heights` and
  `rows_above`: the layout of the exports, see below.

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

These optional keys of a table in the answer change how the Excel and web page exports lay it
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
  fill and does not make it bold, and the Excel sheet hides its grid (with all tables on one
  sheet, when every table there is plain). Number formats and merged cells stay; only the
  plugin's styles show.
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
- `freeze_columns`: the number of columns, from the left of the exported table (after a
  stacked layout dropped its outer column), that Excel keeps in view when scrolling right, as
  with `"freeze_columns": 3` for the labels, the statistic column and the total. More columns
  than the table has are dropped. With all tables on one sheet only columns are kept in view,
  the fewest that any table there asks for, and none when a table or chart there asks for none.
- `header_heights`: Excel row heights in points of the header lines, one number per line from
  line 0, as in `"header_heights": [30, 15]`. Each is kept within 1 to 409; lines without a
  number keep Excel's height, and numbers beyond the last header line are dropped.
- `rows_above`: blank sheet rows above the table, its title (if any) below them, as in
  `"rows_above": 1`. At most 10. With a sheet per table the frozen header lines move down with
  the table; with all tables on one sheet each table gets its own blank rows, after the one
  blank row that always separates two tables.

How cells map with stacked row labels: `col` always counts the columns of the request. The
style and comment of the first row of a run in column 0 go to that run's label row; column 0 of
the other rows of the run is not shown, and nor are its styles and comments. A cell or header
cell in column `c` from 1 on shows in column `c - 1`. A width given for column 0 is dropped;
one for column `c` sets the width of column `c - 1`. In the answer above, row 0 of column 0
styles the label row of the first run, and column 2 (a statistic label column) is 3 characters
wide in the second column of the sheet.

### Updates

Statscraft knows nothing about where a plugin comes from or who may use it: the plugin itself
says where its new version is. A plugin that can tell says `"updates": true` in its
description, and Statscraft then runs `plugin --update-check` (it stops a plugin that takes
longer than 15 seconds). The plugin prints one JSON object on its standard output and exits
with 0:

```json
{"version": "0.3.0", "url": "https://example.com/downloads/my-plugin-0.3.0",
 "sha256": "<the 64 hex digits of the new program's SHA-256>", "notes": "Faster tables"}
```

when a newer version exists, and `{}` when it is up to date. `url` is the new program itself
(the executable file, not an archive) and must start with `https://`; the download and any
redirect stay on https. `notes` is optional, and only its first line is shown. `version` is
compared with the `version` of `--describe` as numbers separated by dots (`1.10` is higher than
`1.9`, and `1.2` is the same as `1.2.0`); only a higher one counts as an update, and one that
is not dotted numbers is refused. When the plugin cannot check (no network, a licence that ran
out), it exits with another code and writes a one-line reason to its standard error. Any
failure, or an answer that breaks these rules, means "no update information", and the row
says why.

How Update installs the new program:

1. It downloads the file to a hidden temporary file in the plugin's own folder.
2. It checks the file's SHA-256 against `sha256`. A file that does not match is deleted.
3. It gives the file the old program's permissions (on Linux and macOS, so it stays
   executable) and renames it to the program's name. On Windows, where a running program
   cannot be replaced, the old program is first renamed to `<name>.old` (an earlier
   `<name>.old` is removed first), and the new one then takes its name; an `.old` that could
   not be removed at once is removed at the next start.

If any step fails, the old program stays where it was. The plugin's folder must be one the
user can write to.

## The example plugin

`examples/plugin-example` is a small plugin in Rust that uses only `serde_json`: a heat map.
It shades every cell of a custom table from the colour of the smallest value to the colour of
the largest, comparing the cells of the same statistic in the table and leaving the totals out.
The largest value of each row takes the style of the setting Largest value of a row (bold
unless changed; its fill stays the shade), and every shaded cell gets a comment with its base.
Build it with:

```
cargo build --release --manifest-path examples/plugin-example/Cargo.toml
```

and add `examples/plugin-example/target/release/statscraft-plugin-example` with **Add
plugin...** in Edit > Settings > Plugins.
