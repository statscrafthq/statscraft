//! Output as a PDF file, for OUTPUT EXPORT /PDF and the window's export.
//!
//! The items stand one below the other on A4 portrait pages with margins of 2 cm, under the
//! heading of the command that made them. Tables are real text in the bundled font: the title
//! above, the header rows with their column groups in a tinted band, the row labels on the
//! left, numbers aligned right and the caption below. A table longer than the page goes on
//! over the next pages with its header rows repeated. A table wider than the page is first set
//! in a smaller size, down to 70 percent; when that is not enough its columns are split into
//! parts that stand one below the other, each with the row labels again. Charts are drawn as
//! vector shapes from the same layout as their pictures, so they stay sharp at any zoom; a
//! chart of a kind not drawn yet goes out as its data table. The TITLE text heads every page,
//! and every page has its number at the foot.
//!
//! With bookmarks, the outline has one entry per command and one below it per item. With the
//! font embedded, only the glyphs used go into the file; without, the text is written in the
//! Windows Latin encoding and refers to the font by name, with its widths, so a reader without
//! the font keeps the layout with a font of its own. Characters outside that encoding are then
//! written as question marks.

use crate::office::Item;
use crate::picture::{self, Anchor, Color, FILE_WIDTH, Palette, Paper, Shape};
use crate::{Table, cell_text, header_rows};
use pdf_writer::types::{FontFlags, LineJoinStyle, PageMode, TextRenderingMode};
use pdf_writer::writers::Destination;
use pdf_writer::{Content, Filter, Finish, Name, Pdf, Rect, Ref, Str, TextStr};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;

/// What OUTPUT EXPORT /PDF lets the user choose, and the text atop every page.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// EMBEDBOOKMARKS: an outline of the commands and their items.
    pub bookmarks: bool,
    /// EMBEDFONTS: the glyphs of the bundled font go into the file.
    pub embed_fonts: bool,
    /// The text of the last TITLE command, at the top of every page.
    pub title: Option<String>,
}

/// A4 portrait in points, and its margins of 2 cm.
const PAGE: (f32, f32) = (595.28, 841.89);
const MARGIN: f32 = 56.69;
const FRAME: f32 = PAGE.0 - 2.0 * MARGIN;
const BOTTOM: f32 = PAGE.1 - MARGIN;

/// Text sizes in points.
const HEADING: f32 = 14.0;
const TITLE: f32 = 10.5;
const BODY: f32 = 9.0;
const NOTE: f32 = 8.0;
/// How far a wide table may shrink before its columns are split.
const SMALLEST: f32 = 0.7;
/// The widest a column grows before its cells wrap, as a share of the frame.
const WIDEST: f32 = 0.4;
/// Room inside a cell, at the body size.
const PAD: (f32, f32) = (3.0, 2.0);
const LEADING: f32 = 1.25;

/// One thing drawn on a page, in points from the top left corner.
#[derive(Debug, Clone, PartialEq)]
enum Op {
    Text {
        x: f32,
        /// The baseline.
        y: f32,
        size: f32,
        text: String,
        color: Color,
        bold: bool,
    },
    Path {
        points: Vec<[f32; 2]>,
        closed: bool,
        fill: Option<Color>,
        stroke: Option<(Color, f32)>,
    },
}

/// A bookmark: 0 for a command, 1 for an item; where it points.
struct Mark {
    level: u8,
    title: String,
    page: usize,
    y: f32,
}

struct Layout {
    pages: Vec<Vec<Op>>,
    y: f32,
    marks: Vec<Mark>,
    palette: Palette,
}

/// A cell's lines of text, how they align, and whether it is a header cell.
struct Cell {
    lines: Vec<String>,
    align: Anchor,
    header: bool,
}

impl Layout {
    fn new() -> Layout {
        Layout {
            pages: vec![Vec::new()],
            y: MARGIN,
            marks: Vec::new(),
            palette: Palette::of(Paper::Light),
        }
    }

    fn new_page(&mut self) {
        self.pages.push(Vec::new());
        self.y = MARGIN;
    }

    /// Starts a new page unless `height` fits below; a page that is still empty takes it.
    fn room(&mut self, height: f32) {
        if self.y + height > BOTTOM && self.y > MARGIN {
            self.new_page();
        }
    }

    fn op(&mut self, op: Op) {
        self.pages.last_mut().expect("a page").push(op);
    }

    fn mark(&mut self, level: u8, title: &str) {
        self.marks.push(Mark {
            level,
            title: title.to_string(),
            page: self.pages.len() - 1,
            y: self.y,
        });
    }

    /// A line of text in the box from `top` that is `size * LEADING` high.
    fn line(&mut self, x: f32, top: f32, size: f32, text: &str, color: Color, bold: bool) {
        if text.is_empty() {
            return;
        }
        self.op(Op::Text {
            x,
            y: top + size * LEADING / 2.0 + 0.36 * size,
            size,
            text: text.to_string(),
            color,
            bold,
        });
    }

    /// Text wrapped to the frame, line by line onto the pages.
    fn paragraph(&mut self, text: &str, size: f32, color: Color, bold: bool) {
        for line in wrap(text, size, FRAME) {
            self.room(size * LEADING);
            self.line(MARGIN, self.y, size, &line, color, bold);
            self.y += size * LEADING;
        }
    }

    fn heading(&mut self, command: &str) {
        // a heading at the foot of a page stays with what follows it
        self.room(HEADING * LEADING + 80.0);
        self.mark(0, command);
        self.paragraph(command, HEADING, self.palette.ink, true);
        self.y += 6.0;
    }

    fn table(&mut self, table: &Table) {
        let columns = table.columns.len();
        let header_cells = header_rows(table);
        let body: Vec<Vec<(String, Anchor)>> = (0..table.rows.len())
            .map(|r| {
                (0..columns)
                    .map(|c| match c < table.rows[r].len() {
                        true => {
                            let right = crate::html::right_aligned(table, r, c);
                            let align = if right { Anchor::End } else { Anchor::Start };
                            (cell_text(table, r, c), align)
                        }
                        false => (String::new(), Anchor::Start),
                    })
                    .collect()
            })
            .collect();
        // natural widths at the body size: the widest cell, at most WIDEST of the frame
        let mut widths = vec![0f32; columns];
        let one_column = header_cells.iter().flatten().filter(|h| h.span == 1);
        let cells = body
            .iter()
            .flat_map(|row| row.iter().map(|(t, _)| t).enumerate());
        for (c, text) in cells.chain(one_column.map(|h| (h.first, &h.label))) {
            widths[c] = widths[c].max(picture::text_width(text, BODY));
        }
        for w in &mut widths {
            *w = w.min(FRAME * WIDEST) + 2.0 * PAD.0;
        }
        let total: f32 = widths.iter().sum();
        let scale = (FRAME / total).clamp(SMALLEST, 1.0);
        widths.iter_mut().for_each(|w| *w *= scale);
        let size = BODY * scale;
        // the parts of a table too wide even when shrunk: the row label columns, then as many
        // others as fit, at least one
        let stub: Vec<usize> = (0..table.row_headers.min(columns)).collect();
        let stub_width: f32 = stub.iter().map(|&c| widths[c]).sum();
        let mut parts: Vec<Vec<usize>> = Vec::new();
        let mut part = stub.clone();
        let mut width = stub_width;
        for (c, &w) in widths.iter().enumerate().skip(stub.len()) {
            if width + w > FRAME + 0.01 && part.len() > stub.len() {
                parts.push(std::mem::replace(&mut part, stub.clone()));
                width = stub_width;
            }
            part.push(c);
            width += w;
        }
        if part.len() > stub.len() || parts.is_empty() {
            parts.push(part);
        }
        let title_height = wrap(&table.title, TITLE, FRAME).len() as f32 * TITLE * LEADING;
        let label = table.outline.as_deref().unwrap_or(&table.title);
        for (i, part) in parts.iter().enumerate() {
            let x: Vec<f32> = part
                .iter()
                .scan(MARGIN, |x, &c| {
                    let at = *x;
                    *x += widths[c];
                    Some(at)
                })
                .collect();
            // the header rows of this part: each header cell cut to the part's columns
            let header: Vec<Vec<(f32, f32, Cell)>> = header_cells
                .iter()
                .map(|row| {
                    row.iter()
                        .filter_map(|h| {
                            let inside: Vec<usize> = (0..part.len())
                                .filter(|&k| (h.first..h.first + h.span).contains(&part[k]))
                                .collect();
                            let (first, last) = (*inside.first()?, *inside.last()?);
                            let w: f32 = (first..=last).map(|k| widths[part[k]]).sum();
                            let lines = wrap(&h.label, size, w - 2.0 * PAD.0 * scale);
                            let align = match h.first < table.row_headers {
                                true => Anchor::Start,
                                false => Anchor::Middle,
                            };
                            let header = true;
                            Some((
                                x[first],
                                w,
                                Cell {
                                    lines,
                                    align,
                                    header,
                                },
                            ))
                        })
                        .collect()
                })
                .collect();
            let rows: Vec<Vec<(f32, f32, Cell)>> = body
                .iter()
                .map(|row| {
                    part.iter()
                        .enumerate()
                        .map(|(k, &c)| {
                            let (text, align) = &row[c];
                            let lines = wrap(text, size, widths[c] - 2.0 * PAD.0 * scale);
                            let (align, header) = (*align, false);
                            (
                                x[k],
                                widths[c],
                                Cell {
                                    lines,
                                    align,
                                    header,
                                },
                            )
                        })
                        .collect()
                })
                .collect();
            let height = |row: &[(f32, f32, Cell)]| {
                let lines = row.iter().map(|(_, _, c)| c.lines.len()).max().unwrap_or(1);
                lines.max(1) as f32 * size * LEADING + 2.0 * PAD.1 * scale
            };
            let header_height: f32 = header.iter().map(|r| height(r)).sum();
            let first_row = rows.first().map_or(0.0, |r| height(r));
            let lead = if i == 0 { title_height + 4.0 } else { 0.0 };
            self.room(lead + header_height + first_row);
            if i == 0 {
                self.mark(1, label);
                self.paragraph(&table.title, TITLE, self.palette.ink, true);
                self.y += 4.0;
            }
            for row in &header {
                self.row(row, height(row), size, scale);
            }
            for row in &rows {
                let h = height(row);
                if self.y + h > BOTTOM {
                    self.new_page();
                    for row in &header {
                        self.row(row, height(row), size, scale);
                    }
                }
                self.row(row, h, size, scale);
            }
            self.y += 8.0;
        }
        if let Some(caption) = &table.caption {
            self.paragraph(caption, NOTE, self.palette.muted, false);
            self.y += 4.0;
        }
    }

    /// One row of a table: each cell's box, its tint when it is a header cell, and its text.
    fn row(&mut self, cells: &[(f32, f32, Cell)], height: f32, size: f32, scale: f32) {
        let top = self.y;
        for (x, w, cell) in cells {
            let corners = vec![
                [*x, top],
                [x + w, top],
                [x + w, top + height],
                [*x, top + height],
            ];
            self.op(Op::Path {
                points: corners,
                closed: true,
                fill: cell.header.then_some(self.palette.tint),
                stroke: Some((self.palette.rule.mix(self.palette.muted, 0.35), 0.5)),
            });
            for (n, line) in cell.lines.iter().enumerate() {
                let width = picture::text_width(line, size);
                let pad = PAD.0 * scale;
                let left = match cell.align {
                    Anchor::Start => x + pad,
                    Anchor::Middle => x + (w - width) / 2.0,
                    Anchor::End => x + w - pad - width,
                };
                let line_top = top + PAD.1 * scale + n as f32 * size * LEADING;
                let color = self.palette.ink;
                self.line(left, line_top, size, line, color, false);
            }
        }
        self.y += height;
    }

    /// A chart as vector shapes, as wide as the frame; false for a kind not drawn yet.
    fn chart(&mut self, chart: &crate::Chart) -> bool {
        let Some(scene) = picture::scene(chart, FILE_WIDTH, Paper::Light) else {
            return false;
        };
        let s = (FRAME / scene.width).min((BOTTOM - MARGIN) / scene.height);
        self.room(scene.height * s);
        self.mark(1, &chart.title);
        let (left, top) = (MARGIN, self.y);
        let at = |[x, y]: [f32; 2]| [left + x * s, top + y * s];
        let shape = |points: Vec<[f32; 2]>, fill: Color| Op::Path {
            points,
            closed: true,
            fill: Some(fill),
            stroke: None,
        };
        for item in &scene.shapes {
            let op = match item {
                Shape::Rect { x, y, w, h, fill } => shape(
                    [[*x, *y], [x + w, *y], [x + w, y + h], [*x, y + h]]
                        .map(at)
                        .to_vec(),
                    *fill,
                ),
                Shape::Line {
                    points,
                    width,
                    color,
                } => Op::Path {
                    points: points.iter().copied().map(at).collect(),
                    closed: false,
                    fill: None,
                    stroke: Some((*color, width * s)),
                },
                Shape::Band { upper, lower, fill } => {
                    let outline = upper.iter().chain(lower.iter().rev());
                    shape(outline.copied().map(at).collect(), *fill)
                }
                Shape::Polygon { points, fill } => {
                    shape(points.iter().copied().map(at).collect(), *fill)
                }
                Shape::Wedge {
                    center,
                    radius,
                    start,
                    end,
                    fill,
                    edge,
                } => {
                    let full = end - start >= std::f32::consts::TAU - 1e-4;
                    let mut points = arc(*center, *radius, *start, *end);
                    if !full {
                        points.push(*center);
                    }
                    Op::Path {
                        points: points.into_iter().map(at).collect(),
                        closed: true,
                        fill: Some(*fill),
                        stroke: Some((*edge, if full { 1.0 } else { 1.5 } * s)),
                    }
                }
                Shape::Circle {
                    center,
                    radius,
                    fill,
                } => {
                    let points = arc(*center, *radius, 0.0, std::f32::consts::TAU);
                    shape(points.into_iter().map(at).collect(), *fill)
                }
                Shape::Text {
                    x,
                    y,
                    text,
                    size,
                    color,
                    anchor,
                } => {
                    let width = picture::text_width(text, size * s);
                    let [x, y] = at([*x, y + 0.36 * size]);
                    let x = match anchor {
                        Anchor::Start => x,
                        Anchor::Middle => x - width / 2.0,
                        Anchor::End => x - width,
                    };
                    Op::Text {
                        x,
                        y,
                        size: size * s,
                        text: text.clone(),
                        color: *color,
                        bold: false,
                    }
                }
            };
            self.op(op);
        }
        self.y += scene.height * s + 14.0;
        true
    }
}

/// Points along a circle from `start` to `end`, clockwise from twelve o'clock, close enough
/// together that the outline looks round.
fn arc(center: [f32; 2], radius: f32, start: f32, end: f32) -> Vec<[f32; 2]> {
    let steps = ((end - start) * radius.sqrt() * 4.0)
        .ceil()
        .clamp(8.0, 360.0) as usize;
    (0..=steps)
        .map(|i| {
            picture::on_circle(
                center,
                radius,
                start + (end - start) * i as f32 / steps as f32,
            )
        })
        .collect()
}

/// Text broken at spaces into lines no wider than `width`; a word wider than that stands on
/// its own line.
// shortcut: a word wider than its column runs over the cell border; break inside words if
// output with very long unbroken labels turns up.
fn wrap(text: &str, size: f32, width: f32) -> Vec<String> {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split(' ') {
            let longer = match line.is_empty() {
                true => word.to_string(),
                false => format!("{line} {word}"),
            };
            if !line.is_empty() && picture::text_width(&longer, size) > width {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = longer;
            }
        }
        lines.push(line);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// The items under the headings of their commands ("" for none), as a PDF file.
pub fn document(items: &[(&str, Item)], options: &Options) -> Vec<u8> {
    let mut layout = Layout::new();
    let mut command = None;
    for (heading, item) in items {
        if command != Some(*heading) {
            command = Some(*heading);
            if !heading.is_empty() {
                layout.heading(heading);
            }
        }
        match item {
            Item::Chart(chart) if layout.chart(chart) => {}
            Item::Chart(chart) => layout.table(&chart.data_table()),
            Item::Table(table) => layout.table(table),
        }
        layout.y += 6.0;
    }
    // the page title at the top and the page number at the foot
    let count = layout.pages.len();
    let muted = layout.palette.muted;
    for (n, page) in layout.pages.iter_mut().enumerate() {
        if let Some(title) = options.title.as_deref().filter(|t| !t.is_empty()) {
            page.push(Op::Text {
                x: MARGIN,
                y: MARGIN / 2.0,
                size: NOTE,
                text: title.to_string(),
                color: muted,
                bold: false,
            });
        }
        let number = format!("{} / {count}", n + 1);
        page.push(Op::Text {
            x: (PAGE.0 - picture::text_width(&number, NOTE)) / 2.0,
            y: PAGE.1 - MARGIN / 2.0,
            size: NOTE,
            text: number,
            color: muted,
            bold: false,
        });
    }
    write(&layout, options)
}

fn deflate(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(bytes).expect("writing to memory");
    encoder.finish().expect("writing to memory")
}

/// The characters 0x80 to 0x9F of the Windows Latin encoding; the others below 0x100 are
/// those of Latin-1.
const WINDOWS: [(u8, char); 27] = [
    (0x80, '\u{20AC}'),
    (0x82, '\u{201A}'),
    (0x83, '\u{0192}'),
    (0x84, '\u{201E}'),
    (0x85, '\u{2026}'),
    (0x86, '\u{2020}'),
    (0x87, '\u{2021}'),
    (0x88, '\u{02C6}'),
    (0x89, '\u{2030}'),
    (0x8A, '\u{0160}'),
    (0x8B, '\u{2039}'),
    (0x8C, '\u{0152}'),
    (0x8E, '\u{017D}'),
    (0x91, '\u{2018}'),
    (0x92, '\u{2019}'),
    (0x93, '\u{201C}'),
    (0x94, '\u{201D}'),
    (0x95, '\u{2022}'),
    (0x96, '\u{2013}'),
    (0x97, '\u{2014}'),
    (0x98, '\u{02DC}'),
    (0x99, '\u{2122}'),
    (0x9A, '\u{0161}'),
    (0x9B, '\u{203A}'),
    (0x9C, '\u{0153}'),
    (0x9E, '\u{017E}'),
    (0x9F, '\u{0178}'),
];

/// The character of a byte in the Windows Latin encoding.
fn windows_char(byte: u8) -> Option<char> {
    match byte {
        0x20..=0x7E | 0xA0..=0xFF => Some(char::from(byte)),
        _ => WINDOWS.iter().find(|(b, _)| *b == byte).map(|(_, c)| *c),
    }
}

/// How text is written: the glyph number of each character in the embedded subset, or the
/// byte of each character in the Windows Latin encoding.
enum Encoding {
    Embedded(BTreeMap<char, u16>),
    Named,
}

impl Encoding {
    fn encode(&self, text: &str) -> Vec<u8> {
        match self {
            Encoding::Embedded(glyphs) => text
                .chars()
                .flat_map(|c| glyphs.get(&c).copied().unwrap_or(0).to_be_bytes())
                .collect(),
            Encoding::Named => text
                .chars()
                .map(|c| match u8::try_from(u32::from(c)) {
                    Ok(b @ (0x20..=0x7E | 0xA0..=0xFF)) => b,
                    _ => WINDOWS
                        .iter()
                        .find(|(_, w)| *w == c)
                        .map_or(b'?', |(b, _)| *b),
                })
                .collect(),
        }
    }
}

const FONT_NAME: &str = "SchibstedGrotesk-Regular";

/// Writes the font: the subset of the glyphs `text` uses with its widths and the map back
/// to the characters, or a reference by name with the widths of the Windows Latin characters.
fn font(
    pdf: &mut Pdf,
    next: &mut impl FnMut() -> Ref,
    font_id: Ref,
    embed: bool,
    text: &BTreeSet<char>,
) -> Encoding {
    use skrifa::MetadataProvider;
    let font = picture::font();
    let metrics = font.metrics(
        skrifa::instance::Size::new(1000.0),
        skrifa::instance::LocationRef::default(),
    );
    let descriptor_id = next();
    let bounds = metrics
        .bounds
        .map_or(Rect::new(-200.0, -300.0, 1200.0, 1000.0), |b| {
            Rect::new(b.x_min, b.y_min, b.x_max, b.y_max)
        });
    let width = |c: char| picture::text_width(&c.to_string(), 1000.0);
    let name = match embed {
        // a subset carries six capitals and a plus sign before the name
        true => format!("SCRAFT+{FONT_NAME}"),
        false => FONT_NAME.to_string(),
    };
    let mut descriptor = pdf.font_descriptor(descriptor_id);
    descriptor
        .name(Name(name.as_bytes()))
        .flags(FontFlags::NON_SYMBOLIC)
        .bbox(bounds)
        .italic_angle(0.0)
        .ascent(metrics.ascent)
        .descent(metrics.descent)
        .cap_height(metrics.cap_height.unwrap_or(metrics.ascent))
        .stem_v(80.0);
    if !embed {
        descriptor.finish();
        let mut dict = pdf.indirect(font_id).dict();
        dict.pair(Name(b"Type"), Name(b"Font"));
        dict.pair(Name(b"Subtype"), Name(b"TrueType"));
        dict.pair(Name(b"BaseFont"), Name(name.as_bytes()));
        dict.pair(Name(b"FirstChar"), 32);
        dict.pair(Name(b"LastChar"), 255);
        dict.insert(Name(b"Widths"))
            .array()
            .items((32..=255u8).map(|b| windows_char(b).map_or(0.0, width)));
        dict.pair(Name(b"Encoding"), Name(b"WinAnsiEncoding"));
        dict.pair(Name(b"FontDescriptor"), descriptor_id);
        dict.finish();
        return Encoding::Named;
    }
    let file_id = next();
    descriptor.font_file2(file_id);
    descriptor.finish();
    let charmap = font.charmap();
    let mut remapper = subsetter::GlyphRemapper::new();
    let mut glyphs = BTreeMap::new();
    for &c in text {
        let old = charmap.map(c).map_or(0, |g| g.to_u32() as u16);
        glyphs.insert(c, remapper.remap(old));
    }
    let subset =
        subsetter::subset(picture::FONT, 0, &remapper).unwrap_or_else(|_| picture::FONT.to_vec());
    let packed = deflate(&subset);
    pdf.stream(file_id, &packed)
        .filter(Filter::FlateDecode)
        .pair(Name(b"Length1"), subset.len() as i32);
    let (cid_id, map_id) = (next(), next());
    let info = pdf_writer::types::SystemInfo {
        registry: Str(b"Adobe"),
        ordering: Str(b"Identity"),
        supplement: 0,
    };
    pdf.type0_font(font_id)
        .base_font(Name(name.as_bytes()))
        .encoding_predefined(Name(b"Identity-H"))
        .descendant_font(cid_id)
        .to_unicode(map_id);
    let mut cid = pdf.cid_font(cid_id);
    cid.subtype(pdf_writer::types::CidFontType::Type2)
        .base_font(Name(name.as_bytes()))
        .system_info(info)
        .font_descriptor(descriptor_id)
        .default_width(0.0)
        .cid_to_gid_map_predefined(Name(b"Identity"));
    let mut widths = cid.widths();
    for (&c, &g) in &glyphs {
        widths.consecutive(g, [width(c)]);
    }
    widths.finish();
    cid.finish();
    let mut cmap = pdf_writer::types::UnicodeCmap::new(Name(b"Custom"), info);
    for (&c, &g) in &glyphs {
        if g != 0 {
            cmap.pair(g, c);
        }
    }
    let cmap = deflate(&cmap.finish());
    pdf.cmap(map_id, &cmap).filter(Filter::FlateDecode);
    Encoding::Embedded(glyphs)
}

/// The opacity of a color when it is not opaque.
fn alpha(color: Color) -> Option<u8> {
    (color.a < 255).then_some(color.a)
}

fn write(layout: &Layout, options: &Options) -> Vec<u8> {
    let mut pdf = Pdf::new();
    let mut n = 0;
    let mut next = || {
        n += 1;
        Ref::new(n)
    };
    let (catalog_id, pages_id, font_id, info_id) = (next(), next(), next(), next());
    let ops = || layout.pages.iter().flatten();
    let text: BTreeSet<char> = ops()
        .filter_map(|op| match op {
            Op::Text { text, .. } => Some(text.chars()),
            Op::Path { .. } => None,
        })
        .flatten()
        .collect();
    let encoding = font(&mut pdf, &mut next, font_id, options.embed_fonts, &text);
    // one transparency state per opacity a shape or a text uses
    let alphas: BTreeSet<u8> = ops()
        .flat_map(|op| match op {
            Op::Text { color, .. } => vec![alpha(*color)],
            Op::Path { fill, stroke, .. } => {
                vec![fill.and_then(alpha), stroke.and_then(|(c, _)| alpha(c))]
            }
        })
        .flatten()
        .collect();
    let states: Vec<(u8, Ref)> = alphas.iter().map(|&a| (a, next())).collect();
    for &(a, id) in &states {
        let a = f32::from(a) / 255.0;
        pdf.ext_graphics(id).non_stroking_alpha(a).stroking_alpha(a);
    }
    let page_ids: Vec<Ref> = layout.pages.iter().map(|_| next()).collect();
    for (ops, &page_id) in layout.pages.iter().zip(&page_ids) {
        let content_id = next();
        let mut content = Content::new();
        content.set_line_join(LineJoinStyle::RoundJoin);
        for op in ops {
            draw(&mut content, op, &encoding);
        }
        let packed = deflate(&content.finish());
        pdf.stream(content_id, &packed).filter(Filter::FlateDecode);
        let mut page = pdf.page(page_id);
        page.media_box(Rect::new(0.0, 0.0, PAGE.0, PAGE.1))
            .parent(pages_id)
            .contents(content_id);
        let mut resources = page.resources();
        resources.fonts().pair(Name(b"F1"), font_id);
        if !states.is_empty() {
            let mut dict = resources.ext_g_states();
            for (a, id) in &states {
                dict.pair(Name(format!("A{a}").as_bytes()), *id);
            }
        }
    }
    pdf.pages(pages_id)
        .kids(page_ids.iter().copied())
        .count(page_ids.len() as i32);
    let mut info = pdf.document_info(info_id);
    info.producer(TextStr("Statscraft"));
    info.title(TextStr(
        options.title.as_deref().unwrap_or("Statscraft output"),
    ));
    info.finish();
    let outline = (options.bookmarks && !layout.marks.is_empty()).then(&mut next);
    let mut catalog = pdf.catalog(catalog_id);
    catalog.pages(pages_id);
    if let Some(outline_id) = outline {
        catalog
            .outlines(outline_id)
            .page_mode(PageMode::UseOutlines);
    }
    catalog.finish();
    if let Some(outline_id) = outline {
        bookmarks(&mut pdf, &mut next, outline_id, &layout.marks, &page_ids);
    }
    pdf.finish()
}

fn set_color(content: &mut Content, color: Color, stroke: bool) {
    let [r, g, b] = [color.r, color.g, color.b].map(|v| f32::from(v) / 255.0);
    match stroke {
        true => content.set_stroke_rgb(r, g, b),
        false => content.set_fill_rgb(r, g, b),
    };
}

fn draw(content: &mut Content, op: &Op, encoding: &Encoding) {
    let flip = |y: f32| PAGE.1 - y;
    match op {
        Op::Text {
            x,
            y,
            size,
            text,
            color,
            bold,
        } => {
            content.save_state();
            if let Some(a) = alpha(*color) {
                content.set_parameters(Name(format!("A{a}").as_bytes()));
            }
            content.begin_text();
            content.set_font(Name(b"F1"), *size);
            set_color(content, *color, false);
            if *bold {
                // the font has no bold face: the letters get a thin outline of their color
                set_color(content, *color, true);
                content.set_line_width(size * 0.035);
                content.set_text_rendering_mode(TextRenderingMode::FillStroke);
            }
            content.set_text_matrix([1.0, 0.0, 0.0, 1.0, *x, flip(*y)]);
            content.show(Str(&encoding.encode(text)));
            content.end_text();
            content.restore_state();
        }
        Op::Path {
            points,
            closed,
            fill,
            stroke,
        } => {
            let Some(([x, y], rest)) = points.split_first() else {
                return;
            };
            content.save_state();
            let a = fill.and_then(alpha).or(stroke.and_then(|(c, _)| alpha(c)));
            if let Some(a) = a {
                content.set_parameters(Name(format!("A{a}").as_bytes()));
            }
            if let Some(color) = fill {
                set_color(content, *color, false);
            }
            if let Some((color, width)) = stroke {
                set_color(content, *color, true);
                content.set_line_width(*width);
            }
            content.move_to(*x, flip(*y));
            for [x, y] in rest {
                content.line_to(*x, flip(*y));
            }
            if *closed {
                content.close_path();
            }
            match (fill, stroke) {
                (Some(_), Some(_)) => content.fill_nonzero_and_stroke(),
                (Some(_), None) => content.fill_nonzero(),
                (None, _) => content.stroke(),
            };
            content.restore_state();
        }
    }
}

/// The outline: an entry per command with an entry per item below it; items without a
/// command stand at the top level.
fn bookmarks(
    pdf: &mut Pdf,
    next: &mut impl FnMut() -> Ref,
    outline_id: Ref,
    marks: &[Mark],
    pages: &[Ref],
) {
    // (the entry, its children)
    let mut tree: Vec<(&Mark, Vec<&Mark>)> = Vec::new();
    for mark in marks {
        match (mark.level, tree.last_mut()) {
            (1, Some((parent, children))) if parent.level == 0 => children.push(mark),
            _ => tree.push((mark, Vec::new())),
        }
    }
    let ids: Vec<(Ref, Vec<Ref>)> = tree
        .iter()
        .map(|(_, children)| (next(), children.iter().map(|_| next()).collect()))
        .collect();
    let total = ids.iter().map(|(_, c)| 1 + c.len()).sum::<usize>();
    pdf.outline(outline_id)
        .first(ids[0].0)
        .last(ids[ids.len() - 1].0)
        .count(total as i32);
    let dest = |d: Destination, mark: &Mark| {
        d.page(pages[mark.page])
            .xyz(0.0, PAGE.1 - mark.y + 4.0, None)
    };
    for (i, ((mark, children), (id, child_ids))) in tree.iter().zip(&ids).enumerate() {
        let mut item = pdf.outline_item(*id);
        item.title(TextStr(&mark.title)).parent(outline_id);
        if i > 0 {
            item.prev(ids[i - 1].0);
        }
        if let Some((next_id, _)) = ids.get(i + 1) {
            item.next(*next_id);
        }
        if let (Some(first), Some(last)) = (child_ids.first(), child_ids.last()) {
            item.first(*first).last(*last).count(child_ids.len() as i32);
        }
        dest(item.dest(), mark);
        item.finish();
        for (k, (child, child_id)) in children.iter().zip(child_ids).enumerate() {
            let mut item = pdf.outline_item(*child_id);
            item.title(TextStr(&child.title)).parent(*id);
            if k > 0 {
                item.prev(child_ids[k - 1]);
            }
            if let Some(next_id) = child_ids.get(k + 1) {
                item.next(*next_id);
            }
            dest(item.dest(), child);
        }
    }
}
