#!/usr/bin/env python3
"""Write the synthetic data of the complex samples tests: cs_households.csv.

Run from the repo root with any Python 3: python3 tests/fixtures/cs_generate.py
Only the standard library is used and the seed is fixed, so the file comes out the same.
Every value is invented for this project.

An invented household survey drawn in two stages. The country has four districts (the strata).
In each district a few villages were drawn without replacement out of all its villages, and in
each drawn village a few households out of all its households. Columns:

- district: the stratum, 1 to 4
- village: the village number, unique in the whole file
- hh: the household number
- nvill: how many villages the district has; svill: how many of them were drawn
- nhh: how many households the village has
- w: the household's weight, (nvill / svill) * (nhh / households drawn in the village)
- town: 1 for a village that counts as a town, 2 otherwise (a domain that cuts across strata)
- members: people in the household
- earn: monthly earnings; four are empty (system-missing)
- food: monthly spending on food
- heat: the main heating, 1 wood, 2 gas, 3 electric, 9 not stated (user-missing in the tests)
- online: 1 when the household has a fixed internet line, else 0
"""
import random
from pathlib import Path

HERE = Path(__file__).resolve().parent
rng = random.Random(20261113)

# villages in each district and how many were drawn
DISTRICTS = {1: (40, 5), 2: (64, 6), 3: (25, 4), 4: (52, 5)}


def households():
    rows = []
    village = 0
    hh = 0
    for district, (nvill, svill) in DISTRICTS.items():
        for _ in range(svill):
            village += 1
            nhh = rng.randint(30, 140)
            drawn = rng.randint(4, 9)
            w = round((nvill / svill) * (nhh / drawn), 4)
            town = 1 if rng.random() < 0.35 else 2
            level = rng.gauss(0, 250)
            for _ in range(drawn):
                hh += 1
                members = max(1, min(8, round(rng.gauss(2.8, 1.3))))
                earn = round(900 + 180 * district + 220 * (town == 1) + 110 * members + level
                             + rng.gauss(0, 300))
                food = round(120 + 55 * members + 0.08 * earn + rng.gauss(0, 40))
                heat = rng.choices([1, 2, 3], [0.5 - 0.25 * (town == 1), 0.3, 0.2 + 0.25 * (town == 1)])[0]
                online = 1 if rng.random() < 0.3 + 0.3 * (town == 1) + 0.04 * members else 0
                rows.append([district, village, hh, nvill, svill, nhh, w, town, members, earn,
                             food, heat, online])
    for k in (11, 40, 77, 103):
        rows[k][9] = ""
    for k in (5, 63, 120):
        rows[k][11] = 9
    header = ["district", "village", "hh", "nvill", "svill", "nhh", "w", "town", "members",
              "earn", "food", "heat", "online"]
    return header, rows


header, rows = households()
with open(HERE / "cs_households.csv", "w", newline="\n") as f:
    f.write(",".join(header) + "\n")
    for row in rows:
        f.write(",".join(str(v) for v in row) + "\n")
