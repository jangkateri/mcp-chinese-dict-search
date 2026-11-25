"""Validate proposed normalization/link-resolution rules against the archive.

Usage: python -I validate_rules.py <path-to-zip>
Read-only. Prints what percentage of links resolve under the planned rules.
"""
import re
import sys
import unicodedata
import zipfile
from collections import Counter

z = zipfile.ZipFile(sys.argv[1])
LINK = re.compile(r'<a\s+[^>]*href="([^"]*)"[^>]*>(.*?)</a>', re.S)
files = [n for n in z.namelist() if n.endswith(".md")]


def normalize(text):
    text = text.strip()
    text = text.replace("\\[", "[").replace("\\]", "]").replace("\\*", "*")
    text = text.replace("\\|", "|").replace("\\.", ".").replace("\\_", "_")
    text = text.replace(" ", " ")
    text = unicodedata.normalize("NFKD", text)
    text = "".join(c for c in text if not unicodedata.combining(c))
    return re.sub(r"\s+", " ", text).lower()


keys = {}
entries = {}
for f in files:
    if not re.match(r"\d\d-[a-z]\.md$", f):
        continue
    lines = z.read(f).decode("utf-8").splitlines()
    for i, line in enumerate(lines):
        if line.startswith("\\*"):
            key = line[2:].strip()
            keys.setdefault(normalize(key), []).append((f, i))
            entries.setdefault(f, []).append((i, key))

# index: line-oriented; a line may have leading term + numbered anchors
index_text = z.read("38-index.md").decode("utf-8")
anchor_map = {}
unresolved_index = []
for line in index_text.splitlines():
    if "<a " not in line:
        continue
    links = LINK.findall(line)
    line_terms = [t for h, t in links if "#" in h]
    if not line_terms:
        continue
    prefix = LINK.sub("", line).strip()
    prefix = re.sub(r"\s+", " ", prefix)
    for href, text in links:
        if "#" not in href:
            continue
        target, anchor = href.split("#", 1)
        if target not in entries:
            continue  # nav/index links
        term = prefix if prefix else text
        anchor_map[anchor] = (target, term)
        if normalize(term) not in keys:
            unresolved_index.append((target, anchor, term))

print("index anchors mapped:", len(anchor_map))
print("index terms unresolved after normalization:", len(unresolved_index))
for u in unresolved_index[:15]:
    print("  ", u)

# link resolution using index
total = resolved = unresolved = 0
missing = Counter()
for f in files:
    for href, text in LINK.findall(z.read(f).decode("utf-8")):
        if "#" not in href:
            continue
        target, anchor = href.split("#", 1)
        if target not in entries:
            continue
        total += 1
        hit = anchor_map.get(anchor)
        if hit and normalize(hit[1]) in keys:
            resolved += 1
        else:
            unresolved += 1
            missing[(target, anchor, text)] += 1
print(f"dictionary-internal links: {total}, resolved via index: {resolved}, unresolved: {unresolved}")
for m, c in missing.most_common(20):
    print("  ", c, m)

# duplicate entry bodies
for f, items in entries.items():
    pass
dups = {k: v for k, v in keys.items() if len(v) > 1}
print("duplicate normalized keys:", len(dups))
for k, v in list(dups.items())[:5]:
    print("  ", k, v)
