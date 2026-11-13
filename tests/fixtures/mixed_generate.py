#!/usr/bin/env python3
"""Write the synthetic data of the MIXED tests: mixed_pupils.csv and mixed_waves.csv.

Run from the repo root with any Python 3: python3 tests/fixtures/mixed_generate.py
Only the standard library is used and the seed is fixed, so the files come out the same.
Every value is invented for this project.

mixed_pupils.csv: pupils in classes in schools. 9 schools with 2 to 4 classes each and 4 to 9
pupils per class. Columns: school, room (the class within its school, 1 to 4), pupil, girl (0/1),
track (a string factor: arts, sci, tech), hours (homework hours a week), mark (a test mark).
The mark has a school intercept, a school slope on hours, a class intercept and noise. Two marks
are empty (system-missing).

mixed_waves.csv: 30 people seen at up to four waves. Columns: person, arm (1 or 2), wave (1 to
4), dose (a covariate that changes over waves), level (the outcome). The errors within a person
follow a first-order autoregressive pattern with a variance that grows over the waves; about one
wave in ten is left out.
"""
import math
import random
from pathlib import Path

HERE = Path(__file__).resolve().parent
rng = random.Random(20261112)


def pupils():
    rows = []
    pupil = 0
    for school in range(1, 10):
        u0 = rng.gauss(0, 3.0)
        u1 = rng.gauss(0, 0.6)
        for room in range(1, rng.randint(2, 4) + 1):
            c0 = rng.gauss(0, 2.0)
            for _ in range(rng.randint(4, 9)):
                pupil += 1
                girl = rng.randint(0, 1)
                track = rng.choice(["arts", "sci", "tech"])
                hours = round(rng.uniform(0, 10), 1)
                effect = {"arts": 0.0, "sci": 2.5, "tech": -1.0}[track]
                mark = (50 + u0 + c0 + (1.2 + u1) * hours + 1.5 * girl + effect
                        + rng.gauss(0, 4.0))
                rows.append([school, room, pupil, girl, track, f"{hours:.1f}", f"{mark:.1f}"])
    for k in (7, 41):
        rows[k][6] = ""
    return ["school", "room", "pupil", "girl", "track", "hours", "mark"], rows


def waves():
    rows = []
    rho, sds = 0.6, [2.0, 2.4, 2.8, 3.2]
    for person in range(1, 31):
        arm = 1 + (person % 2)
        base = rng.gauss(0, 1.5)
        e = rng.gauss(0, 1.0)
        for wave in range(1, 5):
            if wave > 1:
                e = rho * e + math.sqrt(1 - rho * rho) * rng.gauss(0, 1.0)
            dose = round(rng.uniform(1, 5), 1)
            level = 20 + base + 1.5 * wave + 2.0 * (arm == 2) + 0.8 * dose + sds[wave - 1] * e
            if rng.random() < 0.1:
                continue
            rows.append([person, arm, wave, f"{dose:.1f}", f"{level:.2f}"])
    return ["person", "arm", "wave", "dose", "level"], rows


for name, (header, rows) in (("mixed_pupils.csv", pupils()), ("mixed_waves.csv", waves())):
    with open(HERE / name, "w", newline="\n") as f:
        f.write(",".join(header) + "\n")
        for row in rows:
            f.write(",".join(str(v) for v in row) + "\n")
