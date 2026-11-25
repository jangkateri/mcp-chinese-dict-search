"""Second-pass structural analysis of the appendix files.

Usage: python -I inspect_appendix.py <path-to-zip>
Read-only. Prints entry counts and format samples for planning.
"""
import re
import sys
import zipfile

z = zipfile.ZipFile(sys.argv[1])
get = lambda name: z.read([n for n in z.namelist() if name in n][0]).decode("utf-8")

mer = get("Meridian")
lines = mer.splitlines()
starts = []
for i, line in enumerate(lines):
    # Candidate point entry: starts with an uppercase ASCII id token then " (".
    m = re.match(r"^([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)+)\s+\(", line)
    if m:
        starts.append((i, m.group(1), line[:70]))
print("appendix-1 candidate point entries:", len(starts))
print("first 5:")
for s in starts[:5]:
    print("  ", s)
print("last 5:")
for s in starts[-5:]:
    print("  ", s)
prefixes = sorted({re.match(r"^([A-Za-z]+)-", s[1]).group(1) for s in starts})
print("id prefixes:", prefixes)
# any section headings not caught
heads = [(i, l) for i, l in enumerate(lines) if re.match(r"^\d+\.\d+ ", l)]
print("section headings:", len(heads), heads[:2], heads[-2:])

for label, needle in [("medicinal", "Medicinal"), ("formulas", "Formulas")]:
    text = get(needle)
    es = re.findall(r"^\\\[(\d\d\.\d\d\.\d\d(?:\.\w+)?)\\\]\s+(.*)$", text, re.M)
    print(f"\nappendix ({label}) numbered entries:", len(es))
    for e in es[:3]:
        print("  ", e[0], "|", e[1][:100])
    print("  last:", es[-1][0], "|", es[-1][1][:100])
    secs = re.findall(r"^\d+\.\d+ .*$", text, re.M)
    print("  section headings:", len(secs), secs[:2], secs[-2:])
    plain = [l for l in text.splitlines() if l and not l.startswith(("\\[", "•", "□")) and len(l) < 70]
    print("  short plain-heading samples:", plain[:8])

w = get("Weights")
wl = w.splitlines()
print("\nappendix-4 lines:", len(wl))
for l in wl:
    if l.startswith("\\") or l.startswith("#"):
        print("  heading:", l)
