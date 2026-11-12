<!-- Generated from the command table by crates/syntax/tests/command_docs.rs. Do not edit. -->

# OUTPUT EXPORT

Write the tables and charts of an output document to a workbook, document, presentation, web page, text or CSV file.

## Syntax

```text
OUTPUT EXPORT [NAME=* | name] [/CONTENTS [EXPORT=ALL | VISIBLE] [LAYERS=...] [MODELVIEWS=...]]
   [/XLSX DOCUMENTFILE='path' [OPERATION=CREATEFILE | CREATESHEET | MODIFYSHEET] [SHEET='name']
          [LOCATION=LASTCOLUMN | LASTROW | STARTCELL('B2')] [NOTESCAPTIONS=YES | NO]]
   [/DOC DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO] [WIDETABLES=...] [PAGESIZE=...]
          [TOPMARGIN=...] [BOTTOMMARGIN=...] [LEFTMARGIN=...] [RIGHTMARGIN=...] [PAGEBREAKS=...]]
   [/PPT DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO] [SLIDETITLES=YES | NO] [WIDETABLES=...]
          [SLIDESIZE=...] [TOPMARGIN=...] [BOTTOMMARGIN=...] [LEFTMARGIN=...] [RIGHTMARGIN=...]]
   [/HTML DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO] [STYLING=YES | NO]
          [IMAGEFORMAT=JPG | BMP | PNG] [INTERACTIVELAYERS=NO | YES]]
   [/TEXT DOCUMENTFILE='path' [ENCODING=LOCALE | UTF8 | UTF16] [NOTESCAPTIONS=YES | NO]
          [COLUMNWIDTH=AUTOFIT | n] [SEPARATOR=SPACE | TAB] [ROWBORDER='-'] [COLUMNBORDER='|']
          [IMAGEFORMAT=JPG | BMP | EMF | EPS | PNG | TIF]]
   [/CSV DOCUMENTFILE='path' [NOTESCAPTIONS=YES | NO]]
   [/PNG [IMAGEROOT='root'] [PERCENTSIZE=n] [DEPTH=SCREEN | BLACKWHITE | GRAYS256 | ...]]
   [/JPG [IMAGEROOT='root'] [PERCENTSIZE=n] [GRAYSCALE=NO | YES]]
   [/BMP [IMAGEROOT='root'] [PERCENTSIZE=n] [COMPRESS=NO | YES]]
   [/TIF [IMAGEROOT='root'] [PERCENTSIZE=n]] [/EMF ...] [/EPS ...].
```

Writes the tables and charts that the designated document (NAME=\*) or the named one holds so far, in order; an existing file is replaced. Subcommands and keywords are spelled in full.

CONTENTS EXPORT=VISIBLE leaves out what OUTPUT MODIFY hid; every table has one layer, so LAYERS and MODELVIEWS change nothing. NOTESCAPTIONS=NO leaves out the notes under the tables. XLSX puts the tables one below the other on one sheet, named by SHEET or Tables, laid out as the Excel export of the window. CREATESHEET and MODIFYSHEET add to a workbook that OUTPUT EXPORT wrote earlier in the session and that has not changed since; another workbook is refused, so its formulas and formatting cannot be lost. CREATESHEET replaces or adds the sheet; MODIFYSHEET places the tables after the last column or row of the sheet, one column or row apart, or from STARTCELL. A bar, pie, line or area chart goes to the sheet as its data table with an editable chart beside it, drawn in the theme's colors.

DOC writes a Word document in the .docx format, whatever the file is called: each table under its title, each chart as an editable chart whose numbers Edit Data opens. PPT writes a presentation in the .pptx format with one slide per table or chart; SLIDETITLES=NO leaves the slide titles out, and a chart then shows its own. Page and slide sizes, margins, WIDETABLES and PAGEBREAKS are accepted and not applied. Other charts, such as a histogram or a boxplot, go in as a picture; on a sheet the picture stands beside the data table. HTML writes one web page with every chart in it as a picture, JPG unless IMAGEFORMAT names another; STYLING=NO leaves out the style sheet. TEXT lines the columns up between the borders, or with SEPARATOR=TAB separates the cells by tabs; COLUMNWIDTH=n wraps longer cells; LOCALE and UTF8 write UTF-8, UTF16 writes UTF-16 with a byte order mark. CSV, a Statscraft addition, writes each table as comma-separated values after its title, with a blank line between tables.

TEXT saves each chart as a picture file in the folder of the text, named after the text file with a number from 0 (out.txt gives out0.jpg, out1.jpg, ...), and writes the chart's title and that file name where the chart stands. CSV shows a chart as its data table.

With no document subcommand, each of /PNG, /JPG, /BMP and /TIF saves every chart in its format: IMAGEROOT='charts/june' gives charts/june0.png, charts/june1.png and so on. With a document subcommand IMAGEROOT is not used, and the subcommand of the format in use sets the pictures' size and colors. PERCENTSIZE (above 0, at most 200) scales a picture: 100 is 1280 pixels wide, the PNG of --charts. GRAYSCALE=YES, DEPTH=GRAYS256 and DEPTH=BLACKWHITE drop the color; the other depths and COMPRESS change nothing. EMF and EPS are not written yet: a warning, and their charts stay data tables, as does a kind that is not drawn yet, such as a stem-and-leaf display. /PDF, /XLS and /XLSM are refused.

## Example

```text
OUTPUT EXPORT /XLSX DOCUMENTFILE='results.xlsx' SHEET='Region 1'.
```

`statscraft help OUTPUT EXPORT` prints the same text in the terminal. [All commands](README.md)
