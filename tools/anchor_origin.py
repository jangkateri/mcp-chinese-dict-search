"""Test whether the a1XXX anchors are sequential, hashed, or opaque.

Usage: python -I anchor_origin.py <path-to-zip>
Read-only.
"""
import hashlib
import re
import sys
import zipfile
import zlib
from collections import Counter

z = zipfile.ZipFile(sys.argv[1])
LINK = re.compile(r'<a\s+[^>]*href="([^"]*)"[^>]*>(.*?)</a>', re.S)
md = [n for n in z.namelist() if n.endswith(".md")]
main = [n for n in md if re.match(r"\d\d-[a-z]\.md$", n)]

pairs = []  # (target, anchor, text)
for f in md:
    for href, text in LINK.findall(z.read(f).decode("utf-8")):
        if "#" in href:
            target, anchor = href.split("#", 1)
            pairs.append((target, anchor, text.strip()))

vals = sorted({int(a[2:], 36) for _, a, _ in pairs})
print("distinct anchor values:", len(vals))
print("min:", vals[0], "max:", vals[-1], "range:", vals[-1] - vals[0] + 1)
print("density:", f"{len(vals) / (vals[-1] - vals[0] + 1):.3f}")
diffs = [b - a for a, b in zip(vals, vals[1:])]
print("sorted gap distribution:", Counter(diffs).most_common(10))
print("gaps > 50:", [d for d in diffs if d > 50][:20])
print("first 20:", vals[:20])
print("last 20:", vals[-20:])

# Is the anchor unique per headword? Group by normalized text.
by_text = {}
for target, anchor, text in pairs:
    by_text.setdefault(text.lower(), set()).add(anchor)
shared = {t: a for t, a in by_text.items() if len(a) > 1}
print("\nheadwords with >1 anchor:", len(shared))
for t, a in list(shared.items())[:10]:
    print("  ", t, sorted(a))

# Hash hypotheses: does base36(hash(text)) truncated/mod dedup match?
def b36(n):
    digits = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ"
    out = ""
    while n:
        out = digits[n % 36] + out
        n //= 36
    return out.rjust(3, "0")

sample = sorted({(text, anchor) for _, anchor, text in pairs})[:200]
checks = {
    "crc32 % 46656": lambda b: zlib.crc32(b) % 46656,
    "adler32 % 46656": lambda b: zlib.adler32(b) % 46656,
    "md5 first2": lambda b: int.from_bytes(hashlib.md5(b).digest()[:2], "big") % 46656,
    "sha1 first2": lambda b: int.from_bytes(hashlib.sha1(b).digest()[:2], "big") % 46656,
}
for name, fn in checks.items():
    hits = sum(1 for text, anchor in sample if b36(fn(text.encode("utf-8"))) == anchor[2:])
    print(f"hash hypothesis {name}: {hits}/{len(sample)}")

# Numeric value vs alphabetical rank across all entries.
entries = []
for f in sorted(main):
    lines = z.read(f).decode("utf-8").splitlines()
    for i, line in enumerate(lines):
        if line.startswith("\\*"):
            entries.append((len(entries), f, i, line[2:].strip()))
text_to_anchor = {}
for target, anchor, text in pairs:
    text_to_anchor.setdefault((target, text.lower()), anchor)

pairs_order = []
for rank, f, line, key in entries:
    a = text_to_anchor.get((f, key.lower()))
    if a:
        pairs_order.append((rank, int(a[2:], 36)))
print("\nentries with anchors (case-insensitive):", len(pairs_order), "of", len(entries))
n = len(pairs_order)
# Spearman correlation between rank and anchor value
def ranks(xs):
    order = sorted(range(len(xs)), key=lambda i: xs[i])
    r = [0] * len(xs)
    for pos, i in enumerate(order):
        r[i] = pos
    return r
rx = ranks([p[0] for p in pairs_order])
ry = ranks([p[1] for p in pairs_order])
mean = (n - 1) / 2
num = sum((a - mean) * (b - mean) for a, b in zip(rx, ry))
den = sum((a - mean) ** 2 for a in rx) ** 0.5 * sum((b - mean) ** 2 for b in ry) ** 0.5
print("Spearman rank correlation (alphabetical order vs anchor value):", round(num / den, 4))

# Which entries have no anchor after case-insensitive matching?
missing = [(f, key) for rank, f, line, key in entries if (f, key.lower()) not in text_to_anchor]
print("entries with no index anchor:", len(missing))
for m in missing[:20]:
    print("  ", m)
