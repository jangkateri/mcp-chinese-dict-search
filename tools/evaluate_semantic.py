#!/usr/bin/env python3
"""Compare lexical and gated hybrid retrieval through the stdio server.

The query file is local and must not be committed when it is derived from the
dictionary. Format: a JSON array of objects with "query" and "expected" keys.

Example:
  python -I tools/evaluate_semantic.py \
    --exe target/release/mcp-chinese-dict-search.exe \
    --data C:/ProgramData/mcp-dict-data \
    --queries C:/temp/queries.json \
    --embedding-model qwen3-embedding:0.6b
"""

from __future__ import annotations

import argparse
import json
import statistics
import subprocess
import sys
import time
from pathlib import Path


def start_server(args: argparse.Namespace, semantic: bool) -> subprocess.Popen:
    command = [
        str(args.exe),
        "--data",
        str(args.data),
        "--cache-dir",
        str(args.cache_dir),
    ]
    if semantic:
        command.extend(
            [
                "--embedding-model",
                args.embedding_model,
                "--ollama-url",
                args.ollama_url,
            ]
        )
        if args.semantic_index:
            command.extend(["--semantic-index", str(args.semantic_index)])
    return subprocess.Popen(
        command,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
        encoding="utf-8",
    )


def call(process: subprocess.Popen, request_id: int, name: str, arguments: dict) -> tuple[dict, float]:
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


def initialize(process: subprocess.Popen) -> None:
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


def run_mode(args: argparse.Namespace, semantic: bool) -> dict:
    process = start_server(args, semantic)
    try:
        initialize(process)
        hits_by_query: list[list[str]] = []
        latencies: list[float] = []
        for index, item in enumerate(args.queries):
            results: list[str] = []
            for repeat in range(args.repeat):
                result, elapsed = call(
                    process,
                    index + 2 + repeat * len(args.queries),
                    "search_dictionary",
                    {"query": item["query"], "limit": args.limit},
                )
                latencies.append(elapsed)
                structured = result.get("structuredContent") or {}
                results = [hit["term"] for hit in structured.get("hits", [])]
            hits_by_query.append(results)
        return metrics(args.queries, hits_by_query, latencies)
    finally:
        process.stdin.close()
        process.terminate()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()


def metrics(queries: list[dict], hits_by_query: list[list[str]], latencies: list[float]) -> dict:
    recall = 0.0
    reciprocal = 0.0
    top1 = 0.0
    for item, hits in zip(queries, hits_by_query):
        expected = item["expected"]
        if expected in hits[:5]:
            recall += 1.0
        if hits and hits[0] == expected:
            top1 += 1.0
        if expected in hits:
            reciprocal += 1.0 / (hits.index(expected) + 1.0)
    count = float(len(queries))
    latencies = sorted(latencies)

    def percentile(fraction: float) -> float:
        if not latencies:
            return 0.0
        position = min(len(latencies) - 1, max(0, int(round(fraction * (len(latencies) - 1)))))
        return latencies[position]

    return {
        "queries": len(queries),
        "recall_at_5": recall / count,
        "mrr": reciprocal / count,
        "top1": top1 / count,
        "p50_ms": statistics.median(latencies) if latencies else 0.0,
        "p95_ms": percentile(0.95),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--exe", type=Path, required=True)
    parser.add_argument("--data", type=Path, required=True)
    parser.add_argument("--queries", type=Path, required=True)
    parser.add_argument("--cache-dir", type=Path, required=True)
    parser.add_argument("--semantic-index", type=Path)
    parser.add_argument("--embedding-model")
    parser.add_argument("--ollama-url", default="http://127.0.0.1:11434")
    parser.add_argument("--limit", type=int, default=5)
    parser.add_argument("--repeat", type=int, default=1)
    args = parser.parse_args()

    args.queries = json.loads(args.queries.read_text(encoding="utf-8"))
    if not isinstance(args.queries, list) or not args.queries:
        parser.error("the query file must contain a non-empty JSON array")

    lexical = run_mode(args, semantic=False)
    print(json.dumps({"lexical": lexical}, ensure_ascii=False))
    if args.embedding_model:
        hybrid = run_mode(args, semantic=True)
        print(json.dumps({"hybrid": hybrid}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
