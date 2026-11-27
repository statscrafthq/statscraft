//! LOOP-END LOOP, VECTOR, DO REPEAT, LEAVE and the spread of missing values through COMPUTE,
//! against R on 12 invented cases with system-missing and user-missing values (a user-missing
//! code is missing in an expression, so in a bound, a condition or an operand). The loop: an index
//! with a step (negative, fractional, zero, from a variable), bounds that are expressions, a
//! start past the end, a missing start, bound or step (no trip), the index variable after a
//! normal end, a zero-trip loop, an END LOOP IF, a BREAK and an IF on LOOP (and a change of the
//! index inside the loop, which does not change the count), IF on LOOP as a while and on END LOOP as
//! an until (a missing condition: no trip on LOOP, one trip on END LOOP), BREAK out of the
//! innermost loop only, SET MXLOOPS (a loop with an index ignores it), scratch variables that live
//! on from case to case and permanent ones that start missing, and loops that make cases in an
//! input program, with scratch and with permanent indices. VECTOR over variables and of the
//! short form (also with an index from the data, and a vector hiding a function of the same
//! name); DO REPEAT with lists of variables, TO lists, value lists, ALL, strings, IF, DO IF and a
//! LOOP inside; LEAVE (a running total that a missing value turns missing, SUM that skips it, a
//! string, DO IF, a list); arithmetic, statistical functions and VALUE on missing values.
//! What R computes itself: the arithmetic, the statistics, the comparisons and the control flow of
//! each case. What follows the manual's words, written out in the R script (R has no such
//! command): the rules of the loop, the vector and the stand-in, and that a result which is not
//! a number is system-missing.

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    /// The values of each variable after the run: numbers separated by spaces with `.` for
    /// missing, strings separated by `|`.
    columns: &'static [(&'static str, &'static str)],
}

/// A run with XSAVE: the files it writes (`{OUT}` stands for their folder) with the variables and
/// values R found in them (read with haven, and worked out in R from the rules of the command).
struct XCase {
    label: &'static str,
    command: &'static str,
    files: &'static [(&'static str, &'static [(&'static str, &'static str)])],
}

const DATA: &str = r####"DATA LIST LIST /id x y z.
BEGIN DATA
1 3 2 1
2 0 5 2
3 4 . 2
4 . 3 1
5 7 -1 3
6 2 2.5 0.5
7 10 4 -1
8 1 0 0
9 6 3 .
10 98 2 1
11 5 99 2
12 2 6 1
END DATA.
MISSING VALUES x (98) y (99).
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"an index loop to a variable: a factorial, zero and fewer trips for 0, negative, fractional and missing bounds"####,
        command: r####"COMPUTE f = 1.
LOOP #i = 1 TO y.
COMPUTE f = f * #i.
END LOOP.
COMPUTE c = 0.
LOOP #j = 1 TO y.
COMPUTE c = c + 1.
END LOOP."####,
        columns: &[
            (r####"f"####, r####"2 120 1 6 1 2 24 1 6 2 1 720"####),
            (r####"c"####, r####"2 5 0 3 0 2 4 0 3 2 0 6"####),
        ],
    },
    Case {
        label: r####"an index loop with a step: negative, fractional, zero (no trip) and from a variable"####,
        command: r####"COMPUTE a = 0.
LOOP #i = 10 TO 1 BY -3.
COMPUTE a = a + #i.
END LOOP.
COMPUTE b = 0.
LOOP #j = 0 TO 1 BY 0.25.
COMPUTE b = b + 1.
END LOOP.
COMPUTE g = 0.
LOOP #k = 1 TO 10 BY z.
COMPUTE g = g + #k.
END LOOP.
COMPUTE h = 0.
LOOP #m = 1 TO 2 BY -1.
COMPUTE h = h + 1.
END LOOP."####,
        columns: &[
            (r####"a"####, r####"22 22 22 22 22 22 22 22 22 22 22 22"####),
            (r####"b"####, r####"5 5 5 5 5 5 5 5 5 5 5 5"####),
            (r####"g"####, r####"55 25 25 55 22 104.5 0 0 0 55 25 55"####),
            (r####"h"####, r####"0 0 0 0 0 0 0 0 0 0 0 0"####),
        ],
    },
    Case {
        label: r####"the same range with BY written first"####,
        command: r####"COMPUTE a = 0.
LOOP #i = 2 BY 2 TO 10.
COMPUTE a = a + #i.
END LOOP."####,
        columns: &[(r####"a"####, r####"30 30 30 30 30 30 30 30 30 30 30 30"####)],
    },
    Case {
        label: r####"bounds that are expressions, equal bounds run once, a start past the end runs never"####,
        command: r####"COMPUTE a = 0.
LOOP #i = x - 2 TO x + 2.
COMPUTE a = a + 1.
END LOOP.
COMPUTE b = 0.
LOOP #j = y TO y.
COMPUTE b = b + 1.
END LOOP.
COMPUTE c = 0.
LOOP #k = x TO x - 1.
COMPUTE c = c + 1.
END LOOP."####,
        columns: &[
            (r####"a"####, r####"5 5 5 0 5 5 5 5 5 0 5 5"####),
            (r####"b"####, r####"1 1 0 1 1 1 1 1 1 1 0 1"####),
            (r####"c"####, r####"0 0 0 0 0 0 0 0 0 0 0 0"####),
        ],
    },
    Case {
        label: r####"a missing start, bound or step gives no trip"####,
        command: r####"COMPUTE a = 0.
LOOP #i = x TO 5.
COMPUTE a = a + 1.
END LOOP.
COMPUTE b = 0.
LOOP #j = 1 TO y BY z.
COMPUTE b = b + 1.
END LOOP."####,
        columns: &[
            (r####"a"####, r####"3 6 2 0 0 4 0 5 0 0 1 4"####),
            (r####"b"####, r####"2 3 0 3 0 4 0 0 0 2 0 6"####),
        ],
    },
    Case {
        label: r####"the index variable after a normal end, a zero-trip loop, an END LOOP IF, a BREAK, an IF on LOOP"####,
        command: r####"COMPUTE s = 0.
LOOP a = 1 TO 5 BY 2.
COMPUTE s = s + 1.
END LOOP.
LOOP e = 5 TO 2.
COMPUTE s = s + 100.
END LOOP.
LOOP b = 1 TO 9.
END LOOP IF b = 4.
LOOP d = 1 TO 9.
DO IF d = 3.
BREAK.
END IF.
END LOOP.
LOOP c = 1 TO 9 IF c < 3.
END LOOP."####,
        columns: &[
            (r####"a"####, r####"7 7 7 7 7 7 7 7 7 7 7 7"####),
            (r####"e"####, r####"5 5 5 5 5 5 5 5 5 5 5 5"####),
            (r####"b"####, r####"4 4 4 4 4 4 4 4 4 4 4 4"####),
            (r####"d"####, r####"3 3 3 3 3 3 3 3 3 3 3 3"####),
            (r####"c"####, r####"3 3 3 3 3 3 3 3 3 3 3 3"####),
            (r####"s"####, r####"3 3 3 3 3 3 3 3 3 3 3 3"####),
        ],
    },
    Case {
        label: r####"changing the index variable inside the loop does not change the count"####,
        command: r####"NUMERIC #n.
LOOP a = 1 TO 3.
COMPUTE #n = #n + 1.
COMPUTE a = 0.
END LOOP.
COMPUTE na = #n.
COMPUTE s = 0.
LOOP b = 1 TO 4.
COMPUTE s = s + b.
COMPUTE b = b * 10.
END LOOP."####,
        columns: &[
            (r####"a"####, r####"4 4 4 4 4 4 4 4 4 4 4 4"####),
            (r####"na"####, r####"3 6 9 12 15 18 21 24 27 30 33 36"####),
            (r####"s"####, r####"10 10 10 10 10 10 10 10 10 10 10 10"####),
            (r####"b"####, r####"5 5 5 5 5 5 5 5 5 5 5 5"####),
        ],
    },
    Case {
        label: r####"LOOP IF is a while: it may run no trip, and a missing condition runs none"####,
        command: r####"COMPUTE a = x.
LOOP IF (a < 5).
COMPUTE a = a + 2.
END LOOP.
COMPUTE b = 0.
LOOP IF (y > 1).
COMPUTE b = b + 1.
END LOOP IF (b >= 3)."####,
        columns: &[
            (r####"a"####, r####"5 6 6 . 7 6 10 5 6 . 5 6"####),
            (r####"b"####, r####"3 3 0 3 0 3 3 0 3 3 0 3"####),
        ],
    },
    Case {
        label: r####"END LOOP IF is an until: at least one trip, a missing condition ends the loop after the first"####,
        command: r####"COMPUTE a = 0.
LOOP.
COMPUTE a = a + 1.
END LOOP IF (a >= x).
COMPUTE b = 0.
LOOP.
COMPUTE b = b + 2.
END LOOP IF (b > y)."####,
        columns: &[
            (r####"a"####, r####"3 1 4 1 7 2 10 1 6 1 5 2"####),
            (r####"b"####, r####"4 6 2 4 2 4 6 2 4 4 2 8"####),
        ],
    },
    Case {
        label: r####"BREAK leaves the innermost loop only, and a loop with an index and an IF"####,
        command: r####"COMPUTE a = 0.
COMPUTE b = 0.
LOOP #i = 1 TO 4.
LOOP #j = 1 TO 5.
COMPUTE a = a + 1.
DO IF #j >= 2.
BREAK.
END IF.
END LOOP.
COMPUTE b = b + #i.
END LOOP.
COMPUTE c = 0.
LOOP #k = 1 TO 10 IF (#k * z < 5).
COMPUTE c = c + #k.
END LOOP."####,
        columns: &[
            (r####"a"####, r####"8 8 8 8 8 8 8 8 8 8 8 8"####),
            (r####"b"####, r####"10 10 10 10 10 10 10 10 10 10 10 10"####),
            (r####"c"####, r####"10 3 3 10 1 45 55 55 0 10 3 10"####),
        ],
    },
    Case {
        label: r####"a loop without an index stops at 40 trips; SET MXLOOPS changes it; an index loop ignores it"####,
        command: r####"COMPUTE a = 0.
LOOP.
COMPUTE a = a + 1.
END LOOP.
SET MXLOOPS=7.
COMPUTE b = 0.
LOOP IF (b < 50).
COMPUTE b = b + 1.
END LOOP.
COMPUTE c = 0.
LOOP #i = 1 TO 100.
COMPUTE c = c + 1.
END LOOP.
COMPUTE d = 0.
LOOP.
COMPUTE d = d + 1.
END LOOP IF (d >= 5)."####,
        columns: &[
            (r####"a"####, r####"40 40 40 40 40 40 40 40 40 40 40 40"####),
            (r####"b"####, r####"7 7 7 7 7 7 7 7 7 7 7 7"####),
            (
                r####"c"####,
                r####"100 100 100 100 100 100 100 100 100 100 100 100"####,
            ),
            (r####"d"####, r####"5 5 5 5 5 5 5 5 5 5 5 5"####),
        ],
    },
    Case {
        label: r####"an END LOOP IF on a user-missing code: the value becomes missing in the test"####,
        command: r####"COMPUTE a = 0.
LOOP.
COMPUTE a = a + 1.
END LOOP IF (a >= x).
COMPUTE s = 0.
LOOP #i = 1 TO 3 IF (NOT MISSING(y)).
COMPUTE s = s + y.
END LOOP."####,
        columns: &[
            (r####"a"####, r####"3 1 4 1 7 2 10 1 6 1 5 2"####),
            (r####"s"####, r####"6 15 0 9 -3 7.5 12 0 9 6 0 18"####),
        ],
    },
    Case {
        label: r####"a scratch variable keeps its value from case to case, a permanent one starts missing"####,
        command: r####"LOOP #i = 1 TO 3.
COMPUTE #run = #run + #i.
COMPUTE p = SUM(p, #i).
END LOOP.
COMPUTE run = #run."####,
        columns: &[
            (
                r####"run"####,
                r####"6 12 18 24 30 36 42 48 54 60 66 72"####,
            ),
            (r####"p"####, r####"6 6 6 6 6 6 6 6 6 6 6 6"####),
        ],
    },
    Case {
        label: r####"nested loops in an input program with scratch indices: one case a trip of the inner loop"####,
        command: r####"NEW FILE.
INPUT PROGRAM.
LOOP #i = 1 TO 3.
LOOP #j = #i TO 4.
COMPUTE var1 = #i.
COMPUTE var2 = #j.
END CASE.
END LOOP.
END LOOP.
END FILE.
END INPUT PROGRAM.
EXECUTE."####,
        columns: &[
            (r####"var1"####, r####"1 1 1 1 2 2 2 3 3"####),
            (r####"var2"####, r####"1 2 3 4 2 3 4 3 4"####),
        ],
    },
    Case {
        label: r####"nested loops in an input program with permanent indices: the outer index is missing after the first trip"####,
        command: r####"NEW FILE.
INPUT PROGRAM.
LOOP i = 1 TO 3.
LOOP j = i TO 4.
COMPUTE var1 = i.
COMPUTE var2 = j.
END CASE.
END LOOP.
END LOOP.
END FILE.
END INPUT PROGRAM.
EXECUTE."####,
        columns: &[
            (r####"i"####, r####"1 . . . 2 . . 3 ."####),
            (r####"j"####, r####"1 2 3 4 2 3 4 3 4"####),
            (r####"var1"####, r####"1 . . . 2 . . 3 ."####),
            (r####"var2"####, r####"1 2 3 4 2 3 4 3 4"####),
        ],
    },
    Case {
        label: r####"a loop that creates cases from the values of a variable"####,
        command: r####"NEW FILE.
INPUT PROGRAM.
LOOP #n = 1 TO 4.
COMPUTE amount = #n ** 2 / 4.
COMPUTE nr = #n.
END CASE.
END LOOP.
END FILE.
END INPUT PROGRAM.
EXECUTE."####,
        columns: &[
            (r####"amount"####, r####"0.25 1 2.25 4"####),
            (r####"nr"####, r####"1 2 3 4"####),
        ],
    },
    Case {
        label: r####"a vector over existing variables: read each element, then change each one"####,
        command: r####"VECTOR v = x TO z.
COMPUTE s = 0.
LOOP #i = 1 TO 3.
COMPUTE s = s + v(#i).
END LOOP.
LOOP #i = 1 TO 3.
COMPUTE v(#i) = v(#i) * 2.
END LOOP."####,
        columns: &[
            (r####"s"####, r####"6 7 . . 9 5 13 1 . . . 9"####),
            (r####"x"####, r####"6 0 8 . 14 4 20 2 12 . 10 4"####),
            (r####"y"####, r####"4 10 . 6 -2 5 8 0 6 4 . 12"####),
            (r####"z"####, r####"2 4 4 2 6 1 -2 0 . 2 4 2"####),
        ],
    },
    Case {
        label: r####"the short form makes variables that start missing, and the index may come from the data"####,
        command: r####"VECTOR w(4).
COMPUTE w(MOD(id, 4) + 1) = id * 10.
VECTOR p, q(2).
COMPUTE p2 = id.
COMPUTE q1 = p2 + 1."####,
        columns: &[
            (r####"w1"####, r####". . . 40 . . . 80 . . . 120"####),
            (r####"w2"####, r####"10 . . . 50 . . . 90 . . ."####),
            (r####"w3"####, r####". 20 . . . 60 . . . 100 . ."####),
            (r####"w4"####, r####". . 30 . . . 70 . . . 110 ."####),
            (r####"q1"####, r####"2 3 4 5 6 7 8 9 10 11 12 13"####),
        ],
    },
    Case {
        label: r####"a vector name takes the place of a function of the same name"####,
        command: r####"VECTOR mean(2).
COMPUTE mean(1) = id + 100.
COMPUTE m = mean(1).
COMPUTE k = MEAN(id, 5)."####,
        columns: &[
            (
                r####"m"####,
                r####"101 102 103 104 105 106 107 108 109 110 111 112"####,
            ),
            (r####"k"####, r####"3 3.5 4 4.5 5 5.5 6 6.5 7 7.5 8 8.5"####),
        ],
    },
    Case {
        label: r####"the mean of the valid values of a case replaces its missing values"####,
        command: r####"DATA LIST FREE /a1 a2 a3 a4 a5.
BEGIN DATA
1 99 2 3 5
2 3 4 5 6
2 3 5 . 99
99 99 . 99 99
7 7 7 7 7
0 99 4 1 3
END DATA.
MISSING VALUES a1 TO a5 (99).
COMPUTE m = MEAN(a1 TO a5).
VECTOR a = a1 TO a5.
LOOP #i = 1 TO 5.
DO IF MISSING(a(#i)).
COMPUTE a(#i) = m.
END IF.
END LOOP."####,
        columns: &[
            (r####"a1"####, r####"1 2 2 . 7 0"####),
            (r####"a2"####, r####"2.75 3 3 . 7 2"####),
            (r####"a3"####, r####"2 4 5 . 7 4"####),
            (r####"a4"####, r####"3 5 3.33333333333333 . 7 1"####),
            (r####"a5"####, r####"5 6 3.33333333333333 . 7 3"####),
        ],
    },
    Case {
        label: r####"an element chosen by a value of the data: one test number in each case"####,
        command: r####"DATA LIST FREE /student score testnum.
BEGIN DATA
1 10 1
1 20 2
1 30 3
1 40 4
2 15 2
2 25 3
3 40 1
3 55 3
3 60 4
END DATA.
VECTOR result(4).
COMPUTE result(testnum) = score."####,
        columns: &[
            (r####"result1"####, r####"10 . . . . . 40 . ."####),
            (r####"result2"####, r####". 20 . . 15 . . . ."####),
            (r####"result3"####, r####". . 30 . . 25 . 55 ."####),
            (r####"result4"####, r####". . . 40 . . . . 60"####),
        ],
    },
    Case {
        label: r####"DO REPEAT steps through lists of variables and of values together"####,
        command: r####"DO REPEAT v = x y z / k = 10 20 30 / r = r1 TO r3.
COMPUTE r = v + k.
END REPEAT.
DO REPEAT r = s1 TO s4 / k = 1 TO 4.
COMPUTE r = id * k.
END REPEAT."####,
        columns: &[
            (r####"r1"####, r####"13 10 14 . 17 12 20 11 16 . 15 12"####),
            (
                r####"r2"####,
                r####"22 25 . 23 19 22.5 24 20 23 22 . 26"####,
            ),
            (
                r####"r3"####,
                r####"31 32 32 31 33 30.5 29 30 . 31 32 31"####,
            ),
            (r####"s1"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
            (r####"s2"####, r####"2 4 6 8 10 12 14 16 18 20 22 24"####),
            (r####"s3"####, r####"3 6 9 12 15 18 21 24 27 30 33 36"####),
            (r####"s4"####, r####"4 8 12 16 20 24 28 32 36 40 44 48"####),
        ],
    },
    Case {
        label: r####"a stand-in variable has no effect on a variable of the same name outside"####,
        command: r####"DO REPEAT x = p q / v = 3 4.
COMPUTE x = id + v.
END REPEAT.
COMPUTE same = x * 1."####,
        columns: &[
            (r####"p"####, r####"4 5 6 7 8 9 10 11 12 13 14 15"####),
            (r####"q"####, r####"5 6 7 8 9 10 11 12 13 14 15 16"####),
            (r####"same"####, r####"3 0 4 . 7 2 10 1 6 . 5 2"####),
        ],
    },
    Case {
        label: r####"ALL in a replacement list means every variable there is"####,
        command: r####"DO REPEAT v = ALL / r = u1 TO u4.
COMPUTE r = v * 2.
END REPEAT."####,
        columns: &[
            (r####"u1"####, r####"2 4 6 8 10 12 14 16 18 20 22 24"####),
            (r####"u2"####, r####"6 0 8 . 14 4 20 2 12 . 10 4"####),
            (r####"u3"####, r####"4 10 . 6 -2 5 8 0 6 4 . 12"####),
            (r####"u4"####, r####"2 4 4 2 6 1 -2 0 . 2 4 2"####),
        ],
    },
    Case {
        label: r####"IF inside DO REPEAT: a missing value never satisfies the condition"####,
        command: r####"DO REPEAT t = t1 TO t3 / lim = 2 5 8.
IF (x > lim) t = 1.
END REPEAT."####,
        columns: &[
            (r####"t1"####, r####"1 . 1 . 1 . 1 . 1 . 1 ."####),
            (r####"t2"####, r####". . . . 1 . 1 . 1 . . ."####),
            (r####"t3"####, r####". . . . . . 1 . . . . ."####),
        ],
    },
    Case {
        label: r####"DO IF and a LOOP with BREAK inside DO REPEAT"####,
        command: r####"DO REPEAT r = r1 r2 / lim = 2 3.
COMPUTE r = 0.
LOOP #i = 1 TO 10.
DO IF #i > lim.
BREAK.
END IF.
COMPUTE r = r + id.
END LOOP.
END REPEAT.
DO REPEAT d = d1 d2 / v = y z.
DO IF v > 1.
COMPUTE d = 1.
ELSE.
COMPUTE d = 0.
END IF.
END REPEAT."####,
        columns: &[
            (r####"r1"####, r####"2 4 6 8 10 12 14 16 18 20 22 24"####),
            (r####"r2"####, r####"3 6 9 12 15 18 21 24 27 30 33 36"####),
            (r####"d1"####, r####"1 1 . 1 0 1 1 0 1 1 . 1"####),
            (r####"d2"####, r####"0 1 1 0 1 0 0 0 . 0 1 0"####),
        ],
    },
    Case {
        label: r####"strings in the replacement lists"####,
        command: r####"STRING s1 s2 (A3).
DO REPEAT s = s1 s2 / t = 'ab' 'cde'.
COMPUTE s = t.
END REPEAT."####,
        columns: &[
            (
                r####"s1"####,
                r####"ab|ab|ab|ab|ab|ab|ab|ab|ab|ab|ab|ab"####,
            ),
            (
                r####"s2"####,
                r####"cde|cde|cde|cde|cde|cde|cde|cde|cde|cde|cde|cde"####,
            ),
        ],
    },
    Case {
        label: r####"LEAVE keeps a running total: it starts at 0 and a missing value makes the rest missing"####,
        command: r####"COMPUTE tot = tot + x.
LEAVE tot.
COMPUTE cnt = cnt + 1.
LEAVE cnt.
COMPUTE ssum = SUM(ssum, x).
LEAVE ssum."####,
        columns: &[
            (r####"tot"####, r####"3 3 7 . . . . . . . . ."####),
            (r####"cnt"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
            (r####"ssum"####, r####"3 3 7 7 14 16 26 27 33 33 38 40"####),
        ],
    },
    Case {
        label: r####"LEAVE with a string, with a list of variables and inside DO IF"####,
        command: r####"STRING s (A4).
IF (id = 3) s = 'ab'.
IF (id = 8) s = 'zz'.
LEAVE s.
DO IF (z = 1).
COMPUTE g = g + 1.
END IF.
COMPUTE c1 = c1 + 1.
COMPUTE c2 = c2 + id.
LEAVE g.
LEAVE c1 TO c2."####,
        columns: &[
            (r####"s"####, r####"||ab|ab|ab|ab|ab|zz|zz|zz|zz|zz"####),
            (r####"g"####, r####"1 1 1 2 2 2 2 2 2 3 3 4"####),
            (r####"c1"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
            (r####"c2"####, r####"1 3 6 10 15 21 28 36 45 55 66 78"####),
        ],
    },
    Case {
        label: r####"arithmetic: a system-missing or user-missing operand gives system-missing; a result that is no number does too"####,
        command: r####"COMPUTE a1 = x + y.
COMPUTE a2 = x - y.
COMPUTE a3 = x * y.
COMPUTE a4 = x / y.
COMPUTE a5 = x ** 2.
COMPUTE a6 = -x.
COMPUTE a7 = ABS(y).
COMPUTE a8 = SQRT(y).
COMPUTE a9 = (x + 1) * (y - 1)."####,
        columns: &[
            (r####"a1"####, r####"5 5 . . 6 4.5 14 1 9 . . 8"####),
            (r####"a2"####, r####"1 -5 . . 8 -0.5 6 1 3 . . -4"####),
            (r####"a3"####, r####"6 0 . . -7 5 40 0 18 . . 12"####),
            (
                r####"a4"####,
                r####"1.5 0 . . -7 0.8 2.5 . 2 . . 0.333333333333333"####,
            ),
            (r####"a5"####, r####"9 0 16 . 49 4 100 1 36 . 25 4"####),
            (r####"a6"####, r####"-3 0 -4 . -7 -2 -10 -1 -6 . -5 -2"####),
            (r####"a7"####, r####"2 5 . 3 1 2.5 4 0 3 2 . 6"####),
            (
                r####"a8"####,
                r####"1.4142135623731 2.23606797749979 . 1.73205080756888 . 1.58113883008419 2 0 1.73205080756888 1.4142135623731 . 2.44948974278318"####,
            ),
            (r####"a9"####, r####"4 4 . . -16 4.5 33 -2 14 . . 15"####),
        ],
    },
    Case {
        label: r####"statistical functions skip missing arguments, and say how many were missing or valid"####,
        command: r####"COMPUTE f1 = SUM(x, y).
COMPUTE f2 = MEAN(x, y, z).
COMPUTE f3 = MIN(x, y).
COMPUTE f4 = MAX(x, y, z).
COMPUTE f5 = NMISS(x, y, z).
COMPUTE f6 = NVALID(x, y, z).
COMPUTE f7 = SUM.3(x, y, z).
COMPUTE f8 = MEAN.2(x, y, z)."####,
        columns: &[
            (r####"f1"####, r####"5 5 4 3 6 4.5 14 1 9 2 5 8"####),
            (
                r####"f2"####,
                r####"2 2.33333333333333 3 2 3 1.66666666666667 4.33333333333333 0.333333333333333 4.5 1.5 3.5 3"####,
            ),
            (r####"f3"####, r####"2 0 4 3 -1 2 4 0 3 2 5 2"####),
            (r####"f4"####, r####"3 5 4 3 7 2.5 10 1 6 2 5 6"####),
            (r####"f5"####, r####"0 0 1 1 0 0 0 0 1 1 1 0"####),
            (r####"f6"####, r####"3 3 2 2 3 3 3 3 2 2 2 3"####),
            (r####"f7"####, r####"6 7 . . 9 5 13 1 . . . 9"####),
            (
                r####"f8"####,
                r####"2 2.33333333333333 3 2 3 1.66666666666667 4.33333333333333 0.333333333333333 4.5 1.5 3.5 3"####,
            ),
        ],
    },
    Case {
        label: r####"VALUE sees the code of a user-missing value and the other functions do not"####,
        command: r####"COMPUTE v1 = VALUE(x) + 1.
COMPUTE v2 = VALUE(y) * 2.
COMPUTE v3 = MISSING(x + y).
COMPUTE v4 = SYSMIS(x + y).
COMPUTE v5 = RND(x / 3)."####,
        columns: &[
            (r####"v1"####, r####"4 1 5 . 8 3 11 2 7 99 6 3"####),
            (r####"v2"####, r####"4 10 . 6 -2 5 8 0 6 4 198 12"####),
            (r####"v3"####, r####"0 0 1 1 0 0 0 0 0 1 1 0"####),
            (r####"v4"####, r####"0 0 1 1 0 0 0 0 0 1 1 0"####),
            (r####"v5"####, r####"1 0 1 . 2 1 3 0 2 . 2 1"####),
        ],
    },
];

const XCASES: &[XCase] = &[
    XCase {
        label: r####"XSAVE writes the cases as they are at the command, whatever comes after it"####,
        command: r####"COMPUTE d = x * 2.
XSAVE OUTFILE='{OUT}/x1.sav'.
COMPUTE d = 0.
SELECT IF (id > 6).
EXECUTE."####,
        files: &[(
            r####"x1.sav"####,
            &[
                (r####"id"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
                (r####"x"####, r####"3 0 4 . 7 2 10 1 6 98 5 2"####),
                (r####"y"####, r####"2 5 . 3 -1 2.5 4 0 3 2 99 6"####),
                (r####"z"####, r####"1 2 2 1 3 0.5 -1 0 . 1 2 1"####),
                (r####"d"####, r####"6 0 8 . 14 4 20 2 12 . 10 4"####),
            ],
        )],
    },
    XCase {
        label: r####"a SELECT IF before XSAVE leaves cases out of the file, and a variable made after it is not in it"####,
        command: r####"SELECT IF (z > 0).
XSAVE OUTFILE='{OUT}/x2.sav'.
COMPUTE w = id.
EXECUTE."####,
        files: &[(
            r####"x2.sav"####,
            &[
                (r####"id"####, r####"1 2 3 4 5 6 10 11 12"####),
                (r####"x"####, r####"3 0 4 . 7 2 98 5 2"####),
                (r####"y"####, r####"2 5 . 3 -1 2.5 2 99 6"####),
                (r####"z"####, r####"1 2 2 1 3 0.5 1 2 1"####),
            ],
        )],
    },
    XCase {
        label: r####"TEMPORARY changes are in the file and not in the data"####,
        command: r####"TEMPORARY.
COMPUTE x = x + 100.
XSAVE OUTFILE='{OUT}/x3.sav'.
EXECUTE."####,
        files: &[(
            r####"x3.sav"####,
            &[
                (r####"id"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
                (
                    r####"x"####,
                    r####"103 100 104 . 107 102 110 101 106 . 105 102"####,
                ),
                (r####"y"####, r####"2 5 . 3 -1 2.5 4 0 3 2 99 6"####),
                (r####"z"####, r####"1 2 2 1 3 0.5 -1 0 . 1 2 1"####),
            ],
        )],
    },
    XCase {
        label: r####"two XSAVE commands with KEEP, DROP and RENAME, the second after a SELECT IF"####,
        command: r####"XSAVE OUTFILE='{OUT}/x4a.sav' /KEEP=z id x /RENAME=(x=xx).
SELECT IF (id <= 8).
XSAVE OUTFILE='{OUT}/x4b.sav' /DROP=z.
EXECUTE."####,
        files: &[
            (
                r####"x4a.sav"####,
                &[
                    (r####"z"####, r####"1 2 2 1 3 0.5 -1 0 . 1 2 1"####),
                    (r####"id"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
                    (r####"xx"####, r####"3 0 4 . 7 2 10 1 6 98 5 2"####),
                ],
            ),
            (
                r####"x4b.sav"####,
                &[
                    (r####"id"####, r####"1 2 3 4 5 6 7 8"####),
                    (r####"x"####, r####"3 0 4 . 7 2 10 1"####),
                    (r####"y"####, r####"2 5 . 3 -1 2.5 4 0"####),
                ],
            ),
        ],
    },
    XCase {
        label: r####"a LEAVE variable is saved with its value at the command; what is kept is the value at the end of the case"####,
        command: r####"COMPUTE tot = tot + id.
LEAVE tot.
XSAVE OUTFILE='{OUT}/x5.sav'.
COMPUTE tot = -1.
EXECUTE."####,
        files: &[(
            r####"x5.sav"####,
            &[
                (r####"id"####, r####"1 2 3 4 5 6 7 8 9 10 11 12"####),
                (r####"x"####, r####"3 0 4 . 7 2 10 1 6 98 5 2"####),
                (r####"y"####, r####"2 5 . 3 -1 2.5 4 0 3 2 99 6"####),
                (r####"z"####, r####"1 2 2 1 3 0.5 -1 0 . 1 2 1"####),
                (r####"tot"####, r####"1 1 2 3 4 5 6 7 8 9 10 11"####),
            ],
        )],
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

fn run(case: &Case) -> Result<Engine, String> {
    let source = format!("{DATA}{}\nEXECUTE.", case.command);
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
fn flow_commands_agree_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let engine = match run(case) {
            Ok(engine) => engine,
            Err(error) => {
                failures.push(format!("{}: error {error}", case.label));
                continue;
            }
        };
        for (name, want) in case.columns {
            let got = show(&engine, name).unwrap_or_else(|| "(no such variable)".into());
            if !same(&got, want) {
                failures.push(format!("{}: {name} is [{got}], R has [{want}]", case.label));
            }
        }
    }
    assert!(
        failures.is_empty(),
        "{} column checks failed in {} cases:\n{}",
        failures.len(),
        CASES.len(),
        failures.join("\n")
    );
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

/// Set `XSAVE_DIR` to a folder to keep the files there, for R to read with haven.
#[test]
fn xsave_files_hold_what_r_finds_in_them() {
    let kept = std::env::var_os("XSAVE_DIR").map(std::path::PathBuf::from);
    let dir = kept.clone().unwrap_or_else(|| {
        std::env::temp_dir().join(format!("statscraft-xsave-{}", std::process::id()))
    });
    std::fs::create_dir_all(&dir).unwrap();
    let mut failures = Vec::new();
    for case in XCASES {
        let source = format!(
            "{DATA}{}",
            case.command.replace("{OUT}", &dir.display().to_string())
        );
        let mut engine = Engine::default();
        if let Err(error) = execute_all(&mut engine, &source) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        for (file, columns) in case.files {
            let mut again = Engine::default();
            let path = dir.join(file);
            if let Err(error) = execute_all(&mut again, &format!("GET FILE='{}'.", path.display()))
            {
                failures.push(format!("{}: {file}: {error}", case.label));
                continue;
            }
            let names: Vec<String> = again
                .dataset()
                .map(|d| d.variable_names().map(str::to_string).collect())
                .unwrap_or_default();
            let wanted: Vec<&str> = columns.iter().map(|(n, _)| *n).collect();
            if names != wanted {
                failures.push(format!(
                    "{}: {file} has the variables {names:?}, R has {wanted:?}",
                    case.label
                ));
                continue;
            }
            for (name, want) in *columns {
                let got = show(&again, name).unwrap_or_default();
                if !same(&got, want) {
                    failures.push(format!(
                        "{}: {file} {name} is [{got}], R has [{want}]",
                        case.label
                    ));
                }
            }
        }
    }
    if kept.is_none() {
        std::fs::remove_dir_all(&dir).ok();
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn xsave_writes_when_the_data_are_read() {
    let dir = std::env::temp_dir().join(format!("statscraft-xsave-wait-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("late.sav");
    let mut engine = Engine::default();
    execute_all(
        &mut engine,
        &format!(
            "{DATA}COMPUTE d = id * 2.\nXSAVE OUTFILE='{}'.",
            file.display()
        ),
    )
    .unwrap();
    assert!(
        !file.exists(),
        "XSAVE waits for the next command that reads the data"
    );
    execute_all(&mut engine, "EXECUTE.").unwrap();
    assert!(file.exists());
    std::fs::remove_dir_all(&dir).ok();
}
