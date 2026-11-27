//! Text with letters of two and three bytes (Estonian letters, a few others, a letter written as base
//! plus combining mark) in the data management commands, against R on sixteen invented words: the
//! length in bytes (`nchar` type bytes) and in characters, UPCASE and LOWER (R's `toupper` and
//! `tolower`, which map one letter to one letter), the character functions (`substr`, `regexpr`,
//! `gregexpr`, padding written out), STRUNC (a cut at a byte count that never goes inside a letter),
//! REPLACE, CONCAT, comparison by code points, the cut of a text to a shorter string width,
//! SORT CASES, AUTORECODE, RECODE and AGGREGATE by a text (R's `order`, `match`, `table` in the
//! byte order of UTF-8), and a table keyed by a text in MATCH FILES.
//! Not compared: a letter whose case mapping is not one letter (a dotted capital I), where R's C
//! library maps by its own table; ß and the fi ligature stay as they are in both.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    columns: &'static [(&'static str, &'static str)],
}

const DATA: &str = r####"DATA LIST LIST /k (F3) w (A20).
BEGIN DATA
1 'Õie'
2 'Šarlota'
3 'Žanna'
4 'Jüri'
5 'õun'
6 'ÜLLE'
7 'mäe'
8 'ööbik'
9 'Ääre'
10 'kuu'
11 'Åsa'
12 'naïve'
13 'ǆß'
14 '日本語'
15 'kuu'
16 'Jüri'
END DATA.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"length in bytes and in characters"####,
        command: r####"COMPUTE nbytes = LENGTH(RTRIM(w)).
COMPUTE nchars = CHAR.LENGTH(w)."####,
        columns: &[
            (
                r####"nbytes"####,
                r####"4 8 6 5 4 5 4 7 6 3 4 6 4 9 3 6"####,
            ),
            (
                r####"nchars"####,
                r####"3 7 5 4 3 4 3 5 4 3 3 5 2 3 3 5"####,
            ),
        ],
    },
    Case {
        label: r####"a letter written as a base letter and a combining mark is not the precomposed letter: one more character, one more byte"####,
        command: r####"COMPUTE same = (w = 'Jüri')."####,
        columns: &[(r####"same"####, r####"0 0 0 1 0 0 0 0 0 0 0 0 0 0 0 0"####)],
    },
    Case {
        label: r####"UPCASE and LOWER change letters one for one"####,
        command: r####"STRING up lo (A24).
COMPUTE up = UPCASE(w).
COMPUTE lo = LOWER(w)."####,
        columns: &[
            (
                r####"up"####,
                r####"ÕIE|ŠARLOTA|ŽANNA|JÜRI|ÕUN|ÜLLE|MÄE|ÖÖBIK|ÄÄRE|KUU|ÅSA|NAÏVE|Ǆß|日本語|KUU|JÜRI"####,
            ),
            (
                r####"lo"####,
                r####"õie|šarlota|žanna|jüri|õun|ülle|mäe|ööbik|ääre|kuu|åsa|naïve|ǆß|日本語|kuu|jüri"####,
            ),
        ],
    },
    Case {
        label: r####"CHAR.SUBSTR and SUBSTR count characters"####,
        command: r####"STRING s1 s2 s3 (A24).
COMPUTE s1 = CHAR.SUBSTR(w, 2, 3).
COMPUTE s2 = CHAR.SUBSTR(w, 3).
COMPUTE s3 = SUBSTR(w, 1, 2)."####,
        columns: &[
            (
                r####"s1"####,
                r####"ie|arl|ann|üri|un|LLE|äe|öbi|äre|uu|sa|aïv|ß|本語|uu|ür"####,
            ),
            (
                r####"s2"####,
                r####"e|rlota|nna|ri|n|LE|e|bik|re|u|a|ïve||語|u|̈ri"####,
            ),
            (
                r####"s3"####,
                r####"Õi|Ša|Ža|Jü|õu|ÜL|mä|öö|Ää|ku|Ås|na|ǆß|日本|ku|Ju"####,
            ),
        ],
    },
    Case {
        label: r####"CHAR.INDEX and CHAR.RINDEX give character positions"####,
        command: r####"COMPUTE i1 = CHAR.INDEX(w, 'ä').
COMPUTE i2 = CHAR.RINDEX(w, 'ö').
COMPUTE i3 = CHAR.INDEX(w, 'ü').
COMPUTE i4 = CHAR.INDEX(w, '語')."####,
        columns: &[
            (r####"i1"####, r####"0 0 0 0 0 0 2 0 2 0 0 0 0 0 0 0"####),
            (r####"i2"####, r####"0 0 0 0 0 0 0 2 0 0 0 0 0 0 0 0"####),
            (r####"i3"####, r####"0 0 0 2 0 0 0 0 0 0 0 0 0 0 0 0"####),
            (r####"i4"####, r####"0 0 0 0 0 0 0 0 0 0 0 0 0 3 0 0"####),
        ],
    },
    Case {
        label: r####"CHAR.LPAD and CHAR.RPAD pad by characters"####,
        command: r####"STRING p1 p2 (A24).
COMPUTE p1 = CHAR.LPAD(RTRIM(w), 8, '*').
COMPUTE p2 = CHAR.RPAD(RTRIM(w), 8, '.')."####,
        columns: &[
            (
                r####"p1"####,
                r####"*****Õie|*Šarlota|***Žanna|****Jüri|*****õun|****ÜLLE|*****mäe|***ööbik|****Ääre|*****kuu|*****Åsa|***naïve|******ǆß|*****日本語|*****kuu|***Jüri"####,
            ),
            (
                r####"p2"####,
                r####"Õie.....|Šarlota.|Žanna...|Jüri....|õun.....|ÜLLE....|mäe.....|ööbik...|Ääre....|kuu.....|Åsa.....|naïve...|ǆß......|日本語.....|kuu.....|Jüri..."####,
            ),
        ],
    },
    Case {
        label: r####"STRUNC cuts at a byte count and never inside a letter"####,
        command: r####"STRING t4 t5 (A24).
COMPUTE t4 = STRUNC(w, 4).
COMPUTE t5 = STRUNC(w, 5)."####,
        columns: &[
            (
                r####"t4"####,
                r####"Õie|Šar|Žan|Jür|õun|ÜLL|mäe|öö|Ää|kuu|Åsa|naï|ǆß|日|kuu|Jü"####,
            ),
            (
                r####"t5"####,
                r####"Õie|Šarl|Žann|Jüri|õun|ÜLLE|mäe|ööb|Äär|kuu|Åsa|naïv|ǆß|日|kuu|Jür"####,
            ),
        ],
    },
    Case {
        label: r####"REPLACE and CONCAT"####,
        command: r####"STRING r1 r2 (A40).
COMPUTE r1 = REPLACE(w, 'õ', 'o').
COMPUTE r2 = CONCAT(RTRIM(w), '/', UPCASE(RTRIM(w)))."####,
        columns: &[
            (
                r####"r1"####,
                r####"Õie|Šarlota|Žanna|Jüri|oun|ÜLLE|mäe|ööbik|Ääre|kuu|Åsa|naïve|ǆß|日本語|kuu|Jüri"####,
            ),
            (
                r####"r2"####,
                r####"Õie/ÕIE|Šarlota/ŠARLOTA|Žanna/ŽANNA|Jüri/JÜRI|õun/ÕUN|ÜLLE/ÜLLE|mäe/MÄE|ööbik/ÖÖBIK|Ääre/ÄÄRE|kuu/KUU|Åsa/ÅSA|naïve/NAÏVE|ǆß/Ǆß|日本語/日本語|kuu/KUU|Jüri/JÜRI"####,
            ),
        ],
    },
    Case {
        label: r####"a text is compared by the code points of its letters"####,
        command: r####"COMPUTE below = (w < 'Õie').
COMPUTE atleast = (w >= 'ä')."####,
        columns: &[
            (r####"below"####, r####"0 0 0 1 0 0 1 0 1 1 1 1 0 0 1 1"####),
            (
                r####"atleast"####,
                r####"0 1 1 0 1 0 0 1 0 0 0 0 1 1 0 0"####,
            ),
        ],
    },
    Case {
        label: r####"a text longer than the width is cut at the last whole letter that fits"####,
        command: r####"STRING c5 (A5).
COMPUTE c5 = w."####,
        columns: &[(
            r####"c5"####,
            r####"Õie|Šarl|Žann|Jüri|õun|ÜLLE|mäe|ööb|Äär|kuu|Åsa|naïv|ǆß|日|kuu|Jür"####,
        )],
    },
    Case {
        label: r####"SORT CASES ascending is by code points, equal texts keep their order"####,
        command: r####"SORT CASES BY w (A)."####,
        columns: &[
            (
                r####"k"####,
                r####"16 4 10 15 7 12 9 11 1 6 5 8 2 3 13 14"####,
            ),
            (
                r####"w"####,
                r####"Jüri|Jüri|kuu|kuu|mäe|naïve|Ääre|Åsa|Õie|ÜLLE|õun|ööbik|Šarlota|Žanna|ǆß|日本語"####,
            ),
        ],
    },
    Case {
        label: r####"SORT CASES descending"####,
        command: r####"SORT CASES BY w (D)."####,
        columns: &[
            (
                r####"k"####,
                r####"14 13 3 2 8 5 6 1 11 9 12 7 10 15 4 16"####,
            ),
            (
                r####"w"####,
                r####"日本語|ǆß|Žanna|Šarlota|ööbik|õun|ÜLLE|Õie|Åsa|Ääre|naïve|mäe|kuu|kuu|Jüri|Jüri"####,
            ),
        ],
    },
    Case {
        label: r####"AUTORECODE numbers the texts in code point order"####,
        command: r####"AUTORECODE VARIABLES=w /INTO wn."####,
        columns: &[(
            r####"wn"####,
            r####"8 12 13 2 10 9 4 11 6 3 7 5 14 15 3 1"####,
        )],
    },
    Case {
        label: r####"RECODE of a text with letters outside ASCII"####,
        command: r####"STRING rc (A2).
RECODE w ('õun'='A') ('Õie' 'ÜLLE'='B') (ELSE='C') INTO rc."####,
        columns: &[(r####"rc"####, r####"B|C|C|C|A|B|C|C|C|C|C|C|C|C|C|C"####)],
    },
    Case {
        label: r####"AGGREGATE by a text: groups in code point order, the count and the first case"####,
        command: r####"AGGREGATE OUTFILE=* /BREAK=w /n=N /firstk=FIRST(k)."####,
        columns: &[
            (
                r####"w"####,
                r####"Jüri|Jüri|kuu|mäe|naïve|Ääre|Åsa|Õie|ÜLLE|õun|ööbik|Šarlota|Žanna|ǆß|日本語"####,
            ),
            (r####"n"####, r####"1 1 2 1 1 1 1 1 1 1 1 1 1 1 1"####),
            (
                r####"firstk"####,
                r####"16 4 10 7 12 9 11 1 6 5 8 2 3 13 14"####,
            ),
        ],
    },
    Case {
        label: r####"MATCH FILES with a table keyed by a text"####,
        command: r####"DATASET NAME main.
DATA LIST LIST /w (A20) v (F3).
BEGIN DATA
'kuu' 10
'Åsa' 20
'õun' 30
'日本語' 40
'Žanna' 50
'none' 60
END DATA.
SORT CASES BY w.
DATASET NAME t.
DATASET ACTIVATE main.
SORT CASES BY w.
MATCH FILES /FILE=* /TABLE=t /BY w."####,
        columns: &[
            (
                r####"k"####,
                r####"16 4 10 15 7 12 9 11 1 6 5 8 2 3 13 14"####,
            ),
            (
                r####"v"####,
                r####". . 10 10 . . . 20 . . 30 . . 50 . 40"####,
            ),
        ],
    },
];

fn show(engine: &Engine, name: &str) -> Option<String> {
    let data = engine.dataset()?;
    Some(match data.variable(name).ok()?.values() {
        Values::Numeric(values) => {
            let text = values
                .iter()
                .map(|v| v.map_or(".".into(), |v| v.to_string()));
            text.collect::<Vec<_>>().join(" ")
        }
        Values::String(values) => values
            .iter()
            .map(str::trim_end)
            .collect::<Vec<_>>()
            .join("|"),
    })
}

/// The same values: numbers to 12 significant digits (R shows 15), anything else exactly.
fn same(got: &str, want: &str) -> bool {
    let (got_numbers, want_numbers): (Vec<&str>, Vec<&str>) =
        (got.split(' ').collect(), want.split(' ').collect());
    let number = |t: &&str| *t == "." || t.parse::<f64>().is_ok();
    if got_numbers.len() == want_numbers.len() && got_numbers.iter().all(number) {
        return got_numbers.iter().zip(&want_numbers).all(|(g, w)| {
            match (g.parse::<f64>(), w.parse::<f64>()) {
                (Ok(g), Ok(w)) => (g - w).abs() <= 1e-12 * w.abs().max(1.0),
                _ => g == w,
            }
        });
    }
    got == want
}

fn execute_all(engine: &mut Engine, source: &str) -> Result<(), String> {
    let (commands, errors) = parse(source);
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    for command in &commands {
        engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
    }
    Ok(())
}

#[test]
fn multibyte_text_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        if let Err(error) = execute_all(&mut engine, &format!("{DATA}{}\nEXECUTE.", case.command)) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
