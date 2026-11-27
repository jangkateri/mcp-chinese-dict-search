# PROJECT CHECKLIST — `mcp-chinese-dict-search`

Live completion tracker for [`PLAN.md`](PLAN.md). **Update this file in the same turn as the work
it records.**

| Field | Value |
|---|---|
| Project version | 0.6.0 |
| Plan | [`PLAN.md`](PLAN.md) |
| Last updated | 2026-11-27 |
| Overall status | Working tree 0.6.0: graph traversal, graph PNGs, appendix networks, and gated semantic retrieval (default feature; runtime opt-in); v0.5.0 remains the last release |

## Status legend

| Mark | Meaning |
|---|---|
| `[ ]` | Not started |
| `[~]` | In progress |
| `[x]` | Complete **and verified** |
| `[!]` | Blocked; state the blocker in the progress log |
| `[-]` | Deferred; explicitly out of scope for this version |

### Rules for maintaining this checklist

1. Mark `[x]` only after the stated verification has run and passed. Code that merely compiles is
   not verified.
2. Record the command or evidence for each completed task in the progress log.
3. A phase is complete only when its exit condition in `PLAN.md` §12 is met.
4. When a task is added, removed, or split, update the phase count and the summary table in the
   same edit.
5. Never mark a task complete because a later task depends on it.

## Progress summary

| Phase | Scope | Tasks | Done | Status |
|---|---|---:|---:|---|
| 0 | Discovery, plan, checklist, working rules | 11 | 11 | Complete |
| 1 | Repository scaffold | 11 | 11 | Complete |
| 2 | Data loading and parsing | 18 | 18 | Complete |
| 3 | Search and retrieval engine | 18 | 18 | Complete |
| 4 | MCP server and tools | 14 | 14 | Complete |
| 5 | Validation and real-data testing | 11 | 11 | Complete |
| 6 | Documentation and deployment bundle | 11 | 11 | Complete |
| 6.1 | EPUB verification and corrections | 12 | 12 | Complete |
| 6.2 | Actionable citation anchors | 7 | 7 | Complete |
| 7A | Pinyin and Chinese lookup | 8 | 8 | Complete (working tree) |
| 7B | Additional books and research mode | 1 | 0 | Deferred |
| 7C | Entry resources and the citation prompt | 5 | 5 | Complete (working tree) |
| 7D | Clickable citation pages | 3 | 3 | Complete (working tree) |
| 7E | Output-schema contract | 5 | 5 | Complete (working tree) |
| 7F | Graph traversal and `related_entries` | 1 | 1 | Complete (tracked in `ENHANCEMENT-CHECKLIST.md`) |
| 7G | Graph PNG rendering and `render_graph` | 1 | 1 | Complete (tracked in `ENHANCEMENT-CHECKLIST.md`) |
| 7H | Appendix networks | 1 | 1 | Complete (tracked in `ENHANCEMENT-CHECKLIST.md`) |
| 7I | Optional gated semantic retrieval | 1 | 1 | Complete (tracked in `ENHANCEMENT-CHECKLIST.md`) |
| **Total (phases 0–7A, 7C–7I)** | | **138** | **138** | **100 %** |

---

## Phase 0 — Discovery, plan, and working rules

**Exit condition:** `PLAN.md`, `CHECKLIST.md`, and `CLAUDE.md` exist and reflect measured facts
about the source archive.

- [x] Read `PROJECT-SETUP-PROMPT.md` and record its requirements.
  - Evidence: scope, tool expectations, appendix rules, enhancement section captured in `PLAN.md`.
- [x] Inspect a reference Rust MCP project for layout,
  module boundaries, MCP protocol era, tests, and documentation conventions.
  - Evidence: `src/{cli,config,error,lib,logging,protocol,server,tools,transport}.rs`, `tests/`,
  `dist/`, `CLAUDE.md`, `BUILDING.md` reviewed.
- [x] Inventory the archive: 39 Markdown files, 7,339,349 bytes of Markdown, 10 referenced images,
  UTF-8 without BOM, CRLF.
- [x] Measure main-dictionary structure: 5,803 entries, 26 letter files, 22 duplicate raw
  headwords, 395 multi-alias entries, 2 entries without aliases, 1 empty entry.
- [x] Measure index and link structure: 5,822 index links, 5,795 anchors, 14,155 internal links;
  the index alone resolves 14,142, and EPUB-verified corrections resolve the other 13. Runtime
  entry-body cross-references: 8,283 of 8,283.
- [x] Measure appendix structure: 403 points / 15 sections, 572 medicinals / 20 sections,
  261 formulas / 21 sections, 3 weights-and-measures table sections.
- [x] Validate the proposed normalization and link-resolution rules against the full archive.
  - Evidence: 0 unresolved index terms after normalization; 100 % runtime link resolution after
    the phase 6.1 corrections.
- [x] Validate header grammars: 6,352/6,352 main alias lines; 403/403 appendix-1 headers with a
  tolerant grammar; 572/572 and 261/261 appendix headers with tolerant parentheses/`⁑` handling.
- [x] Write `PLAN.md` with architecture, parser rules, tool contracts, milestones, and risks.
- [x] Write `CHECKLIST.md` (this file) with phase tasks and verification gates.
- [x] Write `CLAUDE.md` with the mandated behaviour rules, research protocol, Karpathy guidelines,
  and project-specific constraints.

---

## Phase 1 — Repository scaffold

**Exit condition:** `cargo check` and `cargo test --all-targets` pass with the module skeleton in
place; the archive is ignored by Git.

- [x] Initialize the Git repository and create `.gitignore` covering `target/`, `*.zip`, logs,
  `.credentials`, and local data directories.
  - Verify: `git status --short` does not list the archive or `target/`.
- [x] Create `Cargo.toml`: package and binary `mcp-chinese-dict-search`, version `0.1.0` (now
  `0.1.1` after the EPUB corrections),
  edition 2024, `rust-version = "1.99"`, dependencies `serde`, `serde_json`, `thiserror`, `tokio`,
  `tokio-util`, `zip`, `unicode-normalization`; release profile with LTO, one codegen unit, strip.
  - Verify: `cargo metadata --no-deps` succeeds and names the binary.
- [x] Add `rustfmt.toml` matching the reference conventions (`edition = "2024"`, `max_width = 100`,
  field-init and `?` shorthands).
- [x] Create `src/lib.rs` exporting `cli`, `config`, `dictionary`, `error`, `logging`, `protocol`,
  `server`, `tools`, `transport`.
- [x] Create `src/main.rs` as a thin `#[tokio::main]` wrapper returning `ExitCode`.
- [x] Port `src/logging.rs` and `src/error.rs`, adapting names and error vocabulary.
  - Verify: unit tests for error display and log-to-file behaviour.
- [x] Create `src/protocol.rs` and `src/transport.rs` from the reference, preserving the legacy
  protocol era and stdout purity.
  - Verify: protocol classification and framing unit tests pass.
- [x] Create `src/config.rs` with `--data`, `--log-file`, `--verbose`, `--help`, `--version`,
  environment fallbacks, and data-path discovery.
  - Verify: unit tests for defaults, flag override, invalid values, missing explicit path.
- [x] Create `src/cli.rs` and `src/server.rs` that compile and serve before the data layer is fully
  populated.
  - Verify: `cargo check` and the early protocol tests.
- [x] Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets`.
  - Verify: all three commands exit 0.
- [x] Create the initial commit when the user requests it.
  - Commit `30c440b` on `main`; the tree contains no archive, extracted text, EPUB, local setup
    notes, or generated release archive.

---

## Phase 2 — Data loading and parsing

**Exit condition:** the real archive loads without panics, the load report matches the measured
counts, and every parser has passing unit tests.

- [x] Define `dictionary::model` types: `Entry`, `Alias`, `Link`, `AppendixEntry`, `Section`,
  `LoadReport`, `SearchHit`, `Source`.
- [x] Implement `dictionary::normalize`: NFKD, combining-mark removal, NBSP folding, Markdown
  unescaping, lowercase, whitespace collapse. Unit-test each rule.
- [x] Implement the directory loader: read `*.md`, sort by name, strip CR, reject invalid UTF-8
  with the file name.
- [x] Implement the zip loader with the same downstream path as the directory loader.
- [x] Implement file classification by numeric prefix; tolerate old and new appendix filenames.
- [x] Implement main-entry parsing: column-0 `\*` boundaries, headword, alias block, body.
  Unit-test single, multi-alias, redirect, empty-body, and duplicate-headword cases.
- [x] Implement alias parsing for both pinyin forms, variant blocks, the missing-`|` line, and the
  `片 pian4` fallback.
- [x] Implement `38-index.md` parsing, including numbered sub-anchors and ignored navigation links.
- [x] Build `anchor → (file, entry)` resolution and unit-test all three index shapes.
- [x] Implement internal `<a href>` parsing and resolution; preserve link text when unresolved.
  All 13 initially unresolved links are now resolved by the Phase 6.1 EPUB corrections.
- [x] Implement appendix-1 parsing with balanced-parenthesis headers, alt names, sections, and
  non-channel IDs.
- [x] Implement appendix-2 parsing with `⁑`, nested parentheses, escaped brackets, and variant IDs.
- [x] Implement appendix-3 parsing with the single-Chinese-form header and composition bullets.
- [x] Implement appendix-4 section/table parsing (3 sections).
- [x] Implement `LoadReport` with counts for entries, appendix entries, resolved and unresolved
  links, duplicate headwords, and header failures; log it at startup.
- [x] Run the loader against the real archive and verify: 5,803 main entries, 403 points,
  572 medicinals, 261 formulas, 3 weights sections, 8,283 entry cross-references resolved,
  0 unresolved after Phase 6.1, 0 header failures.
- [x] Add malformed-input unit tests proving no panic on empty files, missing sections, truncated
  headers, and binary garbage.
- [x] Update `PLAN.md`/`CHECKLIST.md` with corrected measured counts (572/261, not 571/250).

---

## Phase 3 — Search and retrieval engine

**Exit condition:** ranking, snippets, scope routing, appendix augmentation, and `get_entry`
behave as specified in `PLAN.md` §7, with unit tests for each.

- [x] Implement query normalization and tokenization (words, punctuation, Han substrings).
- [x] Implement headword and alias scoring (exact, prefix, contains, token coverage).
- [x] Implement body scoring with phrase and token coverage.
- [x] Implement deterministic tie-breaking by book order.
- [x] Implement snippet extraction for headword, body, and redirect matches (caps per §7.3).
- [x] Implement scope filtering for `dictionary`, `meridian`, `medicinal`, `formulas`, `weights`.
- [x] Implement appendix term/identifier/alias search.
- [x] Implement `all`-scope appendix routing: exact/prefix name or ID first; appendix bodies are
  searched only when the scope names that appendix.
- [x] Implement measurement-query detection and appendix-4 augmentation.
- [x] Implement point-ID extraction from entry bodies and the `point_ids` lookup.
- [x] Implement `get_entry` resolution order and `source` restriction.
- [x] Implement one-hop `See …` redirect following with disclosure.
- [x] Implement `related` appendix summaries (max 10 points).
- [x] Implement `max_chars` truncation with `truncated` and `total_chars`.
- [x] Implement not-found near matches (up to five) in an `isError: true` tool result.
- [x] Add ranking/routing/retrieval unit tests and fixture-based assertions.
- [x] Measure and record load time and typical query latency against the real archive
  (release: 405 ms load; 6–86 ms per call).
- [x] Update this checklist.

---

## Phase 4 — MCP server and tools

**Exit condition:** the real binary completes a stdio handshake, lists both tools, executes both
over a pipe, and keeps stdout pure JSON-RPC.

- [x] Define server constants: `SERVER_NAME = "mcp-chinese-dict-search"`, title, version from
  `CARGO_PKG_VERSION`, and a short `SERVER_INSTRUCTIONS` text.
- [x] Implement `initialize`, `ping`, `tools/list`, `tools/call`, `logging/setLevel`, and
  `server/discover` → `-32601`.
- [x] Implement `tools/list` schemas for `search_dictionary` and `get_entry` inside the narrow
  JSON Schema subset; include output schemas.
- [x] Implement `tools/call` dispatch with `name` and `arguments` validation.
- [x] Implement argument validation (`query`, `scope`, `limit`, `term`, `source`, `max_chars`) and
  JSON-RPC `-32602` faults.
- [x] Implement tool text rendering plus `structuredContent` for both tools.
- [x] Implement cancellation and in-flight bookkeeping; local calls are short but still obey
  `notifications/cancelled`.
- [x] Wire `cli.rs`: resolve config, load dictionary, log the report, install signal handling,
  serve stdio, exit with the documented codes.
- [x] Implement `--help` and `--version` on stdout; every diagnostic on stderr.
- [x] Write `tests/stdio_contract.rs`: handshake, tools/list, discover fallback, ping, stdout
  purity, help/version, configuration-error exit code.
- [x] Create an invented corpus covering every source shape. It is written at test time by
  `tests/support/fixture.rs`; no dictionary data is stored in the repository. (Originally committed
  as `tests/fixtures/dictionary/`, removed 2026-11-25 — see the progress log.)
- [x] Write `tests/tools_e2e.rs` against the real binary and the generated corpus.
- [x] Write `tests/real_archive.rs` with the golden counts and representative lookups; skip with a
  printed reason when the archive is absent.
- [x] Update this checklist.

---

## Phase 5 — Validation and real-data testing

**Exit condition:** all quality gates pass on Windows, representative client sessions work, and
the measured data checks are green.

- [x] `cargo fmt --check` passes.
- [x] `cargo clippy --all-targets -- -D warnings` passes.
- [x] `cargo test --all-targets` passes (55 unit + 1 real-archive + 4 stdio contract + 4 tool e2e
  + 5 dist guards = 69 tests).
- [x] Golden counts pass: 5,803 / 403 / 572 / 261 / 3.
- [x] Golden lookups pass: `abdominal fullness`, `ST-36` → `Leg Three Li`, `ma huang` →
  ephedra, `Ephedra Decoction`, `liang` → weights appendix, `abdomen` → `greater abdomen`.
- [x] Link resolution is 8,283 entry cross-references with 0 unresolved after Phase 6.1; any source
  link that is still unresolved renders as plain text and never causes an error result.
- [x] Load time is 405 ms (release) / 2.6–2.7 s (debug); typical calls measured at 6–86 ms.
- [x] Manual stdio session reproduces a complete `initialize` → `search_dictionary` →
  `get_entry` exchange.
- [x] Confirm the archive, extracted text, images, and logs are not tracked by Git
  (`git check-ignore` confirms the archive, extracted directory, and `target/`).
- [x] Record all measured values in the progress log.
- [x] Update this checklist.

---

## Phase 6 — Documentation and deployment bundle

**Exit condition:** documentation agrees with the code, `dist/` is self-contained and data-free,
and the bundled binary passes a handshake on the target machine.

- [x] Write `README.md`: purpose, data/copyright notice, requirements, quick start, client
  configuration for Claude Code / OpenCode / DeepSeek Harness, tool tables, configuration table,
  architecture, and limitations.
- [x] Write `BUILDING.md`: Windows/Linux/macOS native builds, cross-compilation, CI matrix, bundle
  packaging, verification commands.
- [x] Write `dist/INSTALL.md` with the exact package version.
- [x] Write `dist/install.ps1` and `dist/mcp-config.example.json`; never include the archive,
  extracted text, or a live credential.
- [x] Build the release binary with `cargo build --release`.
- [x] Generate `dist/SHA256SUMS.txt`.
- [x] Add `tests/dist_bundle.rs` guards for version and absence of data.
- [x] Verify the bundled binary: `--version`, `--help`, and a piped `initialize` + `tools/list`
  + `search_dictionary` for `ST-36`.
- [x] Confirm every model-facing surface agrees: tool descriptions, `SERVER_INSTRUCTIONS`,
  `--help`, README tables, error messages.
- [x] Create the private GitHub repository and publish the `v0.1.0` release.
  - Repository: https://github.com/jangkateri/mcp-chinese-dict-search (private)
  - Release: https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.1.0
  - Assets: `mcp-chinese-dict-search-0.1.0-windows-x64.zip`, `SHA256SUMS.txt`
- [x] Update this checklist; v0.1.0 released.

---

## Phase 6.1 — EPUB verification and corrections

**Exit condition:** every `a1…` id and every Markdown entry has been compared with the exact EPUB;
source omissions are either corrected or documented; regression tests cover the corrections.

- [x] Extract the EPUB read-only into `data/epub-verify-2026-11-24/`; never execute its contents.
  - Evidence: 62 files, 32 MB extracted; `mimetype`, `container.xml`, `content.opf` inspected.
- [x] Record metadata and reading order: ASIN `B0GTMZRK1L`, Paradigm Publications, 2026-03-23,
  41 spine items, 41 XHTML documents, 10 images, 5 fonts.
- [x] Map all 5,869 `a1…` ids: 5,854 on `div`, 15 on `p`; classify the 47 ids absent from the
  index (4 entry containers, 26 TOC letters, 14 section paragraphs, 3 introduction sections).
- [x] Verify every index anchor exists in the EPUB (5,822 of 5,822) and that every index term agrees
  with the EPUB entry headword (0 disagreements).
- [x] Verify entry boundaries and text: 5,803 Markdown entries equal 5,803 EPUB entries; normalized
  text matches except 3 Markdown-only `PATTERNS` labels and 2 duplicate-alignment artifacts.
- [x] Verify appendices and artifacts: point-ID sets equal (403), medicinals 572, formulas 261,
  weights 3 sections; both `No Match` artifacts; 24 duplicate headwords; 9 inline images match.
- [x] Resolve all 13 remaining links: 5 to four index-omitted entry anchors, 8 to `a1SSD`, a
  paragraph id inside `wind stroke`.
- [x] Add the EPUB-verified correction table and map `a1SSD` to `wind stroke`.
- [x] Fix duplicate-headword anchor assignment: index sub-anchors are now assigned positionally
  (both `qi4` senses and all 24 duplicate groups get distinct anchors).
- [x] Add unit and real-archive regression tests; keep all 13 links resolved and report 4 anchorless
  entries.
- [x] Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets`; update `PLAN.md`, `README.md`, and this checklist.
- [x] Publish v0.1.1 with the rebuilt Windows bundle and `SHA256SUMS.txt`.

---

## Phase 6.2 — Actionable citation anchors

**Exit condition:** a citation tag can be turned back into its entry text, and the rendered answer
says how.

- [x] Accept `a1XXX` citation anchors in `get_entry`, case-insensitively, before headword lookup.
- [x] End `get_entry` output with `To reopen this citation, ask: tcm citation <tag>`.
- [x] End search output with the citation prompt when any hit has an anchor.
- [x] Add the `tcm citation <anchor>` instruction to `SERVER_INSTRUCTIONS`.
- [x] Accept anchors in `search_dictionary`; return exactly one hit with the full entry text
  (up to 20,000 characters) so a citation tag can be fed straight back into search.
- [x] Require the model to resolve every cited anchor with `get_entry`; never leave a bare tag.
- [x] Test `a1TAK` → `qi4` and `a1VJ4` → `Qi4`; update docs, bump to 0.1.3, and rebuild the
  Windows bundle.

---

## Phase 7 — Enhancements

Phase 7A is implemented in the working tree; Phase 7B remains deferred.

### Phase 7A — Pinyin and Chinese lookup

- [x] Generate a tone-numbered pinyin index (`qi4`, `ma2 huang2`) and a toneless index (`qi`,
  `ma huang`). Persist them in a title + content-fingerprint cache and reuse the cache on later
  startups; rebuild only when the dictionary or cache schema changes.
- [x] Generate a simplified/traditional-Chinese index (`气`, `氣`, `麻黄`, `麻黃`), including
  appendix simplified/traditional forms and Han alias runs such as `下陵`.
- [x] Normalize tone marks to tone numbers, repair the source's `u9` spelling of `ü` (`nu94` →
  `nu4`), and fold `v`/`ü` to `u`.
- [x] `search_dictionary` answers exact tone-numbered pinyin and Han queries from the index;
  toneless pinyin exact hits rank first while text search continues.
- [x] `get_entry` resolves pinyin and Han terms through the same index and prefers a non-redirect
  pinyin-like headword for ambiguous toneless queries.
- [x] Tests: `qi`/`qi4`/`qì`/`qi2`/`qí`, `气`/`氣`, `麻黄`/`麻黃`, `qīng rè`/`qing1 re4`,
  `zu3 san1 li3`, `足三里`, and `下陵`.
- [x] Add `--build-indices`: rebuild the pinyin and Han indices, replace the cache file for the
  current fingerprint, report the key counts and cache path on stderr, and exit without serving
  stdio (exit 1 when the cache cannot be written).
  - Verification: `tests/lookup_cache.rs::rebuilding_replaces_a_current_cache_and_stays_reusable`
    and `tests/stdio_contract.rs::build_indices_writes_the_cache_and_exits_without_serving`, plus a
    real-archive run that prints `built … keys … cache: …` and leaves a cache the next startup hits.
- [x] Update tool descriptions, server instructions, README, and PLAN.

### Phase 7B — Deferred

- [-] Add additional Chinese-medicine books and a research/RAG or small-language-model mode.

### Phase 7C — Entry resources and the citation prompt

- [x] Publish `dictionary://about` and `dictionary://entry/{reference}` through `resources/list`,
  `resources/templates/list`, and `resources/read`, with percent-decoding and `-32002` for an
  unknown URI or a reference naming no entry. The text is the same `get_entry` renders.
- [x] Publish the `citation` prompt through `prompts/list` and `prompts/get`, with a required
  single-token `reference` argument, resolving to the rendered entry as a `user` message;
  `-32602` for an unknown prompt, a missing argument, or a reference naming no entry.
- [x] Advertise `resources` and `prompts` in `initialize`, and name both surfaces in
  `SERVER_INSTRUCTIONS` and the unknown-method message.
  - Rationale: DeepSeek Harness supports resources but states MCP prompt templates are
    unsupported, so prompts alone would not help there; no client turns a hyperlink into a tool
    call.
  - Verification: `src/resources.rs`, `src/prompts.rs`, and `src/server.rs` unit tests;
    `tests/stdio_contract.rs::resources_and_the_citation_prompt_serve_fixture_entries`; real-archive
    probes below.
- [x] Update README, PLAN §8.4–8.5, and the crate module list in `src/lib.rs`.
- [x] Cite appendix 4 by the file name discovered in the archive instead of a hard-coded
  `37-appendix-4-Weights-Measurements.md`: `Section` now carries `file`, the weights search hit and
  the `get_entry` citation both use it, and `WEIGHTS_FILE` is gone.
  - Verification: `src/dictionary/search.rs::weights_cite_the_discovered_file_name`; appendix
    parser tests assert the carried name; a real-archive run from an extracted copy whose file was
    renamed to `37-appendix-4.md` cited `the reference dictionary,
    37-appendix-4.md:21` and the weights search hit reported `37-appendix-4.md`.

### Phase 7D — Clickable citation pages

- [x] Add `--pages-dir <path>` / `MCP_CHINESE_DICT_PAGES_DIR`: `get_entry` writes a Markdown page
  per result (`<source>-<id>.md`) and the result ends with `Entry page: <absolute path>` and
  `Clickable link: [open the entry](file:///…)`. The bare path is what VS Code's terminal opens in
  the editor; the link covers OSC 8 terminals.
- [x] Degrade safely: an unwritable page appends a bracketed note and still returns the entry; a
  page directory containing a space logs a warning at startup.
  - Verification: `src/pages.rs` unit tests (`page_keys_are_stable_per_source_and_identifier`,
    `the_link_percent_encodes_spaces_and_keeps_drive_letters`,
    `writing_a_page_names_it_in_the_result`, `a_write_failure_degrades_to_a_note`) and
    `tests/stdio_contract.rs::pages_dir_writes_a_clickable_entry_page`.
- [x] Document the mechanism and its VS Code rationale in README ("Clickable citations") and PLAN
  §8.6, and list `pages` in `src/lib.rs`.

### Phase 7E — Output-schema contract

Discovered when OpenCode rejected every `tools/call`: the hand-written `outputSchema` still
described the pre-0.4.0 payload and declared optional fields as non-nullable `string`. Claude Code
does not validate `structuredContent`, so only a strict client exposed the drift.

- [x] Derive each tool's `outputSchema` from the serialized response type with `schemars`
  (`src/schema.rs`), inlining subschemas and narrowing the result to the documented keyword
  subset. Hand-written output schemas are gone.
- [x] Align the payload: `suggestions`, `source_file`, and `line` are declared; optional fields use
  `skip_serializing_if = "Option::is_none"` so no `null` reaches a concretely typed property.
  - Verification: generated schema for `search_dictionary` declares `suggestions` and hit
    `source_file`/`line`; `get_entry` declares `source_file`/`line`; no `$ref`, `$defs`, `anyOf`,
    or `null` type remains.
- [x] Add `tests/schema_contract.rs`: run real stdio calls, validate each `structuredContent`
  against the advertised schema with `jsonschema`, and prove the validator rejects an undeclared
  field.
- [x] Snapshot `tools/list` in `tests/fixtures/tools-list.json`; the test compares the live list
  and reports how to accept an intentional change
  (`MCP_UPDATE_TOOLS_LIST=1 cargo test --test schema_contract`).
- [x] Record the invariant in `CLAUDE.md`: output schemas are derived, never hand-edited; the
  surfaces list and completion gates now name the contract and the snapshot.
  - Verification: `cargo test --all-targets` (122 passed) and
    `cargo clippy --all-targets -- -D warnings` (clean).

### Phase 7F — Graph traversal and `related_entries`

Tracked in detail by [`ENHANCEMENT-CHECKLIST.md`](ENHANCEMENT-CHECKLIST.md) phases G1–G2.

- [x] Build outgoing/incoming adjacency from resolved cross-references, including subentry anchors;
  expose `related_entries` with direction, depth 1–3, limit 1–50, deterministic BFS, citations, a
  derived output schema, and a reviewed `tools/list` snapshot.
  - Verification: `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`,
    `cargo test --all-targets` (108 unit + 27 integration), and a real-data graph smoke
    (`qi` both depth 1: 28 reachable; depth 2: 144; 5,803 nodes, 8,275 edges).

### Phase 7H — Appendix networks

Tracked in detail by [`ENHANCEMENT-CHECKLIST.md`](ENHANCEMENT-CHECKLIST.md) phase A1.

- [x] Add formula-family, formula-medicinal, and named point-combination networks with
  `related_network` and `render_network`.
  - Verification: unit tests for formula prose, ingredient lines, point grouping/boundary matching;
    schema/snapshot/e2e; real-data Four Gentlemen and LU-9 checks; fmt/clippy/all tests.

### Phase 7G — Graph PNG rendering and `render_graph`

Tracked in detail by [`ENHANCEMENT-CHECKLIST.md`](ENHANCEMENT-CHECKLIST.md) phase I1.

- [x] Render graph neighborhoods as PNG with a layered layout, labels, direction colours, an
  absolute path, and an MCP image block.
  - Verification: `cargo test --lib graph_image`, `cargo test --test tools_e2e render_graph`,
    schema contract/snapshot, clippy, and a real `spleen` graph image.

### Phase 7I — Optional gated semantic retrieval

Tracked in detail by [`ENHANCEMENT-CHECKLIST.md`](ENHANCEMENT-CHECKLIST.md) phases S0–S4.

- [x] Add the `semantic` Cargo feature and loopback-only Ollama client, digest-keyed flat index,
  300-point lexical gate, RRF fusion, lifecycle commands, and local evaluation harness. The feature
  is now in the default build; the server remains lexical until `--embedding-model` is supplied,
  and `--no-default-features` gives the minimal lexical-only binary.
  - Verification: hermetic and mock-Ollama tests; real `embeddinggemma:latest` index
    (7,042 records, 768 dimensions, 20.66 MiB); 30-query hybrid recall@5 0.567 vs lexical 0.233,
    MRR 0.455 vs 0.183, added p95 75 ms, MCP RSS +20.66 MB; exact/pinyin/Chinese regression
    top-1 unchanged. Decision: accepted with documented limits.

---

## Progress log

Append newest entries at the bottom. Include date, task, command/evidence, and result.

### 2026-11-24 — Phase 0 complete

- Read the specification and the reference project.
- Inventory: 39 Markdown files, 7,339,349 bytes; 10 referenced images; UTF-8/CRLF.
- Main dictionary: 5,803 entries across 26 files; 22 duplicate raw headwords; 395 multi-alias
  entries; 2 without aliases; 1 empty.
- Links: 5,822 index links / 5,795 anchors; 14,155 internal links; 14,142 resolved through the
  index (13 unresolved); 0 unresolved index terms after normalization.
- Appendices: 403 points / 15 sections; 572 medicinals / 20 sections; 261 formulas / 21 sections;
  3 weights/measures sections. The initial probe reported 571/250 because its regex rejected a
  trailing-space ID and eleven `04a.`/`05a.` IDs; the parser and golden tests use 572/261.
- Header grammar coverage: 6,352/6,352 main aliases; 403/403 appendix-1; 572/572 appendix-2;
  261/261 appendix-3 with tolerant parsing rules.
- Created `PLAN.md`, `CHECKLIST.md`, and `CLAUDE.md`.
- One-time evidence scripts `inspect_dict.py`, `inspect_appendix.py`, `inspect_entries.py`,
  `validate_rules.py`, and `validate_headers.py` were run with `python -I` against the archive.
  They were removed after verification; recover them from git history before commit `1f54ede`.
- Anchor-origin follow-up: all 14,225 fragment links use `a1` + three base-36 characters; the
  5,869 distinct values form a dense serial allocation (5,075 adjacent gaps of 1); CSS class IDs
  (`s1NTM`, `s1P94`, …) interleave exactly with them, proving a shared element-ID allocator;
  no hash or dictionary-order correlation was found. Conclusion recorded in `PLAN.md` §3.3.1:
  treat anchors as opaque source-EPUB foreign keys, resolved only through `38-index.md`.
- External verification was pending at this point; the EPUB was transferred and verified later the
  same day (see the Phase 6.1 entry below).

### 2026-11-24 — Phases 1–4 implementation

- Initialized the Git repository; `.gitignore` excludes the archive, extracted directory,
  `target/`, logs, and credentials.
- Created the Cargo project, module tree, protocol/transport/server layers, configuration, data
  model, parsers, index resolver, search engine, and both MCP tools.
- Loaded the real archive: 5,803 main entries, 403 points, 572 medicinals, 261 formulas,
  3 weights sections; 5,795 anchors; 8,270 entry cross-references resolved, 13 unresolved,
  8 anchorless entries, 24 duplicate normalized headwords, 0 alias/header failures.
- Added synthetic fixtures, `tests/stdio_contract.rs`, `tests/tools_e2e.rs`, and
  `tests/real_archive.rs`.
- No initial commit was created; the workspace policy is not to commit unless the user asks.

### 2026-11-24 — Phase 5 validation

- `cargo fmt --check` passes; `cargo clippy --all-targets -- -D warnings` passes.
- `cargo test --all-targets` passes: 53 unit + 1 real-archive + 4 stdio contract + 4 tool e2e
  + 5 dist guards = 67 tests.
- Release load: 405 ms. Debug load: 2.6–2.7 s. Tool calls: 6–86 ms (several concurrent calls).
- Manual stdio session verified `initialize` → `search_dictionary` → `get_entry` for
  `abdominal fullness`, `ST-36`, `ma huang`, `Ephedra Decoction`, and `liang`.
- `git check-ignore` confirms the archive, extracted directory, and `target/` are ignored.

### 2026-11-24 — Phase 6 complete

- Wrote `README.md`, `BUILDING.md`, `LICENSE`, `dist/INSTALL.md`, `dist/install.ps1`, and
  `dist/mcp-config.example.json`.
- Rebuilt the release binary, refreshed `dist/SHA256SUMS.txt`, and re-ran the distribution tests.
- Verified `dist/mcp-chinese-dict-search.exe`: `--version` reports `0.1.0`; a piped
  `initialize` + `tools/list` + `search_dictionary("ST-36")` session returns both tools and the
  `Leg Three Lǐ` point.
- The initial Git commit and the private-repository release are recorded below.

### 2026-11-24 — Private repository and v0.1.0 release

- Initial commit `30c440b` on `main`; 59 files, 9,634 insertions.
- Created the private GitHub repository
  https://github.com/jangkateri/mcp-chinese-dict-search. The first push failed because Git used
  SSH and `github.com` did not resolve for SSH; switched the remote to HTTPS and used the
  authenticated `gh` credential helper. `main` is the default branch.
- Release `v0.1.0`:
  https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.1.0
  with the Windows x64 bundle and `SHA256SUMS.txt`.
- `git ls-files` confirms the dictionary archive, extracted directory, EPUB, local
  `DictionarySetupMCP.txt`, and release zip are not tracked.

### 2026-11-24 — Phase 6.1, EPUB verification and corrections

- Received the reference EPUB (10,856,521 bytes; ASIN
  `B0GTMZRK1L`). Extracted read-only to `data/epub-verify-2026-11-24/`; used a one-time
  `verify_epub.py` script against the extracted Markdown directory and the EPUB. The script was
  removed after verification; recover it from git history before commit `1f54ede`.
- EPUB has 5,869 `a1…` ids (5,854 on `div`, 15 on `p`); all 5,822 index anchors exist. 47 ids are
  absent from the index: 4 entry containers, 26 TOC letters, 14 section paragraphs, 3
  introduction sections.
- Entries match one-for-one (5,803 each). Text matches after normalization except 3 Markdown-only
  `PATTERNS` labels and 2 duplicate-alignment artifacts. Appendices match exactly (403/572/261/3);
  both `No Match` artifacts and 24 duplicate headwords match.
- Added `MISSING_ENTRY_ANCHORS` (`a1UN9`, `a1PDE`, `a1PB6`, `a1R3X`) and `SUBENTRY_TARGETS`
  (`a1SSD` → `wind stroke`) in `src/dictionary/index.rs`; `a1SSD` is a paragraph id inside the
  `wind stroke` entry.
- Found and fixed the duplicate-headword bug: `term_anchors` stored one anchor per term, so both
  `qi4` senses (and 23 other duplicate groups) shared the first anchor. It now stores a vector and
  assigns anchors in entry order.
- Load report after the fixes: 5,795 index anchors, 4 recovered entry anchors, 1 recovered link
  target, 8,283 resolved links, 0 unresolved, 4 anchorless entries, 24 duplicate headwords,
  0 alias/header failures; 393 ms release load.
- `cargo fmt --check` passes; `cargo clippy --all-targets -- -D warnings` passes;
  `cargo test --all-targets` passes 55 unit + 5 dist + 1 real-archive + 4 stdio + 4 e2e = 69 tests.
- Updated `README.md`, `PLAN.md` §3.3.1, and this checklist.

### 2026-11-24 — v0.1.1 release

- Code commit `eeb3f33` on `main`.
- Release: https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.1.1
- Assets: `mcp-chinese-dict-search-0.1.1-windows-x64.zip` (684,361 bytes, SHA-256
  `86e96910226d893d9215e05c6ddf6291376c2b34b92f3fcae175c91e2b8d89d1`) and
  `SHA256SUMS.txt`.
- Bundled executable reports `0.1.1`; a piped `initialize` + `tools/list` + `get_entry
  ("provisioning")` session cites `23-p.md#a1UN9`.

### 2026-11-24 — v0.1.2 release

- Code commit `11d16b8` on `main`.
- Release: https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.1.2
- Assets: `mcp-chinese-dict-search-0.1.2-windows-x64.zip` (685,247 bytes, SHA-256
  `726438295c2654efb26cb7b125f915e3f7d1b2e81a890e2889ed5952a24dcd6e`) and
  `SHA256SUMS.txt`.
- Bundled executable reports `0.1.2`; `get_entry("a1TAK")` returns `qi4` with the citation and the
  `tcm citation a1TAK` prompt.

### 2026-11-24 — Phase 7A pinyin and Chinese lookup

- Added `src/dictionary/pinyin.rs`: tone marks → tone numbers (`qì` → `qi4`, `má huáng` →
  `ma2 huang2`), toneless keys (`qi`, `ma huang`), `u9` repair (`nu94` → `nu4`), `v`/`ü` → `u`,
  and Han keys/runs.
- Added `src/dictionary/lookup.rs`: generated exact tone-numbered, toneless, and Han maps for main
  aliases/headwords and appendix pinyin, simplified/traditional forms, and Han alias runs.
- Added a fingerprinted cache:
  `<dictionary title>.lookup-cache-v1.<fingerprint>.json` (2,027,916 bytes for the real archive).
  The fingerprint is FNV-1a over the sorted Markdown names and contents; a hit also requires the
  cache version and package version to match. A changed dictionary rebuilds and prunes the old
  fingerprint file. Default location is beside the dictionary; `--cache-dir` /
  `MCP_CHINESE_DICT_CACHE_DIR` override it. The cache is git-ignored and never shipped in `dist/`.
- Load report: 7,763 tone-numbered keys, 12,282 toneless keys, 13,730 Han keys; release load
  527 ms cold, 438–469 ms warm (cache hit).
- `search_dictionary` and `get_entry` use the indices. Verified over stdio:
  - `qi` → `qi4` (`a1TAK`), then `Qi4` (`a1VJ4`);
  - `qi2` → `Qi4` (`a1VJ4`), `umbilicus`, `navel`; not the `qi4` air/energy sense;
  - `氣` and `气` → `qi4`; `qīng rè` and `qing1 re4` → `clearing heat`;
  - `麻黄` and `麻黃` → medicinal `ephedra` (`01.01.01`);
  - `zu3 san1 li3`, `足三里`, and `下陵` → `ST-36`.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo test --all-targets` → 66 unit + 16 integration tests pass.
- Included in the v0.2.0 release, together with the citation-search work staged after v0.1.2.

### 2026-11-24 — v0.2.0 release

- Code commit `51e35e6` on `main`.
- Release: https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.2.0
- Assets: `mcp-chinese-dict-search-0.2.0-windows-x64.zip` (730,513 bytes, SHA-256
  `645216903b6c3d76f39aaa29fb07fe1850da255cc718fb406049685d5d7afefa`) and `SHA256SUMS.txt`.
- Bundled executable reports `0.2.0`; cache hit verified across repeated startups; pinyin/Chinese
  lookups verified over stdio.
- Gates: 66 unit + 16 integration tests; `cargo fmt --check`; `cargo clippy --all-targets -- -D
  warnings`.

### 2026-11-25 — `--build-indices`

- Added `--build-indices`: loads the dictionary, rebuilds the tone-numbered, toneless, and Han
  maps, replaces the cache file for the current fingerprint, logs the key counts and cache path to
  stderr, and exits without touching stdin. `--help`/`--version` still win.
- Plumbed through `Options.build_indices` and `Dictionary::load_rebuilding_cache`; `save_cache`
  gained a `replace` flag so a deliberate rebuild overwrites a still-usable cache, while the
  ordinary startup path keeps its "another process won the race" behaviour. `LoadReport` gained
  `lookup_cache_ready` so the command can tell a written cache from a failed write.
- Real archive, release build:
  `--build-indices --data <archive> --cache-dir <temp>` → exit 0,
  `built 7763 pinyin tone keys, 12282 pinyin toneless keys, 13730 Han keys … in 254 ms`, cache
  2,027,916 bytes, stdout empty. Re-running it replaced the file; a normal startup afterwards logged
  `lookup cache: hit`. `--cache-dir` under a regular file → warning plus
  `could not write the lookup indices` and exit 1. Missing `--data` → exit 2.
- Surfaces: `src/config.rs` usage text, README configuration table + cache paragraph, PLAN §9
  configuration/startup/exit codes and the lookup-cache section, BUILDING "Verify a build".
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test --all-targets`
  → 67 unit + 18 integration tests pass (new: `build_indices_is_a_mode_flag`,
  `rebuilding_replaces_a_current_cache_and_stays_reusable`,
  `build_indices_writes_the_cache_and_exits_without_serving`);
  `MCP_CHINESE_DICT_DATA=<archive> cargo test --test real_archive` passes in 2.33 s.
- The `dist/` bundle is rebuilt at 0.3.0 with the flag as part of the release commit.

### 2026-11-25 — v0.3.0 release

- Code commit `abcc532` on `main`, tag `v0.3.0`.
- Release: https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.3.0
- Assets: `mcp-chinese-dict-search-0.3.0-windows-x64.zip` (724,331 bytes, SHA-256
  `ddf22a098b5f0fbc903f0a2c399f131c9d142fb19832e17a4a727cd7924b31ac`) and `SHA256SUMS.txt`.
- Version bumped to `0.3.0` in `Cargo.toml`, `Cargo.lock`, `dist/INSTALL.md`, `PLAN.md`,
  `BUILDING.md`, and this file; bundle refreshed (`dist/mcp-chinese-dict-search.exe` reports
  `0.3.0`, `--help` lists `--build-indices`) and `dist/SHA256SUMS.txt` regenerated.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 67 unit + 18 integration tests.
- Release-binary checks on the real archive: `--build-indices` → exit 0, 7,763 tone-numbered /
  12,282 toneless / 13,730 Han keys, cache 2,027,916 bytes, stdout empty; next startup
  `lookup cache: hit`; unwritable cache dir → exit 1; missing `--data` → exit 2.
- Note: the first commit message carried a stray `@` from a shell quoting mismatch; it was fixed
  with an amend and both `main` and the tag were force-pushed before the release existed.

### 2026-11-25 — Phase 7C entry resources and the citation prompt

- Added `src/resources.rs`: `dictionary://about` (name/version, measured counts, tools, URI scheme)
  and `dictionary://entry/<reference>` (anchor, headword, pinyin, or Han; percent-decoded), served
  through `resources/list`, `resources/templates/list` (`dictionary://entry/{reference}`), and
  `resources/read` as one `text/markdown` block identical to `get_entry`'s text.
- Added `src/prompts.rs`: the `citation` prompt with a required single-token `reference`; the
  rendered entry arrives as a `user` message. Rationale: DeepSeek Harness supports resources but
  states MCP prompt templates are unsupported, and no client turns a hyperlink into a tool call.
- `initialize` now advertises `resources` (no subscribe, no listChanged) and `prompts`;
  `SERVER_INSTRUCTIONS` and the unknown-method message name both. `-32002`
  (`RESOURCE_NOT_FOUND`) is the new stable code for an unknown URI or an entry-less reference.
- Real archive, release binary, one stdio session: capabilities carried `tools`, `resources`, and
  `prompts`; `resources/list` → `dictionary://about`; templates → `dictionary://entry/{reference}`;
  reads resolved `a1TAK` → `qi4` (`24-q.md#a1TAK`), `%E6%B0%94` → `qi4`, `ma2%20huang2` →
  `ephedra`; `dictionary://bogus` → `-32002`; `prompts/list` → `citation`; `prompts/get` with
  `a1TAK` returned the entry text; an empty `arguments` object → `-32602`.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 79 unit + 19 integration tests.
- `dist/` and the 0.3.0 release asset are unchanged; the bundle gains both surfaces at the next
  release.

### 2026-11-25 — Weights citations follow the archive

- `Section` (`src/dictionary/model.rs`) gained `file`; `appendix::parse_weights` takes the source
  file name and sets it; `Dictionary::load_impl` passes the discovered name through.
- `src/dictionary/search.rs` no longer hard-codes `WEIGHTS_FILE`: the weights `SearchHit.file` and
  the `get_entry` citation both use `Section::file`, matching how the other three appendices cite
  `AppendixEntry.file`.
- Real archive, release binary, extracted copy with `37-appendix-4-Weights-Measurements.md`
  renamed to `37-appendix-4.md`: `get_entry(term="liang", source="weights")` cited
  `the reference dictionary, 37-appendix-4.md:21` and
  `search_dictionary(query="liang", scope="weights")` reported `file: 37-appendix-4.md`. The
  unmodified archive still cites `37-appendix-4-Weights-Measurements.md:21`.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 80 unit + 19 integration tests.
- PLAN documents the rule (appendix-4 file names are never hard-coded). `dist/` and the v0.3.0
  release asset are unchanged.

### 2026-11-25 — Phase 7D clickable citation pages

- Added `--pages-dir <path>` / `MCP_CHINESE_DICT_PAGES_DIR`: `get_entry` writes `<source>-<id>.md`
  into that directory and appends `Entry page: <absolute path>` plus
  `Clickable link: [open the entry](file:///…)`. The bare path is what VS Code's terminal opens in
  the editor; the link covers OSC 8 terminals.
- Pages use `render_entry_page` (the tool text without the reopen hint) plus a generated-file
  footer. When the caller's `max_chars` truncated the entry, the page is re-read at the default
  20,000-character bound, so a clicked page always holds the full entry.
- `Pages` resolves the directory with `std::path::absolute`, creates it at startup, and
  percent-encodes the `file://` URL. A directory containing a space logs a warning; a failed write
  appends `[citation page could not be written: …]` and still returns the entry.
- Real archive, release binary: `get_entry("ma huang tang", max_chars=300)` returned truncated tool
  text and a 2,448-character page holding the full 2,113-character entry (tail present, no
  truncation note). `--pages-dir "…/Temp/spaced pages"` logged the space warning.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 84 unit + 20 integration tests.
- Surfaces: README configuration row and "Clickable citations", PLAN §8.6 and §9, CLAUDE.md
  description and architecture, `src/lib.rs` module list. `dist/` and the v0.3.0 release asset are
  unchanged.

### 2026-11-25 — Citation pages link their cross-references

- Symptom: in VS Code's Markdown preview of a generated page, the source citation was resolved as a
  relative link against the page's folder, failed, and fell back to `https://10-c.md/#a1RMS`. The
  built-in preview also ignores `file:///` links (microsoft/vscode#182870), so absolute URLs were
  not a fix.
- `get_entry` now writes a page for every resolved cross-reference as well (looked up by anchor at
  the default 20,000-character bound) and links it from the entry page:
  `- [clearing heat](dictionary-a1PV0.md)`. Target pages keep plain cross-references, so the walk
  stops after one hop, and only pages actually written are linked.
- The source citation on a page (header and footer) is now an inline code span, so no preview turns
  it into a dead link. Tool text is unchanged.
- Real archive (extracted folder, release build): `get_entry("constipation")` wrote the entry page
  plus nine cross-reference pages (`a1RRD`, `a1PT0`, `a1PF6`, `a1PHX`, `a1RMT`, `a1R9X`, `a1PZ8`,
  `a1RMU`, `a1RUN`); the page holds nine Markdown links, each resolving to a file in the same
  directory.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 85 unit + 20 integration tests. `pages_dir_writes_a_clickable_entry_page`
  asserts the link and the linked page's contents.
- Surfaces: README "Clickable citations", PLAN §8.6, `src/tools.rs`, `src/pages.rs`,
  `tests/stdio_contract.rs`. `dist/` and the v0.3.0 release asset are unchanged.

### 2026-11-25 — One-time linked data build

- Measured the conversion's loss: 39 Markdown documents, 5,803 headword lines, 14,226
  `file.md#a1XXXX` fragment links, and no document defining most of those ids (anchors survive only
  in `38-index.md`). A relative citation such as `10-c.md#a1RMS` therefore cannot be opened even
  when the file is found.
- Added `--build-linked-data <dir>`: reads every Markdown document from the zip or folder, inserts
  one `<a id="…"></a>` line before each entry's headword using the anchors already resolved from
  the index, writes the copy, re-loads it, and compares every headword, anchor, alias, and body
  plus the appendix and weights counts before reporting. `parse_entries` ignores standalone anchor
  marker lines, so entry text is unchanged.
- Real dictionary build into `C:\ProgramData\mcp-dict-data`: 39 files, 5,803 entries, 5,799 anchors
  inserted, 4 entries without an anchor; fragment links 14,147 resolved, 79 unresolved (37 table of
  contents, 27 index lines, 6 introduction, 8 into `wind stroke` paragraph `a1SSD`, 1 missing HTML
  file). Content mismatches: 0. Served from the copy: `get_entry("constipation")` cites
  `10-c.md#a1RMS` and the content is the unchanged 1,642 characters.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 89 unit + 20 integration tests, including the fixture build and a serve-from-copy
  session. Surfaces: README, PLAN §8.7 and §9, `src/config.rs` usage, `src/cli.rs`,
  `src/dictionary/linked.rs`, `src/dictionary/source.rs`, `src/dictionary/parse.rs`.

### 2026-11-25 — Absolute source lines in tool text

- The terminal still linkified the canonical `10-c.md#a1RMS` printed inside the tool result and
  resolved it against the working directory, so the click opened Quick Open. The data was not at
  fault; the rendered form was.
- With a directory source, the `get_entry` header is now
  `dictionary · Example Dictionary` followed by
  `Source: C:\ProgramData\mcp-dict-data\10-c.md:3819 · anchor a1RMS`, and search hits end with the
  same absolute file and line. `EntryResponse` and `SearchHit` carry `source_file` and `line`,
  filled from the absolute data root. Zip sources keep the canonical citation, and `citation` itself
  is unchanged in `structuredContent`.
- Pages keep the canonical citation as code: a Markdown preview resolves relative to the page, and
  code stops the fallback to `https://10-c.md/#a1RMS`.
- The cross-reference list was the last bare form in tool text. It now prints only the target
  anchor (`- difficult defecation → difficult defecation (anchor a1RRD)`), so a terminal has no dead
  path to open; the anchor goes back to `get_entry`. Verified on the real copy: zero
  `NN-x.md#anchor` substrings remain in the tool text.

### 2026-11-25 — Misspelling suggestions

- Added `src/dictionary/fuzzy.rs`: optimal string alignment distance, plus English term
  suggestions (headwords, appendix terms, weights titles; a single-word query may match one word of
  a multi-word term) and pinyin suggestions over toneless keys. Thresholds: distance 1 up to six
  characters, 2 above; queries under four characters or containing Han produce nothing.
- `search_dictionary` exposes `structuredContent.suggestions` and prints `Did you mean: …?` for
  empty or weak results (best score under 300). `get_entry` merges them after its prefix/substring
  suggestions in the not-found error.
- Found and fixed an index defect: every headword was indexed as a toneless pinyin key, so English
  headwords produced suggestions such as `constipation (constipation)`. Only pinyin-shaped
  headwords are indexed now; the cache version became 2, and the documentation names
  `.lookup-cache-v2.<fingerprint>.json`.
- Real dictionary checks: `ma huag` → `ma huang (ephedra)`, `constipaton` → `constipation` (plus
  pattern terms), `abdomnal fullness` (weak hits) → `abdominal fullness`; `xue4 ha` resolves through
  the ordinary prefix fallback to `xue4 han2` (blood cold) and so is not a suggestion case.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 95 unit + 22 integration tests, including fixture-level English and pinyin
  suggestions. Surfaces: README, PLAN §8.8, `src/dictionary/{fuzzy,search,lookup,model}.rs`,
  `src/tools.rs`, `tests/stdio_contract.rs`.
- Suggestions became `{text, term, page}` and, with `--pages-dir`, carry the absolute page written
  for their entry: `"ma huang (ephedra)" — C:\ProgramData\mcp-dict-pages\medicinal-01.01.01.md`.
  The page holds the full entry; weak search results and not-found errors both print it. Without a
  page directory the field stays `None` and the text is unchanged. `DictionaryError` gained
  `suggestion_terms` so the error result can be enriched without changing its wording.
- Real dictionary check: `get_entry("ma huag")` wrote `medicinal-01.01.01.md` (ephedra),
  `dictionary-a1S27.md`, and `dictionary-a1P6Y.md`; `search_dictionary("abdomnal fullness")`
  suggested `abdominal fullness — C:\ProgramData\mcp-dict-pages\dictionary-a1S5W.md`.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 95 unit + 23 integration tests.

### 2026-11-25 — Linked copy is preview-safe for escaped brackets

- Symptom: opening `35-appendix-2.md` in VS Code showed `\[01.01.01\] …` as a LaTeX block, hiding
  the monograph heading. The conversion escapes literal brackets, and VS Code's math rendering
  reads `\[` as display math. The copy had 833 leading monograph headings plus 2,210 inline
  escaped brackets.
- `--build-linked-data` now unescapes `\[`/`\]` throughout the copy. `monograph_line` accepts the
  escaped and unescaped forms; entry verification compares bodies after the same unescaping, so
  "content unchanged" is still proven. Server output is unaffected because `plain_text` already
  removed the escapes.
- Rebuilt the copy into `C:\ProgramData\mcp-dict-data`: 5,803 entries, 5,799 anchors, 14,147 of
  14,226 fragments resolved, zero escaped opening brackets left. Appendix entry `01.01.01` serves
  `ephedra` with source line 9.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 97 unit + 23 integration tests, including the fixture copy assertion and the
  unescaped-monograph parser test.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 90 unit + 20 integration tests. A unit test asserts the absolute header and the
  absence of the bare form, and that pages keep the canonical citation.

### 2026-11-25 — v0.4.0 release

- Code commit `88cf665` on `main`, tag `v0.4.0`.
- Release: https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.4.0
- Assets: `mcp-chinese-dict-search-0.4.0-windows-x64.zip` (770,202 bytes, SHA-256
  `aad94606a101db9e34b9e75f881108962e250d389cc9e02203bad59f0ae01f29`) and
  `SHA256SUMS.txt` (418 bytes).
- Version bumped to `0.4.0` in `Cargo.toml`, `Cargo.lock`, `PLAN.md`, `CHECKLIST.md`,
  `BUILDING.md`, and `dist/INSTALL.md`. `dist/mcp-chinese-dict-search.exe` was rebuilt
  (1,762,816 bytes), reports `0.4.0`, lists `--pages-dir` and `--build-linked-data` in `--help`, and
  `dist/SHA256SUMS.txt` was regenerated from the five bundle files.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 97 unit + 23 integration tests.
- Release notes cover the linked-data build (5,803 entries, 5,799 anchors, 14,147/14,226 fragment
  links, unescaped brackets), clickable pages and absolute source lines, resources and prompt, and
  pinyin/English suggestions with clickable pages (cache version 2).
- The installed `~/.claude.json` entry is repointed from the working-tree binary to
  `dist/mcp-chinese-dict-search.exe`; `--data C:/ProgramData/mcp-dict-data` and
  `--pages-dir C:/ProgramData/mcp-dict-pages` are unchanged.

### 2026-11-25 — Output-schema contract (Phase 7E)

- OpenCode rejected every `tools/call` with a schema mismatch while Claude Code accepted the same
  responses. The advertised `outputSchema` was stale: it omitted `suggestions`,
  `source_file`, and `line`, and typed optional fields as `string` although serde emitted `null`.
  With `additionalProperties: false`, every result failed a strict validator.
- `src/schema.rs` now derives each output schema from the response type with `schemars`
  (`inline_subschemas = true`, no meta-schema), then narrows it to the documented keyword subset.
  `src/tools.rs` calls it for both tools; the hand-written schemas are removed.
- Response types in `src/dictionary/model.rs` derive `JsonSchema`; optional fields use
  `skip_serializing_if = "Option::is_none"`, so the payload never carries `null`.
- `tests/schema_contract.rs` (new) validates real `structuredContent` from five calls against the
  advertised schema with `jsonschema`, confirms an undeclared field is rejected, and compares
  `tools/list` with `tests/fixtures/tools-list.json`.
- `CLAUDE.md` records the invariant and adds `src/schema.rs` and the snapshot to the surfaces list
  and completion gates.
- Gates: `cargo fmt`; `cargo clippy --all-targets -- -D warnings` clean; `cargo test
  --all-targets` → 97 unit + 25 integration = 122 passed, 0 failed.
- `dist/mcp-chinese-dict-search.exe` was rebuilt from this tree at version `0.4.0` (SHA-256
  `6f003065129e7a2bc59298e5d842238c524458103628267052f1cbc6d28efab3`) and
  `dist/SHA256SUMS.txt` regenerated. Both running server processes were stopped to replace the
  locked executable. (Superseded: the repository was reset on 2026-11-25 and all releases,
  including `v0.4.0`, were deleted.)

### 2026-11-25 — Test harness serves the linked copy

- Production serves the one-time linked copy (`--build-linked-data`), not the source archive, so
  the integration tests now exercise that artifact instead of the raw source fixture.
- `tests/support/mod.rs`: `fixture_source_dir()` is the source-style input; `fixture_dir()` builds
  the linked copy once per test process in the temp directory and `server_command()` points the
  server at it. `tests/lookup_cache.rs` copies the linked copy for the same reason.
- `tests/stdio_contract.rs`: the build test now names `fixture_source_dir()` explicitly, and a new
  test asserts the served copy contains `<a id="a1S5W"></a>` and `[01.01.01] MÁ HUÁNG` with no
  `\[`/`\]`.
- The fixture `.md` files themselves stay in source form (escaped brackets, no anchors): they are
  the input to the build, and the source parser must stay covered too. Committing a pre-fixed copy
  would add a second artifact that can go stale.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings` clean; `cargo test
  --all-targets` → 97 unit + 26 integration = 123 passed, 0 failed.

### 2026-11-25 — Book derivatives removed; corpus invented and generated

- The EPUB is the source work; the Markdown conversion is its derivative; the linked copy is a
  further derivative. The eight fixture `.md` files contained verbatim entry text, so they were
  removed from the index and the working tree, and `tests/fixtures/dictionary/` is now ignored.
- `tests/support/fixture.rs` holds an invented corpus — `alpha`, `alpha swelling`, `alpha alias`,
  `beta`, `clear heat`, `greater alpha`, point `AA-1`, medicinal `jia root`, formula
  `Jia Decoction`, a weights table, and an index with `a1ALP`…`a1CHT`. It is written to a
  temporary directory at test time, and the linked copy is built from it.
- Every integration assertion and unit-test sample was rewritten to that vocabulary; the
  user-facing tool descriptions and examples use `jia3` and `a1ALP` instead of book terms.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 97 unit + 26 integration = 123 passed, 0 failed.
- `dist/mcp-chinese-dict-search.exe` was rebuilt at `0.4.0` (SHA-256
  `b285d41c8e9e9b042adb8333cba85705506187a816765e676b4a1b3df89dc6d6`) and the checksums
  regenerated, so the shipped tool text also uses the invented examples.
- History purge: the local history was reset to the single root commit `5bc0a05`, all six tags
  and six GitHub releases were deleted, and the GitHub repository was deleted and recreated
  (private) to purge cached objects. The old commit `50303ab` and the old fixture path now return
  422/404; the local pre-rewrite bundle was deleted. The recreated repository has one branch
  (`main`), one commit, no tags, and no releases.

### 2026-11-25 — Space-free pinyin suggestions

- `src/dictionary/fuzzy.rs`: a query with no space is also compared against every multi-word key
  with its spaces removed, and that compact match ranks ahead of an equally distant shorter key.
  `mahuag` suggests `ma huang (ephedra)`; `mhuang` now includes it in the top five instead of only
  the shorter `huang` fragments.
- Tests: unit cases for `jiage`, `jigen`, and `jiagen`, a ranking case, and a `jiazhong` stdio case.
- Gates: `cargo fmt --check`; clippy clean; `cargo test --all-targets` → 99 unit + 26 integration =
  125 passed, 0 failed.
- `dist/mcp-chinese-dict-search.exe` rebuilt (SHA-256
  `67e8bde0afc951daa9169e3071c3a75317f56a416e3ff1abf947b55af6aa9505`) and checksums regenerated.

### 2026-11-25 — Agent citation instructions

- Added `AGENTS.md` for OpenCode and other agents: use the MCP tools for every dictionary question,
  never read the data directory directly, and copy the server's absolute `Source:` and
  `Entry page:` lines verbatim. Bare relative citations such as `35-appendix-2.md` resolve against
  the terminal's working directory and fall back to the command palette. README's “Keep
  model-written citations clickable” states the same for anyone using the server code, including
  the global instruction files needed outside this repository.

### 2026-11-25 — v0.4.1 release

- Version bumped to `0.4.1` in `Cargo.toml`, `Cargo.lock`, `BUILDING.md`, `PLAN.md`,
  `CHECKLIST.md`, and `dist/INSTALL.md`.
- `dist/mcp-chinese-dict-search.exe` rebuilt (1,874,944 bytes, SHA-256
  `96ad95a25c0ef89b3948fc94fb6edec589f0d27dda829e09125c788287c998a4`), reports `0.4.1`.
- Release asset `mcp-chinese-dict-search-0.4.1-windows-x64.zip` (821,889 bytes, SHA-256
  `a71c7c579a3f4c10319bcd98b3d95774cedb69319f6a1bbbe423ccb424450fde`) contains the executable,
  `install.ps1`, `INSTALL.md`, `mcp-config.example.json`, `LICENSE`, and `SHA256SUMS.txt`.
- Tag `v0.4.1` and GitHub release:
  https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.4.1
- Content: output schemas derived from the response types with a validating contract test and
  `tools/list` snapshot; space-free pinyin suggestions (`mahuag`, `mhuang`, `guizh`); invented
  test corpus with the book-derived fixtures removed; strengthened server instructions to copy
  absolute `Source:`/`Entry page:` lines; repository reset to a single commit.
- Gates: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`; `cargo test
  --all-targets` → 99 unit + 25 integration = 124 passed, 0 failed.

### 2026-11-25 — Dictionary-agnostic conversion (0.5.0 working tree)

- Citations, server instructions, resource and prompt text no longer name a dictionary. The title
  comes from `--title` / `MCP_CHINESE_DICT_TITLE`, then a linked copy's `dictionary-title.txt`
  sidecar, then the archive file name or directory name.
- `--data` / `MCP_CHINESE_DICT_DATA` is required; filename-based discovery and the book-named
  constants are removed. `--help` uses the generic title.
- `--build-linked-data` writes the title sidecar, so a linked copy keeps its source title.
- `tests/real_archive.rs` (book counts and headwords) was replaced by `tests/real_data.rs`, which
  runs only when `MCP_CHINESE_DICT_DATA` is set and asserts structural invariants plus the
  title-derived citation. Integration fixtures use the title `Test Dictionary`.
- README, BUILDING, PLAN, INSTALL, install.ps1, the config example, CLAUDE.md, and
  PROJECT-SETUP-PROMPT were generalized; personal reference-project paths were removed.
- Version is `0.5.0` (unreleased; `v0.4.1` remains the last release).
- Gates: `cargo fmt --check`; clippy clean; `cargo test --all-targets` → 100 unit + 26 integration =
  126 passed, 0 failed. `dist` rebuilt at 0.5.0 (SHA-256
  `f09a1f4c43519996091fe3bba45fbaa3924e7b8038f426485ba393980004a636`).

### 2026-11-25 — v0.5.0 release; repository published

- Version `0.5.0`; single root commit “Release v0.5.0”.
- Release asset `mcp-chinese-dict-search-0.5.0-windows-x64.zip` (822,548 bytes, SHA-256
  `61d2157e3dbbc201b9a2f46363b1adba68f73f058364792fe87e1fedadd2a70c`) contains the executable,
  `install.ps1`, `INSTALL.md`, `mcp-config.example.json`, `LICENSE`, and `SHA256SUMS.txt`.
- Tag `v0.5.0` and GitHub release:
  https://github.com/jangkateri/mcp-chinese-dict-search/releases/tag/v0.5.0
- The repository was deleted and recreated from the single generic commit to purge pre-0.5.0
  history, then made public. The `v0.4.1` tag and release no longer exist.
- Checks: no dictionary title, personal path, credential, or dictionary data in the tracked tree;
  100 unit + 26 integration tests pass.

### 2026-11-27 — Phase 7F graph traversal

- Added `src/dictionary/graph.rs`, `GraphRequest`/`GraphResponse`/`GraphNode`, and the
  `related_entries` tool. The graph is built from resolved links only; subentry anchors fall back
  to the containing entry via `(target_file, target_term)`.
- Added derived output schema, `tools/list` snapshot, e2e and schema-contract coverage, and updated
  README/PLAN/AGENTS/CLAUDE/INSTALL.
- Real data: 5,803 nodes, 8,275 edges, 8 unmapped links; `qi` both depth 1 = 28 reachable, depth
  2 = 144; 0 ms query time.
- Gates: `cargo fmt --check`; clippy clean; `cargo test --all-targets` → 108 unit + 27 integration
  = 135 passed, 0 failed.
- No commit made; the semantic enhancement remains separate.

### 2026-11-27 — Phase 7G graph PNG rendering

- Added `src/graph_image.rs` and the `render_graph` tool: layered PNG, labels, green outgoing /
  blue incoming edges, gold start node, image content block, absolute path, and structured graph
  metadata.
- Writes to `--pages-dir` when configured; otherwise the system temporary directory.
- Real `spleen` outgoing depth-2 image: 1000×1130 PNG.
- Gates: clippy clean; `cargo test --all-targets` → 110 unit + 28 integration = 138 passed.
- No commit made.

### 2026-11-27 — Phase 7H appendix networks

- Added `src/dictionary/network.rs`: formula-family edges from explicit prose, formula ingredient
  edges from `□` lines, and named point-combination records grouped by roles and indications.
- Added `related_network` and `render_network`; domain filtering and inverse relations keep the
  three views distinct.
- Real data: Four Gentlemen → Six/Eight; Four Gentlemen → 5 ingredient medicinals; source/network
  method → 11 combinations; LU-9 → its Lung combination.
- Gates: clippy clean; `cargo test --all-targets` → 114 unit + 29 integration = 143 passed.
- No commit made.

### 2026-11-27 — Point-combination parser fix

- Fixed multi-line combination lists: role-prefixed and continuation point lines stay in one
  group; short group headings start a new group; long indication sentences attach to the current
  group. This removes the shifted context/indication labels in local/distant and transport/alarm
  lists.
- Real data: 52 records with shifted context/indication labels → 44 correct groups; `LI-4` →
  Forehead, Eye, Nose, Mouth and Teeth, Throat, and Upper limbs.
- Gates: `cargo fmt --check`; clippy clean; `cargo test --all-targets` → 115 unit + 29 integration
  = 144 passed; `dist` checksums verify.
- Rebuilt `dist/mcp-chinese-dict-search.exe` (2,351,616 bytes; SHA-256
  `5926377f70e8a085f881239d6eab177d7b3ffbc20911778054e982c6a05a7c2a`).
- No commit made.

### 2026-11-27 — Semantic S0–S1 core

- S0: pinned `ureq` 3.5.2 behind the then-optional `semantic` feature; at that stage the default
  tree had no HTTP client. Added an environment-gated real-Ollama smoke test; it passed against
  `qwen3-embedding:0.6b` with finite unit vectors. Go decision recorded.
- S1: added the feature-gated `semantic` module with a fake embedder, deterministic document
  construction, a flat cosine index with a binary header and atomic save/load, digest-keyed cache
  paths, identity validation, the 300-point lexical gate, RRF fusion, and `MatchKind::Semantic`.
- Extracted `content_fingerprint(sources)` for both caches and exposed it on `Dictionary`.
- Gates: fmt; default and feature clippy with `-D warnings`; default tests 115 unit + 29
  integration = 144; feature tests 142 unit + 30 integration = 172 with local Ollama.
- At that stage no model or HTTP client was required by the default build or suite; the feature
  later became a default feature. No commit made.

### 2026-11-27 — Semantic S2–S3 implementation

- Added the loopback Ollama client, model profiles, batched index build, digest validation,
  `spawn_blocking` search path, digest re-checks, lifecycle flags, and the local evaluation
  harness.
- README, `dist/INSTALL.md`, `BUILDING.md`, `PLAN.md`, and the MCP configuration example document
  the optional feature, update procedure, withdrawal/rollback rule, and loopback-only boundary.
- Feature suite: 169 unit + 31 integration = 200 passed; both clippy configurations clean.
- Real lexical baseline on 30 local queries: recall@5 0.233, MRR 0.183, p50 149 ms, p95 185 ms.
- No commit made.

### 2026-11-27 — Phase 7I semantic evaluation and acceptance

- Built the real `embeddinggemma:latest` index: 7,042 documents, 768 dimensions, 20.66 MiB,
  816 s; `--check-semantic` reports `status: active`.
- 30-query English/mixed set: hybrid recall@5 0.5667 vs lexical 0.2333, MRR 0.4550 vs 0.1833,
  top-1 0.4000 vs 0.1333; added p95 75 ms; MCP RSS +20.66 MB; model + index 642 MB.
- Exact/pinyin/Chinese regression set: both modes 1.0 recall@5/MRR/top-1, zero top-1 changes.
- Optimization: fusion reuses one full ranked lexical result and resolves hits through hash maps,
  removing a second lexical search and O(candidates × entries) scans without changing metrics.
- `bge-m3` full-index comparison was stopped as impractical at 8K on this CPU; direct probes and
  build feasibility are recorded in `ENHANCEMENT-CHECKLIST.md` (S4.7).
- Decision: accepted with documented limits. Version bumped to 0.6.0; final gates and bundle
  remain.
- No commit made.

### 2026-11-27 — Post-release: semantic default and deployable evaluation CLI

- `semantic` is now a default Cargo feature (`default = ["semantic"]`). A plain
  `cargo build --release` and `dist` include it; the server stays lexical at runtime until
  `--embedding-model` is supplied. `--no-default-features` builds the minimal lexical-only binary.
- Rebuilt `dist` semantic-capable: 3,103,744 bytes, SHA-256
  `ca4a7a04b3bb5b26b48bebd829d94da452578dd3f8765dffe28e73d0b38e3736`. The shipped bundle validated
  the Qwen3 index (`status: active`) and answered the two semantic probes at rank 1.
- Consolidated the evaluation/diagnostic code into `tools/evaluate_semantic.py`, adding
  `--embedding-profile`, `--mode`, and `--per-query`; added `tools/README.md`; removed the local
  diagnostic scripts.
- Gates: default 170 unit + 31 integration; minimal 120 unit + 29 integration; clippy clean in
  both configurations; fmt clean.
- No push, tag, or release.

### 2026-11-27 — Scope-filtered appendix policy and Rust evaluator

- Semantic candidates are now filtered by the requested `scope` before top-k selection
  (`SemanticIndex::search_scoped`). Appendix scopes use conservative fusion: the lexical head is
  pinned and one slot is reserved for the best remaining semantic candidate by RRF. `All` and
  `Dictionary` keep the aggressive RRF behavior.
- Scoped 17-query point/formula probe: lexical 0.706 recall@5 / 0.510 MRR / 0.412 top-1;
  conservative hybrid **0.765 / 0.522 / 0.412**. The previous aggressive hybrid was 0.588 /
  0.520 / 0.471 and demoted correct scoped hits. `Inner Pass` entered at rank 5.
- Main 30-query Qwen3 set unchanged (0.6333 / 0.4289 / 0.3); regression set 1.0 across all
  metrics.
- Replaced the Python evaluation script with the Rust `evaluate-semantic` binary; `dist/` now
  contains `evaluate-semantic.exe` (1,803,264 bytes, SHA-256
  `11b0517b0ea913358c1c154ea44a8aa5b9192de101f3da46607428d526fb72f1`) and the policy-enabled
  server binary (3,110,400 bytes, SHA-256
  `cb5ef22f2e2907c70b3637eca0b35b0982dcfa8d5a0d118e19448b70320e7fa5`). The installer copies both.
- Gates: default 175 unit + 2 evaluator + 31 integration; minimal 120 unit + 29 integration;
  clippy clean in both; fmt clean.
- No push, tag, or release.

---

## Completed external input

### Reference EPUB

Status: **received and verified read-only on 2026-11-24.** Exact edition; unencrypted; 10,856,521
bytes. Extracted to `data/epub-verify-2026-11-24/` (ignored).

Results:

1. The EPUB contains 5,869 distinct `a1…` ids: 5,854 on `div` elements and 15 on `p` elements. All
   5,822 index anchors exist in the EPUB; 47 EPUB ids are absent from the index.
2. Its 5,803 entry containers match the Markdown one-for-one: 5,797 `class_sGC` divs with ids, four
   `class_sGC` divs without ids, one `class_sRD9` div, and one p-only headword (`abating heat`).
3. Four index-omitted entry anchors were added (`a1UN9`, `a1PDE`, `a1PB6`, `a1R3X`); `a1SSD` is a
   paragraph inside `wind stroke` and now resolves there. All 13 links resolve.
4. Four entries have no id in the EPUB and remain anchorless: `dancing of the arms and legs`,
   `food diarrhea2`, `provisioning aspect`, `provisioning, defense, qi4, and blood`.
5. Duplicate-headword assignment was wrong: all 24 duplicate groups shared one anchor. The resolver
   now assigns the index's numbered sub-anchors in entry order; tests cover both `qi4` senses.
6. Appendix counts and identifier sets match exactly (403 points, 572 medicinals, 261 formulas,
   3 weights sections); both `No Match` artifacts and all 24 duplicate headwords match.
7. Anchor allocation is opaque: ids are on entry containers, Spearman correlation with EPUB spine
   position is 0.010, and only 49.9 % of adjacent ids increase within a file.
8. Confirmed facts are recorded in `PLAN.md` §3.3.1 and the progress log below.

The EPUB remains a **verification oracle**, not a runtime dependency. The server still reads the
Markdown archive. The EPUB and its extracted XHTML/CSS are not committed.

---

## Decisions to confirm

Recommended defaults are in `PLAN.md` §16. Checked when the user confirmed or accepted the default;
the user's instruction to start executing the plan on 2026-11-24 accepted these defaults.

- [x] Data source supports both zip and extracted directory (recommended).
- [x] Tool surface: `search_dictionary`, `get_entry`, `related_entries`, `render_graph`,
  `related_network`, and `render_network`.
- [x] Appendix augmentation is query/headword/identifier driven, plus point IDs found in bodies
  (recommended).
- [x] `See …` redirects are followed automatically for one hop (recommended).
- [x] Defaults: 5 hits, 20,000 characters per entry (recommended).
- [x] Version `0.1.0` and Windows-first `dist/` bundle (recommended).
- [x] Git is initialized at Phase 1; the archive is never committed (recommended).
