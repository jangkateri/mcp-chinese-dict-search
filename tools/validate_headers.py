"""Probe header grammar coverage for main entries and appendices.

Usage: python -I validate_headers.py <path-to-zip>
Read-only. Reports how many headers a candidate grammar parses.
"""
import re
import sys
import zipfile

z = zipfile.ZipFile(sys.argv[1])
get = lambda name: z.read([n for n in z.namelist() if name in n][0]).decode("utf-8")

# --- main alias lines ---
alias_re = re.compile(r"^(?P<simp>.*?)\s*\\\|\s*(?P<trad>.*?)\s\xa0?\\\*(?P<toned>.*?)\s*/\s*(?P<ascii>.*)$")
total = ok = 0
bad = []
for n in z.namelist():
    if not re.match(r"\d\d-[a-z]\.md$", n):
        continue
    for line in z.read(n).decode("utf-8").splitlines():
        if "\\*" in line and "|" in line and " / " in line:
            total += 1
            m = alias_re.match(line)
            if m:
                ok += 1
            else:
                bad.append(line)
print(f"main alias lines: {total}, parsed: {ok}, failed: {len(bad)}")
for b in bad[:5]:
    print("  ", repr(b))

# --- appendix-1 headers ---
mer = get("Meridian")
h_re = re.compile(r"^(?P<id>[A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)+) \((?P<rest>.*)\)(?::\s*(?P<tail>.*))?$")
total = ok = 0
bad = []
for line in mer.splitlines():
    if h_re.match(line):
        total += 1
        ok += 1
print(f"appendix-1 headers matched by loose grammar: {total}")

# split header content on ' / ' and ', '
inner_re = re.compile(r"^(?P<chinese>.*?)\s*/\s*(?P<chinese2>.*?)\s+(?P<toned>.*?)\s*/\s*(?P<ascii>[^,]+),\s*(?P<english>.*)$")
inner_ok = 0
inner_bad = []
for line in mer.splitlines():
    m = h_re.match(line)
    if not m:
        continue
    inner = inner_re.match(m.group("rest"))
    if inner:
        inner_ok += 1
    else:
        inner_bad.append(m.group("rest"))
print(f"appendix-1 inner headers parsed: {inner_ok}, failed: {len(inner_bad)}")
for b in inner_bad[:10]:
    print("  ", repr(b[:140]))

# --- appendix-2/3 headers ---
for label, needle in [("medicinal", "Medicinal"), ("formulas", "Formulas")]:
    text = get(needle)
    entries = re.findall(r"^\\\[(?P<id>\d\d\.\d\d\.\d\d(?:\.\w+)?)\\\]\s+(?P<header>.*)$", text, re.M)
    h = re.compile(
        r"^(?P<py_up>[^/]+?)\s*/\s*(?P<py_low>.*?)\s+(?P<simp>[一-鿿]+)\s*/\s*(?P<trad>[一-鿿]+)\s*\((?P<latin>[^,()]+),\s*(?P<english>[^)]+)\)(?::\s*(?P<rest>.*))?$"
    )
    ok = 0
    bad = []
    for eid, header in entries:
        if h.match(header):
            ok += 1
        else:
            bad.append((eid, header))
    print(f"appendix-{label}: entries {len(entries)}, header parsed {ok}, failed {len(bad)}")
    for b in bad[:8]:
        print("  ", b[0], repr(b[1][:150]))

# ID suffix variants
for label, needle in [("medicinal", "Medicinal"), ("formulas", "Formulas")]:
    ids = re.findall(r"^\\\[(\d\d\.\d\d\.\d\d(?:\.\w+)?)\\\]", get(needle), re.M)
    suffixes = sorted({i.split(".", 3)[3] for i in ids if i.count(".") > 2})
    print(label, "suffix variants:", suffixes[:20])
