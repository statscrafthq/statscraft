//! The DATE command against R on 40 invented observations. Calendar series (years, quarters,
//! months, and the label MMM YYYY) are R's `seq` of dates by month, quarter and year; the others
//! (week, day, hour, minute, second, cycle, observation, an increment with BY) are a counter that R does
//! in mixed radix from the manual's rules, written out in the R script: the lowest keyword steps by
//! the increment from its starting value, each one carries into the next higher at its periodicity,
//! hours, minutes and seconds count from 0 and the others from 1, a month implies its quarter (and a
//! quarter its first month), and the highest keyword with a periodicity gets a CYCLE_ above it.
//! The variables the command makes are those of the table, in its order, and DATE_ (a text made for
//! every command: its values are compared for the calendar series, the others only have to exist).
//! Keywords are tried in full and abbreviated (Y, M, Q, D, H, MI, S, W, OBS).

use statscraft_data::Values;
use statscraft_engine::Engine;
use statscraft_syntax::parse;

struct Case {
    label: &'static str,
    command: &'static str,
    /// The variables the command makes, in order, without DATE_ (checked on its own).
    order: &'static [&'static str],
    columns: &'static [(&'static str, &'static str)],
}

const DATA: &str = r####"DATA LIST FREE /id.
BEGIN DATA
1
2
3
4
5
6
7
8
9
10
11
12
13
14
15
16
17
18
19
20
21
22
23
24
25
26
27
28
29
30
31
32
33
34
35
36
37
38
39
40
END DATA.
"####;

const CASES: &[Case] = &[
    Case {
        label: r####"DATE Y 1960 M: year, month and the label MMM YYYY"####,
        command: r####"DATE Y 1960 M."####,
        order: &[r####"YEAR_"####, r####"MONTH_"####],
        columns: &[
            (
                r####"YEAR_"####,
                r####"1960 1960 1960 1960 1960 1960 1960 1960 1960 1960 1960 1960 1961 1961 1961 1961 1961 1961 1961 1961 1961 1961 1961 1961 1962 1962 1962 1962 1962 1962 1962 1962 1962 1962 1962 1962 1963 1963 1963 1963"####,
            ),
            (
                r####"MONTH_"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4"####,
            ),
            (
                r####"DATE_"####,
                r####"JAN 1960|FEB 1960|MAR 1960|APR 1960|MAY 1960|JUN 1960|JUL 1960|AUG 1960|SEP 1960|OCT 1960|NOV 1960|DEC 1960|JAN 1961|FEB 1961|MAR 1961|APR 1961|MAY 1961|JUN 1961|JUL 1961|AUG 1961|SEP 1961|OCT 1961|NOV 1961|DEC 1961|JAN 1962|FEB 1962|MAR 1962|APR 1962|MAY 1962|JUN 1962|JUL 1962|AUG 1962|SEP 1962|OCT 1962|NOV 1962|DEC 1962|JAN 1963|FEB 1963|MAR 1963|APR 1963"####,
            ),
        ],
    },
    Case {
        label: r####"DATE Y 1950 Q 2 M: the month starts where the quarter says"####,
        command: r####"DATE Y 1950 Q 2 M."####,
        order: &[r####"YEAR_"####, r####"QUARTER_"####, r####"MONTH_"####],
        columns: &[
            (
                r####"YEAR_"####,
                r####"1950 1950 1950 1950 1950 1950 1950 1950 1950 1951 1951 1951 1951 1951 1951 1951 1951 1951 1951 1951 1951 1952 1952 1952 1952 1952 1952 1952 1952 1952 1952 1952 1952 1953 1953 1953 1953 1953 1953 1953"####,
            ),
            (
                r####"QUARTER_"####,
                r####"2 2 2 3 3 3 4 4 4 1 1 1 2 2 2 3 3 3 4 4 4 1 1 1 2 2 2 3 3 3 4 4 4 1 1 1 2 2 2 3"####,
            ),
            (
                r####"MONTH_"####,
                r####"4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7"####,
            ),
            (
                r####"DATE_"####,
                r####"APR 1950|MAY 1950|JUN 1950|JUL 1950|AUG 1950|SEP 1950|OCT 1950|NOV 1950|DEC 1950|JAN 1951|FEB 1951|MAR 1951|APR 1951|MAY 1951|JUN 1951|JUL 1951|AUG 1951|SEP 1951|OCT 1951|NOV 1951|DEC 1951|JAN 1952|FEB 1952|MAR 1952|APR 1952|MAY 1952|JUN 1952|JUL 1952|AUG 1952|SEP 1952|OCT 1952|NOV 1952|DEC 1952|JAN 1953|FEB 1953|MAR 1953|APR 1953|MAY 1953|JUN 1953|JUL 1953"####,
            ),
        ],
    },
    Case {
        label: r####"DATE Y 1950 Q M 4 gives the same"####,
        command: r####"DATE Y 1950 Q M 4."####,
        order: &[r####"YEAR_"####, r####"QUARTER_"####, r####"MONTH_"####],
        columns: &[
            (
                r####"YEAR_"####,
                r####"1950 1950 1950 1950 1950 1950 1950 1950 1950 1951 1951 1951 1951 1951 1951 1951 1951 1951 1951 1951 1951 1952 1952 1952 1952 1952 1952 1952 1952 1952 1952 1952 1952 1953 1953 1953 1953 1953 1953 1953"####,
            ),
            (
                r####"QUARTER_"####,
                r####"2 2 2 3 3 3 4 4 4 1 1 1 2 2 2 3 3 3 4 4 4 1 1 1 2 2 2 3 3 3 4 4 4 1 1 1 2 2 2 3"####,
            ),
            (
                r####"MONTH_"####,
                r####"4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7"####,
            ),
        ],
    },
    Case {
        label: r####"DATE Y 1999 Q: quarters run on from year to year"####,
        command: r####"DATE Y 1999 Q."####,
        order: &[r####"YEAR_"####, r####"QUARTER_"####],
        columns: &[
            (
                r####"YEAR_"####,
                r####"1999 1999 1999 1999 2000 2000 2000 2000 2001 2001 2001 2001 2002 2002 2002 2002 2003 2003 2003 2003 2004 2004 2004 2004 2005 2005 2005 2005 2006 2006 2006 2006 2007 2007 2007 2007 2008 2008 2008 2008"####,
            ),
            (
                r####"QUARTER_"####,
                r####"1 2 3 4 1 2 3 4 1 2 3 4 1 2 3 4 1 2 3 4 1 2 3 4 1 2 3 4 1 2 3 4 1 2 3 4 1 2 3 4"####,
            ),
        ],
    },
    Case {
        label: r####"DATE Y 2000 M 11: a starting month"####,
        command: r####"DATE Y 2000 M 11."####,
        order: &[r####"YEAR_"####, r####"MONTH_"####],
        columns: &[
            (
                r####"YEAR_"####,
                r####"2000 2000 2001 2001 2001 2001 2001 2001 2001 2001 2001 2001 2001 2001 2002 2002 2002 2002 2002 2002 2002 2002 2002 2002 2002 2002 2003 2003 2003 2003 2003 2003 2003 2003 2003 2003 2003 2003 2004 2004"####,
            ),
            (
                r####"MONTH_"####,
                r####"11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2"####,
            ),
        ],
    },
    Case {
        label: r####"DATE Y 1980: years only"####,
        command: r####"DATE Y 1980."####,
        order: &[r####"YEAR_"####],
        columns: &[(
            r####"YEAR_"####,
            r####"1980 1981 1982 1983 1984 1985 1986 1987 1988 1989 1990 1991 1992 1993 1994 1995 1996 1997 1998 1999 2000 2001 2002 2003 2004 2005 2006 2007 2008 2009 2010 2011 2012 2013 2014 2015 2016 2017 2018 2019"####,
        )],
    },
    Case {
        label: r####"DATE WEEK DAY 1 5 HOUR 1 8: a five day week of eight hours, hours from 0 to 7"####,
        command: r####"DATE WEEK DAY 1 5 HOUR 1 8."####,
        order: &[r####"WEEK_"####, r####"DAY_"####, r####"HOUR_"####],
        columns: &[
            (
                r####"WEEK_"####,
                r####"1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 2"####,
            ),
            (
                r####"DAY_"####,
                r####"1 1 1 1 1 1 1 2 2 2 2 2 2 2 2 3 3 3 3 3 3 3 3 4 4 4 4 4 4 4 4 5 5 5 5 5 5 5 5 1"####,
            ),
            (
                r####"HOUR_"####,
                r####"1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0"####,
            ),
        ],
    },
    Case {
        label: r####"DATE DAY 1 5 HOUR 3 8: the highest keyword has a periodicity, so CYCLE_ is made"####,
        command: r####"DATE DAY 1 5 HOUR 3 8."####,
        order: &[r####"CYCLE_"####, r####"DAY_"####, r####"HOUR_"####],
        columns: &[
            (
                r####"CYCLE_"####,
                r####"1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 2 2 2"####,
            ),
            (
                r####"DAY_"####,
                r####"1 1 1 1 1 2 2 2 2 2 2 2 2 3 3 3 3 3 3 3 3 4 4 4 4 4 4 4 4 5 5 5 5 5 5 5 5 1 1 1"####,
            ),
            (
                r####"HOUR_"####,
                r####"3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2 3 4 5 6 7 0 1 2"####,
            ),
        ],
    },
    Case {
        label: r####"DATE DAY HOUR 1 24 BY 2: every second hour"####,
        command: r####"DATE DAY HOUR 1 24 BY 2."####,
        order: &[r####"DAY_"####, r####"HOUR_"####],
        columns: &[
            (
                r####"DAY_"####,
                r####"1 1 1 1 1 1 1 1 1 1 1 1 2 2 2 2 2 2 2 2 2 2 2 2 3 3 3 3 3 3 3 3 3 3 3 3 4 4 4 4"####,
            ),
            (
                r####"HOUR_"####,
                r####"1 3 5 7 9 11 13 15 17 19 21 23 1 3 5 7 9 11 13 15 17 19 21 23 1 3 5 7 9 11 13 15 17 19 21 23 1 3 5 7"####,
            ),
        ],
    },
    Case {
        label: r####"DATE OBS 9 17: observations from 9 in cycles of 17"####,
        command: r####"DATE OBS 9 17."####,
        order: &[r####"CYCLE_"####, r####"OBS_"####],
        columns: &[
            (
                r####"CYCLE_"####,
                r####"1 1 1 1 1 1 1 1 1 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 2 3 3 3 3 3 3 3 3 3 3 3 3 3 3"####,
            ),
            (
                r####"OBS_"####,
                r####"9 10 11 12 13 14 15 16 17 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 1 2 3 4 5 6 7 8 9 10 11 12 13 14"####,
            ),
        ],
    },
    Case {
        label: r####"DATE W H 1 168: a skipped level needs the periodicity of the hour"####,
        command: r####"DATE W H 1 168."####,
        order: &[r####"WEEK_"####, r####"HOUR_"####],
        columns: &[
            (
                r####"WEEK_"####,
                r####"1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1"####,
            ),
            (
                r####"HOUR_"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39 40"####,
            ),
        ],
    },
    Case {
        label: r####"DATE DAY HOUR MINUTE: the default periodicities of the short keywords"####,
        command: r####"DATE DAY HOUR MINUTE."####,
        order: &[r####"DAY_"####, r####"HOUR_"####, r####"MINUTE_"####],
        columns: &[
            (
                r####"DAY_"####,
                r####"1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1"####,
            ),
            (
                r####"HOUR_"####,
                r####"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0"####,
            ),
            (
                r####"MINUTE_"####,
                r####"0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39"####,
            ),
        ],
    },
    Case {
        label: r####"the same with the abbreviations D H MI"####,
        command: r####"DATE D H MI."####,
        order: &[r####"DAY_"####, r####"HOUR_"####, r####"MINUTE_"####],
        columns: &[
            (
                r####"DAY_"####,
                r####"1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1 1"####,
            ),
            (
                r####"HOUR_"####,
                r####"0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0 0"####,
            ),
            (
                r####"MINUTE_"####,
                r####"0 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 19 20 21 22 23 24 25 26 27 28 29 30 31 32 33 34 35 36 37 38 39"####,
            ),
        ],
    },
    Case {
        label: r####"SECOND and its abbreviation S count on from the start, and BY steps them"####,
        command: r####"DATE S 5 BY 5."####,
        order: &[r####"SECOND_"####],
        columns: &[(
            r####"SECOND_"####,
            r####"5 10 15 20 25 30 35 40 45 50 55 60 65 70 75 80 85 90 95 100 105 110 115 120 125 130 135 140 145 150 155 160 165 170 175 180 185 190 195 200"####,
        )],
    },
    Case {
        label: r####"DATE YEAR 1960 MONTH, the keywords in full"####,
        command: r####"DATE YEAR 1960 MONTH."####,
        order: &[r####"YEAR_"####, r####"MONTH_"####],
        columns: &[
            (
                r####"YEAR_"####,
                r####"1960 1960 1960 1960 1960 1960 1960 1960 1960 1960 1960 1960 1961 1961 1961 1961 1961 1961 1961 1961 1961 1961 1961 1961 1962 1962 1962 1962 1962 1962 1962 1962 1962 1962 1962 1962 1963 1963 1963 1963"####,
            ),
            (
                r####"MONTH_"####,
                r####"1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4 5 6 7 8 9 10 11 12 1 2 3 4"####,
            ),
            (
                r####"DATE_"####,
                r####"JAN 1960|FEB 1960|MAR 1960|APR 1960|MAY 1960|JUN 1960|JUL 1960|AUG 1960|SEP 1960|OCT 1960|NOV 1960|DEC 1960|JAN 1961|FEB 1961|MAR 1961|APR 1961|MAY 1961|JUN 1961|JUL 1961|AUG 1961|SEP 1961|OCT 1961|NOV 1961|DEC 1961|JAN 1962|FEB 1962|MAR 1962|APR 1962|MAY 1962|JUN 1962|JUL 1962|AUG 1962|SEP 1962|OCT 1962|NOV 1962|DEC 1962|JAN 1963|FEB 1963|MAR 1963|APR 1963"####,
            ),
        ],
    },
    Case {
        label: r####"DATE MI 50 S 30 BY 15: minutes and seconds, an increment"####,
        command: r####"DATE MI 50 S 30 BY 15."####,
        order: &[r####"MINUTE_"####, r####"SECOND_"####],
        columns: &[
            (
                r####"MINUTE_"####,
                r####"50 50 51 51 51 51 52 52 52 52 53 53 53 53 54 54 54 54 55 55 55 55 56 56 56 56 57 57 57 57 58 58 58 58 59 59 59 59 60 60"####,
            ),
            (
                r####"SECOND_"####,
                r####"30 45 0 15 30 45 0 15 30 45 0 15 30 45 0 15 30 45 0 15 30 45 0 15 30 45 0 15 30 45 0 15 30 45 0 15 30 45 0 15"####,
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
fn the_date_command_agrees_with_r() {
    let mut failures = Vec::new();
    for case in CASES {
        let mut engine = Engine::default();
        if let Err(error) = execute_all(&mut engine, &format!("{DATA}{}\nEXECUTE.", case.command)) {
            failures.push(format!("{}: error {error}", case.label));
            continue;
        }
        let names: Vec<String> = engine
            .dataset()
            .map(|d| d.variable_names().map(str::to_string).collect())
            .unwrap_or_default();
        // the new variables, without DATE_ (a string made for every command), are the ones R found
        let made: Vec<&str> = names
            .iter()
            .map(String::as_str)
            .filter(|n| *n != "id" && *n != "DATE_")
            .collect();
        if made != case.order {
            failures.push(format!(
                "{}: the variables are {made:?}, R has {:?}",
                case.label, case.order
            ));
            continue;
        }
        if !names.iter().any(|n| n == "DATE_") {
            failures.push(format!("{}: no DATE_", case.label));
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
