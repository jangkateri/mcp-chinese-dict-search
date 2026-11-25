"""Relate class_s1XXX-N names to a1XXX anchors and enclosing entries.

Usage: python -I analyze_classes.py <path-to-zip>
Read-only.
"""
import re
import sys
import zipfile
from collections import Counter, defaultdict

z = zipfile.ZipFile(sys.argv[1])
md = [n for n in z.namelist() if n.endswith(".md")]
main = [n for n in md if re.match(r"\d\d-[a-z]\.md$", n)]
ANCHOR = re.compile(r'href="[^"]*#(a1[A-Z0-9]{3})"')
CLASS = re.compile(r'class="class_s1([A-Z0-9]{3})(?:-(\d+))?"')
LINK = re.compile(r'<a\s+([^>]*)>(.*?)</a>', re.S)

anchor_codes = set()
class_codes = set()
class_variants = Counter()
link_rows = []  # (file, enclosing_anchor_code, target_code, class_code, variant, text)

for f in md:
    text = z.read(f).decode("utf-8")
    anchor_codes.update(ANCHOR.findall(text))
    for attrs, body in LINK.findall(text):
        c = CLASS.search(attrs)
        a = ANCHOR.search(attrs)
        if not c or not a:
            continue
        class_codes.add(c.group(1))
        class_variants[c.group(2)] += 1
        # enclosing entry: last line starting with \* before the link
        pos = text.find(attrs)
        before = text[:pos]
        enc = None
        for m in re.finditer(r"(?m)^\\\*.*$", before):
            enc = m.group(0)
        enc_code = None
        # find the anchor code for this enclosing entry via index map later
        link_rows.append((f, enc, a.group(1), c.group(1), c.group(2), body.strip()))

print("anchor codes:", len(anchor_codes))
print("class codes:", len(class_codes))
print("class codes also used as anchors:", len(class_codes & anchor_codes))
print("class codes not anchors:", sorted(class_codes - anchor_codes)[:20])
print("anchor codes never used as class:", len(anchor_codes - class_codes))
print("class variants:", class_variants.most_common())

# Build entry headword -> anchor code via index
index_text = z.read("38-index.md").decode("utf-8")
head_to_anchor = {}
for line in index_text.splitlines():
    for m in re.finditer(r'<a\s+href="(\d\d-[a-z]\.md)#(a1[A-Z0-9]{3})"[^>]*>(.*?)</a>', line):
        head_to_anchor[m.group(3).strip().lower()] = m.group(2)

# Enclosing headword extraction
def enclosing_head(f, body_text):
    return None

# Instead, walk files line by line with entry context.
matches_source = matches_target = neither = 0
examples = []
for f in main:
    lines = z.read(f).decode("utf-8").splitlines()
    current = None
    for line in lines:
        if line.startswith("\\*"):
            current = line[2:].strip()
        for m in re.finditer(r'<a\s+([^>]*)>(.*?)</a>', line):
            attrs, body = m.group(1), m.group(2)
            c = CLASS.search(attrs)
            a = ANCHOR.search(attrs)
            if not c:
                continue
            code = c.group(1)
            src_anchor = head_to_anchor.get((current or "").lower())
            src_code = src_anchor[2:] if src_anchor else None
            tgt_code = a.group(1)[2:] if a else None
            if code == src_code:
                matches_source += 1
            elif code == tgt_code:
                matches_target += 1
            else:
                neither += 1
                if len(examples) < 10:
                    examples.append((f, current, code, src_code, tgt_code, body.strip()))
print("\nclass code == enclosing entry anchor:", matches_source)
print("class code == link target anchor:", matches_target)
print("class code == neither:", neither)
for e in examples:
    print("  ", e)

# Do link classes group by source file / position?
by_pair = Counter((src is not None, tgt is not None) for _, _, _, src, tgt, _ in examples)
print("example pair presence:", by_pair)

# Count classes per code and see if one code maps to many targets
code_targets = defaultdict(set)
for f, enc, tgt, code, variant, body in link_rows:
    code_targets[code].add(tgt)
print("\nclass code -> distinct target anchors:")
print("  max:", max(len(v) for v in code_targets.values()),
      "avg:", round(sum(len(v) for v in code_targets.values()) / len(code_targets), 2))
print("  codes with 1 target:", sum(1 for v in code_targets.values() if len(v) == 1))
print("  codes with >10 targets:", sum(1 for v in code_targets.values() if len(v) > 10))
