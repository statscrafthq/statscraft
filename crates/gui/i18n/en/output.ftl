# The output window: its toolbar, the outline beside it and the menu of a table or chart.

output-none-yet = No output yet
output-copy = Copy
output-copy-hint = Copy the selected tables with their layout (Ctrl+C)
output-copy-as-text = Copy as text
output-copy-as-text-hint = Tab-separated text, for a spreadsheet
output-delete = Delete
output-delete-hint = Delete the selected tables (Delete)
output-export-hint = Export the selected tables
output-select-all = Select all
output-clear = Clear output
output-selected =
    { $count ->
        [one] 1 table selected
       *[other] { $count } tables selected
    }
output-show-element = Show in the output; Edit > Settings shows or hides whole kinds
output-heading = Heading
output-title = Title
output-notes = Notes
output-run-again = Run again
output-syntax-copied = syntax copied
output-selected-hidden = the selected tables are hidden: nothing copied
output-chart-copied = chart copied: it pastes as an editable chart
output-copied =
    { $count ->
        [one] 1 table copied
       *[other] { $count } tables copied
    }
output-copied-as-text =
    { $count ->
        [one] 1 table copied as text
       *[other] { $count } tables copied as text
    }
output-copied-no-rich-text =
    { $count ->
        [one] 1 table copied as text: the clipboard took no rich text
       *[other] { $count } tables copied as text: the clipboard took no rich text
    }
output-show-chart = Show chart
output-show-data = Show data
output-save-png = Save chart as PNG...
output-save-svg = Save chart as SVG...
output-open-in-powerpoint = Open in PowerPoint
output-open-in-excel = Open in Excel
output-open-in-word = Open in Word
