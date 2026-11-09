#!/usr/bin/env python3
"""Generate the synthetic .sav/.zsav test fixtures and their .expected.json sidecars.

Built and verified with pyreadstat 1.3.7 (pandas 3.1.1, Python 3.13.2). Run from the repo root:

    uv run --with pyreadstat==1.3.7 --with pandas==3.1.1 python tests/fixtures/generate.py

Every run writes all pyreadstat fixtures, then reads each one back and checks it against the
in-script definitions. `generate.py verify` only runs the check (use it after generate_php.php).
All data and texts are invented for this project. See README.md.
"""
import datetime as dt
import json
import random
import struct
import sys
from pathlib import Path

import pandas as pd
import pyreadstat

HERE = Path(__file__).resolve().parent
SEED = 20261109
COMPRESSION_CODES = {"none": 0, "bytecode": 1, "zlib": 2}  # header field at byte offset 72


# ---------------------------------------------------------------- definition helpers

def num(name, label, fmt, measure, values, labels=None, missing=(), missing_range=None):
    return dict(name=name, type="numeric", label=label, format=fmt, measure=measure,
                value_labels=labels or {}, missing_values=list(missing),
                missing_range=missing_range, values=list(values))


def text(name, label, measure, values, labels=None):
    values = list(values)
    assert all(v == v.rstrip(" ") for v in values), f"{name}: trailing spaces cannot survive a .sav"
    # pyreadstat sizes a string variable by its longest value in UTF-8 bytes (minimum 1).
    width = max([1] + [len(v.encode("utf-8")) for v in values])
    return dict(name=name, type="string", width=width, label=label, format=f"A{width}",
                measure=measure, value_labels=labels or {}, missing_values=[],
                missing_range=None, values=values)


def sav_date(year, month, day):
    """the reference program stores dates as seconds since 1582-10-14 00:00:00."""
    return (dt.date(year, month, day) - dt.date(1582, 10, 14)).days * 86400


def key(value):
    """Value label key as a string: 1.0 -> "1", 2.5 -> "2.5", "EE" -> "EE"."""
    return str(int(value)) if isinstance(value, float) and value.is_integer() else str(value)


# ---------------------------------------------------------------- fixture definitions

LONG_COMMENT = ("I have been a customer for six years and the service has mostly been reliable, but the "
                "last two invoices were wrong and it took three phone calls to fix them. Please make the "
                "billing easier to follow.")
COMMENTS = [
    "The support team solved my problem quickly.",
    "Prices have gone up too much this year.",
    "I would like a clearer invoice.",
    "The mobile app keeps logging me out.",
    "Very happy with the service overall.",
    "Delivery took longer than promised.",
    "Please add more payment options.",
    "The store staff were friendly and helpful.",
    "Waiting time on the phone is too long.",
    "Nothing to add.",
]
CHANNELS = ["Phone", "Email", "Live chat", "Mobile app", "In-store visit"]
MENTIONS = ["First mention", "Second mention", "Third mention"]
PROVIDERS = {1: "Norvale Telecom", 2: "Bluefinch Mobile", 3: "Harborline", 4: "Summit and Pine",
             5: "Quillnet", 6: "Other provider"}


def survey_basic():
    rng = random.Random(SEED)
    n = 200
    assert len(LONG_COMMENT) == 200
    sat = rng.choices([1, 2, 3, 4, 5, 98, 99], weights=[6, 12, 22, 34, 18, 5, 3], k=n)
    rec = rng.choices(list(range(11)) + [97, 98, 99], weights=[2] * 6 + [6, 9, 12, 9, 7] + [2, 3, 2], k=n)
    assert {98, 99} <= set(sat) and {97, 98, 99} <= set(rec)
    comments = ["" if rng.random() < 0.45 else " ".join(rng.sample(COMMENTS, rng.choice([1, 1, 2])))
                for _ in range(n)]
    comments[7] = LONG_COMMENT
    mentions = [(rng.sample(sorted(PROVIDERS), rng.choice([0, 1, 1, 2, 2, 3])) + [None] * 3)[:3]
                for _ in range(n)]
    first_day = dt.date(2019, 1, 1)
    dates = [first_day + dt.timedelta(days=rng.randrange(2500)) for _ in range(n)]
    variables = [
        num("resp_id", "Respondent ID", "F6.0", "nominal", range(1001, 1001 + n)),
        num("gender", "Gender", "F1.0", "nominal", rng.choices([1, 2, 3], weights=[48, 49, 3], k=n),
            {1: "Male", 2: "Female", 3: "Other or prefer to self-describe"}),
        num("age", "Age in years", "F3.0", "scale", [rng.randint(18, 84) for _ in range(n)]),
        num("region", "Region of residence", "F1.0", "nominal", [rng.randint(1, 5) for _ in range(n)],
            {1: "North", 2: "South", 3: "East", 4: "West", 5: "Central"}),
        num("q1_satisfaction", "Q1. Overall, how satisfied are you with our service?", "F2.0", "ordinal", sat,
            {1: "Very dissatisfied", 2: "Dissatisfied", 3: "Neither satisfied nor dissatisfied",
             4: "Satisfied", 5: "Very satisfied", 98: "Don't know", 99: "Refused"}, missing=[98, 99]),
        num("q2_recommend", "Q2. How likely are you to recommend us to a friend or colleague?", "F2.0",
            "scale", rec, {0: "Not at all likely", 10: "Extremely likely", 97: "Not applicable",
                           98: "Don't know", 99: "Refused"}, missing_range=[97, 99]),
        num("monthly_spend", "Average monthly spend (EUR)", "F8.2", "scale",
            [None if rng.random() < 0.08 else round(rng.uniform(5, 400), 2) for _ in range(n)]),
        text("plan", "Current subscription plan", "nominal",
             rng.choices(["BASIC", "PLUS", "PREMIUM", "ENTERPRISE"], weights=[40, 30, 20, 10], k=n)),
        text("q3_comment", "Q3. Is there anything else you would like to tell us?", "nominal", comments),
        num("signup_date", "Date the customer signed up", "DATE11", "scale",
            [sav_date(d.year, d.month, d.day) for d in dates]),
    ]
    for i, channel in enumerate(CHANNELS, 1):
        variables.append(num(f"q5_{i}", f"Q5. Which of these channels have you used to contact us? {channel}",
                             "F1.0", "nominal", [int(rng.random() < 0.2 + 0.1 * i) for _ in range(n)],
                             {0: "Not selected", 1: "Selected"}))
    for i, mention in enumerate(MENTIONS):
        variables.append(num(f"q6_mention{i + 1}", f"Q6. Which other providers come to mind? {mention}",
                             "F1.0", "nominal", [m[i] for m in mentions], PROVIDERS))
    variables.append(num("weight", "Post-stratification weight", "F8.4", "scale",
                         [round(rng.uniform(0.4, 2.2), 4) for _ in range(n)]))
    assert variables[7]["width"] == 10 and variables[8]["width"] == 200 and "" in comments
    return dict(file_label="StatsCraft synthetic customer survey", variables=variables)


# Only the php-sav writer can store these, so only survey_basic_phpwriter.sav carries them.
SURVEY_MR_SETS = [
    dict(name="$q5", type="dichotomy", label="Q5. Contact channels used", counted_value=1,
         variables=[f"q5_{i}" for i in range(1, 6)]),
    dict(name="$q6", type="category", label="Q6. Other providers mentioned", counted_value=None,
         variables=[f"q6_mention{i}" for i in range(1, 4)]),
]


def unicode_fixture():
    rng = random.Random(SEED + 1)
    n = 20
    cities = ["Tõrva", "Jõgeva", "Šiauliai", "Žagarė", "Zürich", "Köln", "Санкт-Петербург",
              "Θεσσαλονίκη", "東京", "北京"]
    greetings = ["Tere hommikust, õde ja äi!", "Grüß Gott, schöne Grüße aus Österreich",
                 "Доброе утро, друзья", "Καλημέρα σας", "こんにちは、お元気ですか", "你好，世界", "Hello 😀👍"]
    comments = ["", "Väga hea teenindus, aitäh! 😀", "Die Lieferung war zu spät, schade.",
                "Очень доволен обслуживанием", "Η εξυπηρέτηση ήταν άψογη", "とても満足しています 🎉",
                "Šokolaad ja žürii: täpitähed õäöü ÕÄÖÜ"]
    variables = [
        num("id", "Vastaja järjekorranumber (õ ä ö ü š ž)", "F4.0", "nominal", range(1, n + 1)),
        num("keel", "Küsitluse keel / Sprache der Befragung / Язык опроса", "F1.0", "nominal",
            [i % 6 + 1 for i in range(n)],
            {1: "Eesti keel (õäöü šž)", 2: "Deutsch (Größe, Straße)", 3: "Русский", 4: "Ελληνικά",
             5: "日本語", 6: "Emoji 😀"}),
        text("linn_õäöü", "Linn / Stadt / Город / Πόλη / 都市", "nominal", [cities[i % 10] for i in range(n)]),
        text("tervitus", "Tervitus vastaja emakeeles 👋", "nominal", [rng.choice(greetings) for _ in range(n)]),
        num("оценка", "Общая оценка / Συνολική βαθμολογία / 総合評価", "F1.0", "ordinal",
            [rng.choice([1, 2, 3, 3, 9]) for _ in range(n)],
            {1: "😞 Halb / schlecht / плохо", 2: "😐 Keskmine / mittel / средне",
             3: "😀 Hea / gut / хорошо", 9: "Ei tea / weiß nicht / не знаю"}, missing=[9]),
        text("コメント", "Vaba kommentaar / Freitext / Комментарий / Σχόλιο / コメント", "nominal",
             [rng.choice(comments) for _ in range(n)]),
    ]
    return dict(file_label="Unicode fixture: õäöü šž ß Я Ω 日本 😀", variables=variables)


def edge_cases():
    n = 12
    sentence = "The respondent wrote a very long answer about the service and kept on typing. "

    def long_text(length):
        cut = (sentence * 20)[:length]
        return cut[:-1] + "." if cut.endswith(" ") else cut

    long_values = [long_text(k) for k in [1200, 1000, 255, 256, 0, 1, 8, 9, 252, 253, 1199]]
    long_values.append("õ" * 600)  # 1200 bytes: a 2-byte character straddles every 255-byte segment edge
    name_64 = "this_variable_name_is_exactly_sixty_four_bytes_long".ljust(64, "x")
    assert len(name_64) == 64
    variables = [
        num("case_id", "Case number", "F4.0", "nominal", range(1, n + 1)),
        num(name_64, "Variable with a 64-character name", "F2.0", "scale", range(1, n + 1)),
        num("this_variable_name_is_a_second_long_one", "Long name sharing its first 8 bytes with another",
            "F2.0", "scale", range(n, 0, -1)),
        text("very_long_text", "Very long string (width 1200)", "nominal", long_values),
        num("all_missing", "Numeric variable where every case is system-missing", "F8.2", "scale", [None] * n),
        text("empty_text", "String variable where every case is empty", "nominal", [""] * n),
        num("extreme_numbers", "Very large, very small and negative numbers", "E16.8", "scale",
            [9.99e307, -9.99e307, 1e-300, 5e-324, -1e15, 1e15, -1e-300, 9007199254740992.0,
             3.141592653589793, 0, 0.30000000000000004, -123456789.12345679]),
        num("three_missing", "Exactly three discrete missing values", "F2.0", "ordinal",
            [97, 98, 99, 1, None, 2, 3, 4, 5, 1, 2, 3],
            {1: "Low", 5: "High", 97: "Not asked", 98: "Don't know", 99: "Refused"}, missing=[97, 98, 99]),
        num("range_plus_one", "Missing range plus one discrete missing value", "F3.0", "scale",
            [0, -1, 90, 99, None, 95, 5, 10, 7, 3, -1, 8],
            {-1: "Not applicable", 99: "Refused"}, missing=[-1], missing_range=[90, 99]),
        num("no_label", None, "F8.2", "scale", [i * 1.5 - 4 for i in range(n)]),
        text("country_code", "Short string (8 bytes or less) with value labels", "nominal",
             ["EE", "DE", "FI", "", "EE", "DE", "FI", "EE", "DE", "FI", "EE", "DE"],
             {"EE": "Estonia", "DE": "Germany", "FI": "Finland"}),
        text("plan_name", "Long string (more than 8 bytes) with value labels", "nominal",
             ["BASIC", "ENTERPRISE", "PLUS", "BASIC", "ENTERPRISE", "PLUS"] * 2,
             {"BASIC": "Basic plan", "ENTERPRISE": "Enterprise plan"}),
    ]
    return dict(file_label="StatsCraft edge cases", variables=variables)


def empty():
    return dict(variables=[
        num("id", "Respondent ID", "F8.0", "nominal", []),
        num("score", "Satisfaction score", "F5.1", "scale", [], {0: "Lowest", 10: "Highest"}, missing=[99]),
        text("name", "Customer name", "nominal", []),
        num("visit_date", "Date of last visit", "DATE11", "scale", []),
    ])


def wide():
    rng = random.Random(SEED + 2)
    return dict(variables=[
        num(f"v{i:03d}", f"Rating item {i} of 300", "F8.2", "scale",
            [None if rng.random() < 0.02 else round(rng.uniform(-50, 150), 2) for _ in range(10)])
        for i in range(1, 301)])


def build():
    survey = survey_basic()
    return [
        dict(survey, file="survey_basic.sav", compression="none"),
        dict(survey, file="survey_basic.zsav", compression="zlib"),
        dict(unicode_fixture(), file="unicode.sav", compression="none"),
        dict(edge_cases(), file="edge_cases.sav", compression="bytecode"),
        dict(empty(), file="empty.sav", compression="none"),
        dict(wide(), file="wide.sav", compression="bytecode"),
        dict(survey, file="survey_basic_phpwriter.sav", compression="bytecode", writer="php",
             mr_sets=SURVEY_MR_SETS, weight="weight"),
    ]


# ---------------------------------------------------------------- output

def cases(fx):
    return [list(row) for row in zip(*(v["values"] for v in fx["variables"]))]


def expected(fx):
    rows = cases(fx)
    out = {"encoding": "UTF-8", "compression": fx["compression"], "case_count": len(rows),
           "variable_count": len(fx["variables"])}
    if fx.get("file_label"):
        out["file_label"] = fx["file_label"]
    out["variables"] = [
        {k: x for k, x in v.items() if k != "values"}
        | {"value_labels": {key(k): label for k, label in v["value_labels"].items()},
           "n_system_missing": sum(x is None for x in v["values"])}
        for v in fx["variables"]]
    out["multiple_response_sets"] = fx.get("mr_sets", [])
    out["weight_variable"] = fx.get("weight")
    out["cases_sample"] = rows[:5] + rows[max(5, len(rows) - 2):]
    return out


def sidecar(fx, suffix=".expected.json"):
    """survey_basic.sav -> survey_basic.expected.json, survey_basic.zsav -> survey_basic.zsav.expected.json"""
    return HERE / (fx["file"].removesuffix(".sav") + suffix)


def dump(obj, path):
    path.write_text(json.dumps(obj, ensure_ascii=False, indent=1) + "\n", encoding="utf-8")


def write(fx):
    vs = fx["variables"]
    df = pd.DataFrame({v["name"]: pd.Series(v["values"], dtype="float64" if v["type"] == "numeric" else object)
                       for v in vs})
    pyreadstat.write_sav(
        df, HERE / fx["file"], file_label=fx.get("file_label", ""),
        compress=fx["compression"] == "zlib", row_compress=fx["compression"] == "bytecode",
        column_labels={v["name"]: v["label"] for v in vs if v["label"] is not None},
        variable_value_labels={v["name"]: v["value_labels"] for v in vs if v["value_labels"]},
        missing_ranges={v["name"]: v["missing_values"]
                        + ([dict(zip(("lo", "hi"), v["missing_range"]))] if v["missing_range"] else [])
                        for v in vs if v["missing_values"] or v["missing_range"]},
        variable_measure={v["name"]: v["measure"] for v in vs},
        variable_format={v["name"]: v["format"] for v in vs if v["type"] == "numeric"})


# ---------------------------------------------------------------- verification

def read_back(path):
    """What pyreadstat sees in the file, in the shape of expected() but with all cases."""
    df, m = pyreadstat.read_sav(path, user_missing=True, disable_datetime_conversion=True)
    variables = []
    for name, label in zip(m.column_names, m.column_labels):
        is_string = m.readstat_variable_types[name] == "string"
        ranges = m.missing_ranges.get(name, [])
        real_ranges = [[r["lo"], r["hi"]] for r in ranges if r["lo"] != r["hi"]]
        v = {"name": name, "type": "string" if is_string else "numeric"}
        if is_string:
            v["width"] = m.variable_storage_width[name]
        variables.append(v | {
            "label": label, "format": m.original_variable_types[name], "measure": m.variable_measure[name],
            "value_labels": {key(k): lab for k, lab in m.variable_value_labels.get(name, {}).items()},
            "missing_values": [r["lo"] for r in ranges if r["lo"] == r["hi"]],
            "missing_range": real_ranges[0] if real_ranges else None,
            "n_system_missing": 0 if is_string else int(df[name].isna().sum())})
    mr_sets = [dict(name=name if name.startswith("$") else "$" + name,
                    type="dichotomy" if s["is_dichotomy"] else "category", label=s["label"],
                    counted_value=s["counted_value"] if s["is_dichotomy"] else None,
                    variables=s["variable_list"]) for name, s in m.mr_sets.items()]
    rows = [[None if pd.isna(x) else x for x in row] for row in df.astype(object).values.tolist()]
    return {"encoding": m.file_encoding, "case_count": m.number_rows, "variable_count": m.number_columns,
            "file_label": m.file_label or None, "variables": variables,
            "multiple_response_sets": mr_sets, "cases": rows}


def weight_index(fx):
    """1-based dictionary position of the weight variable, counting 8-byte string segments."""
    if not fx.get("weight"):
        return 0
    position = 1
    for v in fx["variables"]:
        if v["name"] == fx["weight"]:
            return position
        # shortcut: 255-byte segment maths not implemented, add it if a wide string precedes the weight
        assert v.get("width", 8) <= 255
        position += -(-v.get("width", 8) // 8)
    raise ValueError("weight variable not found")


def verify(fx):
    path, exp, problems = HERE / fx["file"], expected(fx), []

    def check(what, got, want):
        if got != want:
            problems.append(f"{what}: file has {got!r:.300}, expected {want!r:.300}")

    check("expected.json on disk", json.loads(sidecar(fx).read_text(encoding="utf-8")), exp)
    head = path.read_bytes()[:84]
    check("magic", head[:4], b"$FL3" if fx["compression"] == "zlib" else b"$FL2")
    check("header compression, weight index, case count", struct.unpack("<3i", head[72:84]),
          (COMPRESSION_CODES[fx["compression"]], weight_index(fx), exp["case_count"]))
    got = read_back(path)
    for field in ("encoding", "case_count", "variable_count", "multiple_response_sets"):
        check(field, got[field], exp[field])
    check("file_label", got["file_label"], exp.get("file_label"))
    check("variable names", [v["name"] for v in got["variables"]], [v["name"] for v in exp["variables"]])
    for g, e in zip(got["variables"], exp["variables"]):
        for field in e:
            check(f"{e['name']}.{field}", sorted(g[field]) if field == "missing_values" else g[field],
                  sorted(e[field]) if field == "missing_values" else e[field])
    rows = got["cases"]
    check("all cases", rows, cases(fx))
    check("cases_sample", rows[:5] + rows[max(5, len(rows) - 2):], exp["cases_sample"])
    return problems


def main():
    fixtures = build()
    print(f"pyreadstat {pyreadstat.__version__}, pandas {pd.__version__}")
    if sys.argv[1:] != ["verify"]:
        for fx in fixtures:
            if fx.get("writer") == "php":  # generate_php.php turns this into the .sav and its sidecar
                dump({"expected": expected(fx), "cases": cases(fx)}, sidecar(fx, ".input.json"))
            else:
                write(fx)
                dump(expected(fx), sidecar(fx))
    failed = False
    for fx in fixtures:
        if not (HERE / fx["file"]).exists():
            print(f"SKIP {fx['file']} (not generated, run generate_php.php)")
            continue
        problems = verify(fx)
        failed |= bool(problems)
        print(f"{'FAIL' if problems else 'ok  '} {fx['file']}")
        for problem in problems:
            print("     " + problem)
    sys.exit(1 if failed else 0)


if __name__ == "__main__":
    main()
