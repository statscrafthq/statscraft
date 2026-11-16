//! A plugin's colours and comments in a workbook: fills and font colours in the styles, and
//! the comments as Excel notes (xl/commentsN.xml with its VML drawing); and the layout a
//! plugin asks for: plain and stacked tables, column widths and alignment, also on a web page.

use statscraft_engine::{Border, Cell, ColumnWidth, Decoration, Table, TableStyle};
use statscraft_export::{Item, write_xlsx};
use std::io::Read;

/// A table of `rows` by `columns` numbers after one row label column, every number decorated.
fn decorated(rows: usize, columns: usize) -> Table {
    let mut table = Table {
        title: "Decorated".into(),
        row_headers: 1,
        columns: (0..=columns).map(|c| format!("c{c}")).collect(),
        rows: (0..rows)
            .map(|r| {
                let label = Cell::Text(format!("row {r}"));
                let numbers = (0..columns).map(|c| Cell::Number((r * columns + c) as f64));
                std::iter::once(label).chain(numbers).collect()
            })
            .collect(),
        ..Table::default()
    };
    let cells = (0..rows).flat_map(|r| (1..=columns).map(move |c| (r, c)));
    let decorations = cells.map(|(row, col)| Decoration {
        row,
        col,
        fill: Some(if col % 2 == 0 { "#a9ecf5" } else { "#df6b00" }.into()),
        text: (col % 2 == 0).then(|| "#094e56".into()),
        comment: Some(format!("cell {row} {col}\nlower 0.1, upper 0.3")),
        ..Decoration::default()
    });
    table.set_decorations(decorations.collect(), TableStyle::default());
    table
}

/// Writes the table and returns the workbook's parts by name.
fn written(test: &str, table: &Table) -> std::collections::HashMap<String, String> {
    let dir = std::env::temp_dir().join(format!("statscraft-plugin-{test}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.xlsx");
    write_xlsx(&path, [Item::Table(table)], false).unwrap();
    let mut zip = zip::ZipArchive::new(std::fs::File::open(&path).unwrap()).unwrap();
    let mut parts = std::collections::HashMap::new();
    for i in 0..zip.len() {
        let mut part = zip.by_index(i).unwrap();
        let mut text = String::new();
        part.read_to_string(&mut text).unwrap();
        parts.insert(part.name().unwrap().to_string(), text);
    }
    std::fs::remove_dir_all(&dir).unwrap();
    parts
}

#[test]
fn fills_text_colours_and_comments_are_written() {
    let parts = written("small", &decorated(2, 2));
    let styles = &parts["xl/styles.xml"];
    assert!(
        styles.contains("FFA9ECF5") && styles.contains("FFDF6B00"),
        "{styles}"
    );
    assert!(styles.contains("FF094E56"), "{styles}");
    let comments = &parts["xl/comments1.xml"];
    assert_eq!(comments.matches("<comment ").count(), 4);
    // B3 is the first number: title row, header row, then the body
    assert!(comments.contains("ref=\"B3\""), "{comments}");
    assert!(comments.contains("cell 0 1"), "{comments}");
    assert!(
        parts
            .keys()
            .any(|k| k.starts_with("xl/drawings/vmlDrawing"))
    );
}

/// 100 000 comments: about 2.5 s in a release build (some 20 s in a debug one), most of it the
/// notes' drawing; run with --ignored --release.
#[test]
#[ignore = "a timing check, for release builds"]
fn a_hundred_thousand_comments_are_written_fast() {
    let table = decorated(1000, 100);
    let dir = std::env::temp_dir().join(format!("statscraft-plugin-large-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("out.xlsx");
    let started = std::time::Instant::now();
    write_xlsx(&path, [Item::Table(&table)], false).unwrap();
    let took = started.elapsed();
    std::fs::remove_dir_all(&dir).unwrap();
    eprintln!("100 000 comments written in {took:?}");
    assert!(took.as_secs_f64() < 5.0, "{took:?}");
}

/// A house style: a serif font, bold and italic text, coloured borders and styled header
/// cells, with 2000 body cells sharing a handful of formats.
#[test]
fn fonts_bold_italic_borders_and_header_cells_are_written() {
    let mut table = decorated(100, 20);
    // one group line above the column titles: header lines 0 and 1
    table.column_groups = (0..=20).map(|c| vec![format!("g{}", c / 10)]).collect();
    let line = |colour: &str| Border {
        bottom: Some(colour.into()),
        left: Some("#7f7f7f".into()),
        ..Border::default()
    };
    let cells = (0..100).flat_map(|r| (0..=20).map(move |c| (r, c)));
    let cells = cells.map(|(row, col)| Decoration {
        row,
        col,
        text: Some("#1f4e5a".into()),
        bold: col == 1,
        italic: row % 2 == 1,
        border: (row % 10 == 9).then(|| line("#4bacc6")),
        ..Decoration::default()
    });
    let header = |row, col| Decoration {
        row,
        col,
        fill: (row == 0).then(|| "#ddebf7".into()),
        bold: true,
        border: (row == 1).then(|| line("#c00000")),
        ..Decoration::default()
    };
    let style = TableStyle {
        font: Some("Cambria".into()),
        font_size: Some(8.0),
        header_cells: (0..2)
            .flat_map(|r| (0..=20).map(move |c| header(r, c)))
            .collect(),
        ..TableStyle::default()
    };
    table.set_decorations(cells.collect(), style);
    assert_eq!(table.style.header_cells.len(), 42);
    let parts = written("house", &table);
    let styles = &parts["xl/styles.xml"];
    for wanted in [
        "<name val=\"Cambria\"/>",
        "<sz val=\"8\"/>",
        "<b/>",
        "<i/>",
        "FF1F4E5A",
        "FFDDEBF7",
        "<bottom style=\"thin\"><color rgb=\"FF4BACC6\"/></bottom>",
        "<bottom style=\"thin\"><color rgb=\"FFC00000\"/></bottom>",
        "<left style=\"thin\"><color rgb=\"FF7F7F7F\"/></left>",
    ] {
        assert!(styles.contains(wanted), "{wanted} in {styles}");
    }
    // the default font only for Excel's own default format
    assert_eq!(
        styles.matches("<name val=\"Calibri\"/>").count(),
        1,
        "{styles}"
    );
    // 2100 body cells and 42 header cells, but only a few distinct looks
    let formats = styles.split("<cellXfs count=\"").nth(1).unwrap();
    let count: usize = formats[..formats.find('"').unwrap()].parse().unwrap();
    assert!(count < 30, "{count} formats");
    // the header cell of the first number column on the group line has the fill: the title
    // takes row 1, so the group line is row 2
    let sheet = &parts["xl/worksheets/sheet1.xml"];
    let b2 = sheet.split("<c r=\"B2\" s=\"").nth(1).unwrap();
    let index: usize = b2[..b2.find('"').unwrap()].parse().unwrap();
    let xf = formats.split("<xf ").nth(index + 1).unwrap();
    let fill: usize = xf.split("fillId=\"").nth(1).unwrap()[..1].parse().unwrap();
    let fills = styles.split("<fill>").nth(fill + 1).unwrap();
    assert!(
        fills.starts_with("<patternFill patternType=\"solid\"><fgColor rgb=\"FFDDEBF7\""),
        "{fills}"
    );
}

/// Two made-up questions stacked in the rows, the statistic labels in a column of their own:
/// row label columns question, answer and statistic, then the two groups.
fn two_questions() -> Table {
    let mut session = statscraft_engine::session::Session::default();
    let source = "DATA LIST LIST /bike car group (3F1).
BEGIN DATA
1 1 1
1 2 1
2 2 2
2 1 2
END DATA.
VARIABLE LABELS bike 'How often do you ride a bike to the shop' car 'Is there a car at home'.
VALUE LABELS bike 1 'Often' 2 'Seldom' /car 1 'Yes' 2 'No'.
CTABLES /TABLE bike [C] [COLPCT] + car [C] [COLPCT] BY group [C] /SLABELS POSITION=ROW.";
    session.run(source, None).unwrap();
    let items = session.output_items("Document1").unwrap();
    statscraft_engine::output_file::leaves(items)[0].1.shown().0
}

/// The house style on `two_questions`: plain, stacked, the statistic column 3 wide, data
/// centred, a fill and comment on the first question's label, comments on a category and a
/// number.
fn house_style(mut table: Table) -> Table {
    let centred = |row, col| Decoration {
        row,
        col,
        align: Some("center".into()),
        ..Decoration::default()
    };
    let mut cells = vec![
        Decoration {
            row: 0,
            col: 0,
            fill: Some("#ddebf7".into()),
            comment: Some("question one".into()),
            ..Decoration::default()
        },
        Decoration {
            row: 1,
            col: 1,
            comment: Some("seldom".into()),
            ..Decoration::default()
        },
        Decoration {
            comment: Some("often in group 1".into()),
            ..centred(0, 3)
        },
    ];
    cells.extend((1..4).map(|r| centred(r, 3)));
    cells.extend((0..4).map(|r| centred(r, 4)));
    let style = TableStyle {
        plain: true,
        row_labels: Some("stacked".into()),
        column_widths: vec![ColumnWidth { col: 2, width: 3.0 }],
        ..TableStyle::default()
    };
    table.set_decorations(cells, style);
    table
}

type Parts = std::collections::HashMap<String, String>;

/// The text of a cell of the first sheet, from the shared strings or the number itself.
fn value(parts: &Parts, place: &str) -> Option<String> {
    let sheet = &parts["xl/worksheets/sheet1.xml"];
    let cell = sheet.split(&format!("<c r=\"{place}\"")).nth(1)?;
    let cell = &cell[..cell.find("</c>").unwrap_or(0)];
    let v = cell.split("<v>").nth(1)?;
    let v = &v[..v.find("</v>").unwrap()];
    if !cell.contains("t=\"s\"") {
        return Some(v.to_string());
    }
    let strings = &parts["xl/sharedStrings.xml"];
    let si = strings.split("<si>").nth(v.parse::<usize>().unwrap() + 1);
    let t = si.unwrap().split('>').nth(1).unwrap();
    Some(t[..t.find('<').unwrap()].to_string())
}

/// The cell format (its xf element) of a cell of the first sheet.
fn xf(parts: &Parts, place: &str) -> String {
    let sheet = &parts["xl/worksheets/sheet1.xml"];
    let cell = sheet
        .split(&format!("<c r=\"{place}\" s=\""))
        .nth(1)
        .unwrap();
    let index: usize = cell[..cell.find('"').unwrap()].parse().unwrap();
    let formats = parts["xl/styles.xml"].split("<cellXfs").nth(1).unwrap();
    let xf = formats.split("<xf ").nth(index + 1).unwrap();
    xf[..xf.find("<xf").unwrap_or(xf.len())].to_string()
}

#[test]
fn stacked_plain_tables_have_the_house_layout() {
    let table = two_questions();
    assert_eq!((table.row_headers, table.columns.len()), (3, 5));
    let table = house_style(table);
    assert_eq!(table.block_starts(), [0, 2]);
    let parts = written("stacked", &table);
    // the title, two header lines (group, its categories), then the body
    let rows = [
        (4, "How often do you ride a bike to the shop", None),
        (5, "Often", Some("Column %")),
        (6, "Seldom", Some("Column %")),
        (7, "Is there a car at home", None),
        (8, "Yes", Some("Column %")),
        (9, "No", Some("Column %")),
    ];
    for (row, a, b) in rows {
        assert_eq!(
            value(&parts, &format!("A{row}")).as_deref(),
            Some(a),
            "A{row}"
        );
        assert_eq!(value(&parts, &format!("B{row}")).as_deref(), b, "B{row}");
    }
    // a block label row is empty to its right, so the label runs on over it
    assert_eq!(value(&parts, "C4"), None);
    assert_eq!(value(&parts, "C5").as_deref(), Some("100"));
    // one column fewer: nothing in E
    let sheet = &parts["xl/worksheets/sheet1.xml"];
    assert!(!sheet.contains("<c r=\"E"), "{sheet}");
    // the header rows frozen, no column; a plain table hides the sheet's grid
    assert!(
        sheet.contains("ySplit=\"3\"") && !sheet.contains("xSplit"),
        "{sheet}"
    );
    assert!(sheet.contains("showGridLines=\"0\""), "{sheet}");
    // the statistic column (2 in the table, B on the sheet) is 3 characters wide
    assert!(
        sheet.contains("<col min=\"2\" max=\"2\" width=\"3.7109375\""),
        "{sheet}"
    );
    // comments on the label row, on a shifted category and on a number
    let comments = &parts["xl/comments1.xml"];
    for (place, text) in [
        ("A4", "question one"),
        ("A6", "seldom"),
        ("C5", "often in group 1"),
    ] {
        let found = comments.split(&format!("ref=\"{place}\"")).nth(1);
        let found = found.and_then(|c| c.split("</comment>").next());
        assert!(
            found.is_some_and(|c| c.contains(text)),
            "{place} {comments}"
        );
    }
    // plain: no grey borders or header fill; the plugin's fill and alignment are there
    let styles = &parts["xl/styles.xml"];
    assert!(
        !styles.contains("FFA6A6A6") && !styles.contains("FFF2F2F2"),
        "{styles}"
    );
    assert!(styles.contains("FFDDEBF7"), "{styles}");
    assert!(
        !xf(&parts, "A4").contains("wrapText"),
        "{}",
        xf(&parts, "A4")
    );
    // stacked category labels do not wrap either: they may run on over empty cells
    assert!(
        !xf(&parts, "A5").contains("wrapText"),
        "{}",
        xf(&parts, "A5")
    );
    for place in ["C5", "D9"] {
        let xf = xf(&parts, place);
        assert!(xf.contains("horizontal=\"center\""), "{place} {xf}");
    }
}

#[test]
fn stacked_tables_on_a_web_page() {
    let table = house_style(two_questions());
    let html = statscraft_engine::table_html(&table, true);
    assert!(html.starts_with("<table class=\"plain\">"), "{html}");
    let label = "<tr><th colspan=\"4\" style=\"background:#ddebf7;\" title=\"question one\">\
                 How often do you ride a bike to the shop</th></tr>\n<tr><th>Often</th>";
    assert!(html.contains(label), "{html}");
    assert!(html.contains("<tr><th colspan=\"4\">Is there a car at home</th></tr>"));
    // the question column is gone from the body and the header
    assert_eq!(html.matches("<td").count(), 8, "{html}");
    assert!(
        !html.contains(">bike<") && !html.contains(">car<"),
        "{html}"
    );
    assert!(html.contains("style=\"text-align:center;\""), "{html}");
}
