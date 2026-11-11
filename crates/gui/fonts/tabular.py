"""Makes the digits of a font tabular by default: maps 0-9 to the glyphs its `tnum` feature
substitutes. The window cannot switch OpenType features on, and numbers in the grids and the
output must line up in columns.

    python3 tabular.py SchibstedGrotesk[wght].ttf SchibstedGrotesk.ttf

Needs fontTools (MIT). The font stays under the SIL Open Font License; it declares no Reserved
Font Name, so the modified copy may keep its name.
"""

import sys

from fontTools.ttLib import TTFont

source, target = sys.argv[1], sys.argv[2]
font = TTFont(source)
gsub = font["GSUB"].table
lookups = [
    i
    for record in gsub.FeatureList.FeatureRecord
    if record.FeatureTag == "tnum"
    for i in record.Feature.LookupListIndex
]
tabular = {}
for i in lookups:
    for subtable in gsub.LookupList.Lookup[i].SubTable:
        tabular.update(getattr(subtable, "mapping", {}))
digits = [chr(c) for c in range(ord("0"), ord("9") + 1)]
for table in font["cmap"].tables:
    for digit in digits:
        glyph = table.cmap.get(ord(digit))
        if glyph in tabular:
            table.cmap[ord(digit)] = tabular[glyph]
widths = {font["hmtx"][font.getBestCmap()[ord(d)]][0] for d in digits}
assert len(widths) == 1, f"digits still differ in width: {widths}"
font.save(target)
