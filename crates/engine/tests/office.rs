//! Charts as office files: the workbook, presentation, document and clipboard package hold the
//! expected parts, the chart parts carry no colors, and the embedded workbooks hold the
//! numbers. Made-up data: drinks ordered at a campus cafe, by cup size.
//!
//! With STATSCRAFT_OFFICE_OUT set to a folder, the files are also written there, for the check
//! by hand in the office programs.

use calamine::{Data, Reader};
use statscraft_engine::Chart;
use statscraft_engine::office::{
    Item, chart_workbook, clipboard_package, document, is_office_chart, presentation,
};
use statscraft_engine::session::Session;
use std::collections::BTreeSet;
use std::io::{Cursor, Read};

const DATA: &str = "DATA LIST LIST /drink cup (F1).
BEGIN DATA
1 1
1 2
1 2
2 1
3 2
1 1
2 2
3 1
1 2
2 1
END DATA.
VALUE LABELS drink 1 'Espresso' 2 'Green tea' 3 'Cocoa' /cup 1 'Small' 2 'Large'.
";

/// The first bytes of every PNG file.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\n";

fn charts(source: &str) -> Vec<Chart> {
    let result = Session::default()
        .run(&format!("{DATA}{source}"), None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    result.commands.into_iter().flat_map(|c| c.charts).collect()
}

fn names(zip: &[u8]) -> BTreeSet<String> {
    let archive = zip::ZipArchive::new(Cursor::new(zip)).expect("a zip file");
    let names = archive
        .file_names()
        .map(|n| n.expect("a part name").to_string());
    names.collect()
}

fn bytes(zip: &[u8], name: &str) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
    let mut out = Vec::new();
    archive
        .by_name(name)
        .unwrap_or_else(|e| panic!("{name}: {e}"))
        .read_to_end(&mut out)
        .unwrap();
    out
}

fn text(zip: &[u8], name: &str) -> String {
    String::from_utf8(bytes(zip, name)).unwrap()
}

/// The cells of the first sheet of a workbook, as text, row by row.
fn cells(workbook: &[u8]) -> Vec<Vec<String>> {
    let mut book = calamine::Xlsx::new(Cursor::new(workbook.to_vec())).unwrap();
    let range = book.worksheet_range_at(0).unwrap().unwrap();
    range
        .rows()
        .map(|row| {
            row.iter()
                .map(|cell| match cell {
                    Data::Float(f) => f.to_string(),
                    Data::Int(i) => i.to_string(),
                    Data::String(s) => s.clone(),
                    _ => String::new(),
                })
                .collect()
        })
        .collect()
}

/// A chart part for the office programs: the pass's additions are there and no color is set.
fn check_part(part: &str, size: u32) {
    for color in ["Clr ", "Clr>", "solidFill", "gradFill", "pattFill"] {
        assert!(!part.contains(color), "a color in the chart part: {color}");
    }
    assert!(part.contains("<c:roundedCorners val=\"0\"/>"));
    assert!(part.contains("<c:overlay val=\"0\"/>"));
    assert!(part.contains(&format!("<a:defRPr sz=\"{size}\"/>")));
    assert!(part.contains("<c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/>"));
    if part.contains("<c:barChart>") {
        assert!(part.contains("<c:varyColors val=\"0\"/>"));
        assert_eq!(part.matches("<c:delete val=\"0\"/>").count(), 2);
        assert_eq!(part.matches("<c:majorTickMark val=\"out\"/>").count(), 2);
        assert_eq!(part.matches("<c:minorTickMark val=\"none\"/>").count(), 2);
    }
}

fn keep(name: &str, bytes: &[u8]) {
    if let Ok(dir) = std::env::var("STATSCRAFT_OFFICE_OUT") {
        std::fs::write(std::path::Path::new(&dir).join(name), bytes).unwrap();
    }
}

#[test]
fn chart_workbook_holds_the_data_and_the_chart() {
    let chart = &charts("CROSSTABS drink BY cup /BARCHART.")[0];
    let book = chart_workbook(chart).unwrap();
    keep("crosstabs-bar.xlsx", &book);
    let parts = names(&book);
    assert!(parts.contains("xl/charts/chart1.xml"));
    assert!(parts.contains("xl/drawings/drawing1.xml"));
    let cells = cells(&book);
    assert_eq!(cells[0][0], chart.title);
    // the title, the legend over the series, the series names, then a row per drink
    assert_eq!(cells[2][1..3], ["Small", "Large"]);
    assert_eq!(cells[3][..3], ["Espresso", "2", "3"]);
    assert_eq!(cells[4][..3], ["Green tea", "2", "1"]);
    assert_eq!(cells[5][..3], ["Cocoa", "1", "1"]);
    // the series point at those cells
    let part = text(&book, "xl/charts/chart1.xml");
    assert!(
        part.contains("<c:f>'drink by cup'!$A$4:$A$6</c:f>"),
        "{part}"
    );
    assert!(part.contains("<c:f>'drink by cup'!$B$4:$B$6</c:f>"));
    assert!(part.contains("<c:majorTickMark val=\"out\"/>"));
}

#[test]
fn error_bars_read_their_half_lengths_from_the_data_table() {
    let source = "COMPUTE spend = drink * 2 + cup.
UNIANOVA spend BY cup /PLOT=PROFILE(cup) ERRORBAR=SE.";
    let chart = &charts(source)[0];
    let bars = chart.error_bars.as_ref().unwrap();
    let book = chart_workbook(chart).unwrap();
    keep("profile-error-bars.xlsx", &book);
    let cells = cells(&book);
    // the category, the mean, then the half length under what it spans
    assert_eq!(cells[1][2], "Error bar \u{b1} 2 SE");
    let margin: f64 = cells[3][2].parse().unwrap();
    assert!((margin - bars.margins[0][0]).abs() < 1e-9);
    let part = text(&book, "xl/charts/chart1.xml");
    assert!(part.contains("<c:errBars>"), "{part}");
    assert!(part.contains("<c:errValType val=\"cust\"/>"));
    assert_eq!(
        part.matches("!$C$4:$C$5</c:f>").count(),
        2,
        "plus and minus: {part}"
    );
}

#[test]
fn uneven_error_bars_take_their_length_below_from_a_column_of_its_own() {
    let chart = &charts("GRAPH /BAR=MEDIAN(drink) BY cup /INTERVAL CI(90).")[0];
    let book = chart_workbook(chart).unwrap();
    let cells = cells(&book);
    assert_eq!(cells[1][2], "Error bar above, 90% CI");
    assert_eq!(cells[1][3], "Error bar below, 90% CI");
    let part = text(&book, "xl/charts/chart1.xml");
    assert!(part.contains("<c:errValType val=\"cust\"/>"), "{part}");
    assert_eq!(part.matches("!$C$4:$C$5</c:f>").count(), 1, "plus: {part}");
    assert_eq!(part.matches("!$D$4:$D$5</c:f>").count(), 1, "minus: {part}");
}

#[test]
fn graph_charts_as_office_charts() {
    let chart = |source: &str| charts(source).remove(0);
    // a Pareto chart: columns, and the cumulative line on the second axis from 0 to 100
    let pareto = chart("GRAPH /PARETO=drink.");
    let book = chart_workbook(&pareto).unwrap();
    keep("pareto.xlsx", &book);
    let part = text(&book, "xl/charts/chart1.xml");
    assert!(
        part.contains("<c:barChart>") && part.contains("<c:lineChart>"),
        "{part}"
    );
    assert_eq!(part.matches("<c:valAx>").count(), 2, "{part}");
    assert!(part.contains("<c:max val=\"100\"/>"), "{part}");
    // error bars: points without a line, with their bars
    let errors = chart("GRAPH /ERRORBAR(STERROR 1)=cup BY drink.");
    let part = text(&chart_workbook(&errors).unwrap(), "xl/charts/chart1.xml");
    assert!(
        part.contains("<c:lineChart>") && part.contains("<c:errBars>"),
        "{part}"
    );
    assert!(part.contains("<a:ln><a:noFill/></a:ln>"), "{part}");
    // drop lines and high-low charts: high-low lines; one pair of range bars: up and down bars
    let drop = chart("GRAPH /LINE(DROP)=MIN(cup) MAX(cup) BY drink.");
    let part = text(&chart_workbook(&drop).unwrap(), "xl/charts/chart1.xml");
    assert!(part.contains("<c:hiLowLines"), "{part}");
    let range = chart("GRAPH /BAR(RANGE)=MIN(cup) MAX(cup) BY drink.");
    let part = text(&chart_workbook(&range).unwrap(), "xl/charts/chart1.xml");
    assert!(part.contains("<c:upDownBars"), "{part}");
    // a difference area goes in as a picture
    let difference = chart("GRAPH /LINE(DIFFERENCE)=COUNT BY drink BY cup.");
    assert!(!is_office_chart(&difference));
    assert!(names(&chart_workbook(&difference).unwrap()).contains("xl/media/image1.png"));
}

#[test]
fn presentation_has_a_slide_per_item_and_each_chart_its_workbook() {
    let all = charts(
        "FREQUENCIES drink /BARCHART.\nFREQUENCIES cup /PIECHART.\nFREQUENCIES drink /HISTOGRAM /FORMAT=NOTABLE.",
    );
    assert_eq!(all.len(), 3);
    assert!(is_office_chart(&all[0]) && is_office_chart(&all[1]));
    assert!(
        !is_office_chart(&all[2]),
        "a histogram is a statistical plot"
    );
    let table = all[0].data_table();
    let items = [
        Item::Chart(&all[0]),
        Item::Chart(&all[1]),
        Item::Chart(&all[2]),
        Item::Table(&table),
    ];
    let deck = presentation(&items, true, true).unwrap();
    keep("charts.pptx", &deck);
    let parts = names(&deck);
    let want: BTreeSet<String> = [
        "[Content_Types].xml",
        "_rels/.rels",
        "ppt/presentation.xml",
        "ppt/_rels/presentation.xml.rels",
        "ppt/presProps.xml",
        "ppt/slideMasters/slideMaster1.xml",
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        "ppt/slideLayouts/slideLayout1.xml",
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        "ppt/theme/theme1.xml",
        "ppt/slides/slide1.xml",
        "ppt/slides/_rels/slide1.xml.rels",
        "ppt/slides/slide2.xml",
        "ppt/slides/_rels/slide2.xml.rels",
        "ppt/slides/slide3.xml",
        "ppt/slides/_rels/slide3.xml.rels",
        "ppt/slides/slide4.xml",
        "ppt/slides/_rels/slide4.xml.rels",
        "ppt/charts/chart1.xml",
        "ppt/charts/_rels/chart1.xml.rels",
        "ppt/charts/chart2.xml",
        "ppt/charts/_rels/chart2.xml.rels",
        "ppt/embeddings/Microsoft_Excel_Worksheet1.xlsx",
        "ppt/embeddings/Microsoft_Excel_Worksheet2.xlsx",
        "ppt/media/image1.png",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(parts, want);
    assert!(bytes(&deck, "ppt/media/image1.png").starts_with(PNG));
    assert!(text(&deck, "[Content_Types].xml").contains("Extension=\"png\""));
    for n in 1..=2 {
        let part = text(&deck, &format!("ppt/charts/chart{n}.xml"));
        check_part(&part, 1400);
        // the slide title shows the chart's title, so the chart has none of its own
        assert!(part.contains("<c:autoTitleDeleted val=\"1\"/>"));
    }
    assert!(text(&deck, "ppt/charts/chart2.xml").contains("<c:pieChart>"));
    let slide = text(&deck, "ppt/slides/slide1.xml");
    assert!(slide.contains(&format!("<a:t>{}</a:t>", all[0].title)));
    assert!(slide.contains("r:id=\"rId2\""));
    // the histogram is a picture, and the plain table a table
    let slide = text(&deck, "ppt/slides/slide3.xml");
    assert!(
        slide.contains("<p:pic>") && !slide.contains("<a:tbl>"),
        "{slide}"
    );
    assert!(
        text(&deck, "ppt/slides/_rels/slide3.xml.rels")
            .contains("relationships/image\" Target=\"../media/image1.png\"")
    );
    assert!(text(&deck, "ppt/slides/slide4.xml").contains("<a:t>Espresso</a:t>"));
    let book = bytes(&deck, "ppt/embeddings/Microsoft_Excel_Worksheet1.xlsx");
    let cells = cells(&book);
    let row = cells
        .iter()
        .find(|r| r[0] == "Espresso")
        .expect("a row per drink");
    assert_eq!(row[1], "5");
    let pie = bytes(&deck, "ppt/embeddings/Microsoft_Excel_Worksheet2.xlsx");
    let cells = self::cells(&pie);
    let row = cells
        .iter()
        .find(|r| r[0] == "Large")
        .expect("a slice per cup");
    assert_eq!(row[1], "5");
    let rels = text(&deck, "ppt/charts/_rels/chart1.xml.rels");
    assert!(rels.contains(
        "relationships/package\" Target=\"../embeddings/Microsoft_Excel_Worksheet1.xlsx\""
    ));
}

#[test]
fn without_slide_titles_a_chart_keeps_its_own() {
    let chart = &charts("FREQUENCIES drink /BARCHART.")[0];
    let deck = presentation(&[Item::Chart(chart)], true, false).unwrap();
    keep("charts-no-titles.pptx", &deck);
    assert!(!text(&deck, "ppt/slides/slide1.xml").contains("<p:ph type=\"title\"/>"));
    let part = text(&deck, "ppt/charts/chart1.xml");
    assert!(part.contains(&format!("<a:t>{}</a:t>", chart.title)));
}

#[test]
fn document_holds_tables_and_charts_with_their_workbooks() {
    let all = charts("CROSSTABS drink BY cup /BARCHART.\nFREQUENCIES cup /PIECHART.");
    let table = all[0].data_table();
    let doc = document(
        &[
            Item::Table(&table),
            Item::Chart(&all[0]),
            Item::Chart(&all[1]),
        ],
        true,
    )
    .unwrap();
    keep("charts.docx", &doc);
    let parts = names(&doc);
    let want: BTreeSet<String> = [
        "[Content_Types].xml",
        "_rels/.rels",
        "word/document.xml",
        "word/_rels/document.xml.rels",
        "word/charts/chart1.xml",
        "word/charts/_rels/chart1.xml.rels",
        "word/charts/chart2.xml",
        "word/charts/_rels/chart2.xml.rels",
        "word/embeddings/Microsoft_Excel_Worksheet1.xlsx",
        "word/embeddings/Microsoft_Excel_Worksheet2.xlsx",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(parts, want);
    let body = text(&doc, "word/document.xml");
    assert_eq!(body.matches("<w:tbl>").count(), 1);
    assert_eq!(body.matches("<wp:inline ").count(), 2);
    assert!(
        body.contains("<w:gridSpan w:val=\"2\"/>"),
        "the legend spans its series"
    );
    check_part(&text(&doc, "word/charts/chart1.xml"), 1000);
    let cells = cells(&bytes(
        &doc,
        "word/embeddings/Microsoft_Excel_Worksheet1.xlsx",
    ));
    assert!(cells.iter().any(|r| r[..3] == ["Cocoa", "1", "1"]));
}

#[test]
fn clipboard_package_has_seven_parts_and_no_theme() {
    let chart = &charts("CROSSTABS drink BY cup /BARCHART.")[0];
    let package = clipboard_package(chart).unwrap().expect("an office chart");
    keep("clipboard-package.zip", &package);
    let parts = names(&package);
    let want: BTreeSet<String> = [
        "[Content_Types].xml",
        "_rels/.rels",
        "clipboard/drawings/drawing1.xml",
        "clipboard/drawings/_rels/drawing1.xml.rels",
        "clipboard/charts/chart1.xml",
        "clipboard/charts/_rels/chart1.xml.rels",
        "clipboard/embeddings/Microsoft_Excel_Worksheet.xlsx",
    ]
    .into_iter()
    .map(String::from)
    .collect();
    assert_eq!(parts, want);
    check_part(&text(&package, "clipboard/charts/chart1.xml"), 1400);
    let drawing = text(&package, "clipboard/drawings/drawing1.xml");
    assert!(drawing.contains("<lc:lockedCanvas"));
    assert!(
        drawing.contains("</a:graphic><a:xfrm>"),
        "the frame's place after its graphic"
    );
    let cells = cells(&bytes(
        &package,
        "clipboard/embeddings/Microsoft_Excel_Worksheet.xlsx",
    ));
    assert!(cells.iter().any(|r| r[..3] == ["Espresso", "2", "3"]));
    let histogram = &charts("FREQUENCIES drink /HISTOGRAM /FORMAT=NOTABLE.")[0];
    assert!(clipboard_package(histogram).unwrap().is_none());
}

#[test]
fn a_statistical_plot_goes_in_as_a_picture() {
    let boxplot = &charts("EXAMINE VARIABLES=drink /PLOT=BOXPLOT /STATISTICS=NONE.")
        .into_iter()
        .find(|c| c.kind == statscraft_engine::chart::ChartKind::Boxplot)
        .expect("a boxplot");
    assert!(!is_office_chart(boxplot));
    // the document: the picture and its relationship, no chart part and no table
    let doc = document(&[Item::Chart(boxplot)], true).unwrap();
    keep("boxplot.docx", &doc);
    let parts = names(&doc);
    assert!(parts.contains("word/media/image1.png"), "{parts:?}");
    assert!(!parts.iter().any(|p| p.starts_with("word/charts/")));
    assert!(bytes(&doc, "word/media/image1.png").starts_with(PNG));
    let body = text(&doc, "word/document.xml");
    assert!(
        body.contains("<pic:pic ") && !body.contains("<w:tbl>"),
        "{body}"
    );
    assert!(
        text(&doc, "word/_rels/document.xml.rels")
            .contains("relationships/image\" Target=\"media/image1.png\"")
    );
    // the workbook: the data table with the picture beside it
    let book = chart_workbook(boxplot).unwrap();
    keep("boxplot.xlsx", &book);
    let parts = names(&book);
    assert!(parts.contains("xl/media/image1.png"), "{parts:?}");
    assert!(parts.contains("xl/drawings/drawing1.xml"));
    assert!(!parts.contains("xl/charts/chart1.xml"));
    assert_eq!(cells(&book)[0][0], boxplot.title);
    // a slide without slide titles shows the picture alone
    let deck = presentation(&[Item::Chart(boxplot)], true, false).unwrap();
    assert!(names(&deck).contains("ppt/media/image1.png"));
}

#[test]
fn output_export_writes_charts_to_workbooks_documents_and_presentations() {
    let dir = std::env::temp_dir().join(format!("statscraft-office-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let path = |name: &str| dir.join(name).display().to_string();
    let source = format!(
        "{DATA}FREQUENCIES drink /BARCHART.\n\
         OUTPUT EXPORT /XLSX DOCUMENTFILE='{}' /DOC DOCUMENTFILE='{}' PAGESIZE=MM(210, 297)\n\
         /PPT DOCUMENTFILE='{}' SLIDETITLES=NO.\n\
         OUTPUT EXPORT /XLSX DOCUMENTFILE='{}' OPERATION=MODIFYSHEET SHEET='Tables' LOCATION=LASTROW.",
        path("out.xlsx"),
        path("out.docx"),
        path("out.pptx"),
        path("out.xlsx"),
    );
    Session::default()
        .run(&source, None)
        .unwrap_or_else(|e| panic!("{e:?}"));
    let read = |name: &str| std::fs::read(dir.join(name)).unwrap();
    let book = read("out.xlsx");
    assert!(names(&book).contains("xl/charts/chart2.xml"));
    let sheet = cells(&book);
    let counts: Vec<&Vec<String>> = sheet.iter().filter(|r| r[0] == "Espresso").collect();
    // the chart's data table, written twice; the frequency table has its labels further right
    assert_eq!(counts.len(), 2, "{sheet:?}");
    let doc = read("out.docx");
    let body = text(&doc, "word/document.xml");
    assert_eq!(body.matches("<w:tbl>").count(), 1, "the frequencies");
    assert_eq!(body.matches("<wp:inline ").count(), 1);
    let deck = read("out.pptx");
    let slides = names(&deck)
        .iter()
        .filter(|n| n.starts_with("ppt/slides/slide"))
        .count();
    assert_eq!(slides, 2, "the table and the chart");
    std::fs::remove_dir_all(&dir).unwrap();
}
