"""Analyze the #anchor identifiers in the dictionary archive.

Usage: python -I analyze_anchors.py <path-to-zip>
Read-only. Tests whether anchors are semantic, sequential, or hashed.
"""
import re
import sys
import zipfile
from collections import Counter, defaultdict

z = zipfile.ZipFile(sys.argv[1])
LINK = re.compile(r'<a\s+[^>]*href="([^"]*)"[^>]*>(.*?)</a>', re.S)
md = [n for n in z.namelist() if n.endswith(".md")]
main = [n for n in md if re.match(r"\d\d-[a-z]\.md$", n)]

anchors = []          # (target, anchor, text, source_file)
for f in md:
    for href, text in LINK.findall(z.read(f).decode("utf-8")):
        if "#" in href:
            target, anchor = href.split("#", 1)
            anchors.append((target, anchor, text, f))

print("== anchor shape ==")
suffixes = [a[2:] for _, a, _, _ in anchors if a.startswith("a1")]
print("total hrefs with #:", len(anchors))
print("starting a1:", len(suffixes), "other:", Counter(a[:2] for _, a, _, _ in anchors if not a.startswith("a1")).most_common(5))
print("anchor lengths:", Counter(len(a) for _, a, _, _ in anchors).most_common())
print("char classes:", Counter(
    "digit" if c.isdigit() else "upper" if c.isupper() else "lower" if c.islower() else "other"
    for a in suffixes for c in a
).most_common())
print("suffix prefix distribution:", Counter(a[0] for a in suffixes).most_common())
print("suffix char-2 distribution:", Counter(a[1] for a in suffixes).most_common())
print("sample:", sorted(set(suffixes))[:20])

# Global uniqueness: is an anchor used for one target only?
by_anchor = defaultdict(set)
for target, anchor, text, _ in anchors:
    by_anchor[anchor].add((target, text))
multi = {a: v for a, v in by_anchor.items() if len(v) > 1}
print("\n== anchor uniqueness ==")
print("distinct anchors:", len(by_anchor))
print("anchors mapping to >1 (target,text):", len(multi))
for a, v in list(multi.items())[:10]:
    print("  ", a, list(v)[:5])

# For dictionary-target anchors, map to entry order in target file.
entries = {}
for f in main:
    lines = z.read(f).decode("utf-8").splitlines()
    entries[f] = [l[2:].strip() for l in lines if l.startswith("\\*")]

target_anchors = defaultdict(list)  # file -> [(index, anchor, text)]
seen = set()
for target, anchor, text, src in anchors:
    if target in entries and (target, anchor) not in seen:
        seen.add((target, anchor))
        try:
            idx = entries[target].index(text.strip())
        except ValueError:
            continue
        target_anchors[target].append((idx, anchor, text))

print("\n== anchor order vs entry order ==")
def b36(s):
    return int(s, 36)

for f in sorted(target_anchors)[:6]:
    rows = sorted(target_anchors[f])
    vals = [b36(a[2:]) for _, a, _ in rows]
    n = len(vals)
    # Spearman correlation between entry index and decoded value
    if n < 3:
        continue
    rank_idx = {v: i for i, v in enumerate(sorted(range(n), key=lambda k: rows[k][0]))}
    # simpler: count increasing adjacent pairs
    inc = sum(1 for i in range(1, len(vals)) if vals[i] > vals[i - 1])
    print(f"{f}: pairs={n}, adjacent increases={inc}/{n-1}, first={vals[:5]}, last={vals[-3:]}")
    print("   order:", [(rows[i][0], rows[i][1], vals[i]) for i in range(min(8, n))])

# Coverage: which entries have no anchor?
anchor_texts = defaultdict(set)
for target, anchor, text, src in anchors:
    anchor_texts[target].add(text.strip())
print("\n== per-file coverage of entries by index anchors ==")
for f in sorted(main):
    covered = sum(1 for e in entries[f] if e in anchor_texts[f])
    print(f"{f}: entries={len(entries[f])}, anchor-named={covered}, missing={len(entries[f]) - covered}")

# Are anchors shared between index and body links with same text?
print("\n== same anchor, different link text ==")
by_anchor_text = defaultdict(set)
for target, anchor, text, src in anchors:
    by_anchor_text[anchor].add((target, text.strip()))
differing = {a: v for a, v in by_anchor_text.items() if len({t for _, t in v}) > 1}
print("anchors with differing link text:", len(differing))
for a, v in list(differing.items())[:10]:
    print("  ", a, sorted(v)[:6])
