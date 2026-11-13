//! OUTPUT EXPORT /PDF: the file is a PDF with its cross-reference table, a long table runs
//! over several pages, a wide one is split into parts that each repeat the row labels, the
//! text can be read back through the font's map to the characters, charts are vector shapes,
//! and the bookmarks and the font follow EMBEDBOOKMARKS and EMBEDFONTS. Made-up data: the
//! litres of milk a small dairy sold on each of 120 days, and the products of four
//! market stalls.
//!
//! With STATSCRAFT_PDF_OUT set to a folder, the files are also written there, for the check
//! by hand in a PDF reader.

use statscraft_engine::session::Session;
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;

fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("statscraft-pdf-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// The decoded streams of a PDF file.
fn streams(pdf: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut rest = pdf;
    while let Some(at) = find(rest, b"stream\n") {
        let body = &rest[at + 7..];
        let end = find(body, b"\nendstream").expect("the end of a stream");
        let mut decoded = Vec::new();
        let mut decoder = flate2::read::ZlibDecoder::new(&body[..end]);
        if decoder.read_to_end(&mut decoded).is_ok() {
            out.push(decoded);
        }
        rest = &body[end + 10..];
    }
    out
}

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// The bytes of every string a page shows with Tj, literal or hexadecimal.
fn shown(content: &[u8]) -> Vec<Vec<u8>> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < content.len() {
        match content[i] {
            b'(' => {
                let mut bytes = Vec::new();
                i += 1;
                while content[i] != b')' {
                    if content[i] == b'\\' {
                        i += 1;
                        match content[i] {
                            b'0'..=b'7' => {
                                let digits = content[i..].iter().take(3);
                                let octal = digits.take_while(|d| d.is_ascii_digit()).count();
                                let text = std::str::from_utf8(&content[i..i + octal]).unwrap();
                                bytes.push(u8::from_str_radix(text, 8).unwrap());
                                i += octal - 1;
                            }
                            b'n' => bytes.push(b'\n'),
                            b'r' => bytes.push(b'\r'),
                            b't' => bytes.push(b'\t'),
                            b'b' => bytes.push(8),
                            b'f' => bytes.push(12),
                            other => bytes.push(other),
                        }
                    } else {
                        bytes.push(content[i]);
                    }
                    i += 1;
                }
                out.push(bytes);
            }
            b'<' if content.get(i + 1) != Some(&b'<') => {
                let end = i + content[i..].iter().position(|&b| b == b'>').unwrap();
                let hex = std::str::from_utf8(&content[i + 1..end]).unwrap();
                let bytes = (0..hex.len() / 2)
                    .map(|k| u8::from_str_radix(&hex[2 * k..2 * k + 2], 16).unwrap())
                    .collect();
                out.push(bytes);
                i = end;
            }
            _ => {}
        }
        i += 1;
    }
    out
}

/// The texts of a file whose font is embedded, read through its map from glyphs to
/// characters, as a reader's copy and search does.
fn embedded_texts(pdf: &[u8]) -> Vec<String> {
    let streams = streams(pdf);
    let mut glyphs: HashMap<u16, char> = HashMap::new();
    for s in &streams {
        let text = String::from_utf8_lossy(s);
        let Some(start) = text.find("beginbfchar") else {
            continue;
        };
        let end = text.find("endbfchar").unwrap();
        for line in text[start + 11..end]
            .lines()
            .filter(|l| !l.trim().is_empty())
        {
            let parts: Vec<&str> = line
                .split(['<', '>', ' '])
                .filter(|p| !p.is_empty())
                .collect();
            let glyph = u16::from_str_radix(parts[0], 16).unwrap();
            let unit = u32::from_str_radix(parts[1], 16).unwrap();
            glyphs.insert(glyph, char::from_u32(unit).unwrap());
        }
    }
    assert!(!glyphs.is_empty(), "a map from glyphs to characters");
    streams
        .iter()
        .filter(|s| find(s, b"Tj").is_some())
        .flat_map(|s| shown(s))
        .map(|bytes| {
            bytes
                .chunks(2)
                .map(|g| {
                    glyphs
                        .get(&u16::from_be_bytes([g[0], g[1]]))
                        .copied()
                        .unwrap_or('?')
                })
                .collect()
        })
        .collect()
}

/// The texts of a file whose font is not embedded: bytes of the Windows Latin encoding.
fn named_texts(pdf: &[u8]) -> Vec<String> {
    let streams = streams(pdf);
    streams
        .iter()
        .filter(|s| find(s, b"Tj").is_some())
        .flat_map(|s| shown(s))
        .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
        .collect()
}

fn pages(pdf: &[u8]) -> usize {
    let text = String::from_utf8_lossy(pdf);
    text.matches("/Type /Page\n").count()
        + text.matches("/Type /Page/").count()
        + text.matches("/Type /Page ").count()
}

fn session(dir: &std::path::Path) -> Session {
    let mut s = Session::default();
    // made up: day number and litres of milk, one day in seven a market day
    let mut days = String::new();
    for day in 1..=120 {
        days.push_str(&format!("{day} {}\n", 200 + (day * 37) % 90));
    }
    // made up: which of 26 products each of four stalls sold, once per product
    let mut stalls = String::from("DATA LIST FREE /stall product.\nBEGIN DATA\n");
    for stall in 1..=4 {
        for product in 101..=126 {
            stalls.push_str(&format!("{stall} {product}\n"));
        }
    }
    stalls.push_str("END DATA.\n");
    let source = format!(
        "CD '{}'.\nTITLE 'Dairy sales'.\nDATA LIST FREE /day litres.\nBEGIN DATA\n{days}END DATA.\n\
         LIST.\nFREQUENCIES litres /FORMAT=NOTABLE /HISTOGRAM.\n{stalls}\
         CROSSTABS stall BY product.\nDESCRIPTIVES stall product.\n",
        dir.display()
    );
    s.run(&source, None).unwrap_or_else(|e| panic!("{e:?}"));
    s
}

fn export(s: &mut Session, dir: &std::path::Path, syntax: &str, name: &str) -> Vec<u8> {
    s.run(syntax, None)
        .unwrap_or_else(|e| panic!("{syntax}\n{e:?}"));
    let bytes = std::fs::read(dir.join(name)).unwrap();
    if let Some(out) = std::env::var_os("STATSCRAFT_PDF_OUT") {
        std::fs::write(PathBuf::from(out).join(name), &bytes).unwrap();
    }
    bytes
}

#[test]
fn output_export_writes_pdf_files() {
    let dir = folder("all");
    let mut s = session(&dir);
    let pdf = export(
        &mut s,
        &dir,
        "OUTPUT EXPORT /PDF DOCUMENTFILE='all.pdf'.",
        "all.pdf",
    );
    // a PDF file: its header, its cross-reference table and its end
    assert!(pdf.starts_with(b"%PDF-1."));
    assert!(find(&pdf, b"\nxref\n0 ").is_some());
    assert!(find(&pdf, b"startxref").is_some() && pdf.ends_with(b"%%EOF"));
    // 120 listed days run over more than one page, and the other items follow
    assert!(pages(&pdf) >= 4, "{} pages", pages(&pdf));
    let texts = embedded_texts(&pdf);
    for wanted in [
        "Dairy sales",
        "LIST",
        "120",
        "1 / ",
        "litres",
        "Descriptive Statistics",
    ] {
        assert!(
            texts.iter().any(|t| t == wanted || t.starts_with(wanted)),
            "{wanted}: {texts:?}"
        );
    }
    // the header rows repeat on every page of the long list: "day" heads each of them
    let heads = texts.iter().filter(|t| *t == "day").count();
    assert!(heads >= 3, "{heads} headers");
    // the 26 products do not fit across, so the crosstabulation comes in parts, each with
    // the row labels again: each part has the five rows of counts
    let rows = texts.iter().filter(|t| *t == "Count").count();
    assert_eq!(rows, 10, "{texts:?}");
    assert!(texts.iter().any(|t| t == "126"), "the last product");
    // the histogram is drawn with shapes and text, not as a picture
    assert!(find(&pdf, b"/Subtype /Image").is_none());
    assert!(texts.iter().any(|t| t == "Frequency"), "{texts:?}");
    // the font goes in as a subset, and the outline is there
    assert!(find(&pdf, b"/FontFile2").is_some());
    assert!(find(&pdf, b"/SCRAFT+SchibstedGrotesk-Regular").is_some());
    assert!(find(&pdf, b"/Outlines").is_some() && find(&pdf, b"/UseOutlines").is_some());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn bookmarks_and_fonts_can_be_left_out() {
    let dir = folder("plain");
    let mut s = session(&dir);
    let syntax = "OUTPUT MODIFY /SELECT TABLES /IF COMMANDS=['descriptives'] \
                  /OBJECTPROPERTIES VISIBLE=NO.\n\
                  OUTPUT EXPORT /CONTENTS EXPORT=VISIBLE \
                  /PDF DOCUMENTFILE='plain.pdf' EMBEDBOOKMARKS=NO EMBEDFONTS=NO.";
    let pdf = export(&mut s, &dir, syntax, "plain.pdf");
    assert!(pdf.starts_with(b"%PDF-1."));
    assert!(find(&pdf, b"/Outlines").is_none());
    assert!(find(&pdf, b"/FontFile2").is_none());
    assert!(find(&pdf, b"/WinAnsiEncoding").is_some());
    let texts = named_texts(&pdf);
    assert!(texts.iter().any(|t| t == "126"), "{texts:?}");
    // the hidden table stays out
    assert!(
        !texts.iter().any(|t| t == "Descriptive Statistics"),
        "{texts:?}"
    );
    std::fs::remove_dir_all(dir).unwrap();
}
