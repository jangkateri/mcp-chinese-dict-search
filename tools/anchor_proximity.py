"""Test whether linked entries occupy nearby anchor-ID ranges.

Usage: python -I anchor_proximity.py <path-to-zip>
Read-only.
"""
import random
import re
import statistics
import sys
import zipfile

z = zipfile.ZipFile(sys.argv[1])
md = [n for n in z.namelist() if n.endswith(".md")]
main = [n for n in md if re.match(r"\d\d-[a-z]\.md$", n)]

# Index: term -> (file, anchor)
term_anchor = {}
anchor_value = {}
for line in z.read("38-index.md").decode("utf-8").splitlines():
    for m in re.finditer(r'href="(\d\d-[a-z]\.md)#(a1[A-Z0-9]{3})"[^>]*>(.*?)</a>', line):
        f, a, text = m.group(1), m.group(2), m.group(3).strip().lower()
        term_anchor.setdefault((f, text), a)
        anchor_value[a] = int(a[2:], 36)

# Links per entry, resolved by index text (the href text usually names the term)
pairs = []
for f in main:
    lines = z.read(f).decode("utf-8").splitlines()
    current = None
    current_anchor = None
    for line in lines:
        if line.startswith("\\*"):
            current = line[2:].strip().lower()
            current_anchor = term_anchor.get((f, current))
        if not current_anchor:
            continue
        for m in re.finditer(r'href="(\d\d-[a-z]\.md)#(a1[A-Z0-9]{3})"[^>]*>(.*?)</a>', line):
            target = m.group(2)
            if target == current_anchor:
                continue
            if target in anchor_value:
                pairs.append((current_anchor, target))

diffs = [abs(anchor_value[a] - anchor_value[b]) for a, b in pairs]
all_values = list(anchor_value.values())
random.seed(7)
baseline = [abs(random.choice(all_values) - random.choice(all_values)) for _ in range(len(diffs))]

print("resolved cross-reference pairs:", len(diffs))
print("median |delta anchor| linked: ", statistics.median(diffs))
print("median |delta anchor| random: ", statistics.median(baseline))
print("mean linked:", round(statistics.mean(diffs), 1), "mean random:", round(statistics.mean(baseline), 1))
for q in (0.1, 0.25, 0.5, 0.75, 0.9):
    print(f"quantile {q}: linked={statistics.quantiles(diffs, n=10)[int(q*10)-1] if q < 1 else max(diffs)}")

# How often is the numeric successor also a dictionary neighbour (same file or linked)?
print("\nsample of smallest and largest deltas:")
for a, b in sorted(pairs, key=lambda p: abs(anchor_value[p[0]] - anchor_value[p[1]]))[:5]:
    print("  near:", a, anchor_value[a], "->", b, anchor_value[b])
for a, b in sorted(pairs, key=lambda p: -abs(anchor_value[p[0]] - anchor_value[p[1]]))[:5]:
    print("  far: ", a, anchor_value[a], "->", b, anchor_value[b])
