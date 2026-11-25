"""List bracketed appendix entry candidates and anomalies.

Usage: python -I inspect_bracketed.py <path-to-zip>
"""
import re
import sys
import zipfile
from collections import Counter

z = zipfile.ZipFile(sys.argv[1])
LINE = re.compile(r"^\s*\\\[([^\]]*)\\\]\s*(.*)$")
ID = re.compile(r"^\d\d\.\d\d\.\d\d(?:\.\w+)?$")

for needle in ["Medicinal", "Formulas"]:
    name = [x for x in z.namelist() if needle in x][0]
    lines = z.read(name).decode("utf-8").splitlines()
    starts = []
    for index, line in enumerate(lines):
        if line.strip().startswith("\\["):
            match = LINE.match(line)
            starts.append(
                (
                    index + 1,
                    match.group(1) if match else None,
                    (match.group(2)[:90] if match else line[:90]),
                    line[:20],
                )
            )
    print(f"== {name}: {len(starts)} trimmed bracketed lines")
    ids = [item[1] for item in starts]
    print("id shape:", Counter(bool(ID.fullmatch(value or "")) for value in ids))
    for item in starts:
        if not ID.fullmatch(item[1] or ""):
            print("  NONSTANDARD ID:", item)
    bad_header = [item for item in starts if " (" not in item[2]]
    print("headers without ' (':", len(bad_header))
    for item in bad_header[:15]:
        print("  HEADER:", item)
    lead = [item for item in starts if item[3] != item[3].lstrip()]
    print("leading whitespace:", len(lead), lead[:5])
    # Duplicate IDs
    dupes = {i: c for i, c in Counter(ids).items() if c > 1}
    print("duplicate ids:", dupes)
