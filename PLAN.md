# PROJECT PLAN — `mcp-chinese-dict-search`

| Field | Value |
|---|---|
| Status | Working tree 0.6.0: graph traversal, graph PNGs, appendix networks, and optional gated semantic retrieval; dictionary-agnostic title, required `--data`, generic text; v0.5.0 is the last release |
| Version | 0.6.0 |
| Last updated | 2026-11-27 |
| Source specification | `PROJECT-SETUP-PROMPT.md` |
| Tracking document | [`CHECKLIST.md`](CHECKLIST.md) |
| Working rules | [`CLAUDE.md`](CLAUDE.md) |
| Reference implementation | a separate Rust MCP project |

---

## 1. Objective

Build a standalone **MCP stdio server in Rust** that searches a user-supplied Markdown dictionary
and its four appendices, and returns complete, citable dictionary entries to an MCP client such as
Claude Code, OpenCode, or DeepSeek Harness. The server is dictionary-agnostic: the data path and
display title are configuration, not compiled constants.

The server must let a model:

1. look up an English Chinese-medicine term and receive its definition, Chinese characters,
   pinyin, and cross-references;
2. retrieve an exact entry on request, without paging through 7 MB of Markdown;
3. receive appendix material when the query belongs to one of the four appendix domains:
   acupuncture meridians/points, medicinal agents, herbal formulas, and weights/measures;
4. follow the dictionary's resolved cross-references from one entry, in either direction and up to
   three hops;
5. render that neighborhood as a PNG with an absolute file path;
6. traverse formula families, formula-medicinal composition, and named point combinations.

## 2. Scope

### In scope (v1)

- MCP stdio transport: newline-delimited JSON-RPC 2.0, `initialize`, `ping`, `tools/list`,
  `tools/call`, `notifications/initialized`, `notifications/cancelled`.
- Load the dictionary directly from the supplied `.zip` **or** from an extracted directory.
- Parse all 26 letter files (`08-a.md` … `33-z.md`), the 4 appendices, and the index used to
  resolve anchors.
- Search by headword, alias, pinyin, Chinese characters, point identifier, and body text.
- Generate pinyin- and Han-keyed lookup indices at load time for tone-numbered, tone-marked,
  toneless, simplified, and traditional forms (Phase 7A; see §15).
- Resolve the book's internal `<a href="…">` links into retrievable term references.
- Build outgoing and incoming graph adjacency from those resolved links.
- Six model-facing tools: `search_dictionary`, `get_entry`, `related_entries`, `render_graph`,
  `related_network`, and `render_network`.
- Deterministic, tested parsers and a hermetic integration suite.
- Documentation, build instructions, and a Windows deployment bundle.

### Out of scope (v1)

- RAG, generated answers, or a language model at query time. The optional `semantic` feature adds
  retrieval, not generation, over a local embedding model; it is off in the default build and
  tracked in [`ENHANCEMENT-PLAN.md`](ENHANCEMENT-PLAN.md).
- Serving images over HTTP. The archive's 10 image references are all in front matter or the
  index, not inside dictionary entries; the server keeps references as text.
- Network access in the default build. The optional semantic feature may call only a loopback
  Ollama server; remote hosts and Ollama Cloud are rejected.
- Editing or authoring dictionary content.
- Additional books.

## 3. Verified source-data facts

These facts were measured from the archive on 2026-11-24 with the read-only scripts in `tools/`
(see §17). They drive the parser and test design; they are not assumptions.

### 3.1 Files

| Item | Value |
|---|---|
| Archive | user-supplied `.zip`, never committed |
| Markdown files | 39 (38 numbered chapters plus `TOC.md`) |
| Markdown payload | 7,339,349 bytes (7.0 MiB) |
| Encoding | UTF-8, no BOM |
| Line endings | CRLF |
| Main dictionary files | `08-a.md` … `33-z.md` (26 files) |
| Appendix files | `34-appendix-1-Meridian.md`, `35-appendix-2-Medicinal.md`, `36-appendix-3-Formulas.md`, `37-appendix-4-Weights-Measurements.md` |
| Index | `38-index.md` |
| Referenced images | 10, all in `01-`, `02-`, `06-`, and `38-` files |

The table of contents links to appendix files under their **old** names (`34-appendix-1.md` …).
Actual archive names carry the topic suffix. Filename classification therefore keys on the
numeric prefix (`34`–`37`) and tolerates either spelling.

### 3.2 Main dictionary entries

| Metric | Value |
|---|---|
| Entries (lines beginning `\*` in column 0) | 5,803 |
| Distinct raw headwords | 5,781 |
| Duplicate raw headwords | 22 |
| Duplicate normalized headwords | 24 |
| Entries with more than one alias line | 395 |
| Entries with no alias line | 2 (`crude medicinal`, `tablet`) |
| Empty entries | 1 (`crude medicinal`) |
| Alias lines with a non-standard shape | 2 (one missing ` / `, one without a vertical bar) |
| Internal `<a href>` links | 14,155 |
| Index links | 5,822 |
| Distinct anchors in the index | 5,795 |
| All internal links resolvable through the index | 14,142 of 14,155 (99.91 %) |
| Entry-body cross-references resolved at runtime | 8,283 of 8,283 (100 %) |
| Unresolved entry-body links | 0; 13 recovered by EPUB-verified corrections |

Entry grammar (verified):

```text
\*<headword>
<blank>
<simplified> | <traditional>  \*<tone-number pinyin> / <diacritic pinyin>
<blank>
<body markdown, until the next line that starts with \* in column 0>
```

- `\*` also appears **inside** alias lines (`... \*fu4 / fù`). A line is an entry boundary only
  when `\*` is at byte offset 0.
- 395 entries have several consecutive alias lines (synonyms); they must all be captured.
- The separator before `\*` in an alias line is a non-breaking space (`U+00A0`) in parts of the
  corpus; normalization must handle it.
- Entry bodies are either prose (`Also …`), a redirect (`See …`), or pattern sections (`PATTERNS`).
- Empty bodies exist and must not crash the parser.

### 3.3 Anchors and links

- The Markdown files contain **no anchor definitions**: `a1S5W` and similar identifiers occur
  only inside `href` values.
- The only anchor→entry map is `38-index.md`. It contains either:
  - `<a href="08-a.md#a1S5W" …>abdominal fullness</a>`, or
  - `cold congealing in the liver vessel <a href="10-c.md#a1SN7" …>1</a> <a … >2</a>`,
    where the plain text before the numbered links is the term and each numbered anchor points
    at one sense of that entry.
- After unescaping Markdown (`\[`→`[`, `\]`→`]`, `\|`→`|`, `\.`→`.`) and stripping diacritics and
  case, **100 % of index terms match an entry headword**. All 101 raw mismatches are explained by
  escaping and numbered sub-anchors.
- 13 links target anchors the index omits (0.09 %). The EPUB confirms 5 of them belong to
  entries whose `div` ids the index skipped, and 8 target `a1SSD`, a paragraph inside the
  `wind stroke` entry. `apply_verified_corrections` resolves all 13.
- Only 4 links anywhere point at appendix files, and all are table-of-contents navigation.
  **Appendix routing therefore cannot rely on hyperlinks; it must be semantic.**

### 3.3.1 What the `#a1XXXX` identifiers are

The fragment IDs are **opaque element identifiers generated by the EPUB/conversion pipeline**, not
semantic codes, page numbers, entry sequence numbers, or hashes of the headword. The exact EPUB
(transferred 2026-11-24; ISBN/ASIN `B0GTMZRK1L`, 62 files) confirms the inference:

- Every fragment is exactly five characters: `a1` followed by three base-36 digits. The EPUB
  contains 5,869 distinct `a1…` ids: 5,854 on `<div>` elements and 15 on `<p>` elements. All 5,822
  index links/anchors exist in the EPUB; none is dangling.
- Entry containers are 5,803, exactly matching the Markdown: 5,797 `class_sGC` divs carrying an
  `a1…` id, four `class_sGC` divs without an id, one `class_sRD9` div, and one headword on a
  `<p id="a1VBZ">` (`abating heat`). The remaining 47 EPUB-only ids are 4 entry ids the index
  omitted, 26 table-of-contents letter divs, 14 section `<p>` ids, and 3 introduction section
  divs.
- Decoding the suffix as base 36 gives a dense allocation: among the sorted values, 5,075 adjacent
  pairs differ by 1, 754 differ by 2, and only a handful differ by more. That is the signature of a
  serial ID allocator, not of random or derived IDs.
- The `s1…` strings are **CSS class names**, not element ids. They use the same allocator —
  `class_s1NTM-1` carries `s1NTM` = 30,874, immediately before `a1NTN` = 30,875 — but they never
  resolve a link. The `-0`/`-1` suffix distinguishes link contexts; the class code names a style
  group, never the target entry.
- Allocation order is unrelated to reading order. Spearman correlation between EPUB spine position
  and anchor value is 0.010; within a file, only 49.9 % of adjacent entry ids increase. Alphabetical
  entry rank, pinyin order, and Chinese-character order are likewise uncorrelated. The sequence
  records an internal object-creation order.
- Common hashes of the headword do not reproduce the suffix: CRC-32, Adler-32, MD5, and SHA-1 each
  matched 0 of 200 sampled entries.

Practical rule: treat `#a1XXXX` as a **foreign key into the source EPUB**. Do not parse it, sort
by it, or infer meaning from it. Build `anchor → entry` from `38-index.md` at load time, then apply
the small verified correction table in `src/dictionary/index.rs`. It remains a stable, unambiguous
citation key for the entries that have one.

**Anchor corrections.** The EPUB has 47 ids that the index does not list. Four are entry
containers the index omitted (`a1UN9` provisioning, `a1PDE` provisioning qi4, `a1PB6`
provisioning-aspect pattern, `a1R3X` provisioning-defense disharmony); one is a paragraph id
(`a1SSD`) inside the `wind stroke` entry, used by every "wind-like stroke" reference. The server
adds the four entry anchors and maps `a1SSD` to `wind stroke`, resolving all 13 links. Four entries
remain anchorless because the EPUB gives them **no id at all** (`dancing of the arms and legs`,
`food diarrhea2`, `provisioning aspect`, `provisioning, defense, qi4, and blood`); they cite
`file:line` and are listed in the load report.

**Duplicate anchors.** The index gives numbered sub-anchors for duplicate headwords, in entry
order. The resolver assigns them positionally. Before this correction, both senses of `qi4` (and
`wei`, and 22 other duplicate groups) shared the first anchor; the EPUB cross-check exposed the
bug. `tests/real_archive.rs` now asserts distinct anchors for both `qi4` senses.

**Other EPUB cross-checks (2026-11-24).** Main-entry text matches after normalization for every
anchored entry except three Markdown-only `PATTERNS` labels and two duplicate-headword alignment
artifacts. Appendix counts and identifier sets match exactly: 403 points (set equality), 572
medicinals, 261 formulas, 3 weights sections. Both `No Match` artifacts occur in both sources. All
24 duplicate normalized headwords match. Alias lines match apart from two spacing/OCR extraction
variants. Nine inline images match; the cover is `image_rsrc1VMG.jpg` in the EPUB and
`14f4…jpg` in the Markdown conversion.

### 3.4 Appendices

| Appendix | File | Entries | Sections | Entry key |
|---|---|---|---|---|
| 1 Meridian / acupuncture | `34-appendix-1-Meridian.md` | 403 points | 15 | `LU-1`, `ST-36`, `M-UE-46`, `N-HN-20`, … |
| 2 Medicinal agents | `35-appendix-2-Medicinal.md` | 572 | 20 | `[01.01.01]`, variants `.A`–`.E` |
| 3 Herbal formulas | `36-appendix-3-Formulas.md` | 261 | 21 | `[01.01.01]`, variants `.A`–`.E`, `04a.`/`05a.` |
| 4 Weights & measures | `37-appendix-4-Weights-Measurements.md` | 3 sections | 3 | `21.`, `22.`, `23.` (tables) |

These counts supersede the initial probe's 571/250. The probe regex rejected one medicinal ID with
a trailing space (`08.01.14.A `) and eleven formula IDs with a letter-suffixed first component
(`04a.*`, `05a.*`). One medicinal ID (`17.02.20`) occurs twice in the source; both entries are
retained. The parser accepts all of these, and the golden tests assert the measured counts.

Verified header shapes:

```text
# Appendix 1
1.3 ST, Foot Yáng Brightness (yáng míng) Stomach Channel
ST-36 (足三里 / 足三里 zú sān lǐ / zu san li, Leg Three Li): Alt. Names: …
• Location: …
• Actions: …
□ Needling: …

# Appendix 2
2.1 Exterior-Resolving Agents
\[01.01.01\] MÁ HUÁNG / ma huang 麻黄 / 麻黃 (Ephedrae Herba, ephedra): Alt. Eng.: …
• Properties: …

# Appendix 3 (single Chinese form, unlike appendix 2)
3.1 Exterior-Resolving Formulas
\[01.01.01\] MÁ HUÁNG TĀNG / ma huang tang 麻黄汤 (Ephedra Decoction): Warm acrid …
□ Má huáng / ma huang 麻黄 (Ephedrae Herba, ephedra) 9g

# Appendix 4
21\. Weights in Different Dynasties
| Dynasty | Date | Grams/liang |
| ... |
```

Header anomalies that the parser must tolerate:

- `⁑` marks a name denoting more than one item (`竹叶⁑`, `zi bei tian kui⁑`).
- Nested parentheses in Latin names: `(Phyllanthi Urinariae Herba (cum Radice), phyllanthus)`.
- Escaped brackets in Latin/English names: `homalomena \[root\]`.
- Two appendix-1 lines end with a stray `No Match` after the closing parenthesis.
- Formula entries use one Chinese form; medicinal entries use simplified / traditional.
- Entry IDs may carry a variant suffix (`.A` … `.E`); variants are separate retrievable entries.

### 3.5 Semantic references in main entries

Point identifiers are frequent in ordinary definitions — for example, `ST-36` occurs 70 times in
`10-c.md` alone. This makes point-ID extraction from an entry body a reliable way to attach the
corresponding appendix-1 monographs.

## 4. Architecture

The crate mirrors the reference implementation: a library owns all behaviour, and the binary is a
thin entry point. This keeps the protocol surface testable in-process.

```text
stdin
  │  newline-delimited JSON-RPC
  ▼
transport::serve ──► protocol::classify_message ──► server::McpServer::handle_request
                                                          │
                                                    tools::call_tool
                                                          │
                                              dictionary::Dictionary (read-only, in memory)
                                                ├── main entries
                                                ├── index anchors
                                                ├── appendix entries
                                                └── search + link resolution
                                                          │
stdout ◄── transport::write_message ◄── result_response / error_response
```

Key design choices:

- **Read-only in-memory index.** The archive is loaded and parsed once at startup. No database,
  no file watcher, no background job. Search is a linear scan over pre-normalized text.
- **No network.** Local files only.
- **Zip or directory source.** Both paths feed the same parser, which makes synthetic fixtures
  possible without constructing a zip in tests.
- **Anchors come from the index.** The parser builds `anchor → (file, term)` from `38-index.md`;
  it never expects anchor definitions in the letter files.
- **Appendix routing is semantic.** Queries and entry headwords are matched against appendix
  aliases; point identifiers found in a body trigger appendix-1 summaries.
- **Bounded output.** Snippets are windowed; full entries are capped by `max_chars`.

## 5. Data model

```rust
pub struct Dictionary {
    entries: Vec<Entry>,                     // main dictionary, book order
    by_headword: HashMap<String, Vec<usize>>, // normalized headword -> indices
    anchors: HashMap<String, AnchorTarget>,  // anchor id -> (file, entry index)
    meridians: Vec<AppendixEntry>,
    medicinals: Vec<AppendixEntry>,
    formulas: Vec<AppendixEntry>,
    weights: Vec<Section>,
    point_ids: HashMap<String, usize>,       // "ST-36" -> meridians index
    appendix_aliases: HashMap<String, Vec<AppendixHit>>,
    report: LoadReport,
}

pub struct Entry {
    pub headword: String,        // raw text after \*
    pub norm_headword: String,
    pub file: String,            // "08-a.md"
    pub line: usize,             // 1-based line in the source file
    pub anchor: Option<String>,  // resolved through the index
    pub aliases: Vec<Alias>,
    pub body: String,            // raw Markdown, CRLF removed
    pub norm_body: String,       // lowercased, diacritic-stripped search text
    pub links: Vec<Link>,
}

pub struct Alias {
    pub simplified: String,
    pub traditional: String,
    pub pinyin_toned: String,    // "fu4 man3"
    pub pinyin_ascii: String,    // "fu man" (diacritics stripped)
}

pub struct AppendixEntry {
    pub source: AppendixKind,    // Meridian | Medicinal | Formulas
    pub id: Option<String>,      // "ST-36" or "01.01.01"
    pub section: String,         // "1.3 ST, Foot Yáng Brightness …"
    pub subsection: Option<String>,
    pub term: String,            // English name
    pub aliases: Vec<String>,    // pinyin, Chinese forms, Latin name, alt names
    pub body: String,
    pub norm_terms: Vec<String>,
    pub norm_body: String,
}

pub struct Section {
    pub number: String,          // "21" … "23" for appendix 4
    pub title: String,
    pub body: String,            // Markdown tables
}
```

Memory estimate: ≈15–25 MB for ~7 MB of Markdown after storing normalized copies. Acceptable for a
stdio server.

## 6. Parsing specification

### 6.1 Source loading

1. Resolve the data path (§9).
2. If it is a directory, read every `*.md` file, sorted by filename.
3. If it is a zip, open it with the `zip` crate and read every `*.md` member, sorted by name.
4. Classify each file:
   - `^\d{2}-[a-z]\.md$` → main letter file;
   - numeric prefix `34` → appendix 1; `35` → 2; `36` → 3; `37` → 4;
   - `38-…` → index;
   - everything else (`01`–`07`, `TOC.md`) → ignored for search, retained only as optional
     front-matter text.
5. Strip CR at end of lines. Reject invalid UTF-8 with a clear error naming the file.
6. Fail startup if no main entries are found. Warn if fewer than 5,000 are found.

### 6.2 Main-entry parsing

- Split the file into lines; an entry starts at every line whose first byte is `\`.
- The headword is the remainder of that line after `\*`, trimmed.
- Skip blank lines after the headword.
- Consume consecutive alias lines while the line matches the alias grammar
  (`… | …  \*… / …`, non-breaking spaces allowed). Continue across blank lines between alias
  lines, but stop at the first non-alias paragraph.
- The body is everything after the alias block up to the next boundary, trimmed of leading and
  trailing blank lines.
- If no alias line is present, leave aliases empty and keep the body intact.
- If the body is empty, store it and let retrieval report that the source entry has no text.

### 6.3 Alias parsing

Primary grammar (6,352 of 6,352 alias-like lines parse):

```text
^(<simplified>) \| (<traditional>)\s*\u{a0}\*(<pinyin toned>)\s*/\s*(<pinyin ascii>)$
```

- Preserve both pinyin spellings: the tone-numbered form (`fu4 man3`) and the diacritic form
  (`fù mǎn`).
- Strip a trailing `\[variant / variant\]` block into an extra alias rather than discarding it.
- Fallback for the one non-standard line `二阴 | 二陰 \u{a0}\*er4 yin1 /èr yīn`.
- Fallback for `片 pian4` (`tablet`): treat the Han characters as simplified/traditional and the
  Latin token as tone-numbered pinyin.

### 6.4 Index and link resolution

1. Parse `38-index.md` line by line.
2. For every line containing `<a href="target#anchor">text</a>`:
   - if `target` is a main letter file, record `anchor → (target, term)`;
   - `term` is the plain text before the links when the links are numbered (`1`, `2`, …);
     otherwise it is the link text itself;
   - ignore navigation links to `38-index.md` and front-matter files.
3. Normalize the term (headword normalization, §6.7) before lookup.
4. Build `term → [anchors]` per target file. Numbered sub-anchors occur in entry order, so assign
   the nth anchor to the nth entry with that headword. Then apply the EPUB-verified corrections:
   four omitted entry anchors and the `a1SSD` sub-entry target.
5. Parse entry bodies for `<a href="file#anchor" …>text</a>`:
   - resolved → `Link { text, target_term, target_file, target_anchor }`;
   - unresolved after corrections → `Link { text, resolved: false }`; keep the text, log once at
     debug level. The current corpus has zero such links.
6. Rendering replaces every `<a …>text</a>` with `text`; links are additionally listed in a
   `cross_references` section so a model can call `get_entry` for the target.

### 6.5 Appendix 1 (meridians and points)

- Section start: line matching `^\d+\.\d+\s+(.+)$` (15 sections).
- Point start: line matching `^([A-Z][A-Za-z0-9]*(?:-[A-Za-z0-9]+)+)\s+\((.*)\)`
  (403 entries; tolerate trailing prose such as `No Match`).
- Parse the header with a balanced-parenthesis scan, then split the inner text:
  - `simplified / traditional` before the tone-numbered pinyin;
  - `tone pinyin / roman pinyin, English name`;
  - optional `: Alt. Names: …` tail.
- Body: `• Field: value` lines and `□` sub-items until the next entry or section. Keep Markdown
  bullets.
- Aliases: identifier, English name, both pinyin forms, simplified and traditional characters,
  and every `Alt. Names` entry.

### 6.6 Appendices 2, 3, and 4

- **Appendix 2 (medicinals).** Section start `^\d+\.\d+\s+(.+)$` (20). Entry start
  `^\\\[(\d\d\.\d\d\.\d\d(?:\.\w+)?)\\\]\s+(.+)$` (572). Header:
  `UPPER PINYIN / lower pinyin <simplified> / <traditional> (<Latin>, <English>): <rest>`.
  Use balanced-parenthesis scanning; strip `⁑`; unescape `\[…\]`.
- **Appendix 3 (formulas).** Same section grammar (21 sections); entry start same (261). Header:
  `UPPER PINYIN / lower pinyin <Chinese> (<English>): <rest>` (single Chinese form).
  Formula composition lines (`□ pinyin / pinyin 中文 (Latin, English) dose`) stay in the body.
- **Appendix 4 (weights and measures).** Section start `^(\d+)\\\.\s+(.+)$` (3 sections: weights by
  dynasty, measures by dynasty, conversion table). Body is Markdown tables, retained verbatim.
- Subsection headings are short plain-text lines between entries (for example
  `Warm Acrid Exterior-Resolving Agents`). Assign the most recent one to each entry for display.
- Parser coverage is a test gate: 100 % of appendix headers must parse on the real archive.

### 6.7 Normalization (used for matching, never for display)

`normalize(text)`:

1. Unicode NFKD decomposition.
2. Remove combining marks (`unicode-normalization` crate) so `MÁ HUÁNG` matches `ma huang`.
3. Replace non-breaking spaces with normal spaces.
4. Unescape Markdown: `\[`→`[`, `\]`→`]`, `\|`→`|`, `\*`→`*`, `\.`→`.`, `\_`→`_`.
5. Lowercase.
6. Collapse whitespace runs.

Chinese characters are preserved unchanged. Punctuation is retained but treated as a space when
tokenizing.

### 6.8 Anomaly policy

| Anomaly | Policy |
|---|---|
| Duplicate headwords (22 raw / 24 normalized) | Keep every entry. Index sub-anchors are assigned positionally, so each sense keeps its own citation. Search deduplicates identical bodies and labels differing bodies `(1)`, `(2)`. |
| Empty entry | Return the headword with a note that the source has no body text. |
| Missing alias line | Leave aliases empty; headword still searchable. |
| Unresolved anchor (0 after EPUB corrections) | Keep link text; omit target; debug log. |
| `No Match` suffix in appendix 1 | Ignore the suffix. |
| `⁑`, nested parentheses, escaped brackets | Tolerate in header parsing; strip marker from aliases. |
| Old appendix filenames in TOC | Classify by numeric prefix, not exact name. |

## 7. Search and retrieval behaviour

### 7.1 Query preprocessing

- Trim; apply `normalize`.
- Tokenize on whitespace and punctuation for token-coverage scoring.
- A query consisting of one Han-character run is matched as a substring.
- A query matching `^[A-Za-z]{1,2}(-[A-Za-z]{1,3})?-\d+[a-z]?(?:-[a-z])?$` is treated as a point
  identifier first (the final optional segment covers non-channel IDs such as `N-UE-19a-c`).

### 7.2 Ranking

| Condition | Score |
|---|---|
| Headword exactly equals query | 1000 |
| Appendix identifier exactly equals query | 1000 |
| Alias exactly equals query | 900 |
| Headword starts with query | 800 |
| Alias starts with query | 750 |
| Headword contains query | 700 |
| Alias contains query | 650 |
| Every token appears in headword/aliases | 600 |
| Body contains the exact query phrase | 500 + 5 × occurrences (capped at 100) |
| Every token appears in body | 400 + 2 × occurrences (capped at 60) |
| Partial token coverage | 100 × coverage |

Ties break by book order (file order, then line). Appendix hits with an exact identifier or exact
name outrank dictionary body hits.

### 7.3 Snippets

- Headword/alias match: first two body paragraphs, whitespace-collapsed, capped at 500 characters.
- Body match: 250 characters on each side of the first occurrence, with ellipses.
- Redirect entries (`See …`): the snippet is the redirect sentence, and the hit is marked
  `redirect`.

### 7.4 Scope and appendix routing

`scope` values: `all` (default), `dictionary`, `meridian`, `medicinal`, `formulas`, `weights`.

- `dictionary`: main entries only.
- `meridian` / `medicinal` / `formulas`: that appendix only. Identifier, term, alias, and body
  matches are all allowed.
- `weights`: appendix-4 sections only; matches section title and body.
- `all`:
  1. Search main entries and appendix **terms/identifiers/aliases**.
  2. If an appendix term or identifier matches exactly or by prefix, return those appendix hits
     first.
  3. Add dictionary hits, ranked normally.
  4. Appendix bodies are searched only when `scope` names that appendix (`meridian`,
     `medicinal`, `formulas`, `weights`). In `all`, appendix hits come from term, identifier, and
     alias matches only. This prevents common words in 1,236 monographs from flooding dictionary
     results.
  5. Measurement queries (`liang`, `cun`, `fen`, `sheng`, `dou`, `tael`, `dosage`, `dose`,
     `weights`, `measures`, `conversion`, or a Han character for one of these) include the
     relevant appendix-4 section. `ge` is deliberately excluded: as an English word it is too
     ambiguous to route on.

The rule that follows the specification: a query that refers to a point, herb, formula, or
measurement is augmented with that appendix; a query that does not is not.

### 7.5 `get_entry`

- Resolve `term` in this order: exact citation anchor (`a1XXX`, case-insensitive) → exact
  normalized headword → exact appendix identifier → exact appendix alias → exact normalized alias
  → prefix → substring, choosing the first match in book order when more than one qualifies.
- If several sources match and `source` is absent, prefer the dictionary, then appendix 1, 2, 3.
- If the matched entry is a redirect (`See …`) and has a resolved target, follow **one** hop and
  return the target body with `redirected_from` set. Detect cycles.
- `related`: extract unique point identifiers from the body (up to 10) and attach a compact
  appendix-1 summary for each (id, English name, Chinese, one-line actions).
- For appendix-4 terms, return the relevant table section.
- Return full body up to `max_chars` (default 20,000; maximum 100,000), with `truncated` and
  `total_chars`.
- Rendered entries end with `To reopen this citation, ask: tcm citation <tag>`. The server
  instruction maps `tcm citation <anchor>` to `get_entry` with that anchor, so a citation tag is
  actionable without a separate tool.

### 7.6 Graph traversal

`related_entries` follows only links the parser resolved. The graph has one node per main
dictionary entry and directed edges for each resolved `Entry.links` entry whose target anchor maps
to an entry.

- `direction`: `outgoing` follows links the entry makes; `incoming` follows links into it; `both`
  follows either. Outgoing edges are considered first; duplicate neighbours collapse.
- `depth`: 1–3. Breadth-first traversal seeds the start entry as visited, so cycles terminate and
  each reachable entry appears once at its shortest distance.
- `limit`: 1–50 returned nodes; an internal 10,000-node visit cap prevents a pathological hub from
  exhausting memory.
- Every node records `distance`, `edge_direction`, `via` (the entry whose link reached it),
  `link_text`, canonical citation, absolute source path and line when available, and `redirect`.
- Nodes are ordered by `(distance, entry index)`. `total` is the unique discovered count before
  `limit`; `truncated` is true when `limit` or the visit cap cut the output.
- The start term uses the same resolution precedence as `get_entry`'s dictionary path. Appendix
  terms are not graph nodes; the tool returns a clear not-found message pointing to `get_entry`.
- Unresolved links and links whose target cannot be mapped are excluded and counted in the load
  report (`graph_unmapped_links`), never invented.

### 7.7 Appendix network

`related_network` adds three domains built at startup:

- **formula**: explicit prose such as “This is X with the addition of…”, “combined with…”, and
  “variant of…”. Edges carry `has_derivative`, `combined_with`, or `has_variant`.
- **medicinal**: formula ingredient lines (`□`) matched to appendix-2 medicinals. Edges carry
  `contains` / `used_in`. Stated alternatives in one ingredient line can create multiple edges.
- **point_combination**: named `combining … points` main entries parsed into combination records;
  role-marked point IDs (`S.`, `N.`, `T.`, `A.`, `L.`, `D.`) become combination→point edges, and
  each combination cites the method entry and its indication text.

`render_network` maps the network nodes to the same PNG renderer. These edges are explicit source
relationships, not inferred clinical equivalence.

### 7.8 Graph PNG rendering

`render_graph` uses the same term/direction/depth/limit arguments as `related_entries` and writes a
PNG to `--pages-dir` when configured, otherwise to the system temporary directory. The image uses
one column per hop, a gold start box, labelled node boxes, green outgoing edges, and blue incoming
edges. The 8x8 bitmap font keeps the build asset-free; labels are truncated to their box width.
The tool returns an MCP `image` content block plus an absolute `Image: <path>` line, and structured
metadata (path, MIME type, dimensions, graph) so a client that cannot display images can still open
the file.

## 8. MCP interface

### 8.1 Protocol

Same deliberate legacy choice as the reference server:

- Supported revisions: `2025-11-25`, `2025-06-18`, `2025-03-26`, `2024-11-05`.
- `initialize` echoes a supported requested version, otherwise answers `2025-11-25`.
- `server/discover` answers `-32601 Method Not Found`, so 2026-era clients fall back.
- `notifications/cancelled` aborts in-flight calls.
- stdout carries JSON-RPC only; diagnostics go to stderr.

### 8.2 `search_dictionary`

Input schema (narrow subset only: `type`, `properties`, `required`, `additionalProperties`,
`items`, `enum`, `description`, `default`):

| Argument | Type | Default | Notes |
|---|---|---|---|
| `query` | string, required | — | English term; pinyin in tone-numbered (`qi4`), tone-marked (`qì`), or toneless (`qi`) form; simplified/traditional Chinese; point ID; herb; formula; measurement; or citation anchor. |
| `scope` | string enum, optional | `all` | `all`, `dictionary`, `meridian`, `medicinal`, `formulas`, `weights`. |
| `limit` | integer, optional | 5 | 1–20; values outside the range are an error, not clamped. |

Structured output:

```json
{
  "query": "abdominal fullness",
  "scope": "all",
  "hits": [
    {
      "source": "dictionary",
      "term": "abdominal fullness",
      "id": null,
      "snippet": "A subjective feeling of fullness …",
      "matched_in": "headword",
      "score": 1000,
      "file": "08-a.md",
      "anchor": "a1S5W",
      "redirect": false
    }
  ],
  "total": 1
}
```

Text rendering is compact Markdown: rank, term, source/citation, match reason, snippet.

A citation anchor returns exactly one hit, with the full entry text in `snippet` (up to 20,000
characters). This lets a citation tag be passed back into search without losing the exact sense.

Exact tone-numbered pinyin (`qi4`, `qì`) and Han strings (`气`, `氣`, `麻黄`, `麻黃`) answer from
the generated indices. A toneless query (`qi`, `ma huang`) receives exact pinyin hits first, then
ordinary ranked text results. `qi2` deliberately returns the `Qi4` dynasty and the navel/umbilicus
entries, not the `qi4` air/energy entry.

### 8.3 `get_entry`

| Argument | Type | Default | Notes |
|---|---|---|---|
| `term` | string, required | — | Headword; pinyin in tone-numbered, tone-marked, or toneless form; simplified/traditional Chinese; point ID; herb/formula name or ID; appendix-4 section title; or citation anchor. |
| `source` | string enum, optional | `all` | Restrict the search domain. |
| `max_chars` | integer, optional | 20000 | 1–100000. |

Structured output:

```json
{
  "source": "dictionary",
  "term": "abdominal fullness",
  "id": null,
  "aliases": {
    "simplified": "腹满",
    "traditional": "腹滿",
    "pinyin_toned": "fu4 man3",
    "pinyin_ascii": "fu man",
    "english": "abdominal fullness"
  },
  "content": "…resolved Markdown…",
  "cross_references": [
    {"text": "abdominal distension", "target_term": "abdominal distension",
     "file": "08-a.md", "anchor": "a1SDZ", "resolved": true}
  ],
  "related": [
    {"source": "meridian", "id": "ST-36", "term": "Leg Three Li",
     "chinese": "足三里", "summary": "Actions: …"}
  ],
  "redirected_from": null,
  "citation": "Example Dictionary, 08-a.md#a1S5W",
  "truncated": false,
  "total_chars": 1234
}
```

Pinyin and Han terms resolve through the same generated indices as search. An ambiguous toneless
term prefers a non-redirect, pinyin-like headword, so `get_entry("qi")` returns `qi4` rather than
the navel redirect. `get_entry("qi2")` returns the `Qi4` dynasty entry.

Every citation names the Markdown file the text came from. For appendix-4 sections that name is
`Section::file`, carried from the archive entry that started with `37-`, so an archive naming it
`37-appendix-4.md` is cited as `…, 37-appendix-4.md:21` and one naming it
`37-appendix-4-Weights-Measurements.md` is cited under that name. No appendix file name is
hard-coded.

If the term is not found, return an MCP `isError: true` tool result listing up to five near
matches, so the model can correct the spelling. A valid query with no match is an execution
outcome, not a protocol violation; JSON-RPC `Invalid Params` is reserved for malformed arguments.

### 8.3.1 `related_entries`

| Argument | Type | Default | Notes |
|---|---|---|---|
| `term` | string, required | — | Main dictionary headword, alias, pinyin, Chinese, or citation anchor. |
| `direction` | string enum, optional | `both` | `outgoing`, `incoming`, `both`. |
| `depth` | integer, optional | 1 | 1–3. |
| `limit` | integer, optional | 20 | 1–50. |

Structured output:

```json
{
  "query": "qi",
  "term": "qi4",
  "anchor": null,
  "direction": "both",
  "depth": 1,
  "nodes": [
    {
      "source": "dictionary",
      "term": "channel qi4",
      "anchor": "a1PDH",
      "file": "23-p.md",
      "citation": "Dictionary, 23-p.md#a1PDH",
      "source_file": "C:\\data\\23-p.md",
      "line": 123,
      "distance": 1,
      "edge_direction": "outgoing",
      "via": "qi4",
      "link_text": "channel qi4",
      "redirect": false
    }
  ],
  "total": 1,
  "truncated": false
}
```

Absent optional fields are omitted. Text rendering includes the canonical citation, the absolute
`Source: <path>:<line>` line when available, the edge direction, `via`, and the visible link text.
Appendix-only terms return `DICTIONARY_NOT_FOUND`; invalid direction/depth/limit are JSON-RPC
`-32602` errors.

### 8.3.2 `render_graph`

Same arguments as `related_entries`. The result has a text block containing `Image: <absolute
path>`, an MCP `image` block with PNG data, and `structuredContent`:

```json
{
  "graph": { "query": "spleen", "term": "spleen", "direction": "outgoing", "depth": 2,
             "nodes": [], "total": 81, "truncated": true },
  "image_path": "C:\\ProgramData\\mcp-dict-pages\\graph-9a3e425d4c7c00d2.png",
  "mime_type": "image/png",
  "width": 1000,
  "height": 1130
}
```

The file is written to `--pages-dir` when configured, otherwise the system temporary directory.
The image is a diagnostic view: one column per hop, gold start node, green outgoing edges, blue
incoming edges, and truncated labels. It does not replace `get_entry` for entry text.

### 8.3.3 `related_network`

Required `term` and `domain` (`formula`, `medicinal`, `point_combination`), plus optional
`direction`, `depth` (1–3), and `limit` (1–50). Output is a `NetworkResponse` whose nodes carry
`node_type`, `relation`, `label`, distance, edge direction, via, citation, and absolute source
line. The formula domain follows formula-family edges; medicinal follows formula↔medicinal edges;
point_combination follows method→combination→point and point→combination edges.

### 8.3.4 `render_network`

Same arguments as `related_network`; returns a PNG image block, an absolute path, and a
`NetworkImageResponse` with the network, image path, MIME type, width, and height.

### 8.4 Server instructions

Published in `initialize` and kept short: the server searches the dictionary and its four
appendices locally; prefer `get_entry` for exact definitions; prefer `search_dictionary` when the
exact headword is unknown; use `related_entries` to follow resolved cross-references; use
`render_graph` for a PNG view; use `related_network`/`render_network` for formula, medicinal, and
point-combination relationships; cite the returned file/anchor. It also names the entry resource
(`dictionary://entry/<anchor>`) and the `citation` prompt.

### 8.5 Resources and the citation prompt

A citation must be reachable without a tool call, because some harnesses expose neither prompts
nor a way to turn a hyperlink into a tool call. The server therefore publishes both optional
surfaces:

- **Resources.** `resources/list` returns `dictionary://about` (server name and version, the
  measured source counts, the tools, and the URI scheme). `resources/templates/list` returns
  `dictionary://entry/{reference}`, where the reference is a citation anchor, headword, pinyin, or
  Han term, percent-encoded where it is not URI-safe. `resources/read` returns one `text/markdown`
  content block holding the same text `get_entry` renders. An unknown URI and a URI naming no
  entry are both `-32002` (`RESOURCE_NOT_FOUND`); a read that fails after parsing is `-32603`.
  Resources are advertised as `{"subscribe": false, "listChanged": false}`.
- **Prompt.** `prompts/list` returns one prompt, `citation`, with a required single-token
  `reference` argument. `prompts/get` resolves it through the same entry path and returns the
  rendered entry as a `user` message, so clients that run prompts see the text immediately.
  Unknown names and missing arguments are `-32602`; a reference naming no entry is `-32602` with
  the not-found text and suggestions.
- **Client support.** Claude Code runs MCP prompts as `/mcp__<server>__<prompt>` and reads
  resources by URI; OpenCode runs prompts; Codex, pi, and DeepSeek Harness have no prompt support
  (DeepSeek Harness states prompt templates are unsupported) but read resources through their
  shared resource tools. The tools and the `tcm citation <anchor>` instruction remain the
  portable path.
- No hyperlink can invoke a tool in any current client: a Markdown link is a URL, so the server
  keeps text, tools, and resources rather than emitting links.

### 8.6 Clickable citation pages

A click can only open an existing target, never call a tool, and the host decides what a link may
open. VS Code's integrated terminal is the constraining case: it restricts OSC 8 schemes, and
`file://` links are unreliable there and break under Remote-SSH/WSL, but it does linkify a plain
absolute path and open that file in the editor. The design therefore makes the citation a file
that already exists:

- With `--pages-dir` / `MCP_CHINESE_DICT_PAGES_DIR`, `get_entry` writes a Markdown page per result
  (`<source>-<id>.md`: `dictionary-a1TAK.md`, `meridian-ST-36.md`, `medicinal-01.01.01.md`,
  `formulas-01.01.01.md`, `weights-21.md`) and appends `Entry page: <absolute path>` plus
  `Clickable link: [open the entry](file:///…)`.
- The bare path serves VS Code (editor on Ctrl+click, local or remote); the `file://` URL serves
  OSC 8 terminals (Windows Terminal, iTerm2, Kitty, WezTerm, GNOME Terminal).
- A page links each resolved cross-reference to a **sibling page** written in the same call, so the
  link resolves inside a Markdown preview, which resolves relative targets against the page's own
  folder. Those target pages keep plain cross-references: generating their targets too would
  recurse through the dictionary. Only a target that was actually written is linked.
- A page shows its source citation as an inline code span (`10-c.md#a1RMS`). A bare relative name is resolved
  against the page's folder, and when it is absent VS Code's preview falls back to a web address
  (`https://10-c.md/#a1RMS`); its built-in preview does not follow `file:///` links at all
  (microsoft/vscode#182870), so code is the honest rendering.
- Pages reuse `render_entry_page`, which is `render_entry_text` without the "reopen this citation"
  instruction, plus a generated-file footer.
- The directory is resolved with `std::path::absolute` and created at startup. A path containing
  a space is accepted with a warning, because space handling in terminal link detection varies.
- A write failure appends a bracketed note and never fails the tool call. Pages are derived
  copyrighted data: outside the repository, never in `dist/`.
- Not implemented: custom URL schemes (`tcm://`). They need a registry key on Windows, a
  `CFBundleURLTypes` app bundle on macOS, and an `x-scheme-handler` desktop file on Linux, and VS
  Code may not follow custom schemes at all. A page link buys the same click for none of that.

### 8.7 Linked data build

The Markdown conversion dropped the `id` attributes from the source EPUB. Only `38-index.md`
still carries the anchor-to-entry map, so the book's own `href="10-c.md#a1RMS"` links have no
target: measured on the extracted folder, 14,226 fragment links, none resolvable, and 5,803
headword lines.

`--build-linked-data <dir>` fixes the data once, at setup, rather than per link:

- Read every Markdown document from the zip or folder (`source::read_all_documents`), including the
  front matter that the server otherwise ignores, so the copy is the whole book.
- Insert one `<a id="…"></a>` line immediately before each entry's headword, using the anchors the
  loader already resolved from `38-index.md` (5,799 of 5,803 entries; four have no anchor in the
  index or the EPUB). Anchors are written only where the parsed line is still the headword, so a
  line drift is an error rather than a misplaced id.
- `parse_entries` ignores standalone anchor marker lines, which fall at the end of the previous
  entry's range. The entry text is therefore byte-for-byte the same as the source; the build
  re-loads the copy and compares every headword, anchor, alias field, body, and the appendix and
  weights counts, failing on any mismatch.
- The build then scans the copy for `href="…"` fragments and reports resolved and unresolved
  counts. On the real dictionary: 14,147 resolved, 79 unresolved — 37 into the table of contents,
  27 into the index's own lines, 6 into the introduction, 8 into a paragraph of `wind stroke`
  (`a1SSD`), and 1 into an HTML file that is not in the archive. Those are not entry starts, and
  the Markdown carries no position for them.
- The copy unescapes literal brackets: `\[` → `[` and `\]` → `]`. A math-enabled Markdown preview
  (VS Code's KaTeX support, Markdown Preview Enhanced) reads `\[` as display math and hides the 833
  appendix monograph headings; the unescaped form renders as text. `monograph_line` accepts both
  forms, and the entry bodies are compared after the same unescaping, so verification still proves
  the text unchanged. The server's rendered output was already unescaped through `plain_text`.
- Running the build from an already linked copy inserts nothing. The source is never modified; the
  copy is derived copyrighted data and stays outside the repository and `dist/`.
- With a directory source, rendered tool text stops printing the bare `file#anchor`: `EntryResponse`
  and `SearchHit` carry the absolute `source_file` and the entry's `line`, and the header becomes
  `Source: <path>:<line> · anchor <a1XXXX>`. A terminal opens that path at the line; a relative
  `10-c.md#a1RMS` cannot be resolved against the client's working directory. Cross-reference lines
  name only `(anchor a1XXXX)`, never a path. The stored `citation` stays canonical, and pages keep
  it as code because a Markdown preview resolves relative to the page.

### 8.8 Misspelling suggestions

Exact lookup stays exact; suggestions are a recovery surface, not a fuzzy match:

- They run only when exact lookup fails or the best ranked hit scores below 300, so a normal
  high-confidence answer never carries them.
- English: optimal string alignment (Damerau–`Levenshtein`, adjacent transposition = one edit) over
  normalized headwords, appendix terms, and weights titles. A single-word query may match one word
  of a multi-word term, so `abdomnal` reaches `abdominal fullness`; multi-word queries compare
  against the whole term.
- Pinyin: the same distance over the toneless key map, displayed as `key (term)`, for example
  `ma huag` → `ma huang (ephedra)`.
- Thresholds: queries shorter than four characters or containing Han produce nothing; distance 1
  up to six characters, distance 2 above. Candidates sort by distance, then key length, then text;
  at most three of each kind, at most five in a search result and eight in a `get_entry` error.
- `get_entry` merges these after its prefix/substring suggestions; `search_dictionary` exposes them
  in `structuredContent.suggestions` and prints `Did you mean: …?` whether or not it found weak
  hits. The caller must reissue the query.
- A suggestion is `{text, term, page}`. `page` stays `None` until the tool layer, with a page
  directory configured, resolves `term` and writes its page; both the search text and the not-found
  error then print `text — <absolute page path>`, so the suggestion is clickable. Writing a page per
  suggestion is at most three short files per failed lookup, overwritten later. Without a page
  directory the field stays `None` and the text is unchanged.
- `DictionaryError` carries `suggestion_terms` alongside `suggestions`, aligned by index, so the
  error result can be enriched without changing its wording. Resources and prompts still render the
  text-only error.
- Cache version 2: only pinyin-shaped headwords (`qi4`, `Qi4`) enter the pinyin indices. English
  headwords used to be indexed as toneless keys, which produced suggestions such as
  `constipation (constipation)` and polluted exact lookups.
- Limitation: a query that is a prefix of an alias still resolves through the normal fallback
  (`xue4 ha` → `xue4 han2`, blood cold) rather than being offered as a suggestion.

## 9. Configuration and startup

| Flag | Environment | Default | Meaning |
|---|---|---|---|
| `--data <path>` | `MCP_CHINESE_DICT_DATA` | required | Zip archive or extracted directory. |
| `--title <text>` | `MCP_CHINESE_DICT_TITLE` | data path name | Display name used in citations. |
| `--log-file <path>` | `MCP_CHINESE_DICT_LOG_FILE` | off | Append diagnostics to a file as well as stderr. |
| `--cache-dir <path>` | `MCP_CHINESE_DICT_CACHE_DIR` | beside the dictionary | Generated lookup-cache and semantic-index directory. |
| `--pages-dir <path>` | `MCP_CHINESE_DICT_PAGES_DIR` | off | Write a Markdown page per `get_entry` result and name it in the result for clickable citations. |
| `--build-indices` | — | off | Rebuild the pinyin and Han indices, write the cache, and exit without serving stdio. |
| `--build-linked-data <dir>` | — | off | Copy the Markdown set, insert the dropped index anchors, verify entries and links, and exit without serving stdio. |
| `--verbose` | — | off | Debug-level request handling and parse report. |
| `-h`, `--help`, `--version` | — | — | Printed to stdout before an MCP client is attached. |

`--data` (or `MCP_CHINESE_DICT_DATA`) is required; there is no filename-based discovery. Any MCP
client may launch the process from another directory, so an absolute path is recommended. The
citation title comes from `--title`, the archive file name, or the directory name; a linked copy
carries a `dictionary-title.txt` sidecar.

Startup sequence:

1. Parse flags/environment.
2. Load and parse the dictionary.
3. Log the load report: main entries, appendix entries, resolved/unresolved links, duplicate
   headwords, parse failures.
4. With `--build-indices`, rebuild the lookup indices, replace the cache file for the current
   fingerprint, log the key counts and cache path, and exit; nothing is read from stdin.
5. Serve stdio.
6. On stdin close, drain in-flight calls and exit 0.

Exit codes: `0` normal, `1` transport failure or a lookup cache that could not be written under
`--build-indices`, `2` configuration or data error.

## 10. Error handling and diagnostics

| Situation | Behaviour |
|---|---|
| Missing data path | Exit 2: `Cannot read the dictionary at <path>. Pass --data <zip-or-directory>.` |
| Corrupt zip / unreadable file | Exit 2, naming the file and the I/O error. |
| No main entries parsed | Exit 2: the loader classified no letter files. |
| Partial parse anomalies | Start normally; log counts; tests gate coverage. |
| Tool argument invalid | JSON-RPC `-32602` with a message naming the field and the allowed range. |
| Unknown tool | JSON-RPC `-32602` listing available tools. |
| Term not found | `isError: true` tool result with up to five near matches. |
| Data loaded but an entry is empty | Normal result with a note that the source has no body text. |
| Internal panic risk | Avoid `unwrap`/indexing on parsed input; malformed data yields warnings, not crashes. |

## 11. Testing strategy

### Unit tests (`src/**`)

- Normalization: diacritics, NBSP, escapes, case, Chinese passthrough.
- Main parser: single entry, multi-alias entry, redirect entry, empty body, duplicate headword,
  CRLF input.
- Alias parser: standard, tone-number and diacritic forms, missing `|`, nested variant block.
- Index parser: plain index link, numbered sub-anchors, navigation links ignored.
- Link resolver: resolved, unresolved, cross-file, self-link.
- Appendix 1 parser: header with alt names, non-channel ID, trailing `No Match`.
- Appendix 2 parser: `⁑`, nested parentheses, escaped brackets, variant IDs.
- Appendix 3 parser: single Chinese form, composition bullets.
- Appendix 4 parser: three sections and table retention.
- Ranking: exact > prefix > contains > body; token coverage; tie-breaking.
- Snippet windowing and truncation.
- `get_entry` redirect following and cycle detection.
- Graph build: resolved vs unresolved links, outgoing/incoming adjacency, unmapped targets.
- Graph traversal: directions, depth 2, cycles, duplicate neighbours, limit truncation, anchorless
  start, appendix-only error.
- Graph image: PNG signature/dimensions, empty graph, image content block, absolute path, and
  structured image metadata.
- Appendix network: formula derivation prose, ingredient-line matching, point-combination grouping,
  point-ID boundary handling, inverse relations, and real-data manual checks.

### Integration tests (`tests/`)

- `stdio_contract.rs`: real binary, initialize/ping/tools/list/server-discover/ping, stdout purity,
  help/version streams, configuration error exit code.
- `tools_e2e.rs`: an invented corpus, written to a temporary directory at test time by
  `tests/support/fixture.rs`, exercises every tool over stdio, scope filtering, appendix routing,
  graph traversal, argument errors, and not-found near matches. No dictionary data is stored in the
  repository.
- `real_data.rs`: when the archive is present, assert graph node/edge/unmapped invariants and a
  traversal from the first dictionary hit, in addition to:
  - 5,803 main entries, 403 point entries, 572 medicinals, 261 formulas;
  - `abdominal fullness` returns its definition;
  - `ST-36` returns `Leg Three Li` (足三里);
  - `ma huang` returns the `Ephedrae Herba` monograph;
  - `Ephedra Decoction` returns the formula;
  - `liang` includes the weights appendix;
  - `abdomen` cross-reference `greater abdomen` resolves;
  - link-resolution count is at least 14,140.
  If the archive is absent, the test prints a skip reason and passes; CI can set
  `MCP_CHINESE_DICT_DATA`.
- `dist_bundle.rs`: version and no-data guards for `dist/`.

### Gates

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

No test may depend on the user's home directory, a network connection, or the archive being in a
fixed location other than the documented discovery rules.

## 12. Milestones

| Phase | Deliverable | Exit condition |
|---|---|---|
| 0 | Discovery, plan, checklist, working rules | This document, `CHECKLIST.md`, `CLAUDE.md` reviewed |
| 1 | Repository scaffold | `cargo check` passes; module map and config skeleton exist |
| 2 | Data loading and parsing | Unit tests for every parser pass; real archive loads with a full report |
| 3 | Search and retrieval engine | Ranking, snippets, scope routing, redirects tested |
| 4 | MCP server and tools | stdio contract and tool integration tests pass |
| 5 | Full test suite and real-data validation | All gates green; golden counts and lookups pass |
| 6 | Documentation and deployment bundle | README, BUILDING, INSTALL, config example, binary and checksums |
| 7 | Deferred enhancement | Not part of v1; see §15 |

## 13. Acceptance criteria

1. `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
   `cargo test --all-targets` pass on Windows.
2. The release binary loads the supplied archive in under 2 seconds (measured: 0.44 s with a warm
   lookup cache and 0.50 s on the first run; 393 ms after Phase 6.1; 405 ms for v0.1.0; debug build
   2.6–3.1 s) and reports the expected counts.
3. `search_dictionary`, `get_entry`, `related_entries`, `render_graph`, `related_network`, and
   `render_network` work over a real stdio session with Claude Code, OpenCode, or a scripted
   client.
4. A query for an acupoint, herb, formula, or measurement returns both the dictionary material
   (when one exists) and the relevant appendix material.
5. Every result carries a file/anchor citation.
6. No dictionary content is committed to the repository or shipped in `dist/`.
7. Model-facing descriptions, README tables, `--help`, and error messages agree with the code.

## 14. Risks and mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| Anchors absent from Markdown | Cross-references unresolved | Resolve through `38-index.md` plus the EPUB-verified corrections; 8,283/8,283 links resolve; graceful text fallback remains. |
| Irregular appendix headers | Loss of appendix entries | Tolerant balanced-parenthesis parser; 100 % coverage test gate. |
| Very long entries (`26-s.md` is 627 KB) | Oversized tool results | Snippets by default; `max_chars` on `get_entry`. |
| Appendix body matches flood results | Poor relevance | Appendix bodies are searched only when `scope` names that appendix; `all` uses term, identifier, and alias matches. |
| Copyright | Cannot redistribute the book | Never commit the zip or extracted text; user supplies the archive; `dist/` excludes data. |
| Windows CRLF, NBSP, diacritics | Missed matches | Normalize on ingest; tests cover each. |
| Schema too wide for a harness | Tool set dropped | Adhere to the narrow JSON Schema subset. |
| Duplicate or empty entries | Ambiguity | Deterministic policy in §6.8. |
| Startup path differs under a client | Data not found | Absolute `--data` recommended; exe-dir fallback; clear error. |
| Panics on malformed input | Server outage | No unchecked indexing on parsed text; warnings not fatal. |

## 15. Deferred enhancements (specification's "eventually")

Phase 7A implements the pinyin- and Chinese-keyed lookup described in the specification:

- `src/dictionary/pinyin.rs` reduces tone-numbered (`qi4`), tone-marked (`qì`), and toneless
  (`qi`) spellings to a tone-numbered key and a toneless key. It also folds the source's `u9`
  encoding of `ü` (`nu94`) to `nu4`.
- `src/dictionary/lookup.rs` builds three maps: exact tone-numbered pinyin, toneless pinyin, and
  Han strings. Main-entry aliases, appendix pinyin fields, appendix simplified and traditional
  forms, and appendix Han alias runs are indexed.
- The maps are cached at
  `<dictionary title>.lookup-cache-v2.<fingerprint>.json`, beside the dictionary or in
  `--cache-dir` / `MCP_CHINESE_DICT_CACHE_DIR`. The fingerprint is FNV-1a over the sorted Markdown
  file names and contents. A hit requires the cache version, package version, and fingerprint to
  match; a change rebuilds and prunes the older fingerprint file. The cache is derived
  copyrighted data and remains ignored by git; it is never copied into `dist/`.
  `--build-indices` rebuilds the maps and replaces the cache file for the current fingerprint
  without serving stdio, so a client's first startup is a hit.
- `search_dictionary` answers exact tone-numbered pinyin and Han queries from the index; toneless
  pinyin is ranked first while ordinary text search continues. `get_entry` prefers a non-redirect
  pinyin-like headword when a toneless query is ambiguous.
- Tests cover `qi`/`qi4`/`qì`/`qi2`/`qí`, `气`/`氣`, `麻黄`/`麻黃`, `qīng rè`/`qing1 re4`,
  `zu3 san1 li3`, `下陵`, and cache reuse/invalidation.

Graph traversal is implemented over the existing resolved links. Gated local-Ollama semantic
retrieval is implemented behind the optional `semantic` feature and tracked in
[`ENHANCEMENT-PLAN.md`](ENHANCEMENT-PLAN.md); the default build remains lexical-only. The feature
builds a digest-keyed flat cosine index from the same entries and appendices, consults the model
only when the best lexical score is below 300, skips pure pinyin, and merges results with
reciprocal-rank fusion. Missing Ollama, a missing model, a stale index, or an embedding failure
degrades to the normal lexical response unless `--require-semantic` is set. On a local 30-query
English/mixed set with `embeddinggemma:latest`, hybrid recall@5 rose from 0.233 to 0.567 and MRR
from 0.183 to 0.455, with 75 ms added p95, 20.66 MB additional MCP working set, and a 20.66 MiB
index; the exact/pinyin/Chinese regression set kept top-1 at 1.0. The feature was accepted with
documented limits in [`ENHANCEMENT-CHECKLIST.md`](ENHANCEMENT-CHECKLIST.md) (S4.7–S4.8).
Additional Chinese-medicine books and a research mode remain deferred.

## 16. Decisions and open questions

Recommended defaults are marked; change them before Phase 2 if desired.

| # | Decision | Recommended default |
|---|---|---|
| 1 | Data source | Support **both** zip and extracted directory. |
| 2 | Tool surface | Six tools: `search_dictionary`, `get_entry`, `related_entries`, `render_graph`, `related_network`, `render_network`. No browse/stat tool in v1. |
| 3 | Appendix augmentation | Query/headword/identifier driven, plus point IDs found in a body. Not every herb or formula mentioned in prose. |
| 4 | Redirect entries | Follow one `See …` hop automatically and disclose it. |
| 5 | Output defaults | 5 hits, 20,000 characters per entry. |
| 6 | Distribution | Windows x64 bundle in `dist/` after Phase 5; other platforms documented, not prebuilt. |
| 7 | Git | Initialize a repository at Phase 1; never commit the archive, extracted text, `target/`, or a buggy build. |
| 8 | Lookup cache | `<dictionary title>.lookup-cache-v2.<content fingerprint>.json` beside the data; `--cache-dir` / `MCP_CHINESE_DICT_CACHE_DIR` override; derived data stays out of git and `dist/`. |

## 17. Planned file layout

```text
mcp-chinese-dict-search/
├── Cargo.toml
├── Cargo.lock
├── rustfmt.toml
├── README.md
├── BUILDING.md
├── PLAN.md                     # this document
├── CHECKLIST.md                # live completion tracker
├── CLAUDE.md                   # working rules
├── LICENSE                     # MIT (code only)
├── .gitignore                  # target/, archive, logs, local data
├── dictionary.zip            # user-supplied; never committed
├── tools/                      # read-only source-data analysis scripts
│   ├── inspect_dict.py
│   ├── inspect_appendix.py
│   ├── inspect_entries.py
│   ├── validate_rules.py
│   ├── validate_headers.py
│   ├── analyze_anchors.py      # anchor shape, uniqueness, per-file coverage
│   ├── anchor_origin.py        # serial/hash/order tests for a1XXXX
│   ├── anchor_sequence.py      # numeric sequence and contiguous-block inspection
│   ├── analyze_classes.py      # class-ID vs anchor-ID namespace relation
│   └── anchor_proximity.py     # linked-term distance vs random baseline
├── src/
│   ├── main.rs                 # thin executable entry point
│   ├── lib.rs                  # public module map
│   ├── cli.rs                  # startup, logging, signal handling, stdio wiring
│   ├── config.rs               # flags, environment, data-path discovery, --help
│   ├── error.rs                # stable error vocabulary
│   ├── logging.rs              # stderr logger with optional file
│   ├── protocol.rs             # JSON-RPC classification and response builders
│   ├── server.rs               # dispatch, initialize, tools/call lifecycle
│   ├── transport.rs            # newline framing, concurrent dispatch, stdout writes
│   ├── tools.rs                # tool schemas, validation, text rendering
│   ├── graph_image.rs          # PNG rendering of graph results
│   └── dictionary/
│       ├── mod.rs              # Dictionary, load entry point, re-exports
│       ├── model.rs            # Entry, Alias, AppendixEntry, Section, Link
│       ├── source.rs           # zip/directory reader and file classification
│       ├── parse.rs            # main-entry and alias parser
│       ├── index.rs            # index parsing and anchor resolution
│       ├── appendix.rs         # appendix 1–4 parsers
│       ├── normalize.rs        # NFKD, escapes, case, whitespace
│       ├── pinyin.rs           # tone-numbered, tone-marked, and toneless keys
│       ├── lookup.rs           # generated pinyin and Han lookup indices
│       ├── graph.rs            # resolved-link adjacency and bounded traversal
│       ├── network.rs          # formula, medicinal, and point-combination networks
│       └── search.rs           # ranking, snippets, scopes, get_entry
└── tests/
    ├── support/mod.rs          # hermetic binary command + stdio session helper
    ├── support/fixture.rs      # invented corpus, written at test time
    ├── stdio_contract.rs
    ├── tools_e2e.rs
    ├── real_data.rs
    └── dist_bundle.rs
```
