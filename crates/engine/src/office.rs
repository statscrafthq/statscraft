//! Charts as office charts, and the files that carry them: a workbook, a presentation, a
//! document and the clipboard package that pastes as an editable chart.
//!
//! One writer makes every chart: rust_xlsxwriter draws it on a sheet beside its data table, and
//! that workbook is both the workbook file and the workbook embedded in the others, so Edit
//! Data opens the chart's numbers. The chart part it wrote serves the presentation, the
//! document and the clipboard after a small pass adds what those programs read differently
//! from a spreadsheet program when it is missing: visible axes, square corners, one color per
//! series, a base font size and the link to the embedded workbook. Nothing sets a color, so a
//! chart takes the theme of the file it lands in.
//!
//! Presentation charts of the kinds bar, pie, line and area are office charts. Statistical
//! plots (the histogram, the boxplot and the rest) and scatter plots are not: those go out as
//! a picture, beside their data table on a sheet, and as their data table while their kind is
//! not drawn.

use crate::chart::{Chart, ChartKind, Grouping, Orientation, Tier};
use crate::html::{escape, right_aligned};
use crate::picture::{FILE_WIDTH, Paper};
use crate::xlsx::{put_table, set_widths, sheet_name};
use crate::{Error, Table, cell_text, header_rows};
use rust_xlsxwriter::{
    Chart as Drawn, ChartAxis, ChartAxisTickType, ChartType, Image, Workbook, Worksheet, XlsxError,
};
use std::fmt::Write as _;
use std::io::{Cursor, Read, Write};

/// The rows and columns a chart covers on a sheet beside its data: 480 by 288 pixels at the
/// default row height and column width.
pub(crate) const CHART_ROWS: u32 = 15;
pub(crate) const CHART_COLUMNS: u16 = 8;

/// The base text size of a chart on a slide or on the clipboard, and in a document, in
/// hundredths of a point; without one every text is 10 points, too small on a slide.
const SLIDE_TEXT: u32 = 1400;
const PAGE_TEXT: u32 = 1000;

/// One item of an exported document: a table, or a chart, which goes out as an office chart
/// when it is one, as a picture when it is drawn and as its data table otherwise.
#[derive(Clone, Copy)]
pub enum Item<'a> {
    Table(&'a Table),
    Chart(&'a Chart),
}

/// Whether the chart goes to the office programs as an editable chart.
pub fn is_office_chart(chart: &Chart) -> bool {
    chart_type(chart).is_some()
}

/// The picture of a chart that has no office form, on light paper: a PNG and its size in
/// scene pixels, without the chart's own title unless `titled`. None for an office chart and
/// for a kind not drawn yet.
fn picture(chart: &Chart, titled: bool) -> Option<(Vec<u8>, f32, f32)> {
    if is_office_chart(chart) {
        return None;
    }
    let untitled;
    let chart = match titled {
        true => chart,
        false => {
            untitled = Chart {
                title: String::new(),
                ..chart.clone()
            };
            &untitled
        }
    };
    let scene = crate::picture::scene(chart, FILE_WIDTH, Paper::Light)?;
    // a picture that cannot be rendered leaves the data table in its place
    let png = crate::picture::png(&scene, 2.0).ok()?;
    Some((png, scene.width, scene.height))
}

/// Whether something stands beside the chart's data table on a sheet: the office chart or the
/// picture.
pub(crate) fn has_beside(chart: &Chart) -> bool {
    is_office_chart(chart) || crate::picture::scene(chart, FILE_WIDTH, Paper::Light).is_some()
}

/// `(width, height)` scaled to fit in `(room_w, room_h)` EMU, keeping its shape.
fn fit((width, height): (f32, f32), (room_w, room_h): (i64, i64)) -> (i64, i64) {
    let scale = (room_w as f32 / width).min(room_h as f32 / height);
    ((width * scale) as i64, (height * scale) as i64)
}

fn chart_type(chart: &Chart) -> Option<ChartType> {
    if chart.tier != Tier::Presentation || chart.categories.is_empty() || chart.series.is_empty() {
        return None;
    }
    let horizontal = chart.orientation == Some(Orientation::Horizontal);
    Some(match (chart.kind, chart.grouping, horizontal) {
        (ChartKind::Bar, Some(Grouping::Stacked), false) => ChartType::ColumnStacked,
        (ChartKind::Bar, Some(Grouping::Percent), false) => ChartType::ColumnPercentStacked,
        (ChartKind::Bar, _, false) => ChartType::Column,
        (ChartKind::Bar, Some(Grouping::Stacked), true) => ChartType::BarStacked,
        (ChartKind::Bar, Some(Grouping::Percent), true) => ChartType::BarPercentStacked,
        (ChartKind::Bar, _, true) => ChartType::Bar,
        (ChartKind::Pie, ..) => ChartType::Pie,
        (ChartKind::Line, Some(Grouping::Stacked), _) => ChartType::LineStacked,
        (ChartKind::Line, Some(Grouping::Percent), _) => ChartType::LinePercentStacked,
        (ChartKind::Line, ..) => ChartType::Line,
        (ChartKind::Area, Some(Grouping::Stacked), _) => ChartType::AreaStacked,
        (ChartKind::Area, Some(Grouping::Percent), _) => ChartType::AreaPercentStacked,
        (ChartKind::Area, ..) => ChartType::Area,
        // shortcut: scatter categories are written as text, which an office scatter cannot
        // plot; write them as numbers when a command makes scatter plots.
        (ChartKind::Scatter | ChartKind::Histogram, ..) => return None,
        // statistical plots: their data table stands in for them
        (
            ChartKind::Boxplot
            | ChartKind::StemLeaf
            | ChartKind::NormalQq
            | ChartKind::DetrendedNormalQq
            | ChartKind::NormalPp
            | ChartKind::SpreadLevel
            | ChartKind::Dendrogram
            | ChartKind::Icicle
            | ChartKind::TreeDiagram
            | ChartKind::TerritorialMap,
            ..,
        ) => return None,
    })
}

/// The office chart of `chart` over its data table `table`, written on the sheet `sheet` from
/// `(top, left)` by `put_table`. With `titled` false the chart shows no title of its own.
fn drawn(
    chart: &Chart,
    table: &Table,
    sheet: &str,
    (top, left): (u32, u16),
    titled: bool,
) -> Option<Drawn> {
    let mut drawn = Drawn::new(chart_type(chart)?);
    let first = top + 1 + header_rows(table).len() as u32;
    let last = first + chart.categories.len() as u32 - 1;
    for s in 0..chart.series.len() {
        let column = left + 1 + s as u16;
        drawn
            .add_series()
            .set_name((sheet, first - 1, column))
            .set_categories((sheet, first, left, last, left))
            .set_values((sheet, first, column, last, column));
    }
    if titled {
        drawn.title().set_name(&chart.title);
    } else {
        drawn.title().set_hidden();
    }
    if chart.kind != ChartKind::Pie {
        let axis = |axis: &mut ChartAxis, title: &str| {
            axis.set_major_tick_type(ChartAxisTickType::Outside)
                .set_minor_tick_type(ChartAxisTickType::None);
            if !title.is_empty() {
                axis.set_name(title);
            }
        };
        // a horizontal bar chart has its categories on the y axis
        let horizontal = chart.orientation == Some(Orientation::Horizontal);
        let categories = if horizontal {
            drawn.y_axis()
        } else {
            drawn.x_axis()
        };
        axis(categories, &chart.category_axis.title);
        let values = if horizontal {
            drawn.x_axis()
        } else {
            drawn.y_axis()
        };
        axis(values, &chart.value_axis.title);
        if let Some(maximum) = chart.value_axis.maximum {
            values.set_max(maximum);
        }
        if chart.series.len() == 1 {
            drawn.legend().set_hidden();
        }
    }
    Some(drawn)
}

/// Writes the chart's data table from `(top, left)` like `put_table` and beside it the office
/// chart, or the picture of a chart that has no office form; returns the row after both. With
/// `titled` false the office chart shows no title of its own.
pub(crate) fn put_chart(
    sheet: &mut Worksheet,
    table: &Table,
    chart: &Chart,
    (top, left): (u32, u16),
    notes: bool,
    widths: &mut Vec<usize>,
    titled: bool,
) -> Result<u32, XlsxError> {
    let end = put_table(sheet, table, (top, left), notes, widths)?;
    let name = sheet.name();
    let column = left + table.columns.len() as u16 + 1;
    if let Some(drawn) = drawn(chart, table, &name, (top, left), titled) {
        sheet.insert_chart(top + 1, column, &drawn)?;
    } else if let Some((png, ..)) = picture(chart, true) {
        // in the box an office chart takes, CHART_COLUMNS by CHART_ROWS cells
        let image = Image::new_from_buffer(&png)?
            .set_scale_to_size(480, 288, true)
            .set_alt_text(&chart.title);
        sheet.insert_image(top + 1, column, &image)?;
    } else {
        return Ok(end);
    }
    Ok(end.max(top + 1 + CHART_ROWS))
}

/// A workbook with the chart's data table and the chart beside it: the Excel file of a chart,
/// and the workbook embedded in the presentation, the document and the clipboard package.
pub fn chart_workbook(chart: &Chart) -> Result<Vec<u8>, Error> {
    workbook(chart, true)
}

fn workbook(chart: &Chart, titled: bool) -> Result<Vec<u8>, Error> {
    let xe = |e: XlsxError| Error(format!("cannot write the chart's workbook: {e}"));
    let mut book = Workbook::new();
    let sheet = book.add_worksheet();
    sheet
        .set_name(sheet_name(&chart.title, &mut Vec::new()))
        .map_err(xe)?;
    let mut widths = Vec::new();
    let table = chart.data_table();
    put_chart(sheet, &table, chart, (0, 0), true, &mut widths, titled).map_err(xe)?;
    set_widths(sheet, &widths).map_err(xe)?;
    book.save_to_buffer().map_err(xe)
}

/// A part of a zip file as text.
fn part(zip: &[u8], name: &str) -> Result<String, Error> {
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|e| Error(e.to_string()))?;
    let mut file = archive
        .by_name(name)
        .map_err(|e| Error(format!("{name}: {e}")))?;
    let mut text = String::new();
    file.read_to_string(&mut text)
        .map_err(|e| Error(format!("{name}: {e}")))?;
    Ok(text)
}

/// The chart of `chart` for a presentation, a document or the clipboard: its embedded
/// workbook and its chart part, which links to that workbook as the relationship rId1.
struct Embedded {
    workbook: Vec<u8>,
    part: String,
}

fn embedded(chart: &Chart, titled: bool, text_size: u32) -> Result<Embedded, Error> {
    let workbook = workbook(chart, titled)?;
    let part = office_part(&part(&workbook, "xl/charts/chart1.xml")?, text_size);
    Ok(Embedded { workbook, part })
}

/// The pass over a chart part as rust_xlsxwriter writes it for a workbook. It adds, without
/// any color: square corners; every axis shown (a missing `c:delete` reads as deleted outside
/// the spreadsheet program); titles and the legend beside the plot, not over it (a missing
/// `c:overlay`); one color per series in bar, line and area charts (a missing `c:varyColors`
/// reads as one color per point); a base text size in hundredths of a point; and the link to
/// the embedded workbook, relationship rId1.
pub(crate) fn office_part(xml: &str, text_size: u32) -> String {
    let mut out = xml.replacen(
        "<c:lang val=\"en-US\"/>",
        "<c:lang val=\"en-US\"/><c:roundedCorners val=\"0\"/>",
        1,
    );
    out = insert_after(&out, "</c:scaling>", "<c:delete", "<c:delete val=\"0\"/>");
    out = beside("title", &out);
    out = beside("legend", &out);
    // c:grouping is written for bar, line and area charts only, right before c:varyColors
    let mut grouped = String::new();
    let mut rest = out.as_str();
    while let Some(at) = rest.find("<c:grouping ") {
        let end = at + rest[at..].find("/>").map_or(0, |e| e + 2);
        grouped.push_str(&rest[..end]);
        rest = &rest[end..];
        if !rest.starts_with("<c:varyColors") {
            grouped.push_str("<c:varyColors val=\"0\"/>");
        }
    }
    grouped.push_str(rest);
    let text = format!(
        "<c:txPr><a:bodyPr/><a:lstStyle/><a:p><a:pPr><a:defRPr sz=\"{text_size}\"/></a:pPr>\
         <a:endParaRPr lang=\"en-US\"/></a:p></c:txPr>\
         <c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/></c:externalData>"
    );
    match grouped.find("<c:printSettings>") {
        Some(at) => grouped.insert_str(at, &text),
        None => {
            let at = grouped.rfind("</c:chartSpace>").unwrap_or(grouped.len());
            grouped.insert_str(at, &text);
        }
    }
    grouped
}

/// `xml` with every `c:{element}` (a title or the legend) placed beside the plot: an overlay
/// of 0 after its layout, unless it has an overlay already.
fn beside(element: &str, xml: &str) -> String {
    let (open, close) = (format!("<c:{element}>"), format!("</c:{element}>"));
    let mut out = String::new();
    let mut rest = xml;
    while let Some(at) = rest.find(&open) {
        let end = rest[at..].find(&close).map_or(rest.len(), |e| at + e);
        out.push_str(&rest[..at]);
        let inner = &rest[at..end];
        match inner.find("<c:layout/>") {
            Some(layout) if !inner.contains("<c:overlay") => {
                let layout = layout + "<c:layout/>".len();
                out.push_str(&inner[..layout]);
                out.push_str("<c:overlay val=\"0\"/>");
                out.push_str(&inner[layout..]);
            }
            _ => out.push_str(inner),
        }
        rest = &rest[end..];
    }
    out + rest
}

/// `xml` with `insert` after every `after` that `unless` does not already follow.
fn insert_after(xml: &str, after: &str, unless: &str, insert: &str) -> String {
    let mut out = String::new();
    let mut pieces = xml.split(after).peekable();
    while let Some(piece) = pieces.next() {
        out.push_str(piece);
        if let Some(next) = pieces.peek() {
            out.push_str(after);
            if !next.starts_with(unless) {
                out.push_str(insert);
            }
        }
    }
    out
}

const XML: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\" standalone=\"yes\"?>\n";
const NS_A: &str = "http://schemas.openxmlformats.org/drawingml/2006/main";
const NS_C: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_R: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";
const NS_P: &str = "http://schemas.openxmlformats.org/presentationml/2006/main";
const NS_W: &str = "http://schemas.openxmlformats.org/wordprocessingml/2006/main";
const NS_WP: &str = "http://schemas.openxmlformats.org/drawingml/2006/wordprocessingDrawing";
const REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships/";
const TYPE: &str = "application/vnd.openxmlformats-officedocument.";
const XLSX_TYPE: &str = "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet";
/// A graphic frame's content: the chart of relationship `id`.
const CHART_URI: &str = "http://schemas.openxmlformats.org/drawingml/2006/chart";
const NS_PIC: &str = "http://schemas.openxmlformats.org/drawingml/2006/picture";
const EMU_PER_INCH: i64 = 914_400;

/// A relationships part: (id, type after `REL`, target).
fn rels(list: &[(String, &str, String)]) -> String {
    let mut out = format!(
        "{XML}<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">"
    );
    for (id, kind, target) in list {
        let _ = write!(
            out,
            "<Relationship Id=\"{id}\" Type=\"{REL}{kind}\" Target=\"{target}\"/>"
        );
    }
    out + "</Relationships>"
}

/// The content types part: the defaults for relationships, XML and embedded workbooks, and
/// (part name, type after `TYPE`) overrides.
fn content_types(overrides: &[(String, &str)]) -> String {
    let mut out = format!(
        "{XML}<Types xmlns=\"http://schemas.openxmlformats.org/package/2006/content-types\">\
         <Default Extension=\"rels\" ContentType=\"application/vnd.openxmlformats-package.relationships+xml\"/>\
         <Default Extension=\"xml\" ContentType=\"application/xml\"/>\
         <Default Extension=\"xlsx\" ContentType=\"{XLSX_TYPE}\"/>\
         <Default Extension=\"png\" ContentType=\"image/png\"/>"
    );
    for (name, kind) in overrides {
        let _ = write!(
            out,
            "<Override PartName=\"/{name}\" ContentType=\"{TYPE}{kind}\"/>"
        );
    }
    out + "</Types>"
}

const CHART_TYPE: &str = "drawingml.chart+xml";

fn zip(parts: &[(String, Vec<u8>)]) -> Result<Vec<u8>, Error> {
    let ze = |e: zip::result::ZipError| Error(format!("cannot write the package: {e}"));
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    for (name, bytes) in parts {
        writer.start_file(name.as_str(), options).map_err(ze)?;
        writer
            .write_all(bytes)
            .map_err(|e| Error(format!("cannot write the package: {e}")))?;
    }
    Ok(writer.finish().map_err(ze)?.into_inner())
}

/// The chart frame's reference to its chart part, relationship `id`.
fn chart_ref(id: &str) -> String {
    format!(
        "<a:graphic xmlns:a=\"{NS_A}\"><a:graphicData uri=\"{CHART_URI}\">\
         <c:chart xmlns:c=\"{NS_C}\" xmlns:r=\"{NS_R}\" r:id=\"{id}\"/></a:graphicData></a:graphic>"
    )
}

/// The clipboard package that pastes `chart` as an editable chart with its workbook into the
/// presentation and word processing programs, under the clipboard format `CLIPBOARD_FORMAT`.
/// It has no theme part, so the chart takes the theme of where it lands. None when the chart
/// is not an office chart.
pub fn clipboard_package(chart: &Chart) -> Result<Option<Vec<u8>>, Error> {
    if !is_office_chart(chart) {
        return Ok(None);
    }
    let Embedded { workbook, part } = embedded(chart, true, SLIDE_TEXT)?;
    let (cx, cy) = (9 * EMU_PER_INCH, 11 * EMU_PER_INCH / 2);
    // in a locked canvas the frame puts its place after the graphic, or the paste is refused
    let drawing = format!(
        "{XML}<a:graphic xmlns:a=\"{NS_A}\">\
         <a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas\">\
         <lc:lockedCanvas xmlns:lc=\"http://schemas.openxmlformats.org/drawingml/2006/lockedCanvas\">\
         <a:nvGrpSpPr><a:cNvPr id=\"0\" name=\"\"/><a:cNvGrpSpPr/></a:nvGrpSpPr>\
         <a:grpSpPr><a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/>\
         <a:chOff x=\"0\" y=\"0\"/><a:chExt cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm></a:grpSpPr>\
         <a:graphicFrame><a:nvGraphicFramePr><a:cNvPr id=\"2\" name=\"Chart 1\"/>\
         <a:cNvGraphicFramePr/></a:nvGraphicFramePr>\
         <a:graphic><a:graphicData uri=\"{CHART_URI}\"><c:chart xmlns:c=\"{NS_C}\" \
         xmlns:r=\"{NS_R}\" r:id=\"rId1\"/></a:graphicData></a:graphic>\
         <a:xfrm><a:off x=\"0\" y=\"0\"/><a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm></a:graphicFrame>\
         </lc:lockedCanvas></a:graphicData></a:graphic>"
    );
    let book = "Microsoft_Excel_Worksheet.xlsx";
    let parts = [
        (
            "[Content_Types].xml".to_string(),
            content_types(&[
                ("clipboard/drawings/drawing1.xml".into(), "drawing+xml"),
                ("clipboard/charts/chart1.xml".into(), CHART_TYPE),
            ]),
        ),
        (
            "_rels/.rels".into(),
            rels(&[(
                "rId1".into(),
                "drawing",
                "clipboard/drawings/drawing1.xml".into(),
            )]),
        ),
        ("clipboard/drawings/drawing1.xml".into(), drawing),
        (
            "clipboard/drawings/_rels/drawing1.xml.rels".into(),
            rels(&[("rId1".into(), "chart", "../charts/chart1.xml".into())]),
        ),
        ("clipboard/charts/chart1.xml".into(), part),
        (
            "clipboard/charts/_rels/chart1.xml.rels".into(),
            rels(&[("rId1".into(), "package", format!("../embeddings/{book}"))]),
        ),
    ];
    let mut parts: Vec<(String, Vec<u8>)> = parts
        .into_iter()
        .map(|(n, t)| (n, t.into_bytes()))
        .collect();
    parts.push((format!("clipboard/embeddings/{book}"), workbook));
    zip(&parts).map(Some)
}

/// The name the clipboard package is registered under.
pub const CLIPBOARD_FORMAT: &str = "Art::GVML ClipFormat";

/// A table as cells for a document or a slide: the header rows, then the body. Each cell
/// with its text, the columns it spans and whether it goes right.
struct Grid {
    rows: Vec<Vec<(String, usize, bool)>>,
    header: usize,
    /// The characters of the widest cell of each column, at least 1 and at most 40.
    widths: Vec<usize>,
}

fn grid(table: &Table) -> Grid {
    let mut rows: Vec<Vec<(String, usize, bool)>> = header_rows(table)
        .into_iter()
        .map(|cells| {
            let cells = cells.into_iter();
            cells.map(|c| (c.label, c.span, false)).collect()
        })
        .collect();
    let header = rows.len();
    for r in 0..table.rows.len() {
        let columns = 0..table.columns.len();
        rows.push(
            columns
                .map(|c| match c < table.rows[r].len() {
                    true => (cell_text(table, r, c), 1, right_aligned(table, r, c)),
                    false => (String::new(), 1, false),
                })
                .collect(),
        );
    }
    let mut widths = vec![1; table.columns.len()];
    for row in &rows {
        let mut column = 0;
        for (text, span, _) in row {
            if *span == 1 && column < widths.len() {
                widths[column] = widths[column].max(text.chars().count().min(40));
            }
            column += span;
        }
    }
    Grid {
        rows,
        header,
        widths,
    }
}

/// Where the presentation's items go: the slide size and the margin, in EMU.
const SLIDE: (i64, i64) = (12_192_000, 6_858_000);
const MARGIN: i64 = EMU_PER_INCH / 2;
const TITLE_HEIGHT: i64 = EMU_PER_INCH;

/// A presentation with one slide per item: a table as a table, a chart as an editable chart
/// with its workbook or as a picture. With `titles` each slide has the item's title as its
/// title, and a chart leaves out its own; with `notes` a table's caption shows below it.
pub fn presentation(items: &[Item], notes: bool, titles: bool) -> Result<Vec<u8>, Error> {
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut overrides: Vec<(String, &str)> = vec![
        (
            "ppt/presentation.xml".into(),
            "presentationml.presentation.main+xml",
        ),
        ("ppt/presProps.xml".into(), "presentationml.presProps+xml"),
        (
            "ppt/slideMasters/slideMaster1.xml".into(),
            "presentationml.slideMaster+xml",
        ),
        (
            "ppt/slideLayouts/slideLayout1.xml".into(),
            "presentationml.slideLayout+xml",
        ),
        ("ppt/theme/theme1.xml".into(), "theme+xml"),
    ];
    let mut slide_list = String::new();
    let mut presentation_rels = vec![
        (
            "rId1".to_string(),
            "slideMaster",
            "slideMasters/slideMaster1.xml".to_string(),
        ),
        ("rId2".into(), "theme", "theme/theme1.xml".into()),
        ("rId3".into(), "presProps", "presProps.xml".into()),
    ];
    let mut theme = None;
    let (mut charts, mut pictures) = (0, 0);
    for (i, item) in items.iter().enumerate() {
        let n = i + 1;
        let title = match item {
            Item::Table(table) => table.title.as_str(),
            Item::Chart(chart) => chart.title.as_str(),
        };
        let mut shapes = String::new();
        let top = match titles {
            true => {
                shapes.push_str(&title_shape(title));
                MARGIN + TITLE_HEIGHT
            }
            false => MARGIN,
        };
        let (width, height) = (SLIDE.0 - 2 * MARGIN, SLIDE.1 - top - MARGIN);
        let mut slide_rels = vec![(
            "rId1".to_string(),
            "slideLayout",
            "../slideLayouts/slideLayout1.xml".to_string(),
        )];
        let (chart, drawn) = match item {
            Item::Chart(chart) if is_office_chart(chart) => (Some(*chart), None),
            Item::Chart(chart) => (None, picture(chart, !titles)),
            Item::Table(_) => (None, None),
        };
        if let Some((png, w, h)) = drawn {
            pictures += 1;
            let (cx, cy) = fit((w, h), (width, height));
            let x = MARGIN + (width - cx) / 2;
            let _ = write!(
                shapes,
                "<p:pic><p:nvPicPr><p:cNvPr id=\"3\" name=\"Picture 2\" descr=\"{}\"/>\
                 <p:cNvPicPr><a:picLocks noChangeAspect=\"1\"/></p:cNvPicPr><p:nvPr/></p:nvPicPr>\
                 <p:blipFill><a:blip r:embed=\"rId2\"/><a:stretch><a:fillRect/></a:stretch>\
                 </p:blipFill><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{top}\"/>\
                 <a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/>\
                 </a:prstGeom></p:spPr></p:pic>",
                escape(title)
            );
            slide_rels.push((
                "rId2".into(),
                "image",
                format!("../media/image{pictures}.png"),
            ));
            parts.push((format!("ppt/media/image{pictures}.png"), png));
        } else if let Some(chart) = chart {
            charts += 1;
            let Embedded { workbook, part } = embedded(chart, !titles, SLIDE_TEXT)?;
            if theme.is_none() {
                theme = Some(self::part(&workbook, "xl/theme/theme1.xml")?);
            }
            let _ = write!(
                shapes,
                "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"3\" name=\"Chart 2\"/>\
                 <p:cNvGraphicFramePr/><p:nvPr/></p:nvGraphicFramePr>\
                 <p:xfrm><a:off x=\"{MARGIN}\" y=\"{top}\"/><a:ext cx=\"{width}\" cy=\"{height}\"/></p:xfrm>\
                 {}</p:graphicFrame>",
                chart_ref("rId2")
            );
            slide_rels.push((
                "rId2".into(),
                "chart",
                format!("../charts/chart{charts}.xml"),
            ));
            let book = format!("Microsoft_Excel_Worksheet{charts}.xlsx");
            parts.push((format!("ppt/charts/chart{charts}.xml"), part.into_bytes()));
            parts.push((
                format!("ppt/charts/_rels/chart{charts}.xml.rels"),
                rels(&[("rId1".into(), "package", format!("../embeddings/{book}"))]).into_bytes(),
            ));
            parts.push((format!("ppt/embeddings/{book}"), workbook));
            overrides.push((format!("ppt/charts/chart{charts}.xml"), CHART_TYPE));
        } else {
            let owned;
            let table = match item {
                Item::Table(table) => *table,
                Item::Chart(chart) => {
                    owned = chart.data_table();
                    &owned
                }
            };
            shapes.push_str(&slide_table(table, notes, (MARGIN, top), width));
        }
        let slide = format!(
            "{XML}<p:sld xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\"><p:cSld><p:spTree>\
             <p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>\
             <p:grpSpPr/>{shapes}</p:spTree></p:cSld>\
             <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sld>"
        );
        parts.push((format!("ppt/slides/slide{n}.xml"), slide.into_bytes()));
        parts.push((
            format!("ppt/slides/_rels/slide{n}.xml.rels"),
            rels(&slide_rels).into_bytes(),
        ));
        overrides.push((
            format!("ppt/slides/slide{n}.xml"),
            "presentationml.slide+xml",
        ));
        let id = format!("rId{}", n + 3);
        let _ = write!(slide_list, "<p:sldId id=\"{}\" r:id=\"{id}\"/>", 255 + n);
        presentation_rels.push((id, "slide", format!("slides/slide{n}.xml")));
    }
    let theme = match theme {
        Some(theme) => theme,
        None => part(
            &Workbook::new()
                .save_to_buffer()
                .map_err(|e| Error(e.to_string()))?,
            "xl/theme/theme1.xml",
        )?,
    };
    let slide_list = match slide_list.is_empty() {
        true => String::new(),
        false => format!("<p:sldIdLst>{slide_list}</p:sldIdLst>"),
    };
    let presentation = format!(
        "{XML}<p:presentation xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\">\
         <p:sldMasterIdLst><p:sldMasterId id=\"2147483648\" r:id=\"rId1\"/></p:sldMasterIdLst>\
         {slide_list}<p:sldSz cx=\"{}\" cy=\"{}\"/><p:notesSz cx=\"6858000\" cy=\"9144000\"/>\
         </p:presentation>",
        SLIDE.0, SLIDE.1
    );
    let fixed = [
        ("[Content_Types].xml".to_string(), content_types(&overrides)),
        (
            "_rels/.rels".into(),
            rels(&[(
                "rId1".into(),
                "officeDocument",
                "ppt/presentation.xml".into(),
            )]),
        ),
        ("ppt/presentation.xml".into(), presentation),
        (
            "ppt/_rels/presentation.xml.rels".into(),
            rels(&presentation_rels),
        ),
        (
            "ppt/presProps.xml".into(),
            format!(
                "{XML}<p:presentationPr xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\"/>"
            ),
        ),
        ("ppt/slideMasters/slideMaster1.xml".into(), master()),
        (
            "ppt/slideMasters/_rels/slideMaster1.xml.rels".into(),
            rels(&[
                (
                    "rId1".into(),
                    "slideLayout",
                    "../slideLayouts/slideLayout1.xml".into(),
                ),
                ("rId2".into(), "theme", "../theme/theme1.xml".into()),
            ]),
        ),
        ("ppt/slideLayouts/slideLayout1.xml".into(), layout()),
        (
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels".into(),
            rels(&[(
                "rId1".into(),
                "slideMaster",
                "../slideMasters/slideMaster1.xml".into(),
            )]),
        ),
        ("ppt/theme/theme1.xml".into(), theme),
    ];
    let mut all: Vec<(String, Vec<u8>)> = fixed
        .into_iter()
        .map(|(n, t)| (n, t.into_bytes()))
        .collect();
    all.extend(parts);
    zip(&all)
}

/// The slide's title placeholder holding `title`.
fn title_shape(title: &str) -> String {
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title 1\"/><p:cNvSpPr><a:spLocks noGrp=\"1\"/>\
         </p:cNvSpPr><p:nvPr><p:ph type=\"title\"/></p:nvPr></p:nvSpPr><p:spPr/>\
         <p:txBody><a:bodyPr/><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\"/><a:t>{}</a:t></a:r>\
         </a:p></p:txBody></p:sp>",
        escape(title)
    )
}

/// The placeholder shape of a master or layout: a title, at its place on the master.
fn title_placeholder(place: bool) -> String {
    let place = match place {
        true => format!(
            "<a:xfrm><a:off x=\"{MARGIN}\" y=\"{}\"/><a:ext cx=\"{}\" cy=\"{TITLE_HEIGHT}\"/></a:xfrm>\
             <a:prstGeom prst=\"rect\"><a:avLst/></a:prstGeom>",
            MARGIN / 2,
            SLIDE.0 - 2 * MARGIN
        ),
        false => String::new(),
    };
    format!(
        "<p:sp><p:nvSpPr><p:cNvPr id=\"2\" name=\"Title Placeholder 1\"/><p:cNvSpPr>\
         <a:spLocks noGrp=\"1\"/></p:cNvSpPr><p:nvPr><p:ph type=\"title\"/></p:nvPr></p:nvSpPr>\
         <p:spPr>{place}</p:spPr><p:txBody><a:bodyPr anchor=\"b\"/><a:lstStyle/><a:p>\
         <a:endParaRPr lang=\"en-US\"/></a:p></p:txBody></p:sp>"
    )
}

const GROUP: &str = "<p:nvGrpSpPr><p:cNvPr id=\"1\" name=\"\"/><p:cNvGrpSpPr/><p:nvPr/>\
                     </p:nvGrpSpPr><p:grpSpPr/>";

/// The slide master: a title placeholder, the theme's colors and title text, no colors of
/// its own.
fn master() -> String {
    format!(
        "{XML}<p:sldMaster xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\"><p:cSld>\
         <p:bg><p:bgRef idx=\"1001\"><a:schemeClr val=\"bg1\"/></p:bgRef></p:bg>\
         <p:spTree>{GROUP}{}</p:spTree></p:cSld>\
         <p:clrMap bg1=\"lt1\" tx1=\"dk1\" bg2=\"lt2\" tx2=\"dk2\" accent1=\"accent1\" \
         accent2=\"accent2\" accent3=\"accent3\" accent4=\"accent4\" accent5=\"accent5\" \
         accent6=\"accent6\" hlink=\"hlink\" folHlink=\"folHlink\"/>\
         <p:sldLayoutIdLst><p:sldLayoutId id=\"2147483649\" r:id=\"rId1\"/></p:sldLayoutIdLst>\
         <p:txStyles><p:titleStyle><a:lvl1pPr><a:defRPr sz=\"3200\"><a:solidFill>\
         <a:schemeClr val=\"tx1\"/></a:solidFill><a:latin typeface=\"+mj-lt\"/>\
         <a:ea typeface=\"+mj-ea\"/><a:cs typeface=\"+mj-cs\"/></a:defRPr></a:lvl1pPr>\
         </p:titleStyle><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>",
        title_placeholder(true)
    )
}

fn layout() -> String {
    format!(
        "{XML}<p:sldLayout xmlns:a=\"{NS_A}\" xmlns:r=\"{NS_R}\" xmlns:p=\"{NS_P}\" \
         type=\"titleOnly\" preserve=\"1\"><p:cSld name=\"Title Only\"><p:spTree>{GROUP}{}\
         </p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>",
        title_placeholder(false)
    )
}

/// A table on a slide from `(x, y)` across `width`, in a built-in table style of the theme,
/// with its caption below it when `notes`.
fn slide_table(table: &Table, notes: bool, (x, y): (i64, i64), width: i64) -> String {
    let grid = grid(table);
    let total: usize = grid.widths.iter().sum();
    let columns: Vec<i64> = grid
        .widths
        .iter()
        .map(|w| width * *w as i64 / total as i64)
        .collect();
    let row_height = EMU_PER_INCH * 3 / 10;
    let height = row_height * grid.rows.len() as i64;
    let mut out = format!(
        "<p:graphicFrame><p:nvGraphicFramePr><p:cNvPr id=\"3\" name=\"Table 2\"/>\
         <p:cNvGraphicFramePr><a:graphicFrameLocks noGrp=\"1\"/></p:cNvGraphicFramePr><p:nvPr/>\
         </p:nvGraphicFramePr><p:xfrm><a:off x=\"{x}\" y=\"{y}\"/><a:ext cx=\"{width}\" cy=\"{height}\"/>\
         </p:xfrm><a:graphic><a:graphicData uri=\"http://schemas.openxmlformats.org/drawingml/2006/table\">\
         <a:tbl><a:tblPr firstRow=\"1\" bandRow=\"1\">\
         <a:tableStyleId>{{5C22544A-7EE6-4342-B048-85BDC9FD1C3A}}</a:tableStyleId></a:tblPr><a:tblGrid>"
    );
    for w in &columns {
        let _ = write!(out, "<a:gridCol w=\"{w}\"/>");
    }
    out.push_str("</a:tblGrid>");
    for (r, row) in grid.rows.iter().enumerate() {
        let _ = write!(out, "<a:tr h=\"{row_height}\">");
        for (text, span, right) in row {
            let align = match (r < grid.header, right) {
                (true, _) => " algn=\"ctr\"",
                (false, true) => " algn=\"r\"",
                (false, false) => "",
            };
            let span_attr = match span {
                1 => String::new(),
                n => format!(" gridSpan=\"{n}\""),
            };
            let _ = write!(
                out,
                "<a:tc{span_attr}><a:txBody><a:bodyPr/><a:lstStyle/><a:p><a:pPr{align}/>\
                 <a:r><a:rPr lang=\"en-US\" sz=\"1200\"/><a:t>{}</a:t></a:r></a:p></a:txBody>\
                 <a:tcPr/></a:tc>",
                escape(text)
            );
            for _ in 1..*span {
                out.push_str(
                    "<a:tc hMerge=\"1\"><a:txBody><a:bodyPr/><a:lstStyle/><a:p>\
                     <a:endParaRPr lang=\"en-US\"/></a:p></a:txBody><a:tcPr/></a:tc>",
                );
            }
        }
        out.push_str("</a:tr>");
    }
    out.push_str("</a:tbl></a:graphicData></a:graphic></p:graphicFrame>");
    if let Some(caption) = table.caption.as_ref().filter(|_| notes) {
        let _ = write!(
            out,
            "<p:sp><p:nvSpPr><p:cNvPr id=\"4\" name=\"Caption 3\"/><p:cNvSpPr txBox=\"1\"/>\
             <p:nvPr/></p:nvSpPr><p:spPr><a:xfrm><a:off x=\"{x}\" y=\"{}\"/>\
             <a:ext cx=\"{width}\" cy=\"{row_height}\"/></a:xfrm><a:prstGeom prst=\"rect\">\
             <a:avLst/></a:prstGeom></p:spPr><p:txBody><a:bodyPr wrap=\"square\"><a:spAutoFit/>\
             </a:bodyPr><a:lstStyle/><a:p><a:r><a:rPr lang=\"en-US\" sz=\"1100\" i=\"1\"/>\
             <a:t>{}</a:t></a:r></a:p></p:txBody></p:sp>",
            y + height + row_height / 2,
            escape(caption)
        );
    }
    out
}

/// A document with the items one below the other: a table as a table with its title above it
/// and, with `notes`, its caption below; a chart as an editable chart with its workbook or as
/// a picture.
pub fn document(items: &[Item], notes: bool) -> Result<Vec<u8>, Error> {
    let mut body = String::new();
    let mut parts: Vec<(String, Vec<u8>)> = Vec::new();
    let mut overrides: Vec<(String, &str)> = vec![(
        "word/document.xml".into(),
        "wordprocessingml.document.main+xml",
    )];
    let mut document_rels = Vec::new();
    let mut charts = 0;
    let paragraph = |text: &str, style: &str| {
        format!(
            "<w:p><w:r><w:rPr>{style}</w:rPr><w:t xml:space=\"preserve\">{}</w:t></w:r></w:p>",
            escape(text)
        )
    };
    for item in items {
        let drawn = match item {
            Item::Chart(chart) => picture(chart, true).map(|p| (p, chart.title.as_str())),
            Item::Table(_) => None,
        };
        if let Some(((png, w, h), title)) = drawn {
            let n = document_rels.len() + 1;
            // six inches wide, or less for a tall picture
            let (cx, cy) = fit((w, h), (6 * EMU_PER_INCH, 8 * EMU_PER_INCH));
            let _ = write!(
                body,
                "<w:p><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
                 <wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>\
                 <wp:docPr id=\"{n}\" name=\"Picture {n}\" descr=\"{}\"/><wp:cNvGraphicFramePr>\
                 <a:graphicFrameLocks noChangeAspect=\"1\"/></wp:cNvGraphicFramePr><a:graphic>\
                 <a:graphicData uri=\"{NS_PIC}\"><pic:pic xmlns:pic=\"{NS_PIC}\"><pic:nvPicPr>\
                 <pic:cNvPr id=\"{n}\" name=\"Picture {n}\"/><pic:cNvPicPr/></pic:nvPicPr>\
                 <pic:blipFill><a:blip r:embed=\"rId{n}\"/><a:stretch><a:fillRect/></a:stretch>\
                 </pic:blipFill><pic:spPr><a:xfrm><a:off x=\"0\" y=\"0\"/>\
                 <a:ext cx=\"{cx}\" cy=\"{cy}\"/></a:xfrm><a:prstGeom prst=\"rect\"><a:avLst/>\
                 </a:prstGeom></pic:spPr></pic:pic></a:graphicData></a:graphic></wp:inline>\
                 </w:drawing></w:r></w:p><w:p/>",
                escape(title)
            );
            document_rels.push((format!("rId{n}"), "image", format!("media/image{n}.png")));
            parts.push((format!("word/media/image{n}.png"), png));
            continue;
        }
        match item {
            Item::Chart(chart) if is_office_chart(chart) => {
                charts += 1;
                let Embedded { workbook, part } = embedded(chart, true, PAGE_TEXT)?;
                let n = document_rels.len() + 1;
                let id = format!("rId{n}");
                let (cx, cy) = (6 * EMU_PER_INCH, 18 * EMU_PER_INCH / 5);
                let _ = write!(
                    body,
                    "<w:p><w:r><w:drawing><wp:inline distT=\"0\" distB=\"0\" distL=\"0\" distR=\"0\">\
                     <wp:extent cx=\"{cx}\" cy=\"{cy}\"/><wp:effectExtent l=\"0\" t=\"0\" r=\"0\" b=\"0\"/>\
                     <wp:docPr id=\"{n}\" name=\"Chart {charts}\"/><wp:cNvGraphicFramePr/>{}\
                     </wp:inline></w:drawing></w:r></w:p>",
                    chart_ref(&id)
                );
                document_rels.push((id, "chart", format!("charts/chart{charts}.xml")));
                let book = format!("Microsoft_Excel_Worksheet{charts}.xlsx");
                parts.push((format!("word/charts/chart{charts}.xml"), part.into_bytes()));
                parts.push((
                    format!("word/charts/_rels/chart{charts}.xml.rels"),
                    rels(&[("rId1".into(), "package", format!("../embeddings/{book}"))])
                        .into_bytes(),
                ));
                parts.push((format!("word/embeddings/{book}"), workbook));
                overrides.push((format!("word/charts/chart{charts}.xml"), CHART_TYPE));
            }
            _ => {
                let owned;
                let table = match item {
                    Item::Table(table) => *table,
                    Item::Chart(chart) => {
                        owned = chart.data_table();
                        &owned
                    }
                };
                body.push_str(&paragraph(&table.title, "<w:b/>"));
                body.push_str(&page_table(table));
                match table.caption.as_ref().filter(|_| notes) {
                    Some(caption) => body.push_str(&paragraph(caption, "<w:i/>")),
                    // a table needs a paragraph after it
                    None => body.push_str("<w:p/>"),
                }
            }
        }
        body.push_str("<w:p/>");
    }
    let document = format!(
        "{XML}<w:document xmlns:w=\"{NS_W}\" xmlns:r=\"{NS_R}\" xmlns:wp=\"{NS_WP}\" \
         xmlns:a=\"{NS_A}\"><w:body>{body}</w:body></w:document>"
    );
    let fixed = [
        ("[Content_Types].xml".to_string(), content_types(&overrides)),
        (
            "_rels/.rels".into(),
            rels(&[("rId1".into(), "officeDocument", "word/document.xml".into())]),
        ),
        ("word/document.xml".into(), document),
        ("word/_rels/document.xml.rels".into(), rels(&document_rels)),
    ];
    let mut all: Vec<(String, Vec<u8>)> = fixed
        .into_iter()
        .map(|(n, t)| (n, t.into_bytes()))
        .collect();
    all.extend(parts);
    zip(&all)
}

/// A table in a document, with thin borders in the text color and the header rows bold.
fn page_table(table: &Table) -> String {
    let grid = grid(table);
    let line = "w:val=\"single\" w:sz=\"4\" w:space=\"0\" w:color=\"auto\"";
    let mut out = format!(
        "<w:tbl><w:tblPr><w:tblW w:w=\"0\" w:type=\"auto\"/><w:tblBorders><w:top {line}/>\
         <w:left {line}/><w:bottom {line}/><w:right {line}/><w:insideH {line}/>\
         <w:insideV {line}/></w:tblBorders></w:tblPr><w:tblGrid>"
    );
    for w in &grid.widths {
        // twips: about a tenth of an inch per character, and room for the cell margins
        let _ = write!(out, "<w:gridCol w:w=\"{}\"/>", w * 140 + 240);
    }
    out.push_str("</w:tblGrid>");
    for (r, row) in grid.rows.iter().enumerate() {
        let header = r < grid.header;
        out.push_str("<w:tr>");
        if header {
            out.push_str("<w:trPr><w:tblHeader/></w:trPr>");
        }
        for (text, span, right) in row {
            let span = match span {
                1 => String::new(),
                n => format!("<w:gridSpan w:val=\"{n}\"/>"),
            };
            let align = match (header, right) {
                (true, _) => "<w:pPr><w:jc w:val=\"center\"/></w:pPr>",
                (false, true) => "<w:pPr><w:jc w:val=\"right\"/></w:pPr>",
                (false, false) => "",
            };
            let bold = if header { "<w:rPr><w:b/></w:rPr>" } else { "" };
            let _ = write!(
                out,
                "<w:tc><w:tcPr>{span}</w:tcPr><w:p>{align}<w:r>{bold}\
                 <w:t xml:space=\"preserve\">{}</w:t></w:r></w:p></w:tc>",
                escape(text)
            );
        }
        out.push_str("</w:tr>");
    }
    out + "</w:tbl>"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pass_adds_its_elements_once() {
        let xml = "<c:chartSpace><c:lang val=\"en-US\"/><c:chart><c:plotArea>\
                   <c:barChart><c:barDir val=\"col\"/><c:grouping val=\"clustered\"/><c:ser/>\
                   </c:barChart><c:catAx><c:scaling/></c:scaling><c:axPos/>\
                   <c:title><c:tx><c:rich/></c:tx><c:layout/></c:title></c:catAx>\
                   <c:valAx><c:scaling/></c:scaling><c:delete val=\"1\"/></c:valAx>\
                   </c:plotArea><c:legend><c:legendPos val=\"r\"/><c:layout/><c:txPr/></c:legend>\
                   </c:chart><c:printSettings></c:printSettings></c:chartSpace>";
        let xml = xml.replace("<c:scaling/>", "<c:scaling>");
        let once = office_part(&xml, 1400);
        assert_eq!(once.matches("<c:roundedCorners val=\"0\"/>").count(), 1);
        // the hidden axis keeps its own c:delete
        assert_eq!(once.matches("<c:delete val=\"0\"/>").count(), 1);
        assert_eq!(once.matches("<c:delete").count(), 2);
        assert!(once.contains("<c:grouping val=\"clustered\"/><c:varyColors val=\"0\"/><c:ser/>"));
        assert!(once.contains("<a:defRPr sz=\"1400\"/>"));
        assert!(once.contains("</c:tx><c:layout/><c:overlay val=\"0\"/></c:title>"));
        assert!(once.contains("<c:layout/><c:overlay val=\"0\"/><c:txPr/></c:legend>"));
        assert_eq!(once.matches("<c:overlay").count(), 2);
        assert!(
            once.contains("</c:externalData><c:printSettings")
                && once.contains("<c:externalData r:id=\"rId1\"><c:autoUpdate val=\"0\"/>")
        );
    }
}
