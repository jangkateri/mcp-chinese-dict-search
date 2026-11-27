# Changelog

All notable changes to this project are documented here. The project follows
[Semantic Versioning](https://semver.org/).

## [0.6.0] — 2026-11-27

This release adds graph traversal, graph and network images, appendix networks,
and optional hybrid retrieval over a local Ollama embedding model. It also makes
semantic retrieval a default build feature, adds a Rust evaluation binary, and
fixes point-combination parsing, point-ID matching, and hybrid fusion latency.

### Added

- **`related_entries`** — deterministic breadth-first traversal of the dictionary's
  resolved cross-references in the outgoing, incoming, or both directions; depth
  1–3; limit 1–50; 10,000-node cap; `via` and link-text provenance; canonical
  citations; a derived output schema and a reviewed `tools/list` snapshot.
- **`render_graph`** — PNG rendering of a graph neighborhood with a layered
  layout, labels, direction colors, an absolute output path, and an MCP image
  content block.
- **`related_network`** — appendix relationships in three domains:
  formula-to-formula derivation, formula-to-medicinal composition, and named
  point combinations. Edges carry a relation and source line; domain filtering
  prevents formula-family edges from leaking into medicinal queries.
- **`render_network`** — PNG rendering of the three appendix network domains.
- **Optional semantic retrieval** — a loopback-only Ollama client, model
  profiles, digest-keyed flat cosine index, exact-cosine top-k search, gated
  hybrid fusion, and lifecycle commands:
  - `--ollama-url`, `--embedding-model`, `--embedding-profile`,
    `--semantic-index`, `--require-semantic`
  - `--build-semantic-index`, `--check-semantic`,
    `--prune-semantic-index`, `--yes`
  - matching environment variables
- **Semantic index lifecycle** — atomic save/load, dictionary-content and model
  identity validation, a 300-second digest re-check, automatic disable on a
  digest change, lexical fallback by default, and strict startup with
  `--require-semantic`.
- **Scope-filtered semantic retrieval** — semantic candidates honor the requested
  `scope` before top-k selection, so a `meridian` query cannot fuse a formula or
  dictionary hit.
- **Conservative appendix fusion** — for `meridian`, `medicinal`, `formulas`, and
  `weights`, the lexical head is pinned and one slot is reserved for the best
  remaining semantic candidate by RRF. `all` and `dictionary` keep aggressive
  RRF, where semantic retrieval is a clear gain.
- **Rust evaluation binary** — `evaluate-semantic` loads the dictionary and
  semantic index directly, supports lexical/hybrid modes, per-query scopes,
  embedding profiles, repetitions, and per-query rank, match-kind, score, gate,
  embedding-time, candidate, and fallback diagnostics.
- **Prebuilt-index documentation** — instructions for copying a semantic index
  between machines, including the model-digest, dictionary-content, server
  version, and embedding-profile checks.
- **Changelog** — this file.

### Changed

- The `semantic` Cargo feature is now a **default feature**. A plain
  `cargo build --release` produces a semantic-capable server and the Windows
  bundle includes it. Retrieval stays lexical until `--embedding-model` is
  supplied; `--no-default-features` produces the minimal lexical-only build with
  no HTTP client.
- The Windows bundle now includes `evaluate-semantic.exe`; `install.ps1` copies
  both executables.
- Semantic candidates are filtered by the requested scope before top-k, and
  appendix scopes use conservative fusion instead of global reranking.
- Fusion reuses one full ranked lexical result and resolves hits through
  per-fusion hash maps instead of a second lexical search and repeated
  entry scans.
- `--cache-dir` is documented and tested as the shared location for both the
  lookup cache and the semantic index.
- Documentation now covers semantic setup, evaluation, model comparison,
  index copying, rollback, pruning, and the default/minimal build split.
- `dist/INSTALL.md` now gives complete semantic arguments for Claude Code, OpenCode,
  and DeepSeek Harness: `--cache-dir`, `--embedding-model`, `--ollama-url`, and the
  Qwen3 `--embedding-profile`, plus the matching environment variables.

### Fixed

- **Point-combination parsing** — multi-line local/distant and transport/alarm
  lists now keep role-prefixed and continuation points in one group, distinguish
  short headings from indication sentences, and attach indications correctly.
  The real-data set changed from 52 shifted records to 44 correct groups; `LI-4`
  resolves to six intact groups: Forehead, Eye, Nose, Mouth and Teeth, Throat,
  and Upper limbs.
- **Point-ID boundary matching** — `LI-1` no longer matches inside `LI-16`;
  point identifiers are matched on identifier boundaries.
- **Hybrid fusion latency** — removed a second full lexical search and
  O(candidates × entries) hit-to-entry scans. On the 30-query evaluation the
  added p95 fell from roughly 250 ms to **75 ms** with identical recall, MRR, and
  top-1.
- **Scoped appendix regressions** — aggressive semantic fusion had demoted
  correct scoped point and formula hits (`Pass Head`, `Leg Three Lǐ`,
  `Great Ravine`, `Free Wanderer Powder`). The conservative policy preserves the
  lexical head; on the 17-query point/formula probe, scoped hybrid recall rose
  from 0.588 to **0.765** while lexical recall was 0.706.
- **Semantic index identity** — package version, dictionary fingerprint, model
  name, model digest, dimensions, and profile hash are validated before serving;
  a stale index produces an actionable rebuild message.
- **Prune UX** — `--prune-semantic-index` lists candidates by default and removes
  them only with `--yes`.
- **Check remediation** — `--check-semantic` includes the embedding profile in
  its rebuild command when one was supplied.

### Removed

- The Python evaluation script and the remaining one-time Python EPUB, anchor,
  appendix, and header inspection scripts. The repository now has one
  implementation language: Rust. The removed scripts remain recoverable from git
  history before commit `1f54ede`.
- The empty `tools/` documentation directory; evaluator documentation now lives
  in the main README.

### Security and data handling

- The semantic path accepts only loopback Ollama URLs (`127.0.0.1`, `localhost`,
  `::1`); remote hosts and Ollama Cloud are rejected.
- No dictionary archive, extracted Markdown, EPUB, image, model weight, lookup
  cache, semantic index, page, or local query set is committed or shipped.
- stdout carries JSON-RPC only; diagnostics go to stderr.

### Verification

- Default build: **175 library + 2 evaluator + 31 integration** tests pass;
  clippy and fmt clean.
- Minimal lexical-only build (`--no-default-features`): **120 library + 29
  integration** tests pass; clippy clean.
- Real semantic index: `embeddinggemma:latest`, 7,042 records, 768 dimensions,
  20.66 MiB, `status: active`; Qwen3 1K index, 7,042 records, 1,024 dimensions,
  27.54 MiB, `status: active`.
- EmbeddingGemma 30-query evaluation: hybrid recall@5 **0.567** vs lexical
  0.233; MRR **0.455** vs 0.183; top-1 **0.400** vs 0.133; added p95 75 ms; MCP
  RSS +20.66 MB.
- Qwen3 1K 30-query evaluation: recall@5 **0.633**, MRR **0.429**, top-1
  **0.300**; added p95 116 ms; build 76 minutes.
- Scoped point/formula probe: lexical 0.706 / 0.510 / 0.412; conservative hybrid
  **0.765 / 0.522 / 0.412**.
- Exact/pinyin/Chinese regression set: recall@5, MRR, and top-1 all **1.0** in
  both lexical and hybrid modes.

### Known limitations

- Semantic quality was measured on one corpus and one English/mixed query set;
  the numbers are local measurements, not a general quality claim.
- Qwen3 was evaluated at a 1K context override, not its 4K default; a full
  `bge-m3` 8K index build was not practical on the test CPU.
- Pure pinyin queries deliberately stay on the lexical/fuzzy path; embeddings
  are not discriminative for raw pinyin.
- Appendix semantic retrieval is conservative: it is strongest as a one-slot
  supplement to scoped lexical retrieval, not as a replacement.
- `related_entries` covers main dictionary entries only; appendix entries are
  served by `related_network`.
- Four entries have no anchor in either the index or the EPUB and cite
  `file:line` instead.

## [0.5.0] — 2026-11-25

- Dictionary-agnostic release: required `--data`, configurable display title,
  generic server text, lookup indices, and the reviewed output-schema contract.
- First public repository release after the history purge.
