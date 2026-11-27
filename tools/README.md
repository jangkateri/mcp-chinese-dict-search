# Offline evaluation and analysis

These tools are **not** part of the MCP server runtime. The evaluation tool is
now a Rust binary so the project, its tests, and its support procedures stay in
one language.

## Rust evaluator: `evaluate-semantic`

`evaluate-semantic` loads the dictionary and semantic index directly; it does
not spawn the MCP server and does not need Python.

### Build

```powershell
cargo build --release --bin evaluate-semantic
```

The binary is `target/release/evaluate-semantic.exe` on Windows.

### Query file

A local JSON array. Do not commit it when it is derived from the dictionary.

```json
[
  {"query": "breathing that is quick and shallow", "expected": "shortness of breath"},
  {"query": "a fluttering or pounding heartbeat", "expected": "heart palpitation"},
  {"query": "a point on the inner wrist used for nausea and vomiting", "expected": "Inner Pass", "scope": "meridian"},
  {"query": "a formula for wind-cold with sweating and aversion to wind", "expected": "Cinnamon Twig Decoction", "scope": "formulas"}
]
```

`scope` is optional and defaults to `all`. Use `meridian` for point queries,
`formulas` for formula monographs, `medicinal` for herbs, and `weights` for
measures. The scope restricts the lexical candidates and therefore the semantic
gate. With the default `all`, unrelated main-dictionary body matches can exceed
the 300-point gate and prevent embedding from running at all.

### Run

```powershell
.\target\release\evaluate-semantic.exe `
  --data C:/ProgramData/mcp-dict-data `
  --cache-dir C:/ProgramData/mcp-dict-semantic `
  --queries C:/ProgramData/mcp-dict-semantic/queries.json `
  --embedding-model qwen3-embedding:0.6b `
  --embedding-profile C:/ProgramData/mcp-dict-semantic/profile.json `
  --per-query `
  --repeat 3
```

Options:

- `--mode both|lexical|hybrid` — default is both with a model, otherwise lexical.
- `--limit <n>` — hits per query, 1–20 (default 5).
- `--repeat <n>` — repeats per query (default 1).
- `--semantic-index <path>` — exact index override.
- `--ollama-url <url>` — loopback Ollama base URL.
- `--per-query` — include rank, match kind, score, top score, gate state,
  embedding time, semantic candidates, and fallback reason per query.

### Output

Without `--per-query`, one JSON object:

```json
{"lexical":{"queries":30,"recall_at_5":0.2333,"mrr":0.1833,"top1":0.1333,"p50_ms":87.4,"p95_ms":110.0},
 "hybrid":{"queries":30,"recall_at_5":0.6333,"mrr":0.4289,"top1":0.3,"p50_ms":177.5,"p95_ms":206.2}}
```

With `--per-query`, each mode also includes a `queries` array with the expected
term's rank, `matched_in`, score, top-scoring term, gate state, embedding time,
semantic candidate count, and fallback reason.

`matched_in: "body"` on a hybrid hit means the entry was present in both the
lexical and semantic candidate lists; semantic fusion improved its rank. A hit
found only by the vector search is labelled `semantic` with a score of 200–299.

### Scoped appendix policy

For appendix scopes (`meridian`, `medicinal`, `formulas`, `weights`), semantic
candidates are filtered to the requested source, the lexical head is pinned, and
one semantic slot is reserved for the best remaining candidate by RRF. On a
17-query point/formula probe:

| Mode | Recall@5 | MRR | Top-1 |
|---|---:|---:|---:|
| Scoped lexical | 0.706 | 0.510 | 0.412 |
| Aggressive hybrid (old) | 0.588 | 0.520 | 0.471 |
| Conservative hybrid | **0.765** | **0.522** | 0.412 |

The conservative policy gains recall without demoting the correct scoped
lexical hits; it gives up some top-1 promotions. Main-dictionary symptom queries
keep the aggressive RRF behavior, where it is a clear gain.

### Deploying the evaluator

Copy `evaluate-semantic.exe` and a local query file to the target machine. No
Python installation is required. The query file and result files derived from the
dictionary must stay out of git.

## Legacy Python analysis scripts

The remaining `*.py` files are historical offline analysis utilities for EPUB,
anchor, appendix, and header inspection. They are not required for the server,
its tests, or semantic evaluation. They can be migrated to Rust or removed in a
later cleanup; no runtime or deployment procedure depends on them.
