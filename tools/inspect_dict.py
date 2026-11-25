"""Read-only structural analysis of the dictionary zip.

Usage: python -I inspect_dict.py <path-to-zip>
Writes nothing; prints statistics used for planning the Rust parser.
"""
import re
import sys
import zipfile
from collections import Counter, defaultdict

zip_path = sys.argv[1]
z = zipfile.ZipFile(zip_path)
names = z.namelist()
md_names = [n for n in names if n.endswith(".md")]

LINK_RE = re.compile(r'<a\s+[^>]*href="([^"]*)"[^>]*>(.*?)</a>', re.S)
IMG_RE = re.compile(r'!\[[^\]]*\]\(([^)]+)\)')

main_files = [n for n in md_names if re.match(r"\d\d-[a-z]\.md", n)]
appendix_files = [n for n in md_names if "appendix" in n]

entry_counts = {}
all_keys = defaultdict(list)  # key -> [(file, index)]
for f in sorted(md_names):
    text = z.read(f).decode("utf-8")
    lines = text.splitlines()
    keys = [(i, ln) for i, ln in enumerate(lines) if ln.startswith("\\*")]
    entry_counts[f] = len(keys)
    for i, ln in keys:
        all_keys[ln[2:].strip()].append((f, i))

print("== file entry counts (lines starting with \\*) ==")
for f in sorted(md_names):
    print(f"{entry_counts[f]:6d}  {f}")
print("total entries:", sum(entry_counts[f] for f in main_files))

print("\n== key anomalies ==")
print("distinct keys:", len(all_keys))
dups = {k: v for k, v in all_keys.items() if len(v) > 1}
print("duplicate keys:", len(dups))
for k, v in list(dups.items())[:15]:
    print("  DUP", repr(k), v)
print("empty keys:", [v for k, v in all_keys.items() if not k][:5])
print("keys containing backslash:", [k for k in all_keys if "\\" in k][:10])
print("keys containing markup:", [k for k in all_keys if "<" in k or "|" in k][:10])

# Index analysis
index_text = z.read("38-index.md").decode("utf-8")
idx = []  # (target_file, anchor, text)
for m in LINK_RE.finditer(index_text):
    href, text = m.group(1), m.group(2).strip()
    if "#" in href:
        target, anchor = href.split("#", 1)
        idx.append((target, anchor, text))
    else:
        idx.append((href, "", text))
print("\n== index ==")
print("index links:", len(idx))
print("targets:", Counter(t.split("/")[0] for t, a, _ in idx).most_common(50))

# coverage: index text vs entry keys per target file
unmatched = []
matched = 0
for target, anchor, text in idx:
    if target in entry_counts and entry_counts[target] > 0:
        if text in all_keys:
            matched += 1
        else:
            unmatched.append((target, anchor, text))
print("index entries matching an entry key:", matched, "unmatched:", len(unmatched))
for u in unmatched[:25]:
    print("  UNMATCHED", u)

# links across all md files
print("\n== links in all md ==")
link_targets = Counter()
dangling = []
total_links = 0
for f in md_names:
    text = z.read(f).decode("utf-8")
    for m in LINK_RE.finditer(text):
        href, text2 = m.group(1), m.group(2).strip()
        total_links += 1
        if "#" not in href:
            link_targets[(href, "")] += 1
            continue
        target, anchor = href.split("#", 1)
        link_targets[(target, anchor)] += 1
        if (target, anchor) not in {(t, a) for t, a, _ in idx}:
            dangling.append((f, href, text2))
print("total links:", total_links)
print("distinct link targets:", len(link_targets))
print("links to appendix files:",
      sum(c for (t, a), c in link_targets.items() if "appendix" in t))
print("links not resolving via index:", len(dangling))
for d in dangling[:20]:
    print("  DANGLING", d)

# href-only index (anchors) duplicates
anchors = [a for _, a, _ in idx if a]
print("\nduplicate anchors in index:", len(anchors) - len(set(anchors)))

# images
imgs = Counter()
for f in md_names:
    text = z.read(f).decode("utf-8")
    for m in IMG_RE.finditer(text):
        imgs[(f, m.group(1))] += 1
print("\n== images referenced ==")
print("total refs:", sum(imgs.values()), "distinct:", len(imgs))
for k, c in imgs.items():
    print(" ", c, k)

# appendix structure
print("\n== appendix-1 structure ==")
mer = z.read([n for n in md_names if "Meridian" in n][0]).decode("utf-8")
heads = re.findall(r"^\d+\.\d+ .*$", mer, re.M)
print("section headings:", len(heads))
print("first:", heads[:3], "... last:", heads[-2:])
points = re.findall(r"^([A-Z]{2,4}-[A-Z0-9]+[a-z]?) \(", mer, re.M)
print("point entries:", len(points), "channels:", len(set(p.split("-")[0] for p in points)))
print("point id samples:", points[:8])

print("\n== appendix-2/3 structure ==")
for fn in [n for n in md_names if "Medicinal" in n or "Formulas" in n]:
    t = z.read(fn).decode("utf-8")
    codes = re.findall(r"^\\?\[(\d\d\.\d\d\.\d\d(?:\.\w+)?)\] ", t, re.M)
    print(fn, "numbered entries:", len(codes), "sample:", codes[:4])

print("\n== appendix-4 structure ==")
w = z.read([n for n in md_names if "Weights" in n][0]).decode("utf-8")
print("lines:", len(w.splitlines()), "headings:", re.findall(r"^\d+\. .*$", w, re.M))

# main-file link distribution to appendix? check terms mentioning channels
print("\n== encoding / line ending check ==")
sample = z.read(main_files[0])
print("CRLF:", sample.count(b"\r\n"), "LF:", sample.count(b"\n"), "BOM:", sample[:3] == b"\xef\xbb\xbf")
