# ENHANCEMENT CHECKLIST — graph traversal and gated hybrid retrieval

Live tracker for [`ENHANCEMENT-PLAN.md`](ENHANCEMENT-PLAN.md).

| Field | Value |
|---|---|
| Base | `0.5.0`, commit `dd684f2` |
| Plan | [`ENHANCEMENT-PLAN.md`](ENHANCEMENT-PLAN.md) |
| Last updated | 2026-11-27 |
| Overall status | All phases complete; release commit `e611d42`; post-release semantic-default change; no push made |
| Commit rule | **Do not commit until every phase is complete and verified.** One commit at the end; do not push unless asked. |
| Network rule | No remote network. Semantic mode may call only loopback Ollama; Ollama Cloud is excluded. |

## Status legend

| Mark | Meaning |
|---|---|
| `[ ]` | Not started |
| `[~]` | In progress |
| `[x]` | Complete **and verified** |
| `[!]` | Blocked; record the blocker and evidence in the progress log |
| `[-]` | Deferred or explicitly not in this phase |

## Rules for maintaining this checklist

1. Mark `[x]` only after the stated verification has run and passed. Compiling is not verifying.
2. Append the command, output summary, or measurement to the progress log in the same turn.
3. A phase is complete only when its exit condition in the plan is met.
4. If a task is added, removed, or split, update the summary counts in the same edit.
5. Never mark a task complete because a later task depends on it.
6. No git commit, amend, tag, push, or release happens before the final phase is verified.
7. Do not record copyrighted text, dictionary excerpts, model weights, indexes, or local query
   files in this checklist. Counts and invented examples only.
8. If the semantic evaluation fails its threshold, do not mark S4 complete by enabling the feature
   anyway. Record the negative result and follow the plan's rollback rule.
9. Model identity is name + digest. Never reuse vectors across a digest change; rebuild instead.

## Progress summary

| Phase | Scope | Tasks | Done | Status |
|---|---|---:|---:|---|
| E0 | Plan, checklist, guardrails | 5 | 5 | Complete |
| G1 | Graph data and traversal | 14 | 14 | Complete |
| G2 | `related_entries` tool | 11 | 11 | Complete |
| I1 | Graph PNG rendering | 8 | 8 | Complete |
| A1 | Appendix networks | 10 | 10 | Complete |
| S0 | Ollama capability gate | 8 | 8 | Complete |
| S1 | Semantic core with fake embedder | 16 | 16 | Complete |
| S2 | Ollama embedder, index, CLI | 12 | 12 | Complete |
| S3 | Model lifecycle and migration | 9 | 9 | Complete |
| S4 | Evaluation and acceptance decision | 8 | 8 | Complete |
| D | Docs, gates, bundle, one commit | 12 | 12 | Complete |
| **Total** | | **113** | **113** | **Complete** |

---

## Phase E0 — Plan, checklist, guardrails

**Exit condition:** both documents exist, cross-link, record the base and the no-commit rule, and
contain measurable gates.

- [x] Create `ENHANCEMENT-PLAN.md` with graph and gated-hybrid design.
- [x] Create `ENHANCEMENT-CHECKLIST.md` with phase tasks and verification rules.
- [x] Record the base commit `dd684f2` / version `0.5.0`.
- [x] Record the no-commit-until-complete rule and the one-commit-at-the-end rule.
- [x] Verify both documents exist and link to each other and to `PLAN.md` / `CLAUDE.md`.

Evidence: files written 2026-11-27; links and task counts checked with a repository search.

---

## Phase G1 — Graph data and traversal

**Exit condition:** the graph is built from resolved links, counts are exact on the synthetic
fixture, traversal is deterministic and bounded, and unit tests pass. No tool surface changes yet.

### Models and build

- [x] G1.1 Add `GraphDirection` (`outgoing`, `incoming`, `both`), `EdgeDirection` (`outgoing`,
  `incoming`), `GraphNode`, and `GraphResponse` with `Serialize`/`Deserialize`/`JsonSchema` as
  appropriate. Verify: `cargo test` compiles schema derivations.
- [x] G1.2 Add `src/dictionary/graph.rs` with `Graph`, `GraphEdge`, `Graph::build`, and module
  tests. Verify: unit test constructs a two-node graph.
- [x] G1.3 Build `entry_by_anchor` from final entry anchors and map each resolved
  `Link.target_anchor` to an entry index, falling back to `(target_file, target_term)` for
  subentry anchors. Verify: fixture maps `a1BET` to `greater alpha` and an `a1SSD`-style anchor to
  its containing entry.
- [x] G1.4 Add outgoing and incoming adjacency in deterministic source order; count links whose
  target cannot be mapped. Verify: fixture has 3 resolved edges and 0 unmapped links.
- [x] G1.5 Add `graph_nodes`, `graph_edges`, and `graph_unmapped_links` to `LoadReport` and log
  them at debug level. Verify: `--verbose` load report shows the counts.

### Shared helpers

- [x] G1.6 Extract `resolve_dictionary_entry` from the `get_entry` path and use it from both
  `entry()` and graph traversal. Verify: existing `get_entry` tests still pass unchanged.
- [x] G1.7 Extract the canonical entry citation helper from `search.rs` and use it for graph nodes
  and entry responses. Verify: existing citation tests pass unchanged.

### Traversal

- [x] G1.8 Implement breadth-first traversal with a visited set seeded by the start entry.
  Verify: a two-entry cycle returns each node once.
- [x] G1.9 Implement `outgoing`, `incoming`, and `both`, including duplicate-neighbour collapse.
  Verify: unit tests assert each direction's exact node set.
- [x] G1.10 Implement depth 1–3, limit 1–50, and the 10,000-visited cap; set `total` and
  `truncated` correctly. Verify: tests at 0/1/3/4 depth and 0/1/50/51 limit.
- [x] G1.11 Sort discovered nodes by `(distance, entry index)` and keep the first deterministic
  `via` / `link_text` when several edges reach a node. Verify: repeated runs produce identical
  JSON.
- [x] G1.12 Test subentry anchors (`a1SSD` shape), anchorless entries (`file:line` citation), and
  a redirect entry that has outgoing links. Verify: target resolves to the containing entry and
  the citation is correct.
- [x] G1.13 Test unknown terms, appendix-only terms, empty dictionary, and empty term. Verify:
  `DICTIONARY_NOT_FOUND` / `DICTIONARY_INVALID_REQUEST` and no panic.
- [x] G1.14 Run `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
  `cargo test --all-targets`. Verify: all pass with no tool-schema diff.

---

## Phase G2 — `related_entries` tool

**Exit condition:** `tools/list` publishes three tools; the output schema is derived and narrow;
stdio calls work; the reviewed snapshot matches; citations are absolute; all gates pass.

- [x] G2.1 Add `RELATED_TOOL_NAME` and the input schema (`term`, `direction`, `depth`, `limit`)
  inside the supported JSON Schema subset. Verify: tool-definition unit test sees three tools.
- [x] G2.2 Derive the output schema from `GraphResponse` through `schema::output_schema`.
  Verify: schema contract test validates a graph payload.
- [x] G2.3 Implement argument validation with the existing numeric and string helpers; reject
  empty terms and out-of-range depth/limit. Verify: unit tests for each rejection.
- [x] G2.4 Implement model-facing text rendering with canonical citation, absolute
  `Source: <path>:<line>` when available, distance, direction, `via`, and `link_text`.
  Verify: text test contains no bare relative citation.
- [x] G2.5 Wire `related_entries` into `tools::call_tool`, `server.rs`, and cancellation handling.
  Verify: stdio e2e call returns `structuredContent` and text.
- [x] G2.6 Update `UnknownToolError` to name all three tools and update the related server
  instruction. Verify: unknown-tool test and instruction test.
- [x] G2.7 Add e2e coverage for outgoing, incoming, both, depth 2, limit truncation, and unknown
  term. Verify: `cargo test --test tools_e2e`.
- [x] G2.8 Add a `related_entries` call to `tests/schema_contract.rs` and validate its payload
  against the advertised schema, including the undeclared-field rejection. Verify:
  `cargo test --test schema_contract`.
- [x] G2.9 Regenerate and review `tests/fixtures/tools-list.json` with
  `MCP_UPDATE_TOOLS_LIST=1 cargo test --test schema_contract`; inspect the diff before accepting.
  Verify: snapshot test passes without the environment variable.
- [x] G2.10 Update `README.md` tool section, `PLAN.md` interface section, and `AGENTS.md` /
  `CLAUDE.md` only where their instructions must name the new tool. Verify: docs mention the tool
  and its bounds; no stale two-tool list remains.
- [x] G2.11 Run all default gates and the real-data graph smoke check. Verify:
  `cargo fmt --check`, clippy, `cargo test --all-targets`, and a real `related_entries` call.

---

## Phase I1 — Graph PNG rendering

**Exit condition:** `render_graph` writes a PNG, returns it as an MCP image block plus an absolute
path, and has schema/e2e coverage and documentation.

- [x] I1.1 Add the PNG renderer and dependencies (`image`, `font8x8`) with a layered layout.
  Verify: decoded PNG dimensions and signature tests.
- [x] I1.2 Add `GraphImageResponse` with nested graph, path, MIME type, width, and height.
  Verify: derived output schema.
- [x] I1.3 Add the `render_graph` tool definition and argument validation. Verify: tool count is 4.
- [x] I1.4 Return an MCP `image` block plus an absolute file path, writing to `--pages-dir` or the
  temp directory. Verify: e2e path and base64 PNG signature.
- [x] I1.5 Add empty-graph and real-data render checks. Verify: `cargo test --lib graph_image` and
  a manual real graph image.
- [x] I1.6 Update README, PLAN, CLAUDE, AGENTS, INSTALL, and server instructions.
- [x] I1.7 Regenerate the `tools/list` snapshot and update schema-contract/tool-count tests.
- [x] I1.8 Run fmt, clippy, all tests, and rebuild `dist`. Verify: clean gates and a `dist` binary
  that returns `render_graph`.

---

## Phase A1 — Appendix networks

**Exit condition:** formula-family, formula-medicinal, and named point-combination networks are
queryable and renderable; each edge carries a relation and source line; no clinical equivalence is
inferred.

- [x] A1.1 Add `NetworkDomain`, `NetworkNodeType`, `NetworkRelation`, `NetworkRequest`,
  `NetworkResponse`, and node/edge storage.
- [x] A1.2 Build formula-family edges from explicit “This is … with/combined with/variant of”
  prose; add inverse relation display.
- [x] A1.3 Build formula→medicinal edges from `□` ingredient lines before the first `•` section;
  add medicinal→formula reverse edges.
- [x] A1.4 Parse named `combining … points` entries into combination nodes with point roles and
  indication text; enforce point-ID boundaries.
- [x] A1.5 Implement domain-filtered BFS, depth/limit/cap bounds, direction, relation inverses, and
  deterministic node order.
- [x] A1.6 Add `related_network` with required `domain` and schema validation.
- [x] A1.7 Add `render_network` using the existing PNG renderer with network metadata.
- [x] A1.8 Add unit tests for formula prose, ingredient lines, point grouping/boundaries, and
  schema/snapshot/e2e coverage.
- [x] A1.9 Document domains, confidence, and examples in README, PLAN, AGENTS, CLAUDE, INSTALL,
  and `--help`.
- [x] A1.10 Verify real-data queries: Four Gentlemen formula/medicinal, LU-9 point combination, and
  `render_network`; run fmt/clippy/all tests.

---

## Phase S0 — Ollama capability gate

**Exit condition:** the local Ollama API contract is proven on Windows x64, or the semantic branch
is blocked with evidence. No semantic feature code is merged before this gate passes.

- [x] S0.1 Verify `GET /api/tags` on `http://127.0.0.1:11434` lists the selected embedding model
  and returns a stable digest. Verify: response captured, name and digest recorded.
- [x] S0.2 Verify `POST /api/embed` returns one finite, unit-normalized vector with the expected
  dimensions for an invented text. Verify: norm ≈ 1.0; no NaN.
- [x] S0.3 Verify batch input, `truncate: true`, `keep_alive`, the `dimensions` parameter, Qwen3
  instruction framing, and `num_ctx` behave as documented. Verify: batch returns one vector per
  input; 4K context holds memory at 1.2 GB; malformed input is rejected.
- [x] S0.4 Verify `/api/show` provides usable diagnostics and that no Cloud endpoint is contacted.
  Verify: only loopback requests in a monitored run.
- [x] S0.5 Measure warm query latency, batch throughput, and Ollama model RSS for the candidate
  model(s). Verify: numbers recorded in the progress log.
- [x] S0.6 Choose and pin the HTTP client (`ureq` preferred), add the optional `semantic` feature
  and optional dependency without changing the default build. Verify: default `cargo tree` has no
  HTTP client; feature build compiles.
- [x] S0.7 Add a feature-gated smoke test that calls the real Ollama server and asserts finite,
  normalized, non-identical vectors for two invented texts. Verify: `cargo test --features
  semantic` with local Ollama.
- [x] S0.8 Record an explicit **go/no-go**. If no-go, mark S1–S4 blocked and stop; do not fake a
  model or call a remote/Cloud service. **Decision: go** (2026-11-27).

---

## Phase S1 — Semantic core with a fake embedder

**Exit condition:** all retrieval and index logic is implemented and tested without a model;
default build and default configuration are unchanged.

- [x] S1.1 Add the `Embedder` trait and a deterministic fake embedder for tests. Verify: fake
  returns configured vectors and counts query calls.
- [x] S1.2 Add `MatchKind::Semantic` to the model and output schema. Verify: schema contract
  snapshot diff is reviewed.
- [x] S1.3 Implement deterministic document construction for main entries, three appendices, and
  weights sections, applying profile prefixes/instructions. Verify: unit test checks order and
  content boundaries.
- [x] S1.4 Implement the flat cosine index over normalized vectors and top-K search. Verify:
  hand-built vectors return the expected order.
- [x] S1.5 Implement the index header/metadata: magic, format version, package version,
  dictionary fingerprint, model name/digest, dimensions, count, profile hash. Verify: round-trip
  test.
- [x] S1.6 Implement atomic save and load. Verify: temp file is renamed; a failed write leaves the
  old index intact.
- [x] S1.7 Add bounds and validation for file size, dimensions, count, record length, finite
  values, and unit norm. Verify: tests for each rejection path.
- [x] S1.8 Extract `content_fingerprint(sources)` independent of the lookup-cache version and use
  it for both caches. Verify: fingerprint tests and one-time lookup-cache rebuild.
- [x] S1.9 Derive the semantic cache path from data stem, content fingerprint, model key, digest
  prefix, and format version. Verify: path unit test and collision check.
- [x] S1.10 Implement semantic hit construction for dictionary, meridian, medicinal, formula, and
  weights refs, reusing snippets and citations. Verify: each source type produces a valid hit.
- [x] S1.11 Implement `HYBRID_GATE_SCORE = 300`, skip embedding for strong lexical and exact
  shortcuts, and embed once for weak queries. Verify: fake-embedder call-count tests.
- [x] S1.12 Implement RRF (`k = 60`), dedupe by `LookupRef`, lexical pin at score ≥ 600, stable
  tie-breakers, and total/limit accounting. Verify: hand-computed ordering test.
- [x] S1.13 Keep suggestions on weak merged results and fall back to lexical on embed errors.
  Verify: failure and suggestion tests.
- [x] S1.14 Add unit tests for all gate/fusion cases, including a semantic-only hit, a hit in both
  lists, threshold filtering, and `matched_in = semantic` scores 200–299.
- [x] S1.15 Add unit tests for stale fingerprint, model-digest mismatch, dimension mismatch,
  profile-hash mismatch, truncated file, oversized header, NaN, infinity, zero norm, and wrong
  record count.
- [x] S1.16 Run default gates. Verify: `cargo fmt --check`, clippy, and all tests pass without a
  model or feature.

---

## Phase S2 — Ollama embedder, index build, and CLI

**Exit condition:** a local model can build an index and serve gated hybrid queries; all failure
paths are actionable; inference is off the async worker; the real smoke test passes when
configured.

- [x] S2.1 Add `--ollama-url`, `--embedding-model`, `--embedding-profile`, `--semantic-index`, and
  `--require-semantic` plus environment variables to `config.rs` and `usage()`. Verify: parse and
  resolve tests, and `--help`.
- [x] S2.2 Add `--build-semantic-index`, require the model and data, and reject combinations with
  `--build-indices` and `--build-linked-data`. Verify: config tests.
- [x] S2.3 Implement the Ollama HTTP client: loopback URL validation, `/api/tags`, `/api/embed`,
  optional `/api/show`, timeouts, and JSON error handling. Verify: mock-server contract tests.
- [x] S2.4 Add known model profiles and the `--embedding-profile` override; validate prefixes,
  instruction, `min_similarity`, and `num_ctx` (Qwen3 uses 4096). Verify: table-driven profile
  tests and memory measurements.
- [x] S2.5 Implement `--build-semantic-index`: deterministic document order, batches, progress,
  digest capture, dimensions probe, atomic write, and a new digest-keyed path. Verify: synthetic
  build test and real build. Real build: 7,042 documents, 768 dimensions, 21,668,504 bytes,
  816,078 ms, digest-keyed path written atomically.
- [x] S2.6 Implement serve startup: load exactly one digest-matched index, validate header,
  dimensions, profile hash, and dictionary fingerprint; never load other model indexes. Verify:
  missing/stale/mismatch tests and a single-active-index check. `--check-semantic` reports
  `status: active` for the built index; startup degrades or fails under `--require-semantic` as
  tested.
- [x] S2.7 Run hybrid search in `tokio::task::spawn_blocking`, apply HTTP timeouts and
  `keep_alive`, and check cancellation before and after. Verify: concurrent-request and
  cancellation tests.
- [x] S2.8 Implement periodic digest re-check (at most every 300 s) and disable semantic for the
  process if the digest changes. Verify: mock digest-change test.
- [x] S2.9 Implement the default lexical fallback and `--require-semantic` strict startup failure
  for Ollama down, model absent, index missing, or index stale. Verify: each state.
- [x] S2.10 Add build/serve logging: model, digest, dimensions, index path, gate opened, embedding
  ms, semantic candidates, fallback reason. Verify: `--verbose` output.
- [x] S2.11 Add `tests/semantic_real.rs`, gated by local Ollama and model env vars, that builds or
  loads an index and runs one weak query. Verify: passes when configured; skips with a reason
  otherwise. Ran with the real embeddinggemma index: 5 hits, total 5,714, passed in 5.44 s; the
  unconfigured path prints its skip reason.
- [x] S2.12 Measure startup, MCP RSS, index size, gate latency, and p95 added query latency; run
  `cargo clippy --all-targets --features semantic -- -D warnings` and
  `cargo test --all-targets --features semantic`. Verify: budget numbers recorded and gates pass.
  Startup load 0.57–0.89 s; MCP RSS 47.84 MB lexical → 68.50 MB hybrid; index 20.66 MiB; gate
  embedding 30–54 ms warm; added p95 75.1 ms. Feature clippy clean; 170 unit + 31 integration
  pass; real `semantic_gate` and `semantic_real` pass with `embeddinggemma:latest`.

---

## Phase S3 — Model lifecycle and migration

**Exit condition:** model identity and digest changes are handled, indexes can be rebuilt without
destroying the last known-good one, rollback works, and the migration instructions are published.

- [x] S3.1 Implement `--check-semantic`: resolve URL/model, show digest, dimensions, expected/found
  index path, record count, active/degraded status, and remediation; never write. Verify: unit
  tests and a real run. Real run: digest `85462619…`, profile hash `d6db7d907270eac4`, 7,042
  records, 768 dimensions, `status: active`.
- [x] S3.2 `--build-semantic-index` writes a new digest-keyed index and leaves the old index
  untouched. Verify: two files exist; the old one still loads.
- [x] S3.3 Implement `--prune-semantic-index` with a dry-run/list and `--yes` confirmation; remove
  only indexes not matching the current model/digest. Verify: mock/synthetic cache test.
- [x] S3.4 Test model removal (404), digest change, dimension change, duplicate tags, and
  `/api/tags` unavailable against the mock server; verify each follows the degrade/strict policy.
- [x] S3.5 Test malformed JSON, HTTP 5xx, timeout, and a hung server; verify fallback, bounded
  latency, and no panic.
- [x] S3.6 Document the routine update procedure (`--check-semantic` → pull → build → evaluate →
  switch → prune) in README and `dist/INSTALL.md`. Verify: commands are copy-pasteable.
- [x] S3.7 Document model-withdrawal and backup guidance: keep the local model, keep the old
  digest-keyed index, choose a replacement if unavailable, and never migrate vectors across
  models. Verify: docs mention rollback and no-vector-conversion rule.
- [x] S3.8 Verify rollback: switch back to the old model/digest and confirm the old index loads and
  serves; confirm the new index remains for retry.
- [x] S3.9 If a second model is available, run a real migration smoke
  (`embeddinggemma` → `bge-m3` or chosen pair); otherwise record the skip and the mock evidence.
  Attempted `bge-m3:latest` at its 8K profile; it completed 0/7,042 documents in roughly six
  minutes, so the build was stopped as impractical on this CPU. The old embeddinggemma index
  remained loadable (`status: active`) and no bge-m3 index file was written; digest-keyed
  retention and rollback are covered by the S3.2/S3.8 tests. Recorded as a documented skip, not a
  model failure.

---

## Phase S4 — Evaluation and acceptance decision

**Exit condition:** lexical vs hybrid is measured on at least 30 held-out queries; thresholds are
met, or semantic release is disabled with the negative result recorded.

- [x] S4.1 Add a synthetic-corpus evaluation set with invented queries and expected synthetic
  entries. Verify: it runs in the hermetic suite.
- [x] S4.2 Add a local real-data evaluation harness under `tools/` that drives the stdio server
  and reports metrics; keep queries/results out of git if derived from the book. Verify:
  `git status` shows no local evaluation data.
- [x] S4.3 Run lexical-only baseline on an English/mixed-weighted query set; record recall@5, MRR,
  top-1, p50/p95 latency, startup, RSS.
- [x] S4.4 Run hybrid on the same queries and model; record the same metrics plus embedding calls.
  30 queries, 3 repeats, `embeddinggemma:latest`: recall@5 **0.5667** (lexical 0.2333), MRR
  **0.4550** (0.1833), top-1 **0.4000** (0.1333), p50 **159.6 ms** (98.3 ms), p95 **185.7 ms**
  (110.7 ms). Gate opened for 22/30 queries; the other 8 were strong enough lexically and skipped
  embedding. Embedding time per gated query: 30–54 ms when the model is warm and not sharing the
  CPU with another Ollama model.
- [x] S4.5 Check the exact/alias/pinyin/Chinese regression set; record any top-1 changes. Four
  queries (exact English, pinyin, toned pinyin, Chinese): lexical and hybrid both recall@5 1.0,
  MRR 1.0, top-1 1.0; **0 top-1 changes**. Hybrid p95 0.24 ms because the gate skipped embedding.
- [x] S4.6 Apply the plan's acceptance thresholds: ≥10 percentage points recall@5 improvement (or
  ≥5 points when baseline >90%), ≤1 exact top-1 regression, ≤150 ms added p95, ≤50 MB MCP RSS
  increase, ≤1.3 GB model+index disk, ≤35 MB index. All pass: **+33.3 points** recall@5
  (baseline 23.3%, so the 10-point rule applies), **0** top-1 regressions, **+75.1 ms** added
  p95, **+20.66 MB** MCP working set (47.84 → 68.50 MB), **642 MB** model + index (621 MB model,
  20.66 MiB index), index **20.66 MiB**.
- [x] S4.7 Compare candidate models if available (`embeddinggemma`, `bge-m3`, optionally
  `qwen3-embedding:0.6b`) on English, non-exact Chinese, and pinyin subsets; record the winner,
  the 2K-vs-8K truncation effect, and whether the model size is justified. Direct probes:
  embeddinggemma had the better non-exact Chinese margin (related 0.70–0.96, unrelated
  0.55–0.58); bge-m3 scored 0.63–0.83 related against 0.52 unrelated; Qwen3 with its instruction
  separated English related 0.40–0.63 from unrelated 0.23 but was much slower. Raw pinyin was not
  discriminative for any model, so it stays on the lexical path. A full bge-m3 8K index did not
  complete a batch in six minutes and was stopped; Qwen3 measured ~0.86 s/doc at 1K versus
  embeddinggemma's ~0.12 s/doc full-corpus average. One document exceeds the 20,000-character
  guard, and the 2K context truncates long entries; 8K would truncate less but is not
  build-feasible here, and the 2K run already produced the accepted recall gain. **Winner for
  this host: embeddinggemma:latest**; 621 MB model + 20.66 MiB index is justified within the
  1.3 GB budget.
- [x] S4.8 Record the decision: **accepted**, **accepted with documented limits**, or **not
  enabled**. If not enabled, update docs and do not turn it on in the release. **Accepted with
  documented limits**: measured on one English/mixed query set and one corpus; the exact pinyin
  and Chinese regression set is unchanged; full cross-model index comparison was not feasible for
  bge-m3 on this CPU. The semantic feature stayed optional at release time; it was made a default
  feature afterward at the user's request (see the post-release entry).

---

## Phase D — Documentation, verification, bundle, one commit

**Exit condition:** every surface agrees with the code, every gate passes, the bundle is clean,
and exactly one commit is made after review. Nothing is pushed unless the user asks.

- [x] D1 Update `README.md`: tool table and `related_entries`; Ollama semantic configuration;
  model lifecycle and migration; architecture; limitations; loopback-only rule. Added the measured
  lexical-vs-hybrid result and the local-measurement caveat.
- [x] D2 Update `PLAN.md`: replace the deferred RAG/graph statements with the implemented scope,
  or link to this plan; document the Ollama gate and feature. Status is 0.6.0; the semantic scope,
  gate, measured result, and acceptance link are recorded.
- [x] D3 Update `CHECKLIST.md` with a Phase 7F/7G entry pointing at this tracker and its final
  status. Added Phase 7I and a dated progress entry.
- [x] D4 Update `CLAUDE.md` / `AGENTS.md` only for new tool or citation instructions that must
  steer the model; do not add generic advice. The six-tool and optional-feature descriptions
  already match the implementation; no new model-steering instruction was needed.
- [x] D5 Update `dist/INSTALL.md`, `dist/install.ps1`, `dist/mcp-config.example.json`, and
  `BUILDING.md` for Ollama setup, model pull, index build, update, and rollback. INSTALL and
  BUILDING document the commands and results; the configuration example uses
  `embeddinggemma:latest`; the installer remains generic, and the default bundle is now
  semantic-capable but lexical until a model is configured.
- [x] D6 Bump the version to `0.6.0` only if the enhancement is accepted; update server/version
  references and release notes. `Cargo.toml`, `Cargo.lock`, `BUILDING.md`, `PLAN.md`,
  `CHECKLIST.md`, and `dist/INSTALL.md` are at 0.6.0; the binary prints `0.6.0`.
- [x] D7 Run default gates: `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, `cargo test --all-targets`. Pass: 120 unit + 29
  integration, clippy clean, fmt clean.
- [x] D8 Run semantic gates: clippy/test with `--features semantic`; run mock-Ollama tests, the
  real-Ollama smoke test, lifecycle tests, and real-data tests. Clippy and 170 unit + 31
  integration pass; the index was rebuilt under 0.6.0 (7,042 records, 768 dimensions, 21,668,504
  bytes, 761 s), `--check-semantic` reports `status: active`, and `semantic_real` passed with 5
  hits; `semantic_gate` passed against the real model.
- [x] D9 Rebuild the Windows x64 bundle; record its size and SHA-256; verify it contains no data,
  model, index, cache, query file, or credential. `dist/mcp-chinese-dict-search.exe` is 2,368,512
  bytes, version 0.6.0, SHA-256 `7300417832a3cbe4709c81d52805cba66b3f662d15206a1cbe99e3145a729418`;
  `dist/SHA256SUMS.txt` regenerated; no archive, model, index, cache, page, or credential.
- [x] D10 Audit `git status`, `git diff --check`, and tracked files for archive/Markdown/EPUB/
  linked-copy/pages/model/index/local-evaluation derivatives; confirm `.gitignore` covers them.
  No tracked `.zip`, `.epub`, `.bin`, image, semantic/lookup cache, or fixture dictionary; the
  ignore rules cover semantic caches, local queries, and evaluation results.
- [x] D11 Review the complete diff against this plan and checklist; verify every surface and test
  is accounted for. Reviewed `git diff --stat`, `git diff --check`, the final search/fusion code,
  the ignore/feature/config diffs, and the docs surfaces; default and feature schema-contract,
  snapshot, e2e, and CLI tests pass.
- [x] D12 Make **one** commit after all gates pass. Do not amend, tag, push, or release unless the
  user asks. Release commit `e611d42` (49 files, 12,456 insertions); no tag, push, or release.

---

## Decision log

| Date | Decision | Evidence |
|---|---|---|
| 2026-11-27 | Use existing resolved links only; no entity extraction | Plan D1, G1 |
| 2026-11-27 | Graph tool named `related_entries`; depth 1–3, limit 1–50 | Plan D2, D3, G2 |
| 2026-11-27 | Flat exact cosine; no ANN in this phase | Plan D5 |
| 2026-11-27 | One vector per document; entry-level, no chunking | Plan D6 |
| 2026-11-27 | Semantic is optional and gated at lexical score < 300 | Plan D4, D9 |
| 2026-11-27 | Use local Ollama `/api/embed`; Cloud unsupported | Plan D7, D8; S0 |
| 2026-11-27 | `embeddinggemma:latest` selected as the CPU-friendly reference primary; `qwen3-embedding:0.6b` Chinese-heavy option; `bge-m3` long-entry challenger | S0 quality checks; S4 batched CPU timing |
| 2026-11-27 | `embeddinggemma:latest` (2K, 768 dims) used for the real S4 evaluation; Qwen3 measured ~0.86 s/doc batched at 1K vs ~0.16 s/doc for EmbeddingGemma on this CPU | Batched local benchmarks; S4.7 |
| 2026-11-27 | Qwen3 1K compared: higher recall@5 (0.633 vs 0.567) but lower MRR (0.429 vs 0.455), lower top-1 (0.30 vs 0.40), 5.6× longer build, and larger index/RSS; EmbeddingGemma remains the default | Real 30-query evaluation, regression set, and RSS measurement |
| 2026-11-27 | Semantic made a default Cargo feature at the user's request; runtime still opt-in via `--embedding-model`; `--no-default-features` gives the minimal lexical-only build; `dist` rebuilt semantic-capable | Default and minimal gates; shipped-bundle semantic probe |
| 2026-11-27 | Appendix queries need an explicit `scope`; scoped lexical is strong and semantic fusion is mixed, so no source-aware server policy was changed without further evidence | 17-query point/formula probe via the scoped evaluation CLI |
| 2026-11-27 | Semantic inference runs in `spawn_blocking`; digest re-check every 300 s; a changed digest disables semantic for the process | S2.7, S2.8 tests |
| 2026-11-27 | `--prune-semantic-index` lists by default and deletes only with `--yes` | S3.3 test |
| 2026-11-27 | Exactly one model/index active; no runtime multi-model routing or vector fusion | Avoid 3× embeddings, incompatible score spaces, and multi-GB memory |
| 2026-11-27 | No MCP switch-model tool; switching is config + restart | Administrative actions are not model-callable; avoids races and non-reproducible sessions |
| 2026-11-27 | Pinyin hardening deferred; evaluation weights English and mixed queries | Reference usage is mostly English with occasional pinyin/Chinese |
| 2026-11-27 | Pure pinyin queries skip semantic embedding | Raw pinyin similarity is not discriminative; use pinyin indexes/fuzzy |
| 2026-11-27 | Model identity is name + digest; vectors never migrate | Plan D12, S3 |
| 2026-11-27 | Stale/unavailable semantic degrades to lexical by default; strict flag available | Plan D13 |
| 2026-11-27 | Digest-keyed indexes retained until explicit prune | Plan D14, S3 |
| 2026-11-27 | No commit until every phase passes | User instruction; checklist rule 6 |

## Open questions

1. S4 selected `embeddinggemma:latest` as the CPU-friendly reference primary for the
   English/mixed profile: it indexed the corpus in practical time and is the model measured in
   the real evaluation. `qwen3-embedding:0.6b` remains the Chinese-heavy option (best non-exact
   Chinese separation in S0, but much slower on CPU at 4K), and `bge-m3` is the 8K long-entry
   challenger. No model solves pinyin.
2. Does the accepted release ship the `semantic` feature in the Windows bundle, or keep it as a
   documented source build? D6/D9 decide after evaluation and size measurement.
3. Should the local Ollama model store be backed up operationally? The server can document the
   procedure but must not copy weights into the repository.

## Progress log

### 2026-11-27 — Enhancement plan authored

- Created `ENHANCEMENT-PLAN.md` and `ENHANCEMENT-CHECKLIST.md`.
- Base recorded as `0.5.0`, commit `dd684f2`.
- Plan revised for local Ollama `/api/embed`, digest-keyed vectors, lifecycle and migration.
- No code changed; no commit made.

### 2026-11-27 — `bge-m3` pulled; S0 model checks

- Model: `bge-m3:latest`, digest `790764642607…`, 1,157,672,605 bytes, F16 BERT,
  566.7M parameters, MIT.
- `/api/show`: 1024 embedding dimensions, 8192 context.
- `/api/embed`: unit-normalized vectors; 1024 dims; `dimensions: 512` honoured.
- Batch of 16 invented texts: 356 ms. Warmed single query: 32–36 ms.
- `ollama ps`: 1.2 GB model, 100% CPU, 8192 context; `keep_alive` held it loaded.
- S0.1–S0.5 checked. S0.6–S0.8 (HTTP client, feature, smoke test, go/no-go) remain.
- No code changed; no commit made.

### 2026-11-27 — Chinese/pinyin model comparison

- Non-exact Chinese related pairs: `embeddinggemma` 0.70–0.96, `bge-m3` 0.63–0.83; unrelated
  pairs: 0.55–0.58 and 0.52 respectively. `embeddinggemma` had the better margin.
- Traditional/simplified pairs: both models 0.92–0.98.
- Raw pinyin is not reliable: `embeddinggemma` scored `qi xu` ↔ `qi deficiency` at 0.70 but also
  `qi xu` ↔ `shi re` at 0.70 and `pi xu` ↔ `qi xu` at 0.80. `bge-m3` scored a related pinyin pair
  below an unrelated one.
- Decision: `qwen3-embedding:0.6b` is the provisional semantic primary; pure pinyin queries skip
  embedding and stay on the pinyin/fuzzy path. `embeddinggemma` is the lightweight fallback;
  `bge-m3` remains the long-entry challenger.
- Next: Phase G1 graph data and traversal; S0.6–S0.8 can follow.

### 2026-11-27 — `qwen3-embedding:0.6b` pulled and checked

- Model: `qwen3-embedding:0.6b`, digest `ac6da0dfba84…`, 639,150,858 bytes, Q8_0,
  595.8M parameters, Apache-2.0.
- `/api/show`: 1024 embedding dimensions, 32768 native context.
- Native 32K context loaded 4.6 GB; `num_ctx=4096` loaded 1.2 GB and kept 1024 dims.
- Warmed query latency at 4K: 29–39 ms (first request 125 ms).
- With the Qwen instruction, non-exact Chinese related pairs scored 0.44–0.61 against unrelated
  0.26–0.27; English related pairs 0.40–0.63 against unrelated 0.23.
- Raw pinyin remains unreliable: `qi xu` ↔ `qi deficiency` 0.30, but `pi xu` ↔ `qi xu` 0.52.
- Decision: Qwen3 provisional primary with instruction and 4K context; pinyin stays fuzzy.
- No code changed; no commit made.

### 2026-11-27 — Phase G1 and G2 complete

- Added `src/dictionary/graph.rs`: outgoing/incoming adjacency from resolved links, subentry-anchor
  fallback via `(target_file, target_term)`, deterministic BFS, depth 1–3, limit 1–50, 10,000-node
  cap.
- Added `related_entries` with derived output schema, validation, absolute `Source:` rendering,
  server instructions, README/PLAN/AGENTS/CLAUDE/dist documentation, and the reviewed
  `tools/list` snapshot.
- Real data: 5,803 graph nodes, 8,275 edges, 8 unmapped links; `qi` both depth 1 returned 28
  reachable entries (5 shown), depth 2 returned 144; 0 ms query time.
- Verification: `cargo fmt --check`; `cargo clippy --all-targets -- -D warnings`;
  `cargo test --all-targets` (108 unit + 27 integration tests); schema contract and snapshot pass.
- No commit made.

### 2026-11-27 — Phase I1 graph PNG rendering

- Added `src/graph_image.rs` with a column-per-hop PNG layout, font8x8 labels, green outgoing /
  blue incoming edges, and a gold start node.
- Added `render_graph`, returning a PNG image block plus an absolute path and structured metadata;
  it writes to `--pages-dir` when configured, otherwise the temp directory.
- Verified a real `spleen` outgoing depth-2 render (1000×1130 PNG) and the updated tool surface.
- Gates: `cargo fmt --check`; clippy clean; `cargo test --all-targets` (110 unit + 28 integration).
- No commit made.

### 2026-11-27 — Phase A1 appendix networks

- Added formula-family edges from explicit prose, formula→medicinal ingredient edges, and named
  point-combination records, all with relations and source lines.
- Added `related_network` and `render_network`; domain filtering prevents formula-family edges from
  leaking into medicinal queries.
- Real data: Four Gentlemen → Six/Eight formulas; Four Gentlemen → ginseng, poria, codonopsis,
  white atractylodes, licorice; `combining source and network points` → 11 combinations; LU-9
  resolves to its source/network combination.
- Fixed point-ID boundary matching (`LI-1` no longer matches inside `LI-16`) and inverse relation
  labels for incoming traversal.
- Gates: `cargo fmt --check`; clippy clean; `cargo test --all-targets` (114 unit + 29 integration).
- No commit made.

### 2026-11-27 — Point-combination parser fix

- Rewrote `build_point_combinations` to parse only the list section of a method entry, carry
  role-prefixed and continuation point lines into one group, and distinguish short group headings
  from indication sentences. Local/distant and transport/alarm groups no longer shift their
  context or indication to the next heading.
- Real data: the point-combination set changed from 52 records with shifted context/indication
  labels to 44 correct groups. `LI-4` now resolves to six intact groups: Forehead, Eye, Nose,
  Mouth and Teeth, Throat, and Upper limbs.
- Added a regression test with a synthetic multi-line local/distant list and a `LI-4` query.
- Gates: `cargo fmt --check`; clippy clean; `cargo test --all-targets` (115 unit + 29 integration
  = 144 passed); `dist` checksums verify.
- Rebuilt `dist/mcp-chinese-dict-search.exe` (2,351,616 bytes; SHA-256
  `5926377f70e8a085f881239d6eab177d7b3ffbc20911778054e982c6a05a7c2a`).
- No commit made.

### 2026-11-27 — Phase S0 complete: go

- Added optional `semantic` Cargo feature and `ureq` 3.5.2 (resolved; `default-features = false`,
  `json` only) behind it.
- Default `cargo tree` contains no `ureq`; `cargo tree --features semantic` does; `cargo check
  --features semantic` compiles.
- Added `tests/semantic_gate.rs`: feature-gated, environment-gated, skips with a reason when
  `MCP_CHINESE_DICT_OLLAMA_URL` or `MCP_CHINESE_DICT_EMBEDDING_MODEL` is unset.
- Ran against local `qwen3-embedding:0.6b`: `/api/tags` found the model and digest; `/api/embed`
  returned two finite unit vectors, norm ≈ 1.0, non-identical.
- Decision: **go**. S1 may proceed; no model or Cloud call is made by the default build.
- No commit made.

### 2026-11-27 — Phase S1 complete: semantic core with a fake embedder

- Added the feature-gated `src/semantic/` module: `Embedder` and `FakeEmbedder`, deterministic
  document construction for main entries, three appendices, and weights, a flat exact-cosine
  index, a binary header with magic/format/package/fingerprint/model/digest/dimensions/profile
  hash, atomic save/load, bounds validation, digest-keyed cache paths, and RRF fusion.
- Added `IndexExpectation` so serving can reject a stale fingerprint, package version, model name,
  model digest, dimensions, or profile hash with a rebuild instruction.
- Extracted `content_fingerprint(sources)` independent of the lookup-cache version, used it in the
  lookup fingerprint, and exposed `Dictionary::content_fingerprint()` for the semantic cache. The
  lookup cache rebuilds once because its fingerprint input changed.
- Added `MatchKind::Semantic` and the tool text label; regenerated and reviewed the `tools/list`
  snapshot (the enum now includes `semantic`).
- Gate: `HYBRID_GATE_SCORE = 300`, strong lexical hits and exact shortcuts skip embedding, pure
  pinyin skips embedding, weak queries embed once.
- Fusion: RRF `k = 60`, dedupe by `LookupRef`, semantic-only score 200–299, lexical pin at ≥600,
  deterministic ties, `total` before `limit`, suggestions retained, lexical fallback on embed or
  index errors.
- Verification: `cargo fmt --check`; default and `--features semantic` clippy with `-D warnings`;
  default `cargo test --all-targets` (115 unit + 29 integration = 144); feature
  `cargo test --all-targets` with local Ollama (142 unit + 30 integration = 172, including the S0
  smoke test). No model is required for the default suite or the feature unit suite.
- No commit made.

### 2026-11-27 — Semantic S2–S3 implementation

- CLI/config: added `--ollama-url`, `--embedding-model`, `--embedding-profile`,
  `--semantic-index`, `--build-semantic-index`, `--check-semantic`, `--prune-semantic-index`,
  `--require-semantic`, and `--yes`, with environment variables, loopback validation, build-mode
  exclusions, and a clear error when the binary lacks the `semantic` feature.
- Ollama client: loopback-only `ureq` client for `/api/tags`, `/api/embed`, and `/api/show`, with
  bounded timeouts, digest resolution, duplicate-tag detection, batch embedding, finite/unit
  validation, and mock-server tests for malformed JSON, 5xx, missing models, duplicate tags, and a
  hung server.
- Profiles: Qwen3 instruction + 4K default, EmbeddingGemma 2K, bge-m3 8K, Nomic prefixes, mxbai
  prefix, default fallback, JSON override, and an embedding-affecting hash that excludes
  `min_similarity`.
- Build/serve: deterministic batched build with progress and atomic digest-keyed save; startup
  loads exactly one index, validates package/fingerprint/model/digest/dimensions/profile hash, and
  degrades to lexical or fails under `--require-semantic`; semantic search runs in
  `spawn_blocking` with cancellation before and after; digest re-checks every 300 s and disables
  semantic on a change.
- Lifecycle: `--check-semantic` reports model/digest/dimensions/index/status/remediation;
  `--prune-semantic-index` lists by default and removes only with `--yes`; a new digest path
  leaves the old index loadable; README and `dist/INSTALL.md` document update, withdrawal, and
  rollback.
- Verification so far: default and feature clippy clean; feature suite 169 unit + 31 integration
  = 200 passed; synthetic evaluation lexical recall@5 0.25 / MRR 0.07 versus hybrid 1.00 / 0.88;
  real lexical baseline on 30 local queries recall@5 0.233 / MRR 0.183 / p50 149 ms / p95 185 ms.
  A real index build is in progress with a local 1024-context profile override; the default Qwen3
  profile remains 4096.
- No commit made.

### 2026-11-27 — Usage profile recorded

- Primary usage: English; secondary: English + pinyin medical terms; occasional pasted Chinese;
  pinyin-only and fuzzy pinyin are rare.
- Evaluation will weight English and mixed queries. Pinyin hardening is deferred until usage
  justifies it; existing exact/fuzzy pinyin remains available.
- No code changed; no commit made.

### 2026-11-27 — S4 real evaluation: embeddinggemma accepted; fusion latency optimized

- Built the real index with `embeddinggemma:latest` (digest `85462619…`): 7,042 documents,
  768 dimensions, 21,668,504 bytes, 816,078 ms. `--check-semantic` reports `status: active`.
- 30-query English/mixed set, three repeats: lexical recall@5 0.2333 / MRR 0.1833 / top-1 0.1333,
  p50 98.3 ms / p95 110.7 ms; hybrid recall@5 **0.5667** / MRR **0.4550** / top-1 **0.4000**,
  p50 159.6 ms / p95 185.7 ms.
- First latency measurement failed the added-p95 gate (≈250 ms). Profiling showed lexical
  ranking ~100 ms, embedding only 30–54 ms, and the remainder in a second full lexical search
  plus O(candidates × entries) hit-to-entry scans. Fusion now reuses the full ranked list from
  one `search_all` call and resolves hits through per-fusion hash maps; added p95 fell to
  **75.1 ms** with identical recall/MRR/top-1.
- Regression set (exact English, pinyin, toned pinyin, Chinese): lexical and hybrid both 1.0
  recall@5/MRR/top-1; **0 top-1 changes**; the gate skipped embedding.
- Budgets: MCP working set 47.84 MB lexical → 68.50 MB hybrid (**+20.66 MB**); model 621 MB +
  index 20.66 MiB = **642 MB**; index **20.66 MiB**.
- S4.4–S4.6 pass. A `bge-m3` challenger index build is running for S4.7; the old embeddinggemma
  index remains in place for rollback.
- No commit made.

### 2026-11-27 — S4.7/S4.8 decision: accepted with documented limits

- `bge-m3:latest` at the 8K profile completed 0/7,042 documents in about six minutes, so the build
  was stopped as impractical on this CPU. No bge-m3 index file was written; the embeddinggemma
  index still loads and `--check-semantic` reports it active.
- Model comparison uses the S0 direct probes plus build feasibility: embeddinggemma has the best
  non-exact Chinese margin; Qwen3 separates English well but is ~7× slower to index; bge-m3's 8K
  context would truncate less but cannot be built here in a practical window. Raw pinyin remains
  non-discriminative and stays lexical.
- Decision: **accepted with documented limits**. Semantic remains an optional feature; the
  default build is lexical-only. Limits: one English/mixed query set, one corpus, and no full
  bge-m3 retrieval comparison.
- The 0.6.0 version bump invalidated the 0.5.0 index because package version is part of the
  index identity. Rebuilt under 0.6.0 with the unused Ollama models unloaded: 7,042 records,
  768 dimensions, 21,668,504 bytes, 761 s. `--check-semantic` reports `status: active`;
  `semantic_real` returned 5 hits and passed.
- Release commit `e611d42`: 49 files, 12,456 insertions. No tag, push, or release.
- No further commit made except this tracker close-out.

### 2026-11-27 — Qwen3 1K real evaluation and comparison

- Built the Qwen3 index with the 1K profile override: 7,042 records, 1,024 dimensions,
  28,879,512 bytes (27.54 MiB), 4,572,432 ms (76.2 min), status active.
- 30-query English/mixed set, three repeats: Qwen3 hybrid recall@5 **0.6333**, MRR **0.4289**,
  top-1 **0.3000**, p50 175.2 ms, p95 208.8 ms; same-run lexical p95 92.7 ms, so added p95
  **116.1 ms**.
- Regression set: lexical and hybrid both 1.0 recall@5/MRR/top-1, zero top-1 changes.
- Qwen3 MCP working set 76.39 MB versus 47.93 MB lexical (**+28.46 MB**); model 639 MB + index
  27.54 MiB.
- Compared with EmbeddingGemma: Qwen3 gains 6.7 recall@5 points but loses 2.6 MRR points and
  10 top-1 points; its build is 5.6× longer and its index/RSS are larger. Qwen3 was evaluated at
  1K, not its 4K default, so this is the practical CPU configuration rather than Qwen3's best.
- Decision: keep **EmbeddingGemma** as the default and installed model. Qwen3 remains a supported
  alternative for Chinese-heavy or recall-first use; its digest-keyed index is retained for
  rollback and comparison.
- No tag, push, or release.

### 2026-11-27 — Post-release: semantic default and deployable evaluation tool

- Changed `[features] default = ["semantic"]`. A plain `cargo build --release` and `dist` now
  include the loopback client and semantic engine; runtime remains lexical until
  `--embedding-model` is supplied. `--no-default-features` builds the minimal lexical-only binary.
- Rebuilt `dist/mcp-chinese-dict-search.exe`: 3,103,744 bytes, SHA-256
  `ca4a7a04b3bb5b26b48bebd829d94da452578dd3f8765dffe28e73d0b38e3736`. The bundle validated the
  Qwen3 profile/index (`status: active`) and answered both semantic probes at rank 1.
- Consolidated the local diagnostics into `tools/evaluate_semantic.py` with
  `--embedding-profile`, `--mode`, and `--per-query`; added `tools/README.md`; removed the
  temporary diagnostic scripts.
- Gates: default 170 unit + 31 integration; minimal 120 unit + 29 integration; clippy clean in
  both; fmt clean. `cargo tree` shows `ureq` by default and zero `ureq` with
  `--no-default-features`.
- Added optional per-query `scope` to `tools/evaluate_semantic.py`. A 17-query point/formula
  probe with `scope: meridian`/`formulas` found scoped lexical recall@5 **0.706** / MRR 0.510 /
  top-1 0.412 versus hybrid **0.588** / 0.520 / 0.471. Semantic fusion helped some point
  paraphrases (`Inner Pass`, `Supreme Surge`) but demoted correct scoped hits, so scoped lexical
  remains the safer default for appendix queries. The unfiltered `scope: all` path often closes
  the 300-point gate on unrelated main-dictionary body matches; the tool and README now document
  this.
- No tag, push, or release.
