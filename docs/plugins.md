# Plugins

A plugin colors the cells of custom tables and adds comments to them. For example, it can shade the cells as a heat map, flag cells with a small base, add a note to a cell, or give your Excel exports a look of your own.

A plugin is a separate program. You add it once in the window, and Statscraft runs it after every run that makes custom tables.

## Add a plugin

1. Choose **Edit > Settings > Plugins**.
2. Click **Add plugin...** and choose the plugin's program file.

Each plugin gets a row with its name and version, a box that turns it on and off, its full path, and the buttons **Up**, **Down**, **Remove** and **Settings...**. If a plugin cannot be used, its row says why.

After the next run, the output window shows the colors. A cell with a small red corner has a comment: point at it to read it. When you change a plugin, its order or its settings, every table is decorated again.

## Several plugins

The plugins run in the order of the list, each with its own settings. Where two plugins style the same thing, for example the fill of a cell, the later one wins. Where they style different things, both show: a fill from one plugin and bold text from another.

A plugin that fails, or takes longer than two minutes, adds nothing, and the status bar says why. The other plugins still apply.

## Plugin settings

**Settings...** opens the plugin's own settings for the active data. Settings with a heading of their own are grouped under it.

- Click **OK** to keep the settings with the data. Save the data file to take them to another computer.
- A setting marked **(all data files)** is kept on this computer instead, the last value you chose, for every data file. You can set it with no data open. On the command line, give it with `DATAFILE ATTRIBUTE` in the syntax.
- A setting you leave empty takes the plugin's default.
- A style setting shows a sample, black on white as in Excel, whatever the window's theme. **Define...** sets its font, size, bold, italic, text color, fill and alignment.
- Some settings have **presets**: saved values you can choose from a list. Type a name and click **Save as new** to add one. Choosing a preset fills in its value and its name; change either and click **Save** to keep the changes in that preset, or **Remove** to delete it. Presets belong to the window, so they are there for every data file.

## Design templates

A design template is a named set of settings for one plugin, for example one look per client.

- Choose a template in the **Template** row at the top of Plugin Settings, or **(none)**.
- Type a name and click **Save as template** to keep the current settings. **Delete** removes the chosen template.
- **OK** makes the chosen template active. While it is active, its settings hold for every data file. Settings that belong to one data file still come from the data, and those marked (all data files) from this computer.
- **Export...** writes the chosen template and the plugin's presets to a text file. **Import...** reads such a file, for example one a colleague exported.

## What shows where

Statscraft saves the colors and comments with the output, so a saved output file (.scout) shows them again without the plugin.

| Place | What a plugin can change |
|---|---|
| Excel workbooks | fills, text colors, bold, italic, alignment, borders, fonts and sizes, column widths, row heights, and comments as Excel notes |
| Web pages and copied tables | the same styles, with comments as tooltips; the browser sets widths and heights |
| The output window | fills, text colors, bold, italic, borders and alignment; the window keeps its own font and layout |

Whether a plugin writes comments is one of its own settings.

## Updates

A plugin that offers updates has **Check now** in its row. While the Plugins page is open, Statscraft also checks each such plugin once a day. When a newer version exists, the row says so, with the plugin's note and an **Update** button.

Nothing is installed until you click **Update**. Statscraft then downloads the new program, checks it against the checksum the plugin gave, puts it in place of the old one and decorates the tables again. If any step fails, the old program stays.

## Plugins without the window

On the command line, `--plugin` lets a plugin style the custom tables of a workbook:

```
statscraft run job.sps --xlsx tables.xlsx --plugin /path/to/plugin
```

- The settings come from the active data file, as the window saved them there. To set one in syntax, give it as a data file attribute named `plugin_<name>_<key>` (the plugin's name and the setting's key, in lower case, other characters as `_`), for example `DATAFILE ATTRIBUTE ATTRIBUTE=plugin_example_comments('No').`
- `--plugin-template FILE` applies a template file made with **Export...**, as an active template does in the window.
- Give `--plugin` more than once for several plugins. They run in that order. Unlike in the window, a plugin that fails stops the run.
- `statscraft update --check --plugin PROGRAM` asks a plugin whether it has a newer version. It never installs anything.

AI agents use the same through the MCP server (see [Tables with a plugin](mcp.md#tables-with-a-plugin)).

## Write your own

[Writing a plugin](https://github.com/statscrafthq/statscraft/blob/main/docs/developers/plugins.md) describes the JSON a plugin reads and writes, with a small example plugin in Rust.
