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
  {"query": "a fluttering or pounding heartbeat", "expected": "heart palpitation"}
]
```

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

### Deploying the tool

Copy `evaluate_semantic.py` and a local query file to the target machine. No
package installation is required. Run it with `-I` so Python ignores user site
packages and the current directory on `sys.path`. The tool reads only the
specified paths and the server's loopback Ollama endpoint.

The query file and any result file derived from the dictionary must stay out of
git. `.gitignore` covers `queries.local.json` and `evaluation-results*.json`;
prefer keeping these files outside the repository as well.
