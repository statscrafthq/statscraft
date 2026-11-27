//! SORT CASES, ADD FILES, MATCH FILES, VARSTOCASES and CASESTOVARS against R, on invented data.
//!
//! The expected tables come from base R (`order`, `rank`, `match`, `rbind`), with each rule of the
//! manual written out on its own: unmatched cases get missing values for the variables of the
//! other files, duplicate keys match first with first, second with second and so on, a table
//! file adds variables and no cases, a common variable takes its value from the first file that
//! has the case, a missing key sorts first and is a key like any other, strings sort by their
//! bytes, descending reverses the order of the keys and puts the missing last, cases with equal
//! keys keep their order, an all-null row is dropped by VARSTOCASES unless NULL=KEEP, a numeric
//! or name index and several indices (the last varying fastest), the row groups of CASESTOVARS
//! are runs of equal keys, a variable that does not vary is copied, the others are spread over
//! the index values (a variable with one value and missing values in every group counts as not
//! varying), and rows with a missing key are left out.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    setup: &'static str,
    command: &'static str,
    /// Names that must appear in this order (not necessarily next to each other).
    order: &'static [&'static str],
    /// Every variable the result has, in any order.
    variables: &'static [&'static str],
    /// The values of each variable: numbers separated by spaces with `.` for missing, strings
    /// separated by `|`.
    columns: &'static [(&'static str, &'static str)],
}

const FILES: &str = r#"DATA LIST LIST /k (F8) x (F8) s (A2).
BEGIN DATA
. 7 'aa'
1 10 'a'
1 11 'b'
2 20 'c'
3 30 'd'
3 31 'e'
3 32 'f'
6 60 'h'
END DATA.
DATASET NAME fa.
DATA LIST LIST /k (F8) y (F8.1) s (A2).
BEGIN DATA
. 5 'p'
1 6 'q'
1 7 ''
1 8 'r'
3 9 't'
4 10 'u'
6 11 'v'
6 12.5 'w'
END DATA.
DATASET NAME fb.
DATA LIST LIST /k (F8) z (F8) t (A5).
BEGIN DATA
1 100 'one'
3 300 'three'
5 500 'five'
6 600 'six'
END DATA.
DATASET NAME fc.
DATA LIST LIST /m (F8) x (F8).
BEGIN DATA
1 -1
2 -2
3 .
END DATA.
DATASET NAME fd.
DATA LIST LIST /n (F8) x (F8).
BEGIN DATA
11 -9
12 -8
13 -7
14 -6
15 -5
16 -4
17 -3
18 -2
19 -1
END DATA.
DATASET NAME fe.
DATA LIST LIST /k1 (F8) k2 (A2) u (F8).
BEGIN DATA
1 'a' 1
1 'a' 2
1 'b' 3
2 'a' 4
2 'c' 5
4 'b' 6
END DATA.
DATASET NAME ft1.
DATA LIST LIST /k1 (F8) k2 (A2) v (F8).
BEGIN DATA
1 'a' 10
1 'b' 20
2 'a' 30
2 'a' 40
3 'a' 50
4 'b' 60
END DATA.
DATASET NAME ft2.
"#;
const SORT: &str = r#"DATA LIST LIST /k (F8) v (F8) w (A2).
BEGIN DATA
2 5 'b'
. 3 'B'
1 5 'a'
2 1 'c'
1 . 'a'
. 2 ''
3 5 'Ab'
2 5 'a'
END DATA.
DATASET NAME fs.
"#;
const WIDE: &str = r#"DATA LIST LIST /id (F8) grp (A2) q1 (F8.1) q2 (F8) q3 (F8) r1 (F8) r2 (F8) r3 (F8) t1 (A2) t2 (A2) t3 (A2).
BEGIN DATA
1 'ab' 3 5 . 10 . 30 'x' '' 'z'
2 'cd' . . . 1 2 3 '' '' ''
3 'ab' 4.5 6 7 . . . 'u' 'v' 'w'
4 'ef' 2 . 9 20 25 . 'a' 'a' ''
END DATA.
DATASET NAME fw.
"#;
const LONG: &str = r#"DATA LIST LIST /grp (A2) id (F8) visit (F8) bp (F8) hr (F8) age (F8) site (A4).
BEGIN DATA
'n' . 1 99 99 . 'zz'
'n' 1 1 120 70 40 'oslo'
'n' 1 2 125 . . 'oslo'
'n' 1 4 130 72 40 'oslo'
'n' 2 2 . 80 51 'rome'
'n' 3 1 110 60 . 'bern'
'n' 3 3 115 . 33 'bern'
'm' 1 1 140 65 29 'oslo'
'm' 1 3 145 66 29 'oslo'
'm' 2 1 . 68 61 'rome'
'm' 2 2 150 69 61 'rome'
END DATA.
DATASET NAME fl.
"#;

const CASES: &[Case] = &[
    Case {
        label: r#"SORT CASES BY k"#,
        setup: SORT,
        command: r#"DATASET ACTIVATE fs.
SORT CASES BY k."#,
        order: &[r#"k"#, r#"v"#, r#"w"#],
        variables: &[r#"k"#, r#"v"#, r#"w"#],
        columns: &[
            (r#"k"#, r#". . 1 1 2 2 2 3"#),
            (r#"v"#, r#"3 2 5 . 5 1 5 5"#),
            (r#"w"#, r#"B||a|a|b|c|a|Ab"#),
        ],
    },
    Case {
        label: r#"SORT CASES BY k (D)"#,
        setup: SORT,
        command: r#"DATASET ACTIVATE fs.
SORT CASES BY k (D)."#,
        order: &[r#"k"#, r#"v"#, r#"w"#],
        variables: &[r#"k"#, r#"v"#, r#"w"#],
        columns: &[
            (r#"k"#, r#"3 2 2 2 1 1 . ."#),
            (r#"v"#, r#"5 5 1 5 5 . 3 2"#),
            (r#"w"#, r#"Ab|b|c|a|a|a|B|"#),
        ],
    },
    Case {
        label: r#"SORT CASES BY w"#,
        setup: SORT,
        command: r#"DATASET ACTIVATE fs.
SORT CASES BY w."#,
        order: &[r#"k"#, r#"v"#, r#"w"#],
        variables: &[r#"k"#, r#"v"#, r#"w"#],
        columns: &[
            (r#"k"#, r#". 3 . 1 1 2 2 2"#),
            (r#"v"#, r#"2 5 3 5 . 5 5 1"#),
            (r#"w"#, r#"|Ab|B|a|a|a|b|c"#),
        ],
    },
    Case {
        label: r#"SORT CASES BY v (D) k"#,
        setup: SORT,
        command: r#"DATASET ACTIVATE fs.
SORT CASES BY v (D) k."#,
        order: &[r#"k"#, r#"v"#, r#"w"#],
        variables: &[r#"k"#, r#"v"#, r#"w"#],
        columns: &[
            (r#"k"#, r#"1 2 2 3 . . 2 1"#),
            (r#"v"#, r#"5 5 5 5 3 2 1 ."#),
            (r#"w"#, r#"a|b|a|Ab|B||c|a"#),
        ],
    },
    Case {
        label: r#"SORT CASES BY k (A) w (D) v"#,
        setup: SORT,
        command: r#"DATASET ACTIVATE fs.
SORT CASES BY k (A) w (D) v."#,
        order: &[r#"k"#, r#"v"#, r#"w"#],
        variables: &[r#"k"#, r#"v"#, r#"w"#],
        columns: &[
            (r#"k"#, r#". . 1 1 2 2 2 3"#),
            (r#"v"#, r#"3 2 . 5 1 5 5 5"#),
            (r#"w"#, r#"B||a|a|c|b|a|Ab"#),
        ],
    },
    Case {
        label: r#"SORT CASES BY w (D)"#,
        setup: SORT,
        command: r#"DATASET ACTIVATE fs.
SORT CASES BY w (D)."#,
        order: &[r#"k"#, r#"v"#, r#"w"#],
        variables: &[r#"k"#, r#"v"#, r#"w"#],
        columns: &[
            (r#"k"#, r#"2 2 1 1 2 . 3 ."#),
            (r#"v"#, r#"1 5 5 . 5 3 5 2"#),
            (r#"w"#, r#"c|b|a|a|a|B|Ab|"#),
        ],
    },
    Case {
        label: r#"ADD no BY"#,
        setup: FILES,
        command: r#"ADD FILES /FILE=fa /FILE=fb."#,
        order: &[r#"k"#, r#"x"#, r#"s"#, r#"y"#],
        variables: &[r#"k"#, r#"x"#, r#"s"#, r#"y"#],
        columns: &[
            (r#"k"#, r#". 1 1 2 3 3 3 6 . 1 1 1 3 4 6 6"#),
            (r#"x"#, r#"7 10 11 20 30 31 32 60 . . . . . . . ."#),
            (r#"s"#, r#"aa|a|b|c|d|e|f|h|p|q||r|t|u|v|w"#),
            (r#"y"#, r#". . . . . . . . 5 6 7 8 9 10 11 12.5"#),
        ],
    },
    Case {
        label: r#"ADD BY k flags"#,
        setup: FILES,
        command: r#"ADD FILES /FILE=fa /IN=ina /FILE=fb /IN=inb /BY k /FIRST=f /LAST=l."#,
        order: &[
            r#"k"#, r#"x"#, r#"s"#, r#"y"#, r#"ina"#, r#"inb"#, r#"f"#, r#"l"#,
        ],
        variables: &[
            r#"k"#, r#"x"#, r#"s"#, r#"y"#, r#"ina"#, r#"inb"#, r#"f"#, r#"l"#,
        ],
        columns: &[
            (r#"k"#, r#". . 1 1 1 1 1 2 3 3 3 3 4 6 6 6"#),
            (r#"x"#, r#"7 . 10 11 . . . 20 30 31 32 . . 60 . ."#),
            (r#"s"#, r#"aa|p|a|b|q||r|c|d|e|f|t|u|h|v|w"#),
            (r#"y"#, r#". 5 . . 6 7 8 . . . . 9 10 . 11 12.5"#),
            (r#"ina"#, r#"1 0 1 1 0 0 0 1 1 1 1 0 0 1 0 0"#),
            (r#"inb"#, r#"0 1 0 0 1 1 1 0 0 0 0 1 1 0 1 1"#),
            (r#"f"#, r#"1 0 1 0 0 0 0 1 1 0 0 0 1 1 0 0"#),
            (r#"l"#, r#"0 1 0 0 0 0 1 1 0 0 0 1 1 0 0 1"#),
        ],
    },
    Case {
        label: r#"ADD three files BY k"#,
        setup: FILES,
        command: r#"ADD FILES /FILE=fb /FILE=fc /FILE=fa /BY k."#,
        order: &[r#"k"#, r#"y"#, r#"s"#, r#"z"#, r#"t"#, r#"x"#],
        variables: &[r#"k"#, r#"y"#, r#"s"#, r#"z"#, r#"t"#, r#"x"#],
        columns: &[
            (r#"k"#, r#". . 1 1 1 1 1 1 2 3 3 3 3 3 4 5 6 6 6 6"#),
            (r#"y"#, r#"5 . 6 7 8 . . . . 9 . . . . 10 . 11 12.5 . ."#),
            (r#"s"#, r#"p|aa|q||r||a|b|c|t||d|e|f|u||v|w||h"#),
            (r#"z"#, r#". . . . . 100 . . . . 300 . . . . 500 . . 600 ."#),
            (r#"t"#, r#"|||||one|||||three|||||five|||six|"#),
            (r#"x"#, r#". 7 . . . . 10 11 20 . . 30 31 32 . . . . . 60"#),
        ],
    },
    Case {
        label: r#"ADD rename and drop"#,
        setup: FILES,
        command: r#"ADD FILES /FILE=fa /FILE=fb /RENAME=(s=s2) /DROP=x."#,
        order: &[r#"k"#, r#"s"#, r#"y"#, r#"s2"#],
        variables: &[r#"k"#, r#"s"#, r#"y"#, r#"s2"#],
        columns: &[
            (r#"k"#, r#". 1 1 2 3 3 3 6 . 1 1 1 3 4 6 6"#),
            (r#"s"#, r#"aa|a|b|c|d|e|f|h||||||||"#),
            (r#"y"#, r#". . . . . . . . 5 6 7 8 9 10 11 12.5"#),
            (r#"s2"#, r#"||||||||p|q||r|t|u|v|w"#),
        ],
    },
    Case {
        label: r#"ADD two keys"#,
        setup: FILES,
        command: r#"ADD FILES /FILE=ft1 /FILE=ft2 /BY k1 k2."#,
        order: &[r#"k1"#, r#"k2"#, r#"u"#, r#"v"#],
        variables: &[r#"k1"#, r#"k2"#, r#"u"#, r#"v"#],
        columns: &[
            (r#"k1"#, r#"1 1 1 1 1 2 2 2 2 3 4 4"#),
            (r#"k2"#, r#"a|a|a|b|b|a|a|a|c|a|b|b"#),
            (r#"u"#, r#"1 2 . 3 . 4 . . 5 . 6 ."#),
            (r#"v"#, r#". . 10 . 20 . 30 40 . 50 . 60"#),
        ],
    },
    Case {
        label: r#"MATCH two files BY k"#,
        setup: FILES,
        command: r#"MATCH FILES /FILE=fa /IN=ina /FILE=fb /IN=inb /BY k."#,
        order: &[r#"k"#, r#"x"#, r#"s"#, r#"y"#, r#"ina"#, r#"inb"#],
        variables: &[r#"k"#, r#"x"#, r#"s"#, r#"y"#, r#"ina"#, r#"inb"#],
        columns: &[
            (r#"k"#, r#". 1 1 1 2 3 3 3 4 6 6"#),
            (r#"x"#, r#"7 10 11 . 20 30 31 32 . 60 ."#),
            (r#"s"#, r#"aa|a|b|r|c|d|e|f|u|h|w"#),
            (r#"y"#, r#"5 6 7 8 . 9 . . 10 11 12.5"#),
            (r#"ina"#, r#"1 1 1 0 1 1 1 1 0 1 0"#),
            (r#"inb"#, r#"1 1 1 1 0 1 0 0 1 1 1"#),
        ],
    },
    Case {
        label: r#"MATCH reversed"#,
        setup: FILES,
        command: r#"MATCH FILES /FILE=fb /FILE=fa /BY k."#,
        order: &[r#"k"#, r#"y"#, r#"s"#, r#"x"#],
        variables: &[r#"k"#, r#"y"#, r#"s"#, r#"x"#],
        columns: &[
            (r#"k"#, r#". 1 1 1 2 3 3 3 4 6 6"#),
            (r#"y"#, r#"5 6 7 8 . 9 . . 10 11 12.5"#),
            (r#"s"#, r#"p|q||r|c|t|e|f|u|v|w"#),
            (r#"x"#, r#"7 10 11 . 20 30 31 32 . 60 ."#),
        ],
    },
    Case {
        label: r#"MATCH table"#,
        setup: FILES,
        command: r#"MATCH FILES /FILE=fa /TABLE=fc /IN=hit /BY k."#,
        order: &[r#"k"#, r#"x"#, r#"s"#, r#"z"#, r#"t"#, r#"hit"#],
        variables: &[r#"k"#, r#"x"#, r#"s"#, r#"z"#, r#"t"#, r#"hit"#],
        columns: &[
            (r#"k"#, r#". 1 1 2 3 3 3 6"#),
            (r#"x"#, r#"7 10 11 20 30 31 32 60"#),
            (r#"s"#, r#"aa|a|b|c|d|e|f|h"#),
            (r#"z"#, r#". 100 100 . 300 300 300 600"#),
            (r#"t"#, r#"|one|one||three|three|three|six"#),
            (r#"hit"#, r#"0 1 1 0 1 1 1 1"#),
        ],
    },
    Case {
        label: r#"MATCH two files and a table"#,
        setup: FILES,
        command: r#"MATCH FILES /FILE=fa /FILE=fb /TABLE=fc /BY k /DROP=s."#,
        order: &[r#"k"#, r#"x"#, r#"y"#, r#"z"#, r#"t"#],
        variables: &[r#"k"#, r#"x"#, r#"y"#, r#"z"#, r#"t"#],
        columns: &[
            (r#"k"#, r#". 1 1 1 2 3 3 3 4 6 6"#),
            (r#"x"#, r#"7 10 11 . 20 30 31 32 . 60 ."#),
            (r#"y"#, r#"5 6 7 8 . 9 . . 10 11 12.5"#),
            (r#"z"#, r#". 100 100 100 . 300 300 300 . 600 600"#),
            (r#"t"#, r#"|one|one|one||three|three|three||six|six"#),
        ],
    },
    Case {
        label: r#"MATCH parallel"#,
        setup: FILES,
        command: r#"MATCH FILES /FILE=fd /FILE=fa /FILE=fe."#,
        order: &[r#"m"#, r#"x"#, r#"k"#, r#"s"#, r#"n"#],
        variables: &[r#"m"#, r#"x"#, r#"k"#, r#"s"#, r#"n"#],
        columns: &[
            (r#"m"#, r#"1 2 3 . . . . . ."#),
            (r#"x"#, r#"-1 -2 . 20 30 31 32 60 -1"#),
            (r#"k"#, r#". 1 1 2 3 3 3 6 ."#),
            (r#"s"#, r#"aa|a|b|c|d|e|f|h|"#),
            (r#"n"#, r#"11 12 13 14 15 16 17 18 19"#),
        ],
    },
    Case {
        label: r#"MATCH two keys"#,
        setup: FILES,
        command: r#"MATCH FILES /FILE=ft1 /FILE=ft2 /BY k1 k2."#,
        order: &[r#"k1"#, r#"k2"#, r#"u"#, r#"v"#],
        variables: &[r#"k1"#, r#"k2"#, r#"u"#, r#"v"#],
        columns: &[
            (r#"k1"#, r#"1 1 1 2 2 2 3 4"#),
            (r#"k2"#, r#"a|a|b|a|a|c|a|b"#),
            (r#"u"#, r#"1 2 3 4 . 5 . 6"#),
            (r#"v"#, r#"10 . 20 30 40 . 50 60"#),
        ],
    },
    Case {
        label: r#"MATCH one file flags"#,
        setup: FILES,
        command: r#"MATCH FILES /FILE=fa /BY k /FIRST=f /LAST=l."#,
        order: &[r#"k"#, r#"x"#, r#"s"#, r#"f"#, r#"l"#],
        variables: &[r#"k"#, r#"x"#, r#"s"#, r#"f"#, r#"l"#],
        columns: &[
            (r#"k"#, r#". 1 1 2 3 3 3 6"#),
            (r#"x"#, r#"7 10 11 20 30 31 32 60"#),
            (r#"s"#, r#"aa|a|b|c|d|e|f|h"#),
            (r#"f"#, r#"1 1 0 1 1 0 0 1"#),
            (r#"l"#, r#"1 0 1 1 0 0 1 1"#),
        ],
    },
    Case {
        label: r#"VARSTOCASES basic"#,
        setup: WIDE,
        command: r#"DATASET ACTIVATE fw.
VARSTOCASES /MAKE q FROM q1 TO q3 /DROP=grp r1 TO t3."#,
        order: &[r#"id"#, r#"q"#],
        variables: &[r#"id"#, r#"q"#],
        columns: &[
            (r#"id"#, r#"1 1 3 3 3 4 4"#),
            (r#"q"#, r#"3 5 4.5 6 7 2 9"#),
        ],
    },
    Case {
        label: r#"VARSTOCASES index id count null keep"#,
        setup: WIDE,
        command: r#"DATASET ACTIVATE fw.
VARSTOCASES /MAKE q FROM q1 q2 q3 /INDEX=i /ID=src /COUNT=n /NULL=KEEP /KEEP=id."#,
        order: &[r#"id"#, r#"i"#, r#"q"#],
        variables: &[r#"id"#, r#"i"#, r#"q"#, r#"src"#, r#"n"#],
        columns: &[
            (r#"id"#, r#"1 1 1 2 2 2 3 3 3 4 4 4"#),
            (r#"i"#, r#"1 2 3 1 2 3 1 2 3 1 2 3"#),
            (r#"q"#, r#"3 5 . . . . 4.5 6 7 2 . 9"#),
            (r#"src"#, r#"1 1 1 2 2 2 3 3 3 4 4 4"#),
            (r#"n"#, r#"3 3 3 3 3 3 3 3 3 3 3 3"#),
        ],
    },
    Case {
        label: r#"VARSTOCASES two makes and a name index"#,
        setup: WIDE,
        command: r#"DATASET ACTIVATE fw.
VARSTOCASES /MAKE q FROM q1 q2 q3 /MAKE r FROM r1 r2 r3 /INDEX=which(q) /COUNT=n /DROP=grp t1 t2 t3."#,
        order: &[r#"id"#, r#"which"#],
        variables: &[r#"id"#, r#"which"#, r#"q"#, r#"r"#, r#"n"#],
        columns: &[
            (r#"id"#, r#"1 1 1 2 2 2 3 3 3 4 4 4"#),
            (r#"which"#, r#"q1|q2|q3|q1|q2|q3|q1|q2|q3|q1|q2|q3"#),
            (r#"q"#, r#"3 5 . . . . 4.5 6 7 2 . 9"#),
            (r#"r"#, r#"10 . 30 1 2 3 . . . 20 25 ."#),
            (r#"n"#, r#"3 3 3 3 3 3 3 3 3 3 3 3"#),
        ],
    },
    Case {
        label: r#"VARSTOCASES strings"#,
        setup: WIDE,
        command: r#"DATASET ACTIVATE fw.
VARSTOCASES /MAKE t FROM t1 t2 t3 /INDEX=a(3) /KEEP=id."#,
        order: &[r#"id"#, r#"a"#, r#"t"#],
        variables: &[r#"id"#, r#"a"#, r#"t"#],
        columns: &[
            (r#"id"#, r#"1 1 3 3 3 4 4"#),
            (r#"a"#, r#"1 3 1 2 3 1 2"#),
            (r#"t"#, r#"x|z|u|v|w|a|a"#),
        ],
    },
    Case {
        label: r#"VARSTOCASES two indices"#,
        setup: WIDE,
        command: r#"DATASET ACTIVATE fw.
VARSTOCASES /MAKE v FROM q1 q2 q3 r1 r2 r3 /INDEX=a(2) b(3) /NULL=KEEP /KEEP=id."#,
        order: &[r#"id"#, r#"a"#, r#"b"#, r#"v"#],
        variables: &[r#"id"#, r#"b"#, r#"a"#, r#"v"#],
        columns: &[
            (
                r#"id"#,
                r#"1 1 1 1 1 1 2 2 2 2 2 2 3 3 3 3 3 3 4 4 4 4 4 4"#,
            ),
            (r#"b"#, r#"1 2 3 1 2 3 1 2 3 1 2 3 1 2 3 1 2 3 1 2 3 1 2 3"#),
            (r#"a"#, r#"1 1 1 2 2 2 1 1 1 2 2 2 1 1 1 2 2 2 1 1 1 2 2 2"#),
            (
                r#"v"#,
                r#"3 5 . 10 . 30 . . . 1 2 3 4.5 6 7 . . . 2 . 9 20 25 ."#,
            ),
        ],
    },
    Case {
        label: r#"VARSTOCASES repeated variable"#,
        setup: WIDE,
        command: r#"DATASET ACTIVATE fw.
VARSTOCASES /MAKE q FROM q1 q1 q2 /INDEX=j /KEEP=id."#,
        order: &[r#"id"#, r#"j"#, r#"q"#],
        variables: &[r#"id"#, r#"j"#, r#"q"#],
        columns: &[
            (r#"id"#, r#"1 1 1 3 3 3 4 4"#),
            (r#"j"#, r#"1 2 3 1 2 3 1 2"#),
            (r#"q"#, r#"3 3 5 4.5 4.5 6 2 2"#),
        ],
    },
    Case {
        label: r#"CASESTOVARS index"#,
        setup: LONG,
        command: r#"DATASET ACTIVATE fl.
CASESTOVARS /ID=grp id /INDEX=visit."#,
        order: &[
            r#"grp"#, r#"id"#, r#"bp.1"#, r#"bp.2"#, r#"bp.3"#, r#"bp.4"#, r#"hr.1"#, r#"hr.2"#,
            r#"hr.3"#, r#"hr.4"#,
        ],
        variables: &[
            r#"grp"#, r#"id"#, r#"age"#, r#"site"#, r#"bp.1"#, r#"bp.2"#, r#"bp.3"#, r#"bp.4"#,
            r#"hr.1"#, r#"hr.2"#, r#"hr.3"#, r#"hr.4"#,
        ],
        columns: &[
            (r#"grp"#, r#"n|n|n|m|m"#),
            (r#"id"#, r#"1 2 3 1 2"#),
            (r#"age"#, r#"40 51 33 29 61"#),
            (r#"site"#, r#"oslo|rome|bern|oslo|rome"#),
            (r#"bp.1"#, r#"120 . 110 140 ."#),
            (r#"bp.2"#, r#"125 . . . 150"#),
            (r#"bp.3"#, r#". . 115 145 ."#),
            (r#"bp.4"#, r#"130 . . . ."#),
            (r#"hr.1"#, r#"70 . 60 65 68"#),
            (r#"hr.2"#, r#". 80 . . 69"#),
            (r#"hr.3"#, r#". . . 66 ."#),
            (r#"hr.4"#, r#"72 . . . ."#),
        ],
    },
    Case {
        label: r#"CASESTOVARS groupby index, separator, indicators and count"#,
        setup: LONG,
        command: r#"DATASET ACTIVATE fl.
CASESTOVARS /ID=grp id /INDEX=visit /GROUPBY=INDEX /SEPARATOR='_' /VIND ROOT=has /COUNT=n /DROP=age site."#,
        order: &[
            r#"bp_1"#, r#"hr_1"#, r#"bp_2"#, r#"hr_2"#, r#"bp_3"#, r#"hr_3"#, r#"bp_4"#, r#"hr_4"#,
        ],
        variables: &[
            r#"grp"#, r#"id"#, r#"bp_1"#, r#"hr_1"#, r#"bp_2"#, r#"hr_2"#, r#"bp_3"#, r#"hr_3"#,
            r#"bp_4"#, r#"hr_4"#, r#"has1"#, r#"has2"#, r#"has3"#, r#"has4"#, r#"n"#,
        ],
        columns: &[
            (r#"grp"#, r#"n|n|n|m|m"#),
            (r#"id"#, r#"1 2 3 1 2"#),
            (r#"bp_1"#, r#"120 . 110 140 ."#),
            (r#"hr_1"#, r#"70 . 60 65 68"#),
            (r#"bp_2"#, r#"125 . . . 150"#),
            (r#"hr_2"#, r#". 80 . . 69"#),
            (r#"bp_3"#, r#". . 115 145 ."#),
            (r#"hr_3"#, r#". . . 66 ."#),
            (r#"bp_4"#, r#"130 . . . ."#),
            (r#"hr_4"#, r#"72 . . . ."#),
            (r#"has1"#, r#"1 0 1 1 1"#),
            (r#"has2"#, r#"1 1 0 0 1"#),
            (r#"has3"#, r#"0 0 1 1 0"#),
            (r#"has4"#, r#"1 0 0 0 0"#),
            (r#"n"#, r#"3 1 2 2 2"#),
        ],
    },
    Case {
        label: r#"CASESTOVARS without index"#,
        setup: LONG,
        command: r#"DATASET ACTIVATE fl.
CASESTOVARS /ID=grp id /DROP=visit age site."#,
        order: &[
            r#"grp"#, r#"id"#, r#"bp.1"#, r#"bp.2"#, r#"bp.3"#, r#"hr.1"#, r#"hr.2"#, r#"hr.3"#,
        ],
        variables: &[
            r#"grp"#, r#"id"#, r#"bp.1"#, r#"bp.2"#, r#"bp.3"#, r#"hr.1"#, r#"hr.2"#, r#"hr.3"#,
        ],
        columns: &[
            (r#"grp"#, r#"n|n|n|m|m"#),
            (r#"id"#, r#"1 2 3 1 2"#),
            (r#"bp.1"#, r#"120 . 110 140 ."#),
            (r#"bp.2"#, r#"125 . 115 145 150"#),
            (r#"bp.3"#, r#"130 . . . ."#),
            (r#"hr.1"#, r#"70 80 60 65 68"#),
            (r#"hr.2"#, r#". . . 66 69"#),
            (r#"hr.3"#, r#"72 . . . ."#),
        ],
    },
    Case {
        label: r#"CASESTOVARS autofix no"#,
        setup: LONG,
        command: r#"DATASET ACTIVATE fl.
CASESTOVARS /ID=grp id /INDEX=visit /AUTOFIX=NO /DROP=age bp hr."#,
        order: &[r#"site.1"#, r#"site.2"#, r#"site.3"#, r#"site.4"#],
        variables: &[
            r#"grp"#,
            r#"id"#,
            r#"site.1"#,
            r#"site.2"#,
            r#"site.3"#,
            r#"site.4"#,
        ],
        columns: &[
            (r#"grp"#, r#"n|n|n|m|m"#),
            (r#"id"#, r#"1 2 3 1 2"#),
            (r#"site.1"#, r#"oslo||bern|oslo|rome"#),
            (r#"site.2"#, r#"oslo|rome|||rome"#),
            (r#"site.3"#, r#"||bern|oslo|"#),
            (r#"site.4"#, r#"oslo||||"#),
        ],
    },
    Case {
        label: r#"CASESTOVARS rename"#,
        setup: LONG,
        command: r#"DATASET ACTIVATE fl.
CASESTOVARS /ID=grp id /INDEX=visit /RENAME bp=sys hr=pulse /SEPARATOR='' /DROP=age site."#,
        order: &[
            r#"sys1"#,
            r#"sys2"#,
            r#"sys3"#,
            r#"sys4"#,
            r#"pulse1"#,
            r#"pulse2"#,
            r#"pulse3"#,
            r#"pulse4"#,
        ],
        variables: &[
            r#"grp"#,
            r#"id"#,
            r#"sys1"#,
            r#"sys2"#,
            r#"sys3"#,
            r#"sys4"#,
            r#"pulse1"#,
            r#"pulse2"#,
            r#"pulse3"#,
            r#"pulse4"#,
        ],
        columns: &[
            (r#"grp"#, r#"n|n|n|m|m"#),
            (r#"id"#, r#"1 2 3 1 2"#),
            (r#"sys1"#, r#"120 . 110 140 ."#),
            (r#"sys2"#, r#"125 . . . 150"#),
            (r#"sys3"#, r#". . 115 145 ."#),
            (r#"sys4"#, r#"130 . . . ."#),
            (r#"pulse1"#, r#"70 . 60 65 68"#),
            (r#"pulse2"#, r#". 80 . . 69"#),
            (r#"pulse3"#, r#". . . 66 ."#),
            (r#"pulse4"#, r#"72 . . . ."#),
        ],
    },
    Case {
        label: r#"CASESTOVARS split file"#,
        setup: LONG,
        command: r#"DATASET ACTIVATE fl.
SPLIT FILE BY grp.
CASESTOVARS /ID=id /INDEX=visit /DROP=age site."#,
        order: &[
            r#"grp"#, r#"id"#, r#"bp.1"#, r#"bp.2"#, r#"bp.3"#, r#"bp.4"#, r#"hr.1"#, r#"hr.2"#,
            r#"hr.3"#, r#"hr.4"#,
        ],
        variables: &[
            r#"grp"#, r#"id"#, r#"bp.1"#, r#"bp.2"#, r#"bp.3"#, r#"bp.4"#, r#"hr.1"#, r#"hr.2"#,
            r#"hr.3"#, r#"hr.4"#,
        ],
        columns: &[
            (r#"grp"#, r#"n|n|n|m|m"#),
            (r#"id"#, r#"1 2 3 1 2"#),
            (r#"bp.1"#, r#"120 . 110 140 ."#),
            (r#"bp.2"#, r#"125 . . . 150"#),
            (r#"bp.3"#, r#". . 115 145 ."#),
            (r#"bp.4"#, r#"130 . . . ."#),
            (r#"hr.1"#, r#"70 . 60 65 68"#),
            (r#"hr.2"#, r#". 80 . . 69"#),
            (r#"hr.3"#, r#". . . 66 ."#),
            (r#"hr.4"#, r#"72 . . . ."#),
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
        Values::String(values) => values.iter().collect::<Vec<_>>().join("|"),
    })
}

fn run(case: &Case) -> Result<Engine, String> {
    let source = format!("{}{}", case.setup, case.command);
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
fn merging_and_restructuring_agree_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let engine = match run(case) {
            Ok(engine) => engine,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        let names: Vec<String> = engine
            .dataset()
            .unwrap()
            .variable_names()
            .map(str::to_string)
            .collect();
        let mut got = names.clone();
        got.sort();
        let mut want: Vec<&str> = case.variables.to_vec();
        want.sort();
        if got != want {
            failures.push(format!(
                "{}: variables {} (wanted {})",
                case.label,
                names.join(" "),
                case.variables.join(" ")
            ));
            continue;
        }
        let position = |name: &str| names.iter().position(|n| n == name).unwrap();
        let positions: Vec<usize> = case.order.iter().map(|n| position(n)).collect();
        if positions.windows(2).any(|w| w[0] >= w[1]) {
            failures.push(format!(
                "{}: order {} (wanted {})",
                case.label,
                names.join(" "),
                case.order.join(" ")
            ));
        }
        for (name, want) in case.columns {
            let got = show(&engine, name).expect("the variable exists");
            if got != *want {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} differ:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
}

/// What the manual says is refused, or given the longest width, when the files do not fit (ADD FILES
/// widens strings, a recorded extension).
#[test]
fn files_that_do_not_fit_follow_the_guide() {
    let probe = |setup: &'static str, command: &'static str| {
        let case = Case {
            label: "probe",
            setup,
            command,
            order: &[],
            variables: &[],
            columns: &[],
        };
        run(&case)
    };
    let types = "DATA LIST LIST /k (F2) v (F2).\nBEGIN DATA\n1 2\nEND DATA.\nDATASET NAME n.\n\
                 DATA LIST LIST /k (F2) v (A2).\nBEGIN DATA\n1 'a'\nEND DATA.\nDATASET NAME s.\n";
    assert!(probe(types, "ADD FILES /FILE=n /FILE=s.").is_err());
    let widths = "DATA LIST LIST /k (F2) v (A2).\nBEGIN DATA\n1 'a'\nEND DATA.\nDATASET NAME a2.\n\
                  DATA LIST LIST /k (F2) v (A5).\nBEGIN DATA\n1 'abcde'\nEND DATA.\nDATASET NAME a5.\n";
    // the manual refuses this; widening to the longest is the recorded extension of ADD FILES
    let engine = probe(widths, "ADD FILES /FILE=a2 /FILE=a5.").unwrap();
    assert_eq!(show(&engine, "v").unwrap(), "a|abcde");
    assert_eq!(
        engine.dataset().unwrap().variable("v").unwrap().format(),
        "A5"
    );
    let twice = "DATA LIST LIST /k (F2) u (F2).\nBEGIN DATA\n1 2\n1 3\nEND DATA.\nDATASET NAME dup.\n\
                 DATA LIST LIST /k (F2) w (F2).\nBEGIN DATA\n1 5\nEND DATA.\nDATASET NAME one.\n";
    assert!(
        probe(twice, "MATCH FILES /FILE=one /TABLE=dup /BY k.").is_err(),
        "a table file with duplicate keys"
    );
    let mixed = "DATA LIST LIST /id (F2) a (F2) b (A2).\nBEGIN DATA\n1 2 'x'\nEND DATA.\nDATASET NAME mixed.\n";
    assert!(probe(mixed, "VARSTOCASES /MAKE v FROM a b.").is_err());
    let lengths = "DATA LIST LIST /id (F2) a (A2) b (A6).\nBEGIN DATA\n1 'x' 'longer'\nEND DATA.\nDATASET NAME lengths.\n";
    let engine = probe(lengths, "VARSTOCASES /MAKE v FROM a b.").unwrap();
    assert_eq!(show(&engine, "v").unwrap(), "x|longer");
    assert_eq!(
        engine.dataset().unwrap().variable("v").unwrap().format(),
        "A6"
    );
}
