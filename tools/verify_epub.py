"""Read-only cross-check of the Markdown conversion against the source EPUB.

Usage:
    python -I verify_epub.py <extracted-markdown-dir> <source.epub>

The EPUB is the source of the Markdown conversion. This script does not write
anything. It reports:

1. the element that carries each `a1XXX` id in the EPUB;
2. Markdown index anchors that are absent from the EPUB and vice versa;
3. the entry links the server cannot resolve through `38-index.md`, with the
   EPUB headword at each target anchor;
4. the Markdown entries that have no index anchor, with EPUB candidates;
5. headword agreement between Markdown entries, index terms, and EPUB entries.

The dictionary is copyrighted; do not commit its data or this report.
"""

import collections
import html
import os
import re
import sys
import unicodedata
import zipfile


ANCHOR = re.compile(r'id="(a1[0-9A-Za-z]{3})"')
DIV_OPEN = re.compile(r'<div\b[^>]*>', re.I)
P_OPEN = re.compile(r"<p\b[^>]*>", re.I)
LINK = re.compile(r'<a\s+[^>]*href="([^"]*)"[^>]*>(.*?)</a>', re.S)
TAG = re.compile(r"<[^>]+>")
MAIN = re.compile(r"^\d\d-[a-z]\.md$")


def norm(text):
    text = unicodedata.normalize("NFKD", text)
    text = "".join(c for c in text if not unicodedata.combining(c))
    text = text.replace(" ", " ").replace(" ", " ")
    text = text.replace("\\", "")
    text = re.sub(r"\s+", " ", text).strip()
    return text.lower()


def plain(fragment):
    return re.sub(r"\s+", " ", html.unescape(TAG.sub("", fragment))).strip()


def load_markdown(root):
    files = {}
    for name in sorted(os.listdir(root)):
        if MAIN.match(name):
            with open(os.path.join(root, name), encoding="utf-8") as handle:
                files[name] = handle.read()
    return files


def parse_entries(files):
    entries = []
    for name, content in files.items():
        lines = content.splitlines()
        starts = [i for i, line in enumerate(lines) if line.startswith("\\*")]
        for n, start in enumerate(starts):
            end = starts[n + 1] if n + 1 < len(starts) else len(lines)
            headword = lines[start][2:].strip()
            body = "\n".join(lines[start + 1:end])
            links = []
            for href, text in LINK.findall(body):
                if "#" in href:
                    target_file, anchor = href.split("#", 1)
                    if target_file in files:
                        links.append((target_file, anchor, plain(text)))
            entries.append({
                "file": name,
                "line": start + 1,
                "headword": headword,
                "body": body,
                "links": links,
            })
    return entries


def parse_index(files, root):
    anchors = {}
    term_anchors = {}
    content = open(os.path.join(root, "38-index.md"), encoding="utf-8").read()
    for line in content.splitlines():
        matches = list(LINK.finditer(line))
        eligible = []
        for match in matches:
            href = match.group(1)
            if "#" not in href:
                continue
            target, anchor = href.split("#", 1)
            if target in files:
                eligible.append((match, target, anchor, plain(match.group(2))))
        if not eligible:
            continue
        prefix = re.sub(r"\s+", " ", line[:eligible[0][0].start()]).strip()
        all_numbered = all(text.isdigit() for _, _, _, text in eligible)
        for _, target, anchor, text in eligible:
            if all_numbered and prefix:
                term = prefix
            elif text:
                term = text
            elif prefix:
                term = prefix
            else:
                continue
            anchors.setdefault(anchor, (target, term))
            term_anchors.setdefault((target, norm(term)), anchor)
    return anchors, term_anchors


def epub_entries(path):
    entries = []
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            if not name.endswith(".xhtml"):
                continue
            content = archive.read(name).decode("utf-8")
            for match in DIV_OPEN.finditer(content):
                id_match = ANCHOR.search(match.group(0))
                if not id_match:
                    continue
                anchor = id_match.group(1)
                depth = 1
                pos = match.end()
                while depth and pos < len(content):
                    next_open = re.search(r"<div\b", content[pos:], re.I)
                    next_close = re.search(r"</div>", content[pos:], re.I)
                    if next_close is None:
                        break
                    if next_open is not None and next_open.start() < next_close.start():
                        depth += 1
                        pos += next_open.end()
                    else:
                        depth -= 1
                        pos += next_close.end()
                inner = content[match.end():pos - 6] if depth == 0 else content[match.end():]
                first_p = P_OPEN.search(inner)
                headword = ""
                if first_p:
                    close = inner.find("</p>", first_p.end())
                    if close != -1:
                        headword = plain(inner[first_p.end():close]).lstrip("*").strip()
                entries.append({
                    "anchor": anchor,
                    "file": name,
                    "headword": headword,
                    "text": plain(inner),
                })
    return entries


def main():
    root, epub_path = sys.argv[1], sys.argv[2]
    files = load_markdown(root)
    entries = parse_entries(files)
    anchors, term_anchors = parse_index(files, root)
    epub = epub_entries(epub_path)

    by_anchor = {entry["anchor"]: entry for entry in epub}
    print(f"Markdown entries: {len(entries)}")
    print(f"Markdown index anchors: {len(anchors)}")
    print(f"EPUB a1 entry-div ids: {len(epub)} (distinct {len(by_anchor)})")

    # Every a1 id anywhere in the EPUB, with its carrying element.
    element_counts = collections.Counter()
    all_ids = set()
    with zipfile.ZipFile(epub_path) as archive:
        for name in archive.namelist():
            if name.endswith(".xhtml"):
                content = archive.read(name).decode("utf-8")
                for match in ANCHOR.finditer(content):
                    all_ids.add(match.group(1))
                    tag = re.search(r"<([a-zA-Z0-9]+)\b[^>]*$", content[:match.start()])
                    element_counts[tag.group(1) if tag else "?"] += 1
    print(f"EPUB a1 ids anywhere: {len(all_ids)}")
    print("EPUB id-carrying elements:", element_counts.most_common())
    print(f"EPUB ids missing from index: {len(all_ids - set(anchors))}")
    print(f"Index anchors missing from EPUB: {len(set(anchors) - all_ids)}")
    for anchor in sorted(set(anchors) - all_ids):
        print(f"  index-only anchor: {anchor} -> {anchors[anchor]}")
    for anchor in sorted(all_ids - set(anchors))[:30]:
        source = by_anchor.get(anchor)
        print(f"  epub-only anchor: {anchor} {source['file'] if source else '?'} "
              f"headword={source['headword'] if source else '?'}")

    print("\n== index anchor → EPUB headword disagreements ==")
    bad = 0
    for anchor, (target, term) in sorted(anchors.items()):
        source = by_anchor.get(anchor)
        if source and norm(term) != norm(source["headword"]):
            bad += 1
            if bad <= 20:
                print(f"  {anchor} {target}: index={term!r} epub={source['headword']!r} ({source['file']})")
    print(f"  total disagreements: {bad}")

    unresolved = []
    for entry in entries:
        for target_file, anchor, text in entry["links"]:
            if anchor not in anchors:
                unresolved.append((entry, target_file, anchor, text))
    print(f"\n== unresolved Markdown links: {len(unresolved)} ==\n")
    for entry, target_file, anchor, text in unresolved:
        source = by_anchor.get(anchor)
        print(f"from {entry['file']}:{entry['line']} {entry['headword']!r} -> {target_file}#{anchor} link={text!r}")
        if source:
            print(f"  EPUB {source['file']} {source['anchor']} headword={source['headword']!r}")
            print(f"  text: {source['text'][:240]}")
        else:
            print("  EPUB: anchor not found")
        # Is the EPUB headword a Markdown entry, and does it have an index anchor?
        matches = [candidate for candidate in entries if norm(candidate["headword"]) == norm(source["headword"])] if source else []
        for candidate in matches[:3]:
            candidate_anchor = term_anchors.get((candidate["file"], norm(candidate["headword"])))
            print(f"  Markdown entry {candidate['file']}:{candidate['line']} anchor={candidate_anchor}")
        if not matches:
            print("  Markdown: no entry with that headword")

    print("\n== entries without an index anchor ==")
    anchorless = [entry for entry in entries
                  if (entry["file"], norm(entry["headword"])) not in term_anchors]
    print(f"count: {len(anchorless)}")
    epub_by_headword = collections.defaultdict(list)
    for source in epub:
        epub_by_headword[norm(source["headword"])].append(source)
    for entry in anchorless:
        print(f"  {entry['file']}:{entry['line']} {entry['headword']!r}")
        for source in epub_by_headword.get(norm(entry["headword"]), [])[:4]:
            print(f"    EPUB {source['file']} {source['anchor']} text={source['text'][:160]!r}")

    print("\n== Markdown entry headword vs index term ==")
    mismatched = 0
    for entry in entries:
        anchor = term_anchors.get((entry["file"], norm(entry["headword"])))
        if anchor and norm(anchors[anchor][1]) != norm(entry["headword"]):
            mismatched += 1
            if mismatched <= 20:
                print(f"  {entry['file']}:{entry['line']} entry={entry['headword']!r} anchor={anchor} index={anchors[anchor][1]!r}")
    print(f"  total mismatches: {mismatched}")


if __name__ == "__main__":
    main()
