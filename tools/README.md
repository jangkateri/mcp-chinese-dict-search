# Offline evaluation tools

These scripts are **not** part of the MCP server runtime. They start the stdio
server, send JSON-RPC requests, and compare retrieval modes. They use only the
Python standard library.

## `evaluate_semantic.py`

Compares lexical-only retrieval with gated hybrid retrieval on the same query
set. It reports recall@5, MRR, top-1, and p50/p95 latency. With `--per-query`,
it also reports the expected term's rank, `matched_in`, score, and latency for
each query.

### Requirements

- Python 3.10 or newer.
- A built `mcp-chinese-dict-search` binary. The semantic path requires a build
  with the `semantic` feature; it is enabled by default in 0.6.0.
- The same dictionary data used for the semantic index.
- For hybrid mode: a local Ollama server, the configured embedding model, and a
  matching digest-keyed index.

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

`scope` is optional and defaults to `all`. Use `meridian` for point queries, `formulas`
for formula monographs, `medicinal` for herbs, and `weights` for measures. The scope
restricts the lexical candidates and therefore the semantic gate. With the default `all`,
unrelated main-dictionary body matches can exceed the 300-point gate and prevent embedding
from running at all.

### Examples

Aggregate comparison with EmbeddingGemma:

```powershell
python -I tools/evaluate_semantic.py `
  --exe target/release/mcp-chinese-dict-search.exe `
  --data C:/ProgramData/mcp-dict-data `
  --cache-dir C:/ProgramData/mcp-dict-semantic `
  --queries C:/ProgramData/mcp-dict-semantic/queries.json `
  --embedding-model embeddinggemma:latest `
  --repeat 3
```

Qwen3 with the 1K profile, including per-query detail:

```powershell
python -I tools/evaluate_semantic.py `
  --exe target/release/mcp-chinese-dict-search.exe `
  --data C:/ProgramData/mcp-dict-data `
  --cache-dir C:/ProgramData/mcp-dict-semantic `
  --queries C:/ProgramData/mcp-dict-semantic/queries.json `
  --embedding-model qwen3-embedding:0.6b `
  --embedding-profile C:/ProgramData/mcp-dict-semantic/profile.json `
  --per-query `
  --repeat 3
```

Lexical baseline only:

```powershell
python -I tools/evaluate_semantic.py ... --mode lexical
```

### Output

Without `--per-query`, one JSON object per mode:

```json
{"lexical": {"queries": 30, "recall_at_5": 0.2333, "mrr": 0.1833, "top1": 0.1333, "p50_ms": 98.3, "p95_ms": 110.7}}
{"hybrid": {"queries": 30, "recall_at_5": 0.5667, "mrr": 0.4550, "top1": 0.4000, "p50_ms": 159.6, "p95_ms": 185.7}}
```

With `--per-query`, one JSON object containing both modes:

```json
{
  "lexical": {"metrics": {"...": "..."}, "queries": [
    {"query": "...", "expected": "...", "rank": null, "matched_in": null, "score": null, "latency_ms": 95.2}
  ]},
  "hybrid": {"metrics": {"...": "..."}, "queries": [
    {"query": "...", "expected": "...", "rank": 1, "matched_in": "body", "score": 83, "latency_ms": 171.4}
  ]}
}
```

`matched_in: "body"` on a hybrid hit means the entry was present in both the
lexical and semantic candidate lists; semantic fusion improved its rank. A hit
found only by the vector search is labelled `semantic` with a score of 200–299.

### Scoped appendix retrieval

A scoped 17-query probe (nine point and eight formula paraphrases) produced:

| Mode | Recall@5 | MRR | Top-1 |
|---|---:|---:|---:|
| Scoped lexical | **0.706** | 0.510 | 0.412 |
| Scoped hybrid (Qwen3 1K) | 0.588 | **0.520** | **0.471** |

Semantic fusion helped individual point paraphrases such as `Inner Pass` and `Supreme
Surge`, but demoted several correct scoped lexical hits. In this corpus, scoped lexical
retrieval is already strong for appendix entries; semantic fusion is query-dependent and
must be evaluated per source. The probe is local and not committed.

### Deploying the tool

Copy `evaluate_semantic.py` and a local query file to the target machine. No
package installation is required. Run it with `-I` so Python ignores user site
packages and the current directory on `sys.path`. The tool reads only the
specified paths and the server's loopback Ollama endpoint.

The query file and any result file derived from the dictionary must stay out of
git. `.gitignore` covers `queries.local.json` and `evaluation-results*.json`;
prefer keeping these files outside the repository as well.
