"""Inspect the anchor values as a sequence and test what order they follow.

Usage: python -I anchor_sequence.py <path-to-zip>
Read-only.
"""
import re
import sys
import unicodedata
import zipfile
from collections import defaultdict

z = zipfile.ZipFile(sys.argv[1])
LINK = re.compile(r'<a\s+[^>]*href="([^"]*)"[^>]*>(.*?)</a>', re.S)
md = [n for n in z.namelist() if n.endswith(".md")]
main = [n for n in md if re.match(r"\d\d-[a-z]\.md$", n)]

# Build entry table with global order, anchor, pinyin, Chinese.
entries = []
for f in sorted(main):
    lines = z.read(f).decode("utf-8").splitlines()
    starts = [i for i, l in enumerate(lines) if l.startswith("\\*")]
    for n, i in enumerate(starts):
        end = starts[n + 1] if n + 1 < len(starts) else len(lines)
        key = lines[i][2:].strip()
        block = [l for l in lines[i + 1:end] if l.strip()]
        pinyin = ""
        chinese = ""
        for l in block[:8]:
            if "\\*" in l and " / " in l and "|" in l:
                left, _, right = l.partition("\\*")
                chinese = left.split("|")[0].strip()
                pinyin = right.split("/")[0].strip()
                break
        entries.append({
            "order": len(entries), "file": f, "line": i, "key": key,
            "pinyin": pinyin, "chinese": chinese,
        })

# anchor -> (target, text)
anchor_info = {}
for f in md:
    for href, text in LINK.findall(z.read(f).decode("utf-8")):
        if "#" not in href:
            continue
        target, anchor = href.split("#", 1)
        if target in main:
            anchor_info.setdefault(anchor, (target, text.strip()))

# map entry -> anchor (case-insensitive)
by_file_key = {(e["file"], e["key"].lower()): e for e in entries}
for a, (target, text) in anchor_info.items():
    e = by_file_key.get((target, text.lower()))
    if e:
        e.setdefault("anchor", a)

with_anchor = [e for e in entries if "anchor" in e]
with_anchor.sort(key=lambda e: int(e["anchor"][2:], 36))

print("== first 60 anchors in numeric order ==")
for e in with_anchor[:60]:
    print(f"{e['anchor']} {int(e['anchor'][2:],36):5d}  {e['file']:8s}  {e['key'][:55]}")

print("\n== runs: consecutive anchor value -> consecutive entry order? ==")
by_val = {int(e["anchor"][2:], 36): e for e in with_anchor}
vals = sorted(by_val)
consecutive_anchor = consecutive_order = 0
for v in vals:
    e = by_val[v]
    nxt = by_val.get(v + 1)
    if nxt:
        consecutive_anchor += 1
        if nxt["order"] == e["order"] + 1:
            consecutive_order += 1
print("adjacent anchor pairs:", consecutive_anchor)
print("of which next entry in book order:", consecutive_order,
      f"({consecutive_order / max(1, consecutive_anchor):.1%})")

print("\n== large gaps ==")
for v in vals:
    nxt = next((w for w in vals if w > v), None)
    if nxt and nxt - v > 20:
        print(f"gap {v} -> {nxt} ({nxt-v})  last={by_val[v]['key'][:55]}  next={by_val[nxt]['key'][:55]}")

# What order does the numeric sequence follow? Compare to pinyin sort.
def rank_map(keyfn):
    ordered = sorted(range(len(with_anchor)), key=lambda i: keyfn(with_anchor[i]))
    rank = {i: r for r, i in enumerate(ordered)}
    return [rank[i] for i in range(len(with_anchor))]

def spearman(xs, ys):
    n = len(xs)
    mx = sum(xs) / n
    my = sum(ys) / n
    num = sum((a - mx) * (b - my) for a, b in zip(xs, ys))
    dx = sum((a - mx) ** 2 for a in xs) ** 0.5
    dy = sum((b - my) ** 2 for b in ys) ** 0.5
    return num / (dx * dy) if dx and dy else 0.0

numeric_rank = list(range(len(with_anchor)))
print("\n== correlation of numeric anchor order with other orders ==")
print("book order (alphabetical):", round(spearman(numeric_rank, rank_map(lambda e: e["order"])), 4))
print("pinyin order:", round(spearman(numeric_rank, rank_map(lambda e: e["pinyin"])), 4))
print("chinese order:", round(spearman(numeric_rank, rank_map(lambda e: e["chinese"])), 4))
print("key length:", round(spearman(numeric_rank, rank_map(lambda e: len(e["key"]))), 4))

# Show a contiguous numeric block with file/key metadata.
print("\n== a contiguous block, numeric order ==")
for e in with_anchor[200:260]:
    print(f"{e['anchor']} -> {e['file']} {e['key'][:60]}")
