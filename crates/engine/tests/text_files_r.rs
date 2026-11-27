//! Reading text files against R: GET DATA /TYPE=TXT (delimited and fixed) and DATA LIST FREE,
//! LIST and FIXED, on invented files.
//!
//! R reads the raw fields (`read.table`, `scan` and `read.fwf`) and the conversion of a field to
//! a value follows the rules of the manual, written out in the R script: with delimited data
//! consecutive delimiters are an empty value (a number then missing), a qualifier holds a value
//! with delimiters in it, spaces can count as one delimiter, the first case and the number of
//! cases can be set, a case can run over several lines; freefield data do not use the widths and
//! decimals of the formats to read; fixed columns have implied decimals when the number has no
//! decimal point, a blank number is missing and so is one with a blank inside, a record shorter
//! than the columns gives blanks; AUTO makes a variable numeric when enough of its non-blank
//! values are numbers. Also dates, dollar amounts, a decimal comma, Windows line ends and a last
//! line without a line end.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    /// The text of the file.
    file: &'static str,
    /// `{FILE}` stands for the path of the file.
    command: &'static str,
    /// The variables the command makes, in order.
    names: &'static str,
    /// The values of each variable: numbers separated by spaces with `.` for missing, strings
    /// separated by `|`.
    columns: &'static [(&'static str, &'static str)],
}

const CASES: &[Case] = &[
    Case {
        label: r####"comma separated with a qualifier, a header and empty fields"####,
        file: "id,name,amount,note\n1,\"Smith, Ann\",12.5,plain\n2,Bo,,\"two words\"\n3,\"\",7,\"a;b\"\n4,Cy Dee,-3.25,\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /ARRANGEMENT=DELIMITED /DELIMITERS="," /QUALIFIER='"' /FIRSTCASE=2
 /VARIABLES=id F3 name A12 amount F8.2 note A12."####,
        names: r####"id name amount note"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4"####),
            (r####"name"####, r####"Smith, Ann|Bo||Cy Dee"####),
            (r####"amount"####, r####"12.5 . 7 -3.25"####),
            (r####"note"####, r####"plain|two words|a;b|"####),
        ],
    },
    Case {
        label: r####"semicolons, a decimal comma, Windows line ends, no final line end"####,
        file: "n;price;label\r\n1;3,5;kg\r\n2;10;\r\n3;;pieces\r\n4;0,25;l",
        command: r####"SET DECIMAL=COMMA.
GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";" /FIRSTCASE=2
 /VARIABLES=n F2 price F6 label A8."####,
        names: r####"n price label"####,
        columns: &[
            (r####"n"####, r####"1 2 3 4"####),
            (r####"price"####, r####"3.5 10 . 0.25"####),
            (r####"label"####, r####"kg||pieces|l"####),
        ],
    },
    Case {
        label: r####"tab separated, empty fields between tabs"####,
        file: "id\tcolor\tweight\n1\tdark red\t4.5\n2\t\t6\n3\tblue\t\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="\t" /FIRSTCASE=2
 /VARIABLES=id F2 color A10 weight F5.1."####,
        names: r####"id color weight"####,
        columns: &[
            (r####"id"####, r####"1 2 3"####),
            (r####"color"####, r####"dark red||blue"####),
            (r####"weight"####, r####"4.5 6 ."####),
        ],
    },
    Case {
        label: r####"space delimited, consecutive spaces are empty values"####,
        file: "1 apple  3\n2 plum 4\n3  fig 5\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=" "
 /VARIABLES=a F1 b A6 c F1 d A4."####,
        names: r####"a b c d"####,
        columns: &[
            (r####"a"####, r####"1 2 3"####),
            (r####"b"####, r####"apple|plum|"####),
            (r####"c"####, r####". 4 ."####),
            (r####"d"####, r####"3||5"####),
        ],
    },
    Case {
        label: r####"space delimited, multiple spaces as one"####,
        file: "1 apple  3\n2 plum 4\n3  fig 5\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=" " /MULTIPLESPACES IGNORE=YES
 /VARIABLES=a F1 b A6 c F1."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"1 2 3"####),
            (r####"b"####, r####"apple|plum|fig"####),
            (r####"c"####, r####"3 4 5"####),
        ],
    },
    Case {
        label: r####"several delimiters"####,
        file: "7,xy;9\n8,z;1\n3;,5\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=",;"
 /VARIABLES=a F2 b A4 c F2."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"7 8 3"####),
            (r####"b"####, r####"xy|z|"####),
            (r####"c"####, r####"9 1 5"####),
        ],
    },
    Case {
        label: r####"first case and a number of cases"####,
        file: "title line\nskip me too\n10,a\n20,b\n30,c\n40,d\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /FIRSTCASE=3 /IMPORTCASE=FIRST 3
 /VARIABLES=x F3 y A2."####,
        names: r####"x y"####,
        columns: &[
            (r####"x"####, r####"10 20 30"####),
            (r####"y"####, r####"a|b|c"####),
        ],
    },
    Case {
        label: r####"three values a case across lines"####,
        file: "1,2,3,4\n5,6,7,8,9\n10,11,12\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELCASE=VARIABLES 3 /DELIMITERS=","
 /VARIABLES=a F2 b F2 c F2."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"1 4 7 10"####),
            (r####"b"####, r####"2 5 8 11"####),
            (r####"c"####, r####"3 6 9 12"####),
        ],
    },
    Case {
        label: r####"leading spaces of a qualified value ignored"####,
        file: "a, \"  padded\",x\nb,\"tail  \", y\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /QUALIFIER='"' /LEADINGSPACES IGNORE=YES
 /VARIABLES=p A3 q A10 r A3."####,
        names: r####"p q r"####,
        columns: &[
            (r####"p"####, r####"a|b"####),
            (r####"q"####, r####"padded|tail"####),
            (r####"r"####, r####"x|y"####),
        ],
    },
    Case {
        label: r####"dates, dollars and a skipped variable"####,
        file: "12-JAN-2024;$1,234.50;zz;03/15/2023;5\n01-FEB-2024;$20;yy;11/30/2022;6\n29-FEB-2024;;xx;12/01/2022;7\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";"
 /VARIABLES=d DATE11 m DOLLAR10.2 skip X a ADATE10 k F1."####,
        names: r####"d m a k"####,
        columns: &[
            (r####"d"####, r####"13924396800 13926124800 13928544000"####),
            (r####"m"####, r####"1234.5 20 ."####),
            (r####"a"####, r####"13898217600 13889145600 13889232000"####),
            (r####"k"####, r####"5 6 7"####),
        ],
    },
    Case {
        label: r####"FREE with commas and blanks, quoted strings, several cases on a line"####,
        file: "2, 19, 'ab c', 7, 5,, \"x\", 8\n",
        command: r####"DATA LIST FILE='{FILE}' FREE / a (F2) b (F2) c (A5) d (F2)."####,
        names: r####"a b c d"####,
        columns: &[
            (r####"a"####, r####"2 5"####),
            (r####"b"####, r####"19 ."####),
            (r####"c"####, r####"ab c|x"####),
            (r####"d"####, r####"7 8"####),
        ],
    },
    Case {
        label: r####"FREE with blanks, a case across two lines"####,
        file: "3 4 'p q'\n1 5 6 r\n2\n",
        command: r####"DATA LIST FILE='{FILE}' FREE / a (F1) b (F1) c (A4) d (F1)."####,
        names: r####"a b c d"####,
        columns: &[
            (r####"a"####, r####"3 5"####),
            (r####"b"####, r####"4 6"####),
            (r####"c"####, r####"p q|r"####),
            (r####"d"####, r####"1 2"####),
        ],
    },
    Case {
        label: r####"LIST with an explicit comma, empty values"####,
        file: "1,2,3\n4,,6\n7,8,\n",
        command: r####"DATA LIST FILE='{FILE}' LIST (",") / a b c."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"1 4 7"####),
            (r####"b"####, r####"2 . 8"####),
            (r####"c"####, r####"3 6 ."####),
        ],
    },
    Case {
        label: r####"LIST with commas inside quotes (default delimiters)"####,
        file: "1, \"a, b\", 3\n2, 'c d', 4\n",
        command: r####"DATA LIST FILE='{FILE}' LIST / n (F1) s (A6) m (F1)."####,
        names: r####"n s m"####,
        columns: &[
            (r####"n"####, r####"1 2"####),
            (r####"s"####, r####"a, b|c d"####),
            (r####"m"####, r####"3 4"####),
        ],
    },
    Case {
        label: r####"FREE with an explicit delimiter across lines"####,
        file: "1;2;3;4\n5;;7;8\n",
        command: r####"DATA LIST FILE='{FILE}' FREE (";") / a (F1) b (F1) c (F1) d (F1)."####,
        names: r####"a b c d"####,
        columns: &[
            (r####"a"####, r####"1 5"####),
            (r####"b"####, r####"2 ."####),
            (r####"c"####, r####"3 7"####),
            (r####"d"####, r####"4 8"####),
        ],
    },
    Case {
        label: r####"freefield widths and decimals are not used to read"####,
        file: "1234, 56.7\n 89,1.5e2\n",
        command: r####"DATA LIST FILE='{FILE}' LIST / a (F5.2) b (COMMA9.2)."####,
        names: r####"a b"####,
        columns: &[
            (r####"a"####, r####"1234 89"####),
            (r####"b"####, r####"56.7 150"####),
        ],
    },
    Case {
        label: r####"DATA LIST FIXED columns, implied decimals, blanks, an embedded blank, short line"####,
        file: "001 Ann    12345 1\n002 Bo      6.78  \n003 Cy     9     2\n004 Dee    1 2   3\n005 Eve          4\n00x Fay    -1.5  5\n007 Gus\n",
        command: r####"DATA LIST FILE='{FILE}' FIXED / id 1-3 name 5-10 (A) amt 12-16 (2) flag 18."####,
        names: r####"id name amt flag"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 . 7"####),
            (r####"name"####, r####"Ann|Bo|Cy|Dee|Eve|Fay|Gus"####),
            (r####"amt"####, r####"123.45 6.78 0.09 . . -1.5 ."####),
            (r####"flag"####, r####"1 . 2 3 4 5 ."####),
        ],
    },
    Case {
        label: r####"GET DATA fixed columns, 0-based, a skipped column, first case, a number of cases"####,
        file: "001 Ann    12345 1\n002 Bo      6.78  \n003 Cy     9     2\n004 Dee    1 2   3\n005 Eve          4\n00x Fay    -1.5  5\n007 Gus\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /ARRANGEMENT=FIXED /FIRSTCASE=2 /IMPORTCASE=FIRST 4
 /VARIABLES=id 0-2 F3 name 4-9 A6 amt 11-15 F5.2 flag 17-17 F1."####,
        names: r####"id name amt flag"####,
        columns: &[
            (r####"id"####, r####"2 3 4 5"####),
            (r####"name"####, r####"Bo|Cy|Dee|Eve"####),
            (r####"amt"####, r####"6.78 0.09 . ."####),
            (r####"flag"####, r####". 2 3 4"####),
        ],
    },
    Case {
        label: r####"DATA LIST FIXED with two records a case"####,
        file: "12 abc\n 3.5 x\n45 def\n 6   y\n",
        command: r####"DATA LIST FILE='{FILE}' FIXED RECORDS=2 /1 a 1-2 b 4-6 (A) /2 c 1-4 (1) d 6 (A)."####,
        names: r####"a b c d"####,
        columns: &[
            (r####"a"####, r####"12 45"####),
            (r####"b"####, r####"abc|def"####),
            (r####"c"####, r####"3.5 0.6"####),
            (r####"d"####, r####"x|y"####),
        ],
    },
    Case {
        label: r####"GET DATA fixed with FIXCASE=2"####,
        file: "12 abc\n 3.5 x\n45 def\n 6   y\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /ARRANGEMENT=FIXED /FIXCASE=2
 /VARIABLES=/1 a 0-1 F2 b 3-5 A3 /2 c 0-3 F4.1 d 5-5 A1."####,
        names: r####"a b c d"####,
        columns: &[
            (r####"a"####, r####"12 45"####),
            (r####"b"####, r####"abc|def"####),
            (r####"c"####, r####"3.5 0.6"####),
            (r####"d"####, r####"x|y"####),
        ],
    },
    Case {
        label: r####"AUTO at 95 percent"####,
        file: "n,m,k\n1,10,a\n2,11,b\n3,12,c\n4,13,\n5,14,e\n6,15,f\n7,16,g\n8,17,h\n9,18,i\n10,19,j\n11,20,k\n12,21,l\n13,22,m\n14,23,n\n15,24,o\n16,25,p\n17,x,q\n18,27,r\n19,28,s\n20,y,t\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /FIRSTCASE=2
 /VARIABLES=n AUTO m AUTO k AUTO."####,
        names: r####"n m k"####,
        columns: &[
            (
                r####"n"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"m"####,
                r####"10|11|12|13|14|15|16|17|18|19|20|21|22|23|24|25|x|27|28|y"####,
            ),
            (
                r####"k"####,
                r####"a|b|c||e|f|g|h|i|j|k|l|m|n|o|p|q|r|s|t"####,
            ),
        ],
    },
    Case {
        label: r####"AUTO at 85 percent"####,
        file: "n,m,k\n1,10,a\n2,11,b\n3,12,c\n4,13,\n5,14,e\n6,15,f\n7,16,g\n8,17,h\n9,18,i\n10,19,j\n11,20,k\n12,21,l\n13,22,m\n14,23,n\n15,24,o\n16,25,p\n17,x,q\n18,27,r\n19,28,s\n20,y,t\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /FIRSTCASE=2 /DATATYPEMIN PERCENTAGE=85
 /VARIABLES=n AUTO m AUTO k AUTO."####,
        names: r####"n m k"####,
        columns: &[
            (
                r####"n"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20"####,
            ),
            (
                r####"m"####,
                r####"10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 . 27 28 ."####,
            ),
            (
                r####"k"####,
                r####"a|b|c||e|f|g|h|i|j|k|l|m|n|o|p|q|r|s|t"####,
            ),
        ],
    },
    Case {
        label: r####"AUTO with exactly the share asked for"####,
        file: "m,k\n1,100\n2,101\n3,102\n4,103\n5,104\n6,105\n7,106\n8,107\n9,108\n10,109\n11,110\n12,111\n13,112\n14,113\n15,114\n16,115\n17,116\n18,117\n19,118\n20,bad\n21,\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /FIRSTCASE=2
 /VARIABLES=m AUTO k AUTO."####,
        names: r####"m k"####,
        columns: &[
            (
                r####"m"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21"####,
            ),
            (
                r####"k"####,
                r####"100 101 102 103 104 105 106 107 108 109 110 111 112 113 114 115 116 117 118 . ."####,
            ),
        ],
    },
    Case {
        label: r####"GET DATA fixed columns from the first line, implied decimals"####,
        file: "001 Ann    12345 1\n002 Bo      6.78  \n003 Cy     9     2\n004 Dee    1 2   3\n005 Eve          4\n00x Fay    -1.5  5\n007 Gus\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /ARRANGEMENT=FIXED
 /VARIABLES=id 0-2 F3 name 4-9 A6 amt 11-15 F5.2 flag 17-17 F1."####,
        names: r####"id name amt flag"####,
        columns: &[
            (r####"id"####, r####"1 2 3 4 5 . 7"####),
            (r####"name"####, r####"Ann|Bo|Cy|Dee|Eve|Fay|Gus"####),
            (r####"amt"####, r####"123.45 6.78 0.09 . . -1.5 ."####),
            (r####"flag"####, r####"1 . 2 3 4 5 ."####),
        ],
    },
    Case {
        label: r####"a doubled qualifier and a line break inside qualifiers"####,
        file: "id,text\n1,\"say \"\"hi\"\" now\"\n2,\"two\nlines\"\n3,plain\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /QUALIFIER='"' /FIRSTCASE=2
 /VARIABLES=id F1 text A20."####,
        names: r####"id text"####,
        columns: &[
            (r####"id"####, r####"1 2 3"####),
            (
                r####"text"####,
                r####"say "hi" now|two
lines|plain"####,
            ),
        ],
    },
    Case {
        label: r####"DOT and COMMA formats"####,
        file: "1.234,50;1,234.50;7\n,5;.5;8\n12;3,000;9\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS=";"
 /VARIABLES=a DOT9.2 b COMMA9.2 c F1."####,
        names: r####"a b c"####,
        columns: &[
            (r####"a"####, r####"1234.5 0.5 12"####),
            (r####"b"####, r####"1234.5 0.5 3000"####),
            (r####"c"####, r####"7 8 9"####),
        ],
    },
    Case {
        label: r####"AUTO in fixed columns"####,
        file: "12345 abc  1\n   67 .    2\n  8.5 de   .\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /ARRANGEMENT=FIXED
 /VARIABLES=n 0-4 AUTO s 6-9 AUTO k 11-11 AUTO."####,
        names: r####"n s k"####,
        columns: &[
            (r####"n"####, r####"12345 67 8.5"####),
            (r####"s"####, r####"abc|.|de"####),
            (r####"k"####, r####"1 2 ."####),
        ],
    },
    Case {
        label: r####"AUTO where most values are a period"####,
        file: "v,w\n1,.\n2,.\n3,.\n4,.\n5,.\n6,.\n7,.\n8,.\n9,.\n10,.\n11,.\n12,.\n13,.\n14,.\n15,.\n16,.\n17,.\n18,.\n19,.\n20,.\n21,.\n22,.\n23,ann\n24,bob\n25,cy\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /DELIMITERS="," /FIRSTCASE=2
 /VARIABLES=v AUTO w AUTO."####,
        names: r####"v w"####,
        columns: &[
            (
                r####"v"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25"####,
            ),
            (
                r####"w"####,
                r####".|.|.|.|.|.|.|.|.|.|.|.|.|.|.|.|.|.|.|.|.|.|ann|bob|cy"####,
            ),
        ],
    },
    Case {
        label: r####"fixed columns with COMMA and DOLLAR formats"####,
        file: "  1234 $5678.5   9,876\n12,345    $99  1,500.5\n",
        command: r####"DATA LIST FILE='{FILE}' FIXED / c 1-6 (COMMA, 2) d 8-15 (DOLLAR, 2) e 17-23 (COMMA)."####,
        names: r####"c d e"####,
        columns: &[
            (r####"c"####, r####"12.34 123.45"####),
            (r####"d"####, r####"5678.5 0.99"####),
            (r####"e"####, r####"9876 500.5"####),
        ],
    },
    Case {
        label: r####"GET DATA fixed with COMMA and DOLLAR"####,
        file: "  1234 $5678.5   9,876\n12,345    $99  1,500.5\n",
        command: r####"GET DATA /TYPE=TXT /FILE='{FILE}' /ARRANGEMENT=FIXED
 /VARIABLES=c 0-5 COMMA6.2 d 7-14 DOLLAR8.2 e 16-22 COMMA7."####,
        names: r####"c d e"####,
        columns: &[
            (r####"c"####, r####"12.34 123.45"####),
            (r####"d"####, r####"5678.5 0.99"####),
            (r####"e"####, r####"9876 500.5"####),
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

fn run(case: &Case, path: &std::path::Path) -> Result<Engine, String> {
    std::fs::write(path, case.file).unwrap();
    let source = case.command.replace("{FILE}", &path.display().to_string());
    let (commands, errors) = parse(&source);
    if !errors.is_empty() {
        return Err(format!("{errors:?}"));
    }
    let mut engine = Engine::default();
    for command in &commands {
        engine
            .execute(&command.command)
            .map_err(|e| format!("line {}: {e}", command.first_line))?;
    }
    Ok(engine)
}

#[test]
fn reading_text_agrees_with_r() {
    let dir = std::env::temp_dir().join(format!("statscraft-text-r-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    for (index, case) in CASES.iter().enumerate() {
        let engine = match run(case, &dir.join(format!("case{index}.txt"))) {
            Ok(engine) => engine,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        let names: Vec<&str> = engine.dataset().unwrap().variable_names().collect();
        if names.join(" ") != case.names {
            failures.push(format!(
                "{}: variables {}, wanted {}",
                case.label,
                names.join(" "),
                case.names
            ));
            continue;
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).expect("the variable exists");
            if got != *want {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    std::fs::remove_dir_all(&dir).ok();
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}
