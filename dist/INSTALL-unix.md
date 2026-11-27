# Linux and macOS installation

The Linux and macOS archives contain:

- `mcp-chinese-dict-search` — the MCP stdio server
- `evaluate-semantic` — the offline Rust evaluator
- `README.md`, `BUILDING.md`, `CHANGELOG.md`, `LICENSE`
- `SHA256SUMS`

They do not contain dictionary data, model weights, caches, indexes, or pages.

## Linux

The x64 archive is statically linked against musl:

```sh
tar -xzf mcp-chinese-dict-search-<version>-linux-x64.tar.gz
chmod +x mcp-chinese-dict-search evaluate-semantic
./mcp-chinese-dict-search --version
```

## macOS

The archive contains a universal binary for Apple Silicon (M1–M4) and Intel:

```sh
tar -xzf mcp-chinese-dict-search-<version>-macos-universal.tar.gz
chmod +x mcp-chinese-dict-search evaluate-semantic
./mcp-chinese-dict-search --version
```

The binary is not code-signed or notarized. If Gatekeeper quarantines a downloaded copy:

```sh
xattr -d com.apple.quarantine mcp-chinese-dict-search evaluate-semantic
```

or allow it from System Settings → Privacy & Security after the first launch attempt.

## Semantic setup

Install Ollama and a local embedding model:

```sh
ollama pull qwen3-embedding:0.6b
# or: ollama pull embeddinggemma:latest
```

Build or copy the semantic index as described in `README.md` and `BUILDING.md`.
For Qwen3, pass the matching profile:

```sh
./mcp-chinese-dict-search \
  --data /path/to/mcp-dict-data \
  --cache-dir /path/to/mcp-dict-semantic \
  --pages-dir /path/to/mcp-dict-pages \
  --embedding-model qwen3-embedding:0.6b \
  --embedding-profile /path/to/mcp-dict-semantic/profile.json
```

For EmbeddingGemma, omit `--embedding-profile` and use
`--embedding-model embeddinggemma:latest`.

Only loopback Ollama is accepted. The package version no longer invalidates an
index; dictionary content, model digest, dimensions, profile, index format, and
document version must match.

## MCP client example

```json
{
  "mcpServers": {
    "chinese-medicine-dictionary": {
      "command": "/absolute/path/to/mcp-chinese-dict-search",
      "args": [
        "--data", "/absolute/path/to/mcp-dict-data",
        "--cache-dir", "/absolute/path/to/mcp-dict-semantic",
        "--pages-dir", "/absolute/path/to/mcp-dict-pages",
        "--embedding-model", "qwen3-embedding:0.6b",
        "--embedding-profile", "/absolute/path/to/mcp-dict-semantic/profile.json",
        "--ollama-url", "http://127.0.0.1:11434"
      ]
    }
  }
}
```

Use absolute paths. Keep the dictionary and caches private; they are derived
from supplied copyrighted data.
