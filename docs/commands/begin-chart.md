<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# BEGIN CHART

A Statscraft extension: a chart given as JSON goes to the output like a procedure's chart.

## Syntax

```text
BEGIN CHART
```

JSON lines END CHART.

Not a command of the language: Statscraft adds it so that an agent or a script can put a chart that no procedure makes into the designated output document, where it is drawn, saved by OUTPUT SAVE and written by OUTPUT EXPORT like any other. BEGIN CHART and END CHART each stand alone on their line; every line between is JSON, read as it is.

The JSON is either a chart as \`statscraft run --json\` writes it (a member of charts), optionally with "version": 1, or a Vega-Lite specification of one view with inline data.values and mark bar, line, area, point or arc, encoded by x, y, color, theta and xOffset or yOffset, without aggregate, bin, timeUnit or transform. Values are taken as they are: a category and series given twice is an error. Fields Statscraft does not know are an error. At most 100 series, 3000 categories, 100000 values and strings of 1000 characters. The MCP tool add\_chart and \`statscraft chart\` write this command.

## Example

```text
BEGIN CHART
{"mark": "bar", "data": {"values": [{"region": "North", "sales": 12}, {"region": "South", "sales": 9}]},
 "encoding": {"x": {"field": "region", "type": "nominal"}, "y": {"field": "sales", "type": "quantitative"}}}
END CHART.
```

`statscraft help BEGIN CHART` prints the same text in the terminal. [All commands](README.md)
