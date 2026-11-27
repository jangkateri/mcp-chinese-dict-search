#!/usr/bin/env python3
"""Compare lexical and gated hybrid retrieval through the stdio server.

This is an offline evaluation tool. It is not imported by the MCP server and
has no third-party dependencies.

The query file is local and must not be committed when it is derived from the
dictionary. Format: a JSON array of objects with "query" and "expected" keys.

Examples:
  python -I tools/evaluate_semantic.py \
    --exe target/release/mcp-chinese-dict-search.exe \
    --data C:/ProgramData/mcp-dict-data \
    --cache-dir C:/ProgramData/mcp-dict-semantic \
    --queries C:/ProgramData/mcp-dict-semantic/queries.json \
    --embedding-model embeddinggemma:latest \
    --repeat 3

  # Qwen3 uses a profile override and per-query detail.
  python -I tools/evaluate_semantic.py ... \
    --embedding-model qwen3-embedding:0.6b \
    --embedding-profile C:/ProgramData/mcp-dict-semantic/profile.json \
    --per-query
"""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path


def start_server(args: argparse.Namespace, hybrid: bool) -> subprocess.Popen:
    command = [
        str(args.exe),
        "--data",
        str(args.data),
        "--cache-dir",
        str(args.cache_dir),
    ]
    if hybrid:
        command.extend(
            [
                "--ollama-url",
                args.ollama_url,
                "--embedding-model",
                args.embedding_model,
            ]
        )
        if args.embedding_profile:
            command.extend(["--embedding-profile", str(args.embedding_profile)])
        if args.semantic_index:
            command.extend(["--semantic-index", str(args.semantic_index)])
    return subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        encoding="utf-8",
    )


def initialize(process: subprocess.Popen) -> None:
    assert process.stdin and process.stdout
    request = {
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {"protocolVersion": "2025-11-25", "capabilities": {}},
    }
    process.stdin.write(json.dumps(request) + "\n")
    process.stdin.flush()
    while True:
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("server closed stdout during initialize")
        response = json.loads(line)
        if response.get("id") == 1:
            if "error" in response:
                raise RuntimeError(f"initialize failed: {response['error']}")
            return


def call(
    process: subprocess.Popen, request_id: int, name: str, arguments: dict
) -> tuple[dict, float]:
    assert process.stdin and process.stdout
    message = {
        "jsonrpc": "2.0",
        "id": request_id,
        "method": "tools/call",
        "params": {"name": name, "arguments": arguments},
    }
    started = time.perf_counter()
    process.stdin.write(json.dumps(message) + "\n")
    process.stdin.flush()
    while True:
        line = process.stdout.readline()
        if not line:
            raise RuntimeError("server closed stdout before replying")
        response = json.loads(line)
        if response.get("id") == request_id:
            elapsed = (time.perf_counter() - started) * 1000.0
            if "error" in response:
                raise RuntimeError(f"JSON-RPC error: {response['error']}")
            return response["result"], elapsed


def metrics(
    queries: list[dict], per_query: list[dict], latencies: list[float]
) -> dict:
    count = float(len(queries))
    recall = sum(
        1.0
        for item in per_query
        if item["rank"] is not None and item["rank"] <= 5
    )
    top1 = sum(1.0 for item in per_query if item["rank"] == 1)
    reciprocal = sum(
        1.0 / item["rank"] for item in per_query if item["rank"] is not None
    )
    ordered = sorted(latencies)

    def percentile(fraction: float) -> float:
        if not ordered:
            return 0.0
        position = min(
            len(ordered) - 1, max(0, int(round(fraction * (len(ordered) - 1))))
        )
        return ordered[position]

    return {
        "queries": len(queries),
        "recall_at_5": recall / count,
        "mrr": reciprocal / count,
        "top1": top1 / count,
        "p50_ms": statistics.median(ordered) if ordered else 0.0,
        "p95_ms": percentile(0.95),
    }


def run_mode(
    args: argparse.Namespace, hybrid: bool, queries: list[dict]
) -> dict:
    process = start_server(args, hybrid)
    try:
        initialize(process)
        per_query: list[dict] = []
        latencies: list[float] = []
        for index, item in enumerate(queries):
            result: dict | None = None
            elapsed = 0.0
            for repeat in range(args.repeat):
                result, elapsed = call(
                    process,
                    index + 2 + repeat * len(queries),
                    "search_dictionary",
                    {"query": item["query"], "limit": args.limit},
                )
                latencies.append(elapsed)
            assert result is not None
            structured = result.get("structuredContent") or {}
            hits = structured.get("hits", [])
            terms = [hit["term"] for hit in hits]
            expected = item["expected"]
            if expected in terms:
                rank = terms.index(expected) + 1
                hit = hits[rank - 1]
                matched_in = hit.get("matched_in")
                score = hit.get("score")
            else:
                rank = None
                matched_in = None
                score = None
            per_query.append(
                {
                    "query": item["query"],
                    "expected": expected,
                    "rank": rank,
                    "matched_in": matched_in,
                    "score": score,
                    "latency_ms": round(elapsed, 2),
                }
            )
        return {
            "metrics": metrics(queries, per_query, latencies),
            "queries": per_query,
        }
    finally:
        if process.stdin:
            process.stdin.close()
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


def load_queries(path: Path, parser: argparse.ArgumentParser) -> list[dict]:
    try:
        queries = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        parser.error(f"cannot read {path}: {error}")
    if not isinstance(queries, list) or not queries:
        parser.error("the query file must contain a non-empty JSON array")
    for index, item in enumerate(queries):
        if (
            not isinstance(item, dict)
            or not isinstance(item.get("query"), str)
            or not item["query"].strip()
            or not isinstance(item.get("expected"), str)
            or not item["expected"].strip()
        ):
            parser.error(
                f"query {index + 1} must have non-empty string 'query' and 'expected'"
            )
    return queries


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--data", type=Path, required=True)
    parser.add_argument("--cache-dir", type=Path, required=True)
    parser.add_argument("--queries", type=Path, required=True)
    parser.add_argument("--semantic-index", type=Path)
    parser.add_argument("--embedding-model")
    parser.add_argument("--embedding-profile", type=Path)
    parser.add_argument("--ollama-url", default="http://127.0.0.1:11434")
    parser.add_argument("--limit", type=int, default=5)
    parser.add_argument("--repeat", type=int, default=1)
    parser.add_argument(
        "--mode",
        choices=("both", "lexical", "hybrid"),
        default=None,
        help="both runs lexical and hybrid; defaults to both with a model, else lexical; "
        "hybrid requires --embedding-model",
    )
    parser.add_argument(
        "--per-query",
        action="store_true",
        help="include expected-term rank, match kind, score, and latency per query",
    )
    args = parser.parse_args()
    if args.mode is None:
        args.mode = "both" if args.embedding_model else "lexical"

    if args.mode in ("both", "hybrid") and not args.embedding_model:
        parser.error("--embedding-model is required for hybrid mode")
    if not 1 <= args.limit <= 20:
        parser.error("--limit must be between 1 and 20")
    if args.repeat < 1:
        parser.error("--repeat must be at least 1")

    queries = load_queries(args.queries, parser)
    outputs: dict[str, dict] = {}
    try:
        if args.mode in ("lexical", "both"):
            outputs["lexical"] = run_mode(args, False, queries)
        if args.mode in ("hybrid", "both"):
            outputs["hybrid"] = run_mode(args, True, queries)
    except RuntimeError as error:
        print(f"error: {error}", file=sys.stderr)
        return 1

    if args.per_query:
        print(json.dumps(outputs, ensure_ascii=False))
    else:
        for name, result in outputs.items():
            print(json.dumps({name: result["metrics"]}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
