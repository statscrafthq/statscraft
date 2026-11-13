//! Charts as pictures. `scene` lays a chart out as a short list of shapes in pixels: rectangles,
//! lines, filled bands, pie slices, circles and text. The window paints those shapes with its
//! own painter, `svg` writes them as an SVG file and `png` renders that SVG to a PNG, so every
//! picture of a chart is the same drawing.
//!
//! The look follows the brand: flat shapes, no shadows, petrol for one end of a scale and amber
//! for the other, labels in muted text, thin rule lines, Schibsted Grotesk with tabular digits.

mod layout;

pub use crate::html::figure;
pub use layout::scene;

use crate::Chart;
use statscraft_syntax::{ImageExport, ImageFormat, ImageTone};
use std::fmt::Write as _;
use std::sync::{Arc, OnceLock};

/// Schibsted Grotesk with tabular digits, the window's own font (SIL Open Font License).
pub const FONT: &[u8] = include_bytes!("../../gui/fonts/SchibstedGrotesk.ttf");
const FAMILY: &str = "Schibsted Grotesk";

/// The width of saved pictures and of the picture beside a copied chart, in pixels.
pub const FILE_WIDTH: f32 = 640.0;

/// Light or dark paper, as the window's output shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paper {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    /// Opacity, 255 for opaque.
    pub a: u8,
}

impl Color {
    pub const fn hex(rgb: u32) -> Color {
        Color {
            r: (rgb >> 16) as u8,
            g: (rgb >> 8) as u8,
            b: rgb as u8,
            a: 255,
        }
    }

    pub fn alpha(self, a: u8) -> Color {
        Color { a, ..self }
    }

    pub fn mix(self, other: Color, t: f32) -> Color {
        let m = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * t).round() as u8;
        Color {
            r: m(self.r, other.r),
            g: m(self.g, other.g),
            b: m(self.b, other.b),
            a: 255,
        }
    }

    fn css(self) -> String {
        format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
    }
}

/// How a text stands at its point: starting there, centred on it or ending there. Vertically
/// the point is the middle of the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Shape {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        fill: Color,
    },
    /// A line through two or more points.
    Line {
        points: Vec<[f32; 2]>,
        width: f32,
        color: Color,
    },
    /// The area between two lines with the same x values, such as a stacked area.
    Band {
        upper: Vec<[f32; 2]>,
        lower: Vec<[f32; 2]>,
        fill: Color,
    },
    /// A filled convex polygon through its corners in order, such as a territory of a map.
    Polygon { points: Vec<[f32; 2]>, fill: Color },
    /// A pie slice: angles in radians, clockwise from twelve o'clock, with an edge between it
    /// and its neighbours.
    Wedge {
        center: [f32; 2],
        radius: f32,
        start: f32,
        end: f32,
        fill: Color,
        edge: Color,
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        fill: Color,
    },
    Text {
        x: f32,
        y: f32,
        text: String,
        size: f32,
        color: Color,
        anchor: Anchor,
    },
}

/// A chart laid out in pixels, from (0, 0) at the top left to (width, height).
#[derive(Debug, Clone, PartialEq)]
pub struct Scene {
    pub width: f32,
    pub height: f32,
    pub background: Color,
    pub shapes: Vec<Shape>,
}

/// The colors of one paper.
pub(crate) struct Palette {
    pub paper: Color,
    pub ink: Color,
    pub muted: Color,
    pub rule: Color,
    pub amber: Color,
    /// Petrol, light petrol, light amber, amber: the two sides of a scale.
    pub ramp: [Color; 4],
    /// The middle of a scale with an odd number of steps, neither side.
    pub neutral: Color,
    /// Quiet panels, such as the boxes of a tree diagram.
    pub tint: Color,
}

impl Palette {
    pub fn of(paper: Paper) -> Palette {
        match paper {
            Paper::Light => Palette {
                paper: Color::hex(0xFFFFFF),
                ink: Color::hex(0x12191C),
                muted: Color::hex(0x55656A),
                rule: Color::hex(0xDCE3E5),
                amber: Color::hex(0xD9942B),
                ramp: [0x0B4F5C, 0x6FA7B2, 0xEDC07A, 0xC98318].map(Color::hex),
                neutral: Color::hex(0xAAB6B9),
                tint: Color::hex(0xEEF4F5),
            },
            // the ends brighter than the middle, so the ends stand out from the dark paper as
            // they do from the light one
            Paper::Dark => Palette {
                paper: Color::hex(0x0C1517),
                ink: Color::hex(0xE4EDEF),
                muted: Color::hex(0x93A9AE),
                rule: Color::hex(0x1F2D30),
                amber: Color::hex(0xF0B54A),
                ramp: [0x5CC2D2, 0x2D6E79, 0x9C7230, 0xF0B54A].map(Color::hex),
                neutral: Color::hex(0x4A5A5E),
                tint: Color::hex(0x132024),
            },
        }
    }

    /// One color per series or slice, in order: the first half petrol, from dark to light, the
    /// second half amber, from light to dark, and the middle one neutral when the count is odd.
    /// Four series take the brand's four chart colors.
    pub fn series(&self, n: usize) -> Vec<Color> {
        let half = n / 2;
        // the k-th of `half` shades from a side's strong color to its light one
        let shade = |strong: Color, light: Color, k: usize| match half {
            1 => strong,
            _ => strong.mix(light, k as f32 / (half - 1) as f32),
        };
        (0..n)
            .map(|i| match n {
                1 => self.ramp[0],
                _ if i < half => shade(self.ramp[0], self.ramp[1], i),
                _ if i >= n - half => shade(self.ramp[3], self.ramp[2], n - 1 - i),
                _ => self.neutral,
            })
            .collect()
    }
}

pub(crate) fn font() -> skrifa::FontRef<'static> {
    skrifa::FontRef::new(FONT).expect("the bundled font parses")
}

/// The width of a text in the bundled font at `size` pixels, as the window and the SVG set it.
pub fn text_width(text: &str, size: f32) -> f32 {
    use skrifa::MetadataProvider;
    let font = font();
    let charmap = font.charmap();
    let metrics = font.glyph_metrics(
        skrifa::instance::Size::new(size),
        skrifa::instance::LocationRef::default(),
    );
    text.chars()
        .map(|c| {
            let glyph = charmap.map(c).unwrap_or_default();
            metrics.advance_width(glyph).unwrap_or(size * 0.55)
        })
        .sum()
}

/// A number as SVG writes it: at most two decimals, no trailing zeros.
fn num(v: f32) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    match s {
        "-0" => "0".into(),
        s => s.into(),
    }
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn fill(color: Color) -> String {
    match color.a {
        255 => format!("fill=\"{}\"", color.css()),
        a => format!(
            "fill=\"{}\" fill-opacity=\"{}\"",
            color.css(),
            num(a as f32 / 255.0)
        ),
    }
}

fn points(points: &[[f32; 2]]) -> String {
    let p: Vec<String> = points
        .iter()
        .map(|[x, y]| format!("{},{}", num(*x), num(*y)))
        .collect();
    p.join(" ")
}

/// The point on a circle at an angle clockwise from twelve o'clock.
pub fn on_circle(center: [f32; 2], radius: f32, angle: f32) -> [f32; 2] {
    [
        center[0] + radius * angle.sin(),
        center[1] - radius * angle.cos(),
    ]
}

/// The scene as an SVG document.
pub fn svg(scene: &Scene) -> String {
    let (w, h) = (num(scene.width), num(scene.height));
    let mut out = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" \
         viewBox=\"0 0 {w} {h}\" font-family=\"'{FAMILY}', sans-serif\" \
         style=\"font-variant-numeric: tabular-nums\">\n\
         <rect width=\"{w}\" height=\"{h}\" {}/>\n",
        fill(scene.background)
    );
    for shape in &scene.shapes {
        let _ = match shape {
            Shape::Rect {
                x,
                y,
                w,
                h,
                fill: f,
            } => writeln!(
                out,
                "<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\" {}/>",
                num(*x),
                num(*y),
                num(*w),
                num(*h),
                fill(*f)
            ),
            Shape::Line {
                points: p,
                width,
                color,
            } => writeln!(
                out,
                "<polyline points=\"{}\" fill=\"none\" stroke=\"{}\" stroke-width=\"{}\" \
                 stroke-linejoin=\"round\"/>",
                points(p),
                color.css(),
                num(*width)
            ),
            Shape::Band {
                upper,
                lower,
                fill: f,
            } => {
                let mut outline = upper.clone();
                outline.extend(lower.iter().rev());
                writeln!(
                    out,
                    "<polygon points=\"{}\" {}/>",
                    points(&outline),
                    fill(*f)
                )
            }
            Shape::Polygon { points: p, fill: f } => {
                writeln!(out, "<polygon points=\"{}\" {}/>", points(p), fill(*f))
            }
            Shape::Wedge {
                center,
                radius,
                start,
                end,
                fill: f,
                edge,
            } if end - start >= std::f32::consts::TAU - 1e-4 => writeln!(
                out,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {} stroke=\"{}\"/>",
                num(center[0]),
                num(center[1]),
                num(*radius),
                fill(*f),
                edge.css()
            ),
            Shape::Wedge {
                center,
                radius,
                start,
                end,
                fill: f,
                edge,
            } => {
                let [x0, y0] = on_circle(*center, *radius, *start);
                let [x1, y1] = on_circle(*center, *radius, *end);
                let large = u8::from(end - start > std::f32::consts::PI);
                writeln!(
                    out,
                    "<path d=\"M{},{} L{},{} A{r},{r} 0 {large} 1 {},{} Z\" {} stroke=\"{}\" \
                     stroke-width=\"1.5\" stroke-linejoin=\"round\"/>",
                    num(center[0]),
                    num(center[1]),
                    num(x0),
                    num(y0),
                    num(x1),
                    num(y1),
                    fill(*f),
                    edge.css(),
                    r = num(*radius),
                )
            }
            Shape::Circle {
                center,
                radius,
                fill: f,
            } => writeln!(
                out,
                "<circle cx=\"{}\" cy=\"{}\" r=\"{}\" {}/>",
                num(center[0]),
                num(center[1]),
                num(*radius),
                fill(*f)
            ),
            Shape::Text {
                x,
                y,
                text,
                size,
                color,
                anchor,
            } => {
                let anchor = match anchor {
                    Anchor::Start => "start",
                    Anchor::Middle => "middle",
                    Anchor::End => "end",
                };
                // from the middle of the line to its baseline: half the height of a capital
                let baseline = y + 0.36 * size;
                writeln!(
                    out,
                    "<text x=\"{}\" y=\"{}\" font-size=\"{}\" fill=\"{}\" text-anchor=\"{anchor}\">{}</text>",
                    num(*x),
                    num(baseline),
                    num(*size),
                    color.css(),
                    escape(text)
                )
            }
        };
    }
    out.push_str("</svg>\n");
    out
}

/// The fonts the PNG renderer knows: the bundled one, for every family the SVG names.
fn fonts() -> Arc<resvg::usvg::fontdb::Database> {
    static FONTS: OnceLock<Arc<resvg::usvg::fontdb::Database>> = OnceLock::new();
    FONTS
        .get_or_init(|| {
            let mut db = resvg::usvg::fontdb::Database::new();
            db.load_font_data(FONT.to_vec());
            db.set_sans_serif_family(FAMILY);
            Arc::new(db)
        })
        .clone()
}

/// The scene as a PNG image, `scale` pixels per scene pixel (2 for sharp text on a screen
/// that scales).
pub fn png(scene: &Scene, scale: f32) -> Result<Vec<u8>, String> {
    pixmap(scene, scale)?
        .encode_png()
        .map_err(|e| e.to_string())
}

/// The scene rendered at `scale` pixels per scene pixel.
fn pixmap(scene: &Scene, scale: f32) -> Result<resvg::tiny_skia::Pixmap, String> {
    let options = resvg::usvg::Options {
        fontdb: fonts(),
        ..Default::default()
    };
    let tree = resvg::usvg::Tree::from_str(&svg(scene), &options).map_err(|e| e.to_string())?;
    let (w, h) = (
        (scene.width * scale).ceil() as u32,
        (scene.height * scale).ceil() as u32,
    );
    let mut pixmap =
        resvg::tiny_skia::Pixmap::new(w, h).ok_or_else(|| format!("no picture of {w}x{h}"))?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    Ok(pixmap)
}

/// The extension and the media type of a picture file in `format`.
pub fn file_kind(format: ImageFormat) -> (&'static str, &'static str) {
    match format {
        ImageFormat::Bmp => ("bmp", "image/bmp"),
        ImageFormat::Emf => ("emf", "image/emf"),
        ImageFormat::Eps => ("eps", "application/postscript"),
        ImageFormat::Jpg => ("jpg", "image/jpeg"),
        ImageFormat::Png => ("png", "image/png"),
        ImageFormat::Tif => ("tif", "image/tiff"),
    }
}

/// The scene as a picture file in a raster format, `scale` pixels per scene pixel, in color,
/// in shades of gray or in black and white. EMF and EPS are not written yet.
pub fn raster(
    scene: &Scene,
    scale: f32,
    format: ImageFormat,
    tone: ImageTone,
) -> Result<Vec<u8>, String> {
    use image::{DynamicImage, ImageFormat as Codec};
    let codec = match format {
        ImageFormat::Png if tone == ImageTone::Color => return png(scene, scale),
        ImageFormat::Png => Codec::Png,
        ImageFormat::Jpg => Codec::Jpeg,
        ImageFormat::Bmp => Codec::Bmp,
        ImageFormat::Tif => Codec::Tiff,
        ImageFormat::Emf | ImageFormat::Eps => {
            let (extension, _) = file_kind(format);
            return Err(format!(
                "{} pictures are not written yet",
                extension.to_uppercase()
            ));
        }
    };
    let pixmap = pixmap(scene, scale)?;
    let (w, h) = (pixmap.width(), pixmap.height());
    // the paper is opaque, so the premultiplied pixels are the plain ones
    let rgba = image::RgbaImage::from_raw(w, h, pixmap.take())
        .ok_or_else(|| format!("no picture of {w}x{h}"))?;
    let rgba = DynamicImage::ImageRgba8(rgba);
    let picture = match tone {
        ImageTone::Color => DynamicImage::ImageRgb8(rgba.to_rgb8()),
        ImageTone::Gray => DynamicImage::ImageLuma8(rgba.to_luma8()),
        ImageTone::BlackWhite => {
            let mut gray = rgba.to_luma8();
            gray.pixels_mut()
                .for_each(|p| p.0[0] = if p.0[0] < 128 { 0 } else { 255 });
            DynamicImage::ImageLuma8(gray)
        }
    };
    let mut out = std::io::Cursor::new(Vec::new());
    let written = match codec {
        // sharper text than the encoder's default quality of 75
        Codec::Jpeg => picture.write_with_encoder(
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 90),
        ),
        codec => picture.write_to(&mut out, codec),
    };
    written.map_err(|e| e.to_string())?;
    Ok(out.into_inner())
}

/// A chart as a picture file with the settings of an image subcommand, on light paper:
/// PERCENTSIZE of the PNG `chart_png` writes. None for a kind not drawn yet.
pub fn chart_file(chart: &Chart, settings: &ImageExport) -> Option<Result<Vec<u8>, String>> {
    let scale = 2.0 * settings.percent as f32 / 100.0;
    scene(chart, FILE_WIDTH, Paper::Light)
        .map(|s| raster(&s, scale, settings.format, settings.tone))
}

/// A chart as an SVG file at the width of saved pictures; None for a kind not drawn yet.
pub fn chart_svg(chart: &Chart, paper: Paper) -> Option<String> {
    scene(chart, FILE_WIDTH, paper).map(|s| svg(&s))
}

/// A chart as a PNG file at the width of saved pictures, twice as sharp; None for a kind not
/// drawn yet.
pub fn chart_png(chart: &Chart, paper: Paper) -> Option<Result<Vec<u8>, String>> {
    scene(chart, FILE_WIDTH, paper).map(|s| png(&s, 2.0))
}
