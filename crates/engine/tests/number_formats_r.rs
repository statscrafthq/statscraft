//! Numeric formats against R, written and read, from the guide's descriptions of the formats.
//!
//! Writing: `STRING(value, format)` of invented values in F, COMMA, DOT, DOLLAR, PCT, E, N and
//! PIBHEX at several widths and decimals. A half is rounded away from zero (R's `sprintf` rounds
//! an exact half to even, so the rounding is written out), a value below one has no zero before
//! the decimal point in F and PCT, the thousands separator is a comma in COMMA and DOLLAR and a
//! period in DOT (with the comma as the decimal mark), a negative DOLLAR value is `-$12.50`, E
//! has the exponent with a sign and three digits, a width that is too small drops decimals one
//! at a time and then gives asterisks.
//!
//! Reading: DATA LIST FIXED of the same formats from texts with and without a decimal point (the
//! decimals of the format are implied without one), signs, thousands separators, exponents (`E`,
//! `D`, a bare sign, `E` and a blank), the signs and letters of a zoned decimal (Z: the last digit written as `A` to `I`,
//! `J` to `R`, `{` or `}`, which also give the sign) and the hexadecimal digits of PIBHEX, and
//! the texts that cannot be read.

use statscraft_engine::Engine;
use statscraft_syntax::parse;

/// System-missing in a table.
const NAN: f64 = f64::NAN;

/// The format, a value and the text the format writes for it.
const WRITES: &[(&str, f64, &str)] = &[
    ("F8.2", 0.0, "     .00"),
    ("F10.2", 0.0, "       .00"),
    ("F6.0", 0.0, "     0"),
    ("F12.3", 0.0, "        .000"),
    ("F9.1", 0.0, "       .0"),
    ("F8.2", 1.0, "    1.00"),
    ("F10.2", 1.0, "      1.00"),
    ("F6.0", 1.0, "     1"),
    ("F12.3", 1.0, "       1.000"),
    ("F9.1", 1.0, "      1.0"),
    ("F8.2", -1.0, "   -1.00"),
    ("F10.2", -1.0, "     -1.00"),
    ("F6.0", -1.0, "    -1"),
    ("F12.3", -1.0, "      -1.000"),
    ("F9.1", -1.0, "     -1.0"),
    ("F8.2", 0.5, "     .50"),
    ("F10.2", 0.5, "       .50"),
    ("F6.0", 0.5, "     1"),
    ("F12.3", 0.5, "        .500"),
    ("F9.1", 0.5, "       .5"),
    ("F8.2", -0.5, "    -.50"),
    ("F10.2", -0.5, "      -.50"),
    ("F6.0", -0.5, "    -1"),
    ("F12.3", -0.5, "       -.500"),
    ("F9.1", -0.5, "      -.5"),
    ("F8.2", 0.125, "     .13"),
    ("F10.2", 0.125, "       .13"),
    ("F6.0", 0.125, "     0"),
    ("F12.3", 0.125, "        .125"),
    ("F9.1", 0.125, "       .1"),
    ("F8.2", 2.5, "    2.50"),
    ("F10.2", 2.5, "      2.50"),
    ("F6.0", 2.5, "     3"),
    ("F12.3", 2.5, "       2.500"),
    ("F9.1", 2.5, "      2.5"),
    ("F8.2", -2.5, "   -2.50"),
    ("F10.2", -2.5, "     -2.50"),
    ("F6.0", -2.5, "    -3"),
    ("F12.3", -2.5, "      -2.500"),
    ("F9.1", -2.5, "     -2.5"),
    ("F8.2", 12.345, "   12.35"),
    ("F10.2", 12.345, "     12.35"),
    ("F6.0", 12.345, "    12"),
    ("F12.3", 12.345, "      12.345"),
    ("F9.1", 12.345, "     12.3"),
    ("F8.2", -12.345, "  -12.35"),
    ("F10.2", -12.345, "    -12.35"),
    ("F6.0", -12.345, "   -12"),
    ("F12.3", -12.345, "     -12.345"),
    ("F9.1", -12.345, "    -12.3"),
    ("F8.2", 1234.5678, " 1234.57"),
    ("F10.2", 1234.5678, "   1234.57"),
    ("F6.0", 1234.5678, "  1235"),
    ("F12.3", 1234.5678, "    1234.568"),
    ("F9.1", 1234.5678, "   1234.6"),
    ("F8.2", -1234.5678, "-1234.57"),
    ("F10.2", -1234.5678, "  -1234.57"),
    ("F6.0", -1234.5678, " -1235"),
    ("F12.3", -1234.5678, "   -1234.568"),
    ("F9.1", -1234.5678, "  -1234.6"),
    ("F8.2", 123456.789, "123456.8"),
    ("F10.2", 123456.789, " 123456.79"),
    ("F6.0", 123456.789, "123457"),
    ("F12.3", 123456.789, "  123456.789"),
    ("F9.1", 123456.789, " 123456.8"),
    ("F8.2", 0.04, "     .04"),
    ("F10.2", 0.04, "       .04"),
    ("F6.0", 0.04, "     0"),
    ("F12.3", 0.04, "        .040"),
    ("F9.1", 0.04, "       .0"),
    ("F8.2", 99.995, "  100.00"),
    ("F10.2", 99.995, "    100.00"),
    ("F6.0", 99.995, "   100"),
    ("F12.3", 99.995, "      99.995"),
    ("F9.1", 99.995, "    100.0"),
    ("F8.2", 1000000.0, " 1000000"),
    ("F10.2", 1000000.0, "1000000.00"),
    ("F12.3", 1000000.0, " 1000000.000"),
    ("F9.1", 1000000.0, "1000000.0"),
    ("F8.2", 7.0, "    7.00"),
    ("F10.2", 7.0, "      7.00"),
    ("F6.0", 7.0, "     7"),
    ("F12.3", 7.0, "       7.000"),
    ("F9.1", 7.0, "      7.0"),
    ("COMMA14.2", 1.0, "          1.00"),
    ("DOT14.2", 1.0, "          1,00"),
    ("DOLLAR14.2", 1.0, "         $1.00"),
    ("COMMA16.2", 1.0, "            1.00"),
    ("DOT16.2", 1.0, "            1,00"),
    ("DOLLAR16.2", 1.0, "           $1.00"),
    ("COMMA12.0", 1.0, "           1"),
    ("DOT12.0", 1.0, "           1"),
    ("DOLLAR12.0", 1.0, "          $1"),
    ("COMMA14.2", -1.0, "         -1.00"),
    ("DOT14.2", -1.0, "         -1,00"),
    ("DOLLAR14.2", -1.0, "        -$1.00"),
    ("COMMA16.2", -1.0, "           -1.00"),
    ("DOT16.2", -1.0, "           -1,00"),
    ("DOLLAR16.2", -1.0, "          -$1.00"),
    ("COMMA12.0", -1.0, "          -1"),
    ("DOT12.0", -1.0, "          -1"),
    ("DOLLAR12.0", -1.0, "         -$1"),
    ("COMMA14.2", 12.5, "         12.50"),
    ("DOT14.2", 12.5, "         12,50"),
    ("DOLLAR14.2", 12.5, "        $12.50"),
    ("COMMA16.2", 12.5, "           12.50"),
    ("DOT16.2", 12.5, "           12,50"),
    ("DOLLAR16.2", 12.5, "          $12.50"),
    ("COMMA12.0", 12.5, "          13"),
    ("DOT12.0", 12.5, "          13"),
    ("DOLLAR12.0", 12.5, "         $13"),
    ("COMMA14.2", -12.5, "        -12.50"),
    ("DOT14.2", -12.5, "        -12,50"),
    ("DOLLAR14.2", -12.5, "       -$12.50"),
    ("COMMA16.2", -12.5, "          -12.50"),
    ("DOT16.2", -12.5, "          -12,50"),
    ("DOLLAR16.2", -12.5, "         -$12.50"),
    ("COMMA12.0", -12.5, "         -13"),
    ("DOT12.0", -12.5, "         -13"),
    ("DOLLAR12.0", -12.5, "        -$13"),
    ("COMMA14.2", 1234.5678, "      1,234.57"),
    ("DOT14.2", 1234.5678, "      1.234,57"),
    ("DOLLAR14.2", 1234.5678, "     $1,234.57"),
    ("COMMA16.2", 1234.5678, "        1,234.57"),
    ("DOT16.2", 1234.5678, "        1.234,57"),
    ("DOLLAR16.2", 1234.5678, "       $1,234.57"),
    ("COMMA12.0", 1234.5678, "       1,235"),
    ("DOT12.0", 1234.5678, "       1.235"),
    ("DOLLAR12.0", 1234.5678, "      $1,235"),
    ("COMMA14.2", -1234.5678, "     -1,234.57"),
    ("DOT14.2", -1234.5678, "     -1.234,57"),
    ("DOLLAR14.2", -1234.5678, "    -$1,234.57"),
    ("COMMA16.2", -1234.5678, "       -1,234.57"),
    ("DOT16.2", -1234.5678, "       -1.234,57"),
    ("DOLLAR16.2", -1234.5678, "      -$1,234.57"),
    ("COMMA12.0", -1234.5678, "      -1,235"),
    ("DOT12.0", -1234.5678, "      -1.235"),
    ("DOLLAR12.0", -1234.5678, "     -$1,235"),
    ("COMMA14.2", 123456.789, "    123,456.79"),
    ("DOT14.2", 123456.789, "    123.456,79"),
    ("DOLLAR14.2", 123456.789, "   $123,456.79"),
    ("COMMA16.2", 123456.789, "      123,456.79"),
    ("DOT16.2", 123456.789, "      123.456,79"),
    ("DOLLAR16.2", 123456.789, "     $123,456.79"),
    ("COMMA12.0", 123456.789, "     123,457"),
    ("DOT12.0", 123456.789, "     123.457"),
    ("DOLLAR12.0", 123456.789, "    $123,457"),
    ("COMMA14.2", 1000000.0, "  1,000,000.00"),
    ("DOT14.2", 1000000.0, "  1.000.000,00"),
    ("DOLLAR14.2", 1000000.0, " $1,000,000.00"),
    ("COMMA16.2", 1000000.0, "    1,000,000.00"),
    ("DOT16.2", 1000000.0, "    1.000.000,00"),
    ("DOLLAR16.2", 1000000.0, "   $1,000,000.00"),
    ("COMMA12.0", 1000000.0, "   1,000,000"),
    ("DOT12.0", 1000000.0, "   1.000.000"),
    ("DOLLAR12.0", 1000000.0, "  $1,000,000"),
    ("COMMA14.2", 7.0, "          7.00"),
    ("DOT14.2", 7.0, "          7,00"),
    ("DOLLAR14.2", 7.0, "         $7.00"),
    ("COMMA16.2", 7.0, "            7.00"),
    ("DOT16.2", 7.0, "            7,00"),
    ("DOLLAR16.2", 7.0, "           $7.00"),
    ("COMMA12.0", 7.0, "           7"),
    ("DOT12.0", 7.0, "           7"),
    ("DOLLAR12.0", 7.0, "          $7"),
    ("COMMA14.2", 999999.995, "  1,000,000.00"),
    ("DOT14.2", 999999.995, "  1.000.000,00"),
    ("DOLLAR14.2", 999999.995, " $1,000,000.00"),
    ("COMMA16.2", 999999.995, "    1,000,000.00"),
    ("DOT16.2", 999999.995, "    1.000.000,00"),
    ("DOLLAR16.2", 999999.995, "   $1,000,000.00"),
    ("COMMA12.0", 999999.995, "   1,000,000"),
    ("DOT12.0", 999999.995, "   1.000.000"),
    ("DOLLAR12.0", 999999.995, "  $1,000,000"),
    ("PCT8.1", 45.25, "   45.3%"),
    ("PCT8.2", 45.25, "  45.25%"),
    ("PCT6.0", 45.25, "   45%"),
    ("PCT8.1", -3.5, "   -3.5%"),
    ("PCT8.2", -3.5, "  -3.50%"),
    ("PCT6.0", -3.5, "   -4%"),
    ("PCT8.1", 0.5, "     .5%"),
    ("PCT8.2", 0.5, "    .50%"),
    ("PCT6.0", 0.5, "    1%"),
    ("PCT8.1", 100.0, "  100.0%"),
    ("PCT8.2", 100.0, " 100.00%"),
    ("PCT6.0", 100.0, "  100%"),
    ("PCT8.1", 12.345, "   12.3%"),
    ("PCT8.2", 12.345, "  12.35%"),
    ("PCT6.0", 12.345, "   12%"),
    ("PCT8.1", 0.0, "     .0%"),
    ("PCT8.2", 0.0, "    .00%"),
    ("PCT6.0", 0.0, "    0%"),
    ("E10.3", 12345.6789, "1.235E+004"),
    ("E12.5", 12345.6789, "1.23457E+004"),
    ("E9.2", 12345.6789, "1.23E+004"),
    ("E10.3", -12345.6789, "-1.23E+004"),
    ("E12.5", -12345.6789, "-1.2346E+004"),
    ("E9.2", -12345.6789, "-1.2E+004"),
    ("E10.3", 0.000123456, "1.235E-004"),
    ("E12.5", 0.000123456, "1.23456E-004"),
    ("E9.2", 0.000123456, "1.23E-004"),
    ("E10.3", 5.0, "5.000E+000"),
    ("E12.5", 5.0, "5.00000E+000"),
    ("E9.2", 5.0, "5.00E+000"),
    ("E10.3", 10000000000.0, "1.000E+010"),
    ("E12.5", 10000000000.0, "1.00000E+010"),
    ("E9.2", 10000000000.0, "1.00E+010"),
    ("E10.3", 0.000015, "1.500E-005"),
    ("E12.5", 0.000015, "1.50000E-005"),
    ("E9.2", 0.000015, "1.50E-005"),
    ("E10.3", 99999.5, "1.000E+005"),
    ("E12.5", 99999.5, "9.99995E+004"),
    ("E9.2", 99999.5, "1.00E+005"),
    ("N5", 0.0, "00000"),
    ("N8", 0.0, "00000000"),
    ("N5", 7.0, "00007"),
    ("N8", 7.0, "00000007"),
    ("N5", 42.0, "00042"),
    ("N8", 42.0, "00000042"),
    ("N5", 1234.0, "01234"),
    ("N8", 1234.0, "00001234"),
    ("N5", 99999.0, "99999"),
    ("N8", 99999.0, "00099999"),
    ("PIBHEX4", 0.0, "0000"),
    ("PIBHEX8", 0.0, "00000000"),
    ("PIBHEX4", 10.0, "000A"),
    ("PIBHEX8", 10.0, "0000000A"),
    ("PIBHEX4", 255.0, "00FF"),
    ("PIBHEX8", 255.0, "000000FF"),
    ("PIBHEX4", 4660.0, "1234"),
    ("PIBHEX8", 4660.0, "00001234"),
    ("PIBHEX4", 65535.0, "FFFF"),
    ("PIBHEX8", 65535.0, "0000FFFF"),
    ("PIBHEX8", 3735928559.0, "DEADBEEF"),
];

/// The format, a text and the value it reads as.
const READS: &[(&str, &str, f64)] = &[
    ("F8.2", "12.5", 12.5),
    ("F8.2", "1250", 12.5),
    ("F8.2", "  -3.75", -3.75),
    ("F8.2", "+5", 0.05),
    ("F8.2", ".5", 0.5),
    ("F8.2", "5.", 5.0),
    ("F8.2", "1.25E2", 125.0),
    ("F8.2", "1.25E+2", 125.0),
    ("F8.2", "1.25D2", 125.0),
    ("F8.2", "12abc", NAN),
    ("F8.2", "1,234", NAN),
    ("F8.2", "1 2", NAN),
    ("F8.2", "-", NAN),
    ("F8.2", "--5", NAN),
    ("F8.2", "0012", 0.12),
    ("F8.2", "  ", NAN),
    ("F6.0", "123", 123.0),
    ("F6.0", "-45", -45.0),
    ("F6.0", "12.7", 12.7),
    ("F6.0", "1e2", 100.0),
    ("COMMA10.2", "1,234.5", 1234.5),
    ("COMMA10.2", "1234.5", 1234.5),
    ("COMMA10.2", "1,2,3", 1.23),
    ("COMMA10.2", "-1,000", -10.0),
    ("COMMA10.2", "1.2.3", NAN),
    ("COMMA10.2", "12345", 123.45),
    ("COMMA10.2", "1,234x", NAN),
    ("COMMA14.2", "12,345,678.90", 12345678.9),
    ("DOT10.2", "1.234,5", 1234.5),
    ("DOT10.2", "1234,5", 1234.5),
    ("DOT10.2", "1.2.3", 1.23),
    ("DOT10.2", "-1.000", -10.0),
    ("DOT10.2", "1,2,3", NAN),
    ("DOT10.2", "12345", 123.45),
    ("DOT14.2", "12.345.678,90", 12345678.9),
    ("DOLLAR10.2", "$1,234.50", 1234.5),
    ("DOLLAR10.2", "$12", 0.12),
    ("DOLLAR10.2", "1,234", 12.34),
    ("DOLLAR10.2", "-$12.50", -12.5),
    ("DOLLAR10.2", "$-12.50", -12.5),
    ("DOLLAR10.2", "$1,2,3", 1.23),
    ("DOLLAR10.2", "1234", 12.34),
    ("PCT8.1", "45%", 4.5),
    ("PCT8.1", "45.5%", 45.5),
    ("PCT8.1", "45", 4.5),
    ("PCT8.1", "-3%", -0.3),
    ("PCT8.1", "%45", NAN),
    ("PCT8.1", "4%5", NAN),
    ("PCT8.1", "455", 45.5),
    ("N6", "000042", 42.0),
    ("N6", "42", 42.0),
    ("N6", "007", 7.0),
    ("N6", "4a", NAN),
    ("N6", "-1", NAN),
    ("N6", "4.2", NAN),
    ("N6", "1 2", NAN),
    ("N6.2", "000123", 1.23),
    ("N6.2", "123", 1.23),
    ("E10.3", "1.5E3", 1500.0),
    ("E10.3", "1.5+3", 1500.0),
    ("E10.3", "15E-1", 1.5),
    ("E10.3", "1.5E 3", 1500.0),
    ("E10.3", "-2.5E-2", -0.025),
    ("E10.3", "1500", 1.5),
    ("E10.3", "1.5E", NAN),
    ("E10.3", "E3", NAN),
    ("Z5", "5071A", 50711.0),
    ("Z5", "5071E", 50715.0),
    ("Z5", "5071I", 50719.0),
    ("Z5", "5071J", -50711.0),
    ("Z5", "5071M", -50714.0),
    ("Z5", "5071R", -50719.0),
    ("Z5", "5071{", 50710.0),
    ("Z5", "5071}", -50710.0),
    ("Z5", "50719", 50719.0),
    ("Z5", "5071a", NAN),
    ("Z5", "50 71", NAN),
    ("Z5.2", "5071A", 507.11),
    ("Z5.2", "5071J", -507.11),
    ("Z5.2", "5.07M", -5.074),
    ("Z5.2", "5071{", 507.1),
    ("PIBHEX4", "01A4", 420.0),
    ("PIBHEX4", "7FFF", 32767.0),
    ("PIBHEX4", "00C8", 200.0),
    ("PIBHEX4", "0E3D", 3645.0),
    ("PIBHEX4", "ABCD", 43981.0),
    ("PIBHEX4", "ab12", 43794.0),
    ("PIBHEX4", "F", NAN),
    ("PIBHEX4", "00G0", NAN),
    ("PIBHEX4", "1 2 ", NAN),
    ("PIBHEX4", "   .", NAN),
    ("PIBHEX8", "DEADBEEF", 3735928559.0),
    ("PIBHEX8", "00010000", 65536.0),
];

fn execute(engine: &mut Engine, source: &str) -> Result<(), String> {
    let (commands, errors) = parse(source);
    if let Some(error) = errors.first() {
        return Err(error.message.clone());
    }
    for c in &commands {
        engine.execute(&c.command).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[test]
fn writing_agrees_with_r() {
    let mut engine = Engine::default();
    execute(
        &mut engine,
        "DATA LIST FREE /x.\nBEGIN DATA\n1\nEND DATA.\nSTRING s (A60).\n",
    )
    .unwrap();
    let mut bad = Vec::new();
    for (format, value, want) in WRITES {
        if let Err(e) = execute(
            &mut engine,
            &format!("COMPUTE s = STRING({value}, {format})."),
        ) {
            bad.push(format!("{format} {value}: error {e}"));
            continue;
        }
        let got = engine
            .dataset()
            .unwrap()
            .variable("s")
            .unwrap()
            .string()
            .unwrap()
            .get(0)
            .to_string();
        if got != *want {
            bad.push(format!("{format} of {value}: {got:?} against {want:?}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ:\n{}",
        bad.len(),
        WRITES.len(),
        bad.join("\n")
    );
}

/// The type, the width and the decimals of a format written as `F8.2` or `PIBHEX4`.
fn parts(format: &str) -> (String, usize, Option<usize>) {
    let kind: String = format
        .chars()
        .take_while(char::is_ascii_alphabetic)
        .collect();
    let rest = &format[kind.len()..];
    let (width, decimals) = match rest.split_once('.') {
        Some((w, d)) => (w, d.parse().ok()),
        None => (rest, None),
    };
    (kind, width.parse().unwrap(), decimals)
}

#[test]
fn reading_agrees_with_r() {
    let mut bad = Vec::new();
    for (format, text, want) in READS {
        let mut engine = Engine::default();
        let (kind, w, decimals) = parts(format);
        // the columns give the width, the format its type and decimals: (F,2)
        let spec = match decimals {
            Some(d) => format!("{kind},{d}"),
            None => kind,
        };
        let source = format!(
            "DATA LIST FIXED /v 1-{w} ({spec}) k {} (F1).\nBEGIN DATA\n{text:<w$}1\nEND DATA.\n",
            w + 1
        );
        if let Err(e) = execute(&mut engine, &source) {
            bad.push(format!("{format} {text:?}: error {e}"));
            continue;
        }
        let data = engine.dataset().unwrap();
        let got = data.variable("v").unwrap().numeric().unwrap().get(0);
        let fine = match got {
            None => want.is_nan(),
            Some(g) => !want.is_nan() && (g - want).abs() <= 1e-9 * want.abs() + 1e-12,
        };
        if !fine {
            bad.push(format!("{format} {text:?}: {got:?} against {want}"));
        }
    }
    assert!(
        bad.is_empty(),
        "{} of {} differ:\n{}",
        bad.len(),
        READS.len(),
        bad.join("\n")
    );
}

#[test]
fn pibhex_widths_are_whole_bytes_and_print_writes_it() {
    // an odd or a too wide PIBHEX is refused where it is written, and a number is rounded to the
    // whole number nearest before it is written in hexadecimal
    for source in [
        "DATA LIST FIXED /h 1-3 (PIBHEX3).\nBEGIN DATA\nABC\nEND DATA.\n",
        "DATA LIST FIXED /h 1-18 (PIBHEX18).\nBEGIN DATA\nABC\nEND DATA.\n",
    ] {
        let mut engine = Engine::default();
        let error = execute(&mut engine, source).unwrap_err();
        assert!(error.contains("PIBHEX needs an even width"), "{error}");
    }
    let mut engine = Engine::default();
    execute(
        &mut engine,
        "DATA LIST FIXED /h 1-4 (PIBHEX).\nBEGIN DATA\n01A4\nEND DATA.\nPRINT /h (PIBHEX4).\nSTRING s (A8).\nCOMPUTE s = STRING(254.6, PIBHEX4).\nEXECUTE.\n",
    )
    .unwrap();
    let got = engine
        .dataset()
        .unwrap()
        .variable("s")
        .unwrap()
        .string()
        .unwrap()
        .get(0)
        .to_string();
    assert_eq!(got, "00FF");
}
