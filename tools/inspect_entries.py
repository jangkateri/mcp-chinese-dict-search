"""Entry-level shape analysis for planning the main-dictionary parser.

Usage: python -I inspect_entries.py <path-to-zip>
"""
import re
import sys
import zipfile
from collections import Counter

z = zipfile.ZipFile(sys.argv[1])
files = [n for n in z.namelist() if re.match(r"\d\d-[a-z]\.md$", n)]

alias_like = re.compile(r"\|.*\\\*")  # "中文 | 繁體  \*pinyin / diacritics"
stats = Counter()
examples = {}
multi_alias = []
no_alias = []
body_first = Counter()

for f in sorted(files):
    lines = z.read(f).decode("utf-8").splitlines()
    starts = [i for i, l in enumerate(lines) if l.startswith("\\*")]
    for n, i in enumerate(starts):
        end = starts[n + 1] if n + 1 < len(starts) else len(lines)
        block = lines[i + 1:end]
        # drop leading blanks
        j = 0
        while j < len(block) and not block[j].strip():
            j += 1
        aliases = []
        while j < len(block) and alias_like.search(block[j]):
            aliases.append(block[j])
            j += 1
            while j < len(block) and not block[j].strip():
                j += 1
        stats["entries"] += 1
        stats["alias_lines"] += len(aliases)
        if len(aliases) == 0:
            no_alias.append((f, lines[i][:80]))
        elif len(aliases) > 1:
            multi_alias.append((f, lines[i][:60], len(aliases)))
        if aliases:
            first = aliases[0]
            if " / " not in first:
                stats["alias_without_slash"] += 1
                examples.setdefault("alias_without_slash", (f, lines[i], first))
            if first.count("\\*") != 1:
                stats["alias_star_count_odd"] += 1
                examples.setdefault("alias_star_count_odd", (f, lines[i], first))
        # first body line kind
        if j < len(block):
            line = block[j]
            if line.startswith("PATTERNS"):
                body_first["PATTERNS"] += 1
            elif line.startswith("See "):
                body_first["See"] += 1
            elif line.startswith("Also "):
                body_first["Also"] += 1
            elif line.startswith("\\*"):
                body_first["star"] += 1
            else:
                body_first["text"] += 1

print(stats)
print("multiple alias lines:", len(multi_alias), multi_alias[:10])
print("no alias line:", len(no_alias), no_alias[:20])
print("examples:", examples)
print("first body paragraph kinds:", body_first.most_common())

# key length and characters
keys = []
for f in sorted(files):
    for l in z.read(f).decode("utf-8").splitlines():
        if l.startswith("\\*"):
            keys.append(l[2:].strip())
print("key char length max:", max(map(len, keys)), "avg:", sum(map(len, keys)) / len(keys))
print("keys with '(' or ')':", [k for k in keys if "(" in k or ")" in k][:10])
print("keys with comma:", [k for k in keys if "," in k][:10])
