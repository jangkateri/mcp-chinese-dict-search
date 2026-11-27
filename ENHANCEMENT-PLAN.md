# ENHANCEMENT PLAN — graph traversal and gated hybrid retrieval

| Field | Value |
|---|---|
| Status | Graph traversal and semantic core S0–S1 implemented and tested; S2 not started |
| Base | `0.5.0`, commit `dd684f2` |
| Tracking | [`ENHANCEMENT-CHECKLIST.md`](ENHANCEMENT-CHECKLIST.md) |
| Main plan | [`PLAN.md`](PLAN.md) |
| Working rules | [`CLAUDE.md`](CLAUDE.md) |
| Last updated | 2026-11-27 |

This document covers two additions chosen from the earlier cost analysis:

1. **Graph traversal** over the dictionary's existing resolved cross-references.
2. **Gated hybrid retrieval**: lexical search stays primary; a local embedding model is consulted
   only when lexical results are weak.

It does not authorize a rewrite of the search engine, a database, Graph RAG, an external service,
or network access at runtime.

## 1. Objective

### 1.1 Graph traversal

Expose the relationships already present in the parsed data. A caller can ask what an entry links
to, what links to it, or both, up to three hops. Results are deterministic and carry the same
canonical and absolute citations as `search_dictionary` and `get_entry`.

### 1.2 Gated hybrid retrieval

Improve recall for paraphrases and concepts that do not share a token with the entry, without
changing exact lookup, alias, pinyin, Chinese, identifier, or strong lexical behaviour.

The required behaviour:

- Lexical retrieval runs first and alone.
- The embedding model is consulted only when the best lexical score is **below 300**.
- Exact-anchor, exact-pinyin, exact-Han, and exact-alias shortcuts never embed.
- Semantic hits are merged with reciprocal-rank fusion, not allowed to displace strong lexical
  hits.
- With no model configured, behaviour, memory, startup time, and dependencies are unchanged.

### 1.3 Reference usage profile

The primary user mostly searches in **English**, sometimes mixes an English medical phrase with a
pinyin term, and occasionally pastes Chinese characters because no IME is available. Pinyin-only
queries and fuzzy pinyin typos are rare. Therefore:

- English and mixed English/pinyin retrieval are the evaluation priority.
- Exact pinyin aliases and the existing fuzzy pinyin suggestions remain available, but no new
  syllable-aware pinyin subsystem is part of this enhancement.
- Chinese-character queries use the Han index first and semantic fallback second.
- Pinyin-only queries do not use the embedding model.
- A pinyin-hardening phase is deferred until real usage justifies it.

### 1.4 Non-goals

- Graph RAG, generated answers, summarisation, or a language model at query time.
- Extracting new entities or relations from prose. The graph uses only links the parser already
  resolves.
- A vector database, ANN service, daemon, or shared server process.
- Network access of any kind at runtime; model files are local.
- Replacing lexical search, or changing `get_entry` resolution order.
- Additional books or prose-book chunking. Entry-level retrieval only in this phase.

## 2. Baseline and constraints

Measured on the reference dictionary before enhancement:

| Metric | Value |
|---|---:|
| Main entries | 5,803 |
| Appendix entries | 1,236 |
| Dictionary Markdown | 7.05 MB |
| Resolved internal links | about 14,000 |
| Executable | 1.87 MB |
| Lookup cache | 1.73 MB |
| Idle private memory | 38 MB |
| Peak working set | 49 MB |
| Load time | about 0.3–0.5 s |
| Query | in-memory lexical scan, milliseconds |
| Network/database/model | none |

Constraints that remain in force:

- `stdout` carries JSON-RPC only; diagnostics go to stderr.
- The archive, extracted Markdown, linked copy, pages, lookup cache, and semantic index are never
  committed and never enter `dist/`. Model weights live in the Ollama store, not in this repository.
- No remote network and no telemetry. When semantic retrieval is enabled, the only network call is
  HTTP to a loopback Ollama server (`127.0.0.1`, `localhost`, or `::1`). Ollama Cloud is not used.
- Tools publish the narrow JSON Schema subset documented in `CLAUDE.md`.
- Output schemas are derived from response types through `src/schema.rs`.
- `tools/list` is snapshotted in `tests/fixtures/tools-list.json`; an intentional change is a
  reviewed diff.
- Malformed source data produces warnings, never a startup crash after the archive is found.
- No commit is made until the complete enhancement plan has passed every gate.

## 3. Design decisions

| ID | Decision | Rationale |
|---|---|---|
| D1 | Graph nodes are main dictionary entries; edges are resolved `Entry.links` | These links are already parsed, verified, and cited; no new extraction or heuristics |
| D2 | Graph is exposed as a new tool, `related_entries` | It is a distinct operation, not a ranking mode |
| D3 | Traversal is bounded to depth 1–3, 50 returned nodes, 10,000 visited nodes | Keeps latency and output bounded on a hub-heavy graph |
| D4 | Semantic retrieval is an optional Cargo feature and CLI configuration | The default binary keeps its 38 MB / 1.87 MB profile |
| D5 | Vectors are stored flat and searched by exact cosine | About 7,000 × 768–1024 floats is a few milliseconds; HNSW is unjustified complexity at this size |
| D6 | One vector per searchable document (entry or appendix entry) | Directly maps results to existing citations; no chunk bookkeeping |
| D7 | Embeddings come from the user's local Ollama server, not an in-process model | The user already runs Ollama; model memory stays outside the MCP process, and no weights enter the repository |
| D8 | The runtime contract is Ollama's local `/api/embed`; the default build does not link an HTTP client | The default build stays lexical-only; the semantic feature adds one small blocking HTTP dependency |
| D9 | Semantic results are fused with RRF only after the lexical gate opens | Prevents semantic near-misses from outranking exact terminology |
| D10 | `MatchKind::Semantic` is added; the rest of the search response shape is unchanged | Makes the provenance of a hit explicit without a second response schema |
| D11 | No commit until all graph, semantic, evaluation, documentation, and release gates pass | Explicit user instruction |
| D12 | A model is identified by name **and** digest; vectors are never migrated between models | An updated tag is a different embedding space; reusing old vectors would silently corrupt ranking |
| D13 | A missing/stale model or index degrades to lexical by default; `--require-semantic` makes it fatal | Keeps dictionary search available if Ollama is down or a model is withdrawn; strict deployments can fail closed |
| D14 | Indexes are digest-keyed and retained until an explicit prune | A new model can be evaluated and rolled back without deleting the last known-good index |
| D15 | Pure pinyin queries skip semantic embedding and use the pinyin indexes plus fuzzy matching | Raw pinyin is romanization, not natural language; tested models rank unrelated pinyin pairs as similar as related ones |
| D16 | Each model profile pins an Ollama `num_ctx`; the native maximum is not used by default | Qwen3 at its native 32K context loaded 4.6 GB; at 4K it loaded 1.2 GB with no useful retrieval loss for entry-level vectors |
| D17 | Exactly one embedding model and one index are active per process; other model indexes are cache artifacts only | Query-time multi-model routing would need multiple embeddings, incompatible score spaces, and 3+ GB of model memory for no demonstrated gain |
| D18 | Model switching is a configuration-and-restart operation, never an MCP tool | An LLM-callable switch would be an unreviewed administrative action, races in-flight calls, and makes results non-reproducible mid-session |
| D19 | Pinyin hardening is deferred; evaluation weights English and mixed queries | The reference usage is mostly English with occasional pinyin/Chinese; building a pinyin subsystem now would optimize a rare path |

## 4. Graph traversal

### 4.1 Data model and build

Add `src/dictionary/graph.rs` and response types in `model.rs`.

```rust
pub struct GraphEdge {
    pub neighbor: usize,   // entry index
    pub owner: usize,      // entry index that owns the Link
    pub link: usize,       // index into owner.links
}

pub struct Graph {
    pub outgoing: Vec<Vec<GraphEdge>>,
    pub incoming: Vec<Vec<GraphEdge>>,
}
```

Build rules:

1. Build after `index::resolve_entries` and after every entry has its final anchor.
2. Build `entry_by_anchor: HashMap<&str, usize>` from entries with anchors.
3. For each entry and each `link` where `link.resolved` and `link.target_anchor` is present,
   resolve the anchor to an entry index. If the anchor names a subentry paragraph rather than an
   entry container, fall back to `(link.target_file, normalize(link.target_term))` and use the
   containing entry.
4. Add an outgoing edge on the source and an incoming edge on the target.
5. Keep a link whose target cannot be mapped in the report as `graph_unmapped_links`; do not
   invent a node.
6. Preserve source order: entries in load order, links in body order. This makes traversal
   deterministic.
7. Do not include appendix entries as graph nodes in this phase. Appendix relationships already
   appear through `EntryResponse.related` for point identifiers.

Add to `LoadReport`:

- `graph_nodes`
- `graph_edges`
- `graph_unmapped_links`

The graph is immutable after startup, like the rest of `Dictionary`. Expected memory is under
3 MB for the reference corpus.

### 4.2 Tool contract

New tool: `related_entries`.

| Argument | Type | Default | Notes |
|---|---|---|---|
| `term` | string, required | — | Headword, alias, pinyin, Chinese, citation anchor, or other term `get_entry` can resolve to a **main dictionary** entry |
| `direction` | enum, optional | `both` | `outgoing`, `incoming`, `both` |
| `depth` | integer, optional | `1` | 1–3; values outside the range are errors |
| `limit` | integer, optional | `20` | 1–50; values outside the range are errors |

Structured response (derived schema):

```json
{
  "query": "alpha",
  "term": "alpha",
  "anchor": "a1ALP",
  "direction": "both",
  "depth": 1,
  "nodes": [
    {
      "source": "dictionary",
      "term": "greater alpha",
      "anchor": "a1BET",
      "file": "14-g.md",
      "citation": "Test Dictionary, 14-g.md#a1BET",
      "distance": 1,
      "edge_direction": "outgoing",
      "via": "alpha",
      "link_text": "greater alpha",
      "redirect": false
    }
  ],
  "total": 1,
  "truncated": false
}
```

Optional fields are omitted when absent:

- `id`
- `anchor`
- `source_file`
- `line`

`source_file` and `line` follow the existing rules: absolute only when the data is an extracted
directory.

### 4.3 Traversal algorithm

- Resolve `term` through the same precedence as `get_entry`'s dictionary path: anchor → exact
  normalized headword → lookup index → exact alias → prefix → substring. Extract this into one
  shared `resolve_dictionary_entry` function so graph and `get_entry` cannot drift.
- If no dictionary entry matches, return `DICTIONARY_NOT_FOUND` with the normal suggestions. If the
  term resolves only to an appendix entry, return a clear not-found message saying the relationship
  graph covers main dictionary entries and pointing to `get_entry`.
- Breadth-first search from the start entry.
- `outgoing` follows `Graph.outgoing`; `incoming` follows `Graph.incoming`; `both` follows both,
  with outgoing edges first and duplicate neighbours collapsed.
- `visited` is seeded with the start entry, so cycles terminate.
- Each discovered node records:
  - shortest `distance`;
  - the edge direction by which it was first reached;
  - `via`, the term of the entry that owned that edge;
  - `link_text`, the visible link text.
- If a node is reachable by several parents at the same distance, the first deterministic edge in
  BFS order wins.
- Sort the final nodes by `(distance, entry index)` so output does not depend on hash iteration.
- `total` is the number of unique discovered nodes before `limit`.
- `truncated` is true when `limit` cut the output or the 10,000-node visit cap was reached.
- `MAX_GRAPH_DEPTH = 3`, `MAX_GRAPH_LIMIT = 50`, `MAX_GRAPH_VISITED = 10_000` are constants with
  tests at both sides of each bound.

### 4.4 Errors, citations, and text

- Empty `term` → `DICTIONARY_INVALID_REQUEST`.
- Non-integer or out-of-range `depth`/`limit` → JSON-RPC Invalid Params via
  `ToolArgumentError`, matching the existing tools.
- Unknown tool message and server instructions list `related_entries`.
- Citations use a shared helper extracted from `search.rs`:
  - canonical: `<title>, <file>#<anchor>` or `<title>, <file>:<line>`;
  - absolute: `Source: <absolute path>:<line>` when `source_file` exists.
- Text rendering is compact Markdown:

```text
Related entries for "alpha" (both, depth 1): 1 result

1. greater alpha — dictionary · C:\data\14-g.md:3 · anchor a1BET
   outgoing · via alpha · link "greater alpha"

To read one, call get_entry with its term or anchor.
```

- No bare relative citation appears in tool text; the existing rule is enforced by a test.

### 4.5 Graph tests

Unit and integration coverage must include:

- edge counts and direction on the synthetic fixture;
- outgoing, incoming, and both;
- depth 1, 2, 3;
- a cycle does not loop or duplicate nodes;
- duplicate links to the same target produce one node;
- unresolved links are absent and counted;
- anchorless start and target entries still cite `file:line`;
- a subentry anchor (`a1SSD` style) resolves to its containing entry;
- limit and depth bounds accept the edge values and reject one below/above;
- limit truncation sets `truncated` and `total` correctly;
- appendix-only term produces the documented error;
- empty dictionary and unknown term do not panic;
- deterministic output across repeated runs;
- stdio e2e call, text rendering, absolute `Source:` line, schema validation, snapshot match.

## 5. Gated hybrid retrieval

### 5.1 Components

```text
search_dictionary
      │
      ▼
search::search_lexical  ──► best lexical score >= 300 ──► return lexical
      │
      │ best < 300 and semantic engine configured
      ▼
semantic::SemanticEngine
      ├── OllamaEmbedder (loopback /api/embed)
      └── SemanticIndex (flat vectors + LookupRef records)
      │
      ▼
RRF merge with lexical hits ──► SearchResponse
```

Proposed modules:

- `src/semantic/mod.rs` — engine, configuration, document construction.
- `src/semantic/embedder.rs` — `Embedder` trait and the `OllamaEmbedder`.
- `src/semantic/ollama.rs` — local Ollama HTTP client, tags/digest parsing, timeouts.
- `src/semantic/profiles.rs` — known model profiles and the override file.
- `src/semantic/index.rs` — flat vector index, metadata, cache read/write.
- `src/semantic/fusion.rs` — gating, semantic hit construction, RRF, deduplication.
- `src/semantic/status.rs` — `--check-semantic` report.

`Embedder` is a trait so all retrieval logic is testable with a deterministic fake and with a mock
Ollama HTTP server:

```rust
pub trait Embedder: Send + Sync {
    fn dimensions(&self) -> usize;
    fn embed_documents(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, String>;
    fn embed_query(&self, text: &str) -> Result<Vec<f32>, String>;
}
```

The real implementation is the Ollama HTTP client. Ollama owns the model session and serialises
requests internally; the MCP process holds no model weights.

### 5.2 Ollama service and model profiles

The server talks only to a **loopback** Ollama endpoint. The default is
`http://127.0.0.1:11434`; `localhost` and `::1` are accepted. An `https://ollama.com` URL is
rejected with a configuration error. Ollama Cloud has no embedding models at present; this design
does not depend on it.

Endpoints:

| Endpoint | Use |
|---|---|
| `GET /api/tags` | Confirm the model exists and read its digest |
| `POST /api/embed` | Embed one query or a batch of documents |
| `GET /api/show` | Optional diagnostics: family, parameter size, context metadata |

Embedding request:

```json
{
  "model": "qwen3-embedding:0.6b",
  "input": ["Instruct: Given a web search query, retrieve relevant passages that answer the query\nQuery:qi deficiency"],
  "truncate": true,
  "keep_alive": "30m",
  "options": { "num_ctx": 4096 }
}
```

`num_ctx` is profile-driven. Qwen3 loads at 4.6 GB with its native 32K context and at 1.2 GB
with 4K; entry-level vectors do not need 32K.

Response fields used: `embeddings`, `model`, `prompt_eval_count`, `total_duration`,
`load_duration`.

Model identity:

- The configured name, for example `embeddinggemma:latest` or `bge-m3`.
- The **digest** reported by `/api/tags` at build and startup. A tag can move to a new digest; that
  is a different embedding space.
- The detected vector dimensions from a probe embedding.
- The embedding-affecting profile fields: query prefix/instruction and document prefix.

A small profile table supplies model-specific behaviour. Unknown models get the default profile
and a warning.

| Model family | Query formatting | Document prefix | Notes |
|---|---|---|---|
| `embeddinggemma` | none | none | 768 dims measured; 2K context; **CPU-friendly reference primary** for English/mixed retrieval |
| `bge-m3` | none | none | 1024 dims; 8K context; long-entry challenger; supports reduced output dimensions |
| `qwen3-embedding` | `Instruct: <task>\nQuery:<text>` | none | 1024 dims for 0.6b; 32K native but profile sets `num_ctx=4096`; Chinese-heavy option, slower on CPU |
| `nomic-embed-text` | `search_query: ` | `search_document: ` | 768 dims; prefix-sensitive |
| `mxbai-embed-large` | `Represent this sentence for searching relevant passages: ` | none | 1024 dims; English-only |
| default | none | none | Detected dimensions; warn that formatting may be suboptimal |

`--embedding-profile <path>` accepts a small JSON override:

```json
{
  "query_prefix": "",
  "document_prefix": "",
  "query_instruction": null,
  "min_similarity": 0.30,
  "num_ctx": 4096
}
```

S4 selected `embeddinggemma:latest` as the CPU-friendly reference primary for the English/mixed
profile: its 2K context indexed the reference corpus in a practical time, while Qwen3's stronger
non-exact Chinese separation costs much more CPU time per document. `qwen3-embedding:0.6b` remains
the documented Chinese-heavy option, and `bge-m3:latest` is the long-entry challenger if 8K
context proves necessary. Pinyin remains a fuzzy-matching problem, not an embedding problem. The
implementation stays model-agnostic; the model name, digest, dimensions, context, and profile are
index metadata, not compiled constants.

**One active model only.** The process loads one model and one digest-matched index. Indexes for
other models may exist on disk after evaluation or before pruning, but they are never loaded
concurrently and never fused. A query is embedded once, by the configured model. Runtime model
routing, multi-model RRF, and vector concatenation are out of scope; they would multiply latency,
memory, and failure modes without evidence that they improve retrieval.

Only the embedding-affecting fields are hashed into the index key. Changing `min_similarity` does
not require re-embedding; changing a prefix or instruction does.

The S0 gate proves the local API contract before the rest is built:

1. `GET /api/tags` returns the model and a stable digest.
2. `POST /api/embed` returns a finite unit vector and the expected dimensions.
3. Batch input, `truncate: true`, and `keep_alive` behave as documented.
4. No Cloud endpoint is contacted.
5. Windows loopback latency, throughput, and model RSS are measured.
6. A small HTTP client is chosen and pinned. `ureq` is the preferred blocking client; `reqwest` is
   acceptable only if the spike shows a concrete need.

Cargo shape:

```toml
[features]
default = []
semantic = ["dep:ureq"]

[dependencies]
ureq = { version = "…", optional = true, features = ["json"] }
```

Exact versions are pinned during the spike. The default build must not compile or ship the HTTP
client.

### 5.3 Index format and cache invalidation

Index file default:

```text
<cache-dir>/<data-stem>.semantic-cache-v2.<dictionary-fingerprint>.<model-key>.<digest-prefix>.bin
```

`model-key` is a sanitised model name. `digest-prefix` is the first 16 hex characters of the
Ollama digest. A new digest therefore gets a new file; it never overwrites the last known-good
index.

Header:

- magic and format version;
- package version;
- dictionary content fingerprint;
- model name and full model digest;
- dimensions, document count;
- embedding-affecting profile hash;
- record count bounds checks.

Records:

- `source` byte (`dictionary`, `meridian`, `medicinal`, `formulas`, `weights`);
- `index` as `u32`;
- `dimensions` × `f32` little-endian unit vector.

Rules:

- Reject files above a bounded size; reject `dimensions > 4096`, count over the document bound, or
  a short/truncated record.
- Reject NaN or infinite components and vectors whose norm is not close to 1.
- Write atomically: temporary file in the same directory, then rename.
- Do not auto-prune across model digests. `--prune-semantic-index` removes files that do not match
  the currently configured model and digest, and requires explicit confirmation.
- A stale fingerprint, model digest, dimensions, or profile hash is not silently reused. By default
  the server starts lexical-only with a warning and the exact rebuild command; with
  `--require-semantic` it is a startup error.
- Extract a `content_fingerprint(sources)` function that does not depend on the lookup-cache
  version, and use it for both caches. This intentionally rebuilds the lookup cache once when the
  fingerprint function changes.

### 5.4 Document construction

One vector per searchable document, in deterministic order:

1. main dictionary entries;
2. meridian appendix entries;
3. medicinal appendix entries;
4. formula appendix entries;
5. weights sections.

Text:

- Main: headword, aliases (simplified, traditional, tone-marked, tone-numbered), then body.
- Appendix: term, identifiers, aliases, then body.
- Weights: section number, title, then body.
- Use plain text, not HTML or Markdown control characters.
- Apply the profile's `document_prefix`.
- Send the text with Ollama's `truncate: true`; do not character-truncate in the client. The
  profile may set a conservative `max_input_chars` guard so one malformed entry cannot create an
  unbounded request. Log when a document is likely truncated.

The index records `LookupRef { source, index }`, so a semantic hit maps directly to the existing
hit builders and citations. Weights use `Source::Weights` with the section index.

### 5.5 Retrieval and fusion

Gate:

```text
if lexical.best_score >= 300            -> return lexical response
if semantic engine is not configured    -> return lexical response
if query is pinyin-only                 -> use pinyin index + fuzzy suggestions; do not embed
otherwise                               -> embed the query once and run cosine search
```

The threshold is a named constant (`HYBRID_GATE_SCORE = 300`) with tests on both sides. Evaluation
may change the constant, but it is not a user-facing option in this phase.

**Pinyin boundary.** A pure pinyin query (`qi xu`, `qixu`, `qi4 xu1`, `jiǎ gēn`, with no Han or
English context) must not be sent to the embedding model. The model has no reliable notion of
pinyin syllables, and the S0 checks showed unrelated pinyin pairs scoring as high as related ones.
The existing pinyin indexes and fuzzy matcher are the correct path; a syllable-aware scorer is a
separate hardening task, not part of the semantic index. Non-exact **Chinese** queries, including
traditional/simplified forms and synonym paraphrases, do use the lexical path and then the
semantic fallback.

Cosine search:

- L2-normalized vectors, so score is the dot product.
- Take the top 20 candidates.
- Discard candidates below `min_similarity`.
- Exact document count and dimensions are validated at load; no per-query allocation of the full
  matrix.

Fusion:

- Use reciprocal-rank fusion with `k = 60`.
- Lexical rank and semantic rank each contribute `1 / (60 + rank)`.
- A semantic-only hit gets `matched_in = "semantic"` and a display `score` of
  `200 + round(99 × similarity)`, capped at 299, so it is visibly a weak/fallback match.
- A hit present in both lists appears once; its lexical `matched_in` and score are retained, while
  its position may improve through RRF.
- Any lexical hit with score ≥ 600 is pinned ahead of semantic-only candidates. In normal operation
  the gate already prevents this case; the pin is a safety rule with a test.
- Deduplicate by `LookupRef`, not by display text or snippet. This requires internal candidates to
  carry their `LookupRef`; public response fields do not change.
- Order ties by lexical score, then source rank, then book order.
- `total` is the merged unique candidate count before `limit`.
- Existing fuzzy suggestions remain when the merged result is still weak.

Semantic hit construction reuses the lexical snippet helpers and citation helper. A redirect entry
is marked `redirect` exactly as in lexical search.

### 5.6 Configuration

New flags and environment variables:

| Flag | Environment | Meaning |
|---|---|---|
| `--ollama-url <url>` | `MCP_CHINESE_DICT_OLLAMA_URL` | Loopback Ollama base URL; default `http://127.0.0.1:11434` |
| `--embedding-model <name>` | `MCP_CHINESE_DICT_EMBEDDING_MODEL` | Ollama embedding model; supplying it enables gated hybrid |
| `--embedding-profile <path>` | `MCP_CHINESE_DICT_EMBEDDING_PROFILE` | Optional JSON profile override |
| `--semantic-index <path>` | `MCP_CHINESE_DICT_SEMANTIC_INDEX` | Override the derived digest-keyed index path |
| `--build-semantic-index` | — | Build a new index for the current model/digest, then exit |
| `--check-semantic` | — | Print model, digest, dimensions, index path/status, and remediation, then exit |
| `--prune-semantic-index` | — | Remove indexes that do not match the current model/digest, then exit |
| `--require-semantic` | `MCP_CHINESE_DICT_REQUIRE_SEMANTIC` | Fail startup when the model/index is unavailable; default is lexical fallback |

Rules:

- No `--embedding-model` means lexical-only; no Ollama call, index load, or added memory.
- `--build-semantic-index` requires `--embedding-model` and `--data`; it is mutually exclusive with
  `--build-indices` and `--build-linked-data`.
- Serving with `--embedding-model` loads the digest-matched index. Missing or stale index/model
  starts lexical-only with a prominent warning by default. `--require-semantic` turns that into a
  startup configuration error. It is never rebuilt silently.
- `--check-semantic` never writes; it reports the resolved model name, digest, dimensions, expected
  and found index paths, record count, and whether semantic will be active.
- `--prune-semantic-index` is the only command that deletes old indexes; it lists what it will
  remove and requires `--yes` or an interactive confirmation.
- The Ollama URL must be loopback. `https://ollama.com` and other hosts are rejected in this phase.
- If the binary was built without the `semantic` feature, using any semantic flag returns a clear
  configuration error naming the required build feature.
- No MCP tool switches the embedding model or index. The tool surface is for dictionary retrieval;
  model selection is deployment configuration. Switching requires changing `--embedding-model` and
  restarting the stdio process, which reloads and validates the matching digest-keyed index.
- `--help`, README, `dist/INSTALL.md`, and the configuration table move together.

### 5.7 Concurrency, cancellation, and failure

- Ollama inference is an HTTP call. With semantic enabled, the hybrid search runs in
  `tokio::task::spawn_blocking`; cancellation is checked before and after. The HTTP request has a
  bounded timeout (30 s query, 300 s build), so a hung Ollama cannot hang the server forever.
- `keep_alive` is set to `30m` for query and build requests, reducing model reload stalls. A first
  query after Ollama has unloaded the model may take seconds; this is logged.
- The index is immutable and loaded once. No request path writes to it; only
  `--build-semantic-index` and `--prune-semantic-index` do.
- The model digest is checked at startup and re-checked at most every 300 s while semantic is
  active. If the digest changes, semantic is disabled for the rest of the process, a warning names
  the rebuild command, and lexical search continues. This prevents old vectors being compared with
  a newly updated model.
- If Ollama is unreachable at startup, or the model is absent: lexical-only by default with a
  warning; `--require-semantic` makes it a startup error. If Ollama fails during a query, log and
  return the lexical response.
- No request writes to the Ollama model store. The only network calls are loopback `/api/tags` and
  `/api/embed`; remote URLs are rejected.

### 5.8 Semantic tests

All retrieval behaviour is tested with a fake embedder, and the Ollama client is tested against a
local mock HTTP server. The normal suite needs no real model:

- strong lexical query (best ≥ 300) never calls `embed_query`;
- exact anchor, exact pinyin, exact Han, and exact alias never call `embed_query`;
- a pure pinyin query (including a typo or tone-number variant) never calls `embed_query`;
- a non-exact Chinese synonym query may call the embedder once;
- weak lexical query calls the embedder once;
- a semantic-only hit is returned with `matched_in = "semantic"`, score 200–299;
- a hit in both lists is deduplicated and keeps lexical provenance;
- RRF ordering is deterministic and matches a hand-computed expectation;
- semantic hits do not outrank a pinned lexical hit ≥ 600;
- similarity below the threshold is discarded;
- embedding failure, HTTP timeout, malformed JSON, and Ollama 5xx fall back to lexical;
- cancellation before/after embedding is honoured;
- `limit` and `total` remain correct after fusion;
- index round-trip preserves vectors and refs;
- stale fingerprint, model-digest mismatch, dimensions mismatch, profile-hash mismatch, truncated
  file, oversized header, NaN, infinite value, zero-norm vector, and wrong record count fail with
  actionable errors;
- building an index from the synthetic corpus is deterministic across two runs;
- default build and default configuration do not allocate a semantic engine;
- mock Ollama tests cover model present, model removed (404), digest changed, dimension changed,
  duplicate model entries, and `/api/tags` unavailable;
- profile tests cover Qwen3's `Instruct: …\nQuery:…` framing, no instruction on documents, and
  `num_ctx=4096`; profile hashes change when formatting or context changes;
- `--check-semantic` reports each state correctly and never writes;
- `--build-semantic-index` writes a new digest-keyed file and leaves the old file intact;
- `--prune-semantic-index` removes only non-matching indexes and requires confirmation;
- a real-Ollama smoke test runs only when `MCP_CHINESE_DICT_OLLAMA_URL` and
  `MCP_CHINESE_DICT_EMBEDDING_MODEL` are set, and otherwise skips with an explicit reason.

### 5.9 Model lifecycle and migration

An Ollama tag is mutable. Pulling a new `:latest`, changing a tag, or the model being withdrawn
upstream can change or remove the model. Vectors are model-specific and cannot be reused across
digests. The lifecycle is therefore explicit.

#### Model identity

- **Name**: `embeddinggemma:latest`, `bge-m3`, etc.
- **Digest**: from `GET /api/tags`.
- **Dimensions**: detected from a probe embedding.
- **Profile hash**: query/document formatting fields that affect the embedding.

All four are stored in the index header. A mismatch is stale, not usable.

#### Routine model update

1. Inspect the current model and index:

   ```sh
   mcp-chinese-dict-search --data <path> --embedding-model embeddinggemma:latest --check-semantic
   ```

2. Pull the replacement model or tag:

   ```sh
   ollama pull bge-m3
   ```

3. Re-check the replacement; record its digest and detected dimensions:

   ```sh
   mcp-chinese-dict-search --data <path> --embedding-model bge-m3 --check-semantic
   ```

4. Build a new digest-keyed index. The old index is retained:

   ```sh
   mcp-chinese-dict-search --data <path> --embedding-model bge-m3 --build-semantic-index
   ```

5. Optionally evaluate the new model against the old one on the held-out query set (§5.10). Do not
   delete the old model or index until the new one is accepted.

6. Update the MCP configuration to the new `--embedding-model` and restart the server. The digest
   is detected at startup; it is not manually configured.

7. After acceptance, prune old indexes explicitly:

   ```sh
   mcp-chinese-dict-search --data <path> --embedding-model bge-m3 --prune-semantic-index --yes
   ```

#### Model withdrawn or unavailable

- A model already pulled into the local Ollama store remains usable. Do not run `ollama rm` on the
  last known-good model.
- If the local copy is gone and the registry no longer serves it, choose a replacement model,
  build a new index, evaluate, then update the configuration. There is no vector conversion path.
- Until a replacement is built, the server runs lexical-only (or fails startup with
  `--require-semantic`). `--check-semantic` prints the exact remediation.
- Keep model backups outside this repository if the model is operationally critical. The Ollama
  model store is the source of truth; do not copy weights into `dist/`.

#### Rollback

- Keep the old model locally and its digest-keyed index. Revert `--embedding-model` (and the
  matching index path, if explicitly set) and restart.
- Do not delete the old index until the new model has passed evaluation and a real query check.

### 5.10 Evaluation protocol and acceptance gate

The feature is not accepted on plausibility. It must beat lexical retrieval on a held-out query
set.

1. Build a local query set of at least 30 queries whose expected result is known. Queries may be
   invented paraphrases; the query file and results stay local and are not committed if they are
   derived from the reference book.
2. Run lexical-only and hybrid against the same dictionary and model.
3. Record recall@5, MRR, top-1 accuracy, p50/p95 latency, number of embedding calls, startup time,
   and RSS. Run Qwen3 with its instruction and `num_ctx=4096`.
4. Weight the query set toward English and mixed English/pinyin queries. Include exact/alias,
   pinyin-only, and Chinese queries as regression checks; pinyin must stay on the lexical/fuzzy
   path.

Acceptance:

- Hybrid recall@5 improves by at least **10 percentage points** over lexical on the weak-query
  subset; or, if the baseline is already above 90%, by at least 5 percentage points.
- No more than one top-1 regression among exact queries.
- p95 added latency for a gated query is at most **150 ms** on the reference machine with the
  selected model and a warm Ollama process.
- MCP server RSS increase is at most **50 MB** (the index plus HTTP client). Ollama's model memory
  is measured separately and is expected to be 0.6–1.5 GB.
- Model plus index disk is at most **1.3 GB** for the selected model; the index alone must stay
  under **35 MB** for the reference corpus.
- If the gate is not met, semantic retrieval is not enabled in the release, and the negative
  result is recorded. Code may be removed or left behind the feature flag only with an explicit
  decision.

### 5.11 Cost budget

| Item | Target |
|---|---|
| Graph RAM | < 5 MB |
| Graph depth-2 query | < 10 ms |
| MCP process semantic RSS | ≤ 50 MB (index + HTTP client) |
| Ollama model RSS | 0.6–1.5 GB, depending on model; outside the MCP process |
| Semantic index RAM/disk | 22–35 MB for the reference corpus |
| Model disk | 622 MB (`embeddinggemma`) to 1.2 GB (`bge-m3`) |
| Semantic query embedding | 20–60 ms warm; seconds after model unload |
| Batch index build | ~50–85 documents/s for short entries |
| Gated query p95 total | ≤ 150 ms added, warm |
| MCP startup increase | ≤ 1 s when Ollama already has the model loaded |
| GPU | not required; Ollama may use one |

These are gates, not aspirations. Measurements go in the checklist.

## 6. Files and surfaces

| Area | Files |
|---|---|
| Graph data | `src/dictionary/graph.rs`, `model.rs`, `mod.rs` |
| Graph sharing | `src/dictionary/citation.rs` (new) or a shared helper |
| Tool | `src/tools.rs`, `src/server.rs`, `src/error.rs` |
| Schema | `src/schema.rs`, `tests/fixtures/tools-list.json` |
| Semantic core | `src/semantic/*` (`ollama.rs`, `profiles.rs`, `status.rs`), `src/dictionary/search.rs`, `model.rs` |
| Config | `src/config.rs`, `src/cli.rs` |
| Dependencies | `Cargo.toml`, `Cargo.lock` (optional `semantic` feature + HTTP client) |
| Tests | unit modules, mock Ollama server, `tests/tools_e2e.rs`, `tests/schema_contract.rs`, `tests/stdio_contract.rs`, `tests/support/*`, optional `tests/semantic_real.rs` |
| Docs | `README.md`, `PLAN.md`, `CHECKLIST.md`, `CLAUDE.md` if instructions change |
| Deployment | `dist/INSTALL.md`, `dist/install.ps1`, `dist/mcp-config.example.json`, `BUILDING.md`, model migration instructions |
| Ignore rules | `.gitignore` for semantic caches, local evaluation files, and any model backup outside Ollama |

Every tool or schema change requires the reviewed snapshot update:

```sh
MCP_UPDATE_TOOLS_LIST=1 cargo test --test schema_contract
```

## 7. Phases and exit conditions

### Phase E0 — Plan and guardrails

Create this plan and the checklist; record the base commit and the no-commit rule.

**Exit:** both documents exist, cross-link, and contain measurable gates.

### Phase G1 — Graph core

Implement graph data, build it from resolved links, add shared resolution and citation helpers,
add report counts, and test traversal.

**Exit:** unit tests pass; graph counts on the synthetic fixture are exact; no tool surface changed
yet.

### Phase G2 — Graph tool

Add `related_entries`, validation, rendering, server wiring, instructions, e2e and schema tests,
and the reviewed snapshot.

**Exit:** `tools/list` publishes three tools; schema contract validates a graph payload; e2e text
contains absolute `Source:` lines and no bare relative citation.

### Phase I1 — Graph PNG rendering

Add the `render_graph` tool, a layered PNG renderer, image content blocks, file output, schema,
tests, and documentation.

**Exit:** PNGs decode and open, paths are absolute, image content and text fallback are both
present, schema contract and snapshot pass, and the tool is documented.

### Phase S0 — Ollama capability gate

Prove the local Ollama API contract on Windows x64 before semantic implementation proceeds.

**Exit:** `/api/tags` and `/api/embed` work with a loopback URL and a local model; a finite unit
vector with expected dimensions is returned; batch input, `truncate`, and `keep_alive` behave as
documented; Cloud is not contacted; latency, throughput, and model RSS are measured; go/no-go is
recorded. No further semantic work starts on a failed gate.

### Phase S1 — Semantic core with a fake embedder

Implement the embedder trait, document construction, flat cosine index, metadata, cache, gating,
fusion, `MatchKind::Semantic`, and all hermetic tests.

**Exit:** the full fake-embedder suite passes; default build unchanged; no model required for CI.

### Phase S2 — Ollama embedder, index build, and CLI

Add the Ollama HTTP client, model profiles, digest handling, index build/load, flags,
spawn-blocking path, logging, degradation policy, and an environment-gated real-Ollama smoke test.

**Exit:** build and serve paths work with a local model; corrupt/stale inputs behave as documented;
real smoke test passes when configured and skips cleanly otherwise.

### Phase S3 — Model lifecycle and migration

Implement `--check-semantic`, digest re-checking, digest-keyed rebuilds, explicit pruning, strict
mode, and the documented migration/rollback procedure.

**Exit:** a model digest change is detected and semantic is disabled or rejected according to
policy; a new index can be built without deleting the old one; rollback works; instructions are in
README and `dist/INSTALL.md`.

### Phase S4 — Evaluation

Run lexical vs hybrid on the held-out set; compare candidate models if available; record recall,
MRR, latency, memory, and startup.

**Exit:** acceptance thresholds in §5.10 are met, or semantic release is disabled with a recorded
negative result.

### Phase D — Documentation, verification, and one commit

Update every surface, bump the version to `0.6.0` if accepted, run all gates, build the bundle,
and make one commit after everything is verified. Do not push unless asked.

**Exit:** `cargo fmt --check`, clippy with `-D warnings`, `cargo test --all-targets`, real-data
tests, schema snapshot, evaluation, and bundle checks all pass; the working tree is reviewed; one
commit exists; nothing derived from the book is tracked.

## 8. Verification gates

Run from the repository root:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

With the real dictionary configured:

```sh
MCP_CHINESE_DICT_DATA="C:/ProgramData/mcp-dict-data" cargo test --test real_data
MCP_CHINESE_DICT_DATA="C:/ProgramData/mcp-dict-data" cargo test --test tools_e2e
```

With the semantic feature and a local Ollama model:

```sh
cargo clippy --all-targets --features semantic -- -D warnings
cargo test --all-targets --features semantic
MCP_CHINESE_DICT_DATA="C:/ProgramData/mcp-dict-data" \
MCP_CHINESE_DICT_OLLAMA_URL="http://127.0.0.1:11434" \
MCP_CHINESE_DICT_EMBEDDING_MODEL="embeddinggemma:latest" \
MCP_CHINESE_DICT_SEMANTIC_INDEX="C:/ProgramData/mcp-dict-semantic/index.bin" \
cargo test --features semantic --test semantic_real
```

Lifecycle commands:

```sh
mcp-chinese-dict-search --data <path> --embedding-model embeddinggemma:latest --check-semantic
mcp-chinese-dict-search --data <path> --embedding-model embeddinggemma:latest --build-semantic-index
mcp-chinese-dict-search --data <path> --embedding-model bge-m3 --build-semantic-index
mcp-chinese-dict-search --data <path> --embedding-model bge-m3 --prune-semantic-index --yes
```

Manual checks:

- `related_entries` on a known entry in all three directions and depths 1 and 2.
- No `related_entries` result contains a bare relative citation.
- Lexical-only server makes no Ollama call and loads no index.
- Hybrid server logs the gate decision, embedding time, model name, digest, and dimensions.
- Corrupt index, stale index, digest change, wrong dimensions, absent model, and Ollama down each
  follow the documented degrade/strict policy.
- `--check-semantic` identifies the model, digest, expected index, and remediation.
- A new digest writes a new index and leaves the old index intact.
- A remote `--ollama-url`, including Ollama Cloud, is rejected.
- The default build does not link the HTTP client.
- `git status` shows no archive, Markdown data, model, index, query file, or evaluation result.

## 9. Risks and mitigations

| Risk | Mitigation |
|---|---|
| Semantic near-misses reduce trust | Gate at weak lexical scores; pin strong lexical hits; label `semantic`; citations unchanged |
| Medical accuracy is harmed by fuzzy matches | Exact terminology remains first; semantic results are only related entries, never generated text |
| Ollama daemon or loopback dependency | Feature off by default; startup health check; lexical fallback; `--require-semantic` for strict deployments |
| Ollama model withdrawn or tag updated | Model name + digest identity; digest re-check; digest-keyed indexes; `--check-semantic`; documented rebuild/rollback |
| Binary or memory growth | Feature off by default; model memory stays in Ollama; measure and enforce §5.11 budget |
| Model licensing or distribution | Ollama store owns weights; never commit or ship them; document licence responsibility |
| Cache staleness or model mismatch | Content fingerprint + model digest + dimensions + profile hash + format version; degrade or fail per policy |
| Loopback HTTP looks like “network access” | Explicitly scoped rule: loopback Ollama only, remote/Cloud URLs rejected, no telemetry |
| Graph becomes a hub explosion | Depth 3, 50 output nodes, 10,000 visited cap, deterministic ordering |
| Graph and `get_entry` resolution drift | One shared resolver and citation helper; tests compare both |
| Concurrency blocks the runtime | `spawn_blocking` for inference; bounded input; gate makes calls rare |
| Evaluation is overfitted or fictional | Held-out queries, exact-query regression set, explicit pass/fail, record negative results |
| Scope creep into Graph RAG or book chunking | Listed as non-goals; no LLM at query time |

## 10. Rollback and disabled behaviour

- Graph can be removed by deleting the tool and graph module; it does not affect lexical search.
- Semantic is off unless both the feature is compiled and `--embedding-model` is supplied.
- If semantic is disabled, Ollama is unavailable, the digest changed, or a query embedding fails,
  the response is the existing lexical response (unless `--require-semantic` made startup fail).
- If evaluation fails, the default release stays lexical + graph; semantic remains a documented,
  feature-gated experiment or is removed.
- Model rollback: keep the old model in Ollama and its digest-keyed index; revert
  `--embedding-model` and restart. Never delete the last known-good index until the replacement is
  accepted.
- No migration is required for existing users: `--data`, `--title`, caches, citations, and the
  existing tools keep their current behaviour.

## 11. Definition of done

1. `related_entries` is implemented, documented, tested, and advertised in the reviewed
   `tools/list` snapshot.
2. Graph results are deterministic, bounded, cited, and derived only from resolved links.
3. Semantic retrieval is optional, loopback-Ollama-only, gated, fused, and falls back safely.
4. `render_graph` writes a readable PNG, returns it as an MCP image block, includes an absolute
   path, and is covered by schema/e2e tests.
5. Formula-family, formula-medicinal, and named point-combination networks are queryable with
   `related_network`, renderable with `render_network`, and every edge carries a relation and
   source line.
6. Model identity includes name and digest; stale vectors are never reused; `--check-semantic`,
   digest-keyed rebuilds, explicit pruning, and rollback instructions exist and are tested.
7. All hermetic tests pass without a model; mock-Ollama, real-Ollama, and real-data tests pass when
   configured.
8. Evaluation meets the thresholds, or semantic release is explicitly disabled with evidence.
9. README, PLAN, CHECKLIST, configuration, and deployment docs agree with the code, including the
   model update/withdrawal procedure.
10. The bundle contains no dictionary, model, cache, or other derivative.
11. One commit is made only after all of the above, and nothing is pushed without a request.

## 12. Appendix networks (implemented)

A unified appendix layer complements the main-entry graph:

- **Formula family:** edges from explicit prose (“This is X with…”, “combined with…”, “variant
  of…”). Edge relations are `has_derivative`, `combined_with`, `has_variant`, with `derived_from`,
  etc. when traversed incoming.
- **Formula-medicinal:** ingredient lines beginning `□` before the first `•` section are matched to
  appendix-2 medicinal entries; edges are `contains` / `used_in`. Stated alternatives can yield
  more than one ingredient edge.
- **Point combinations:** named `combining … points` main entries are parsed into combination
  nodes grouped by context and role markers (`S.`, `N.`, `T.`, `A.`, `L.`, `D.`), with indication
  text and source line. Point IDs are matched on token boundaries, so `LI-1` does not match inside
  `LI-16`.

Tools:

- `related_network(term, domain, direction, depth, limit)` where domain is `formula`, `medicinal`,
  or `point_combination`.
- `render_network` writes a PNG using the existing renderer.

Known limits:

- Formula-family relations are prose-derived candidates, not a manually reviewed genealogy.
- Formula ingredients include stated alternatives.
- Point combinations are the named method entries; arbitrary co-occurrence in narrative prose is
  not a graph edge.
- These are book relationships, not recommendations, substitutions, or clinical equivalence.
