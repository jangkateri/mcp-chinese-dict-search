# mcp-chinese-dict-search — Windows x64 installation

**Version:** 0.6.0

This bundle contains the MCP stdio server for a local Markdown dictionary. It does **not** contain
dictionary data. You supply your own `.zip` archive (or the extracted Markdown directory) in the
supported layout.

## Contents

| File | Purpose |
|---|---|
| `mcp-chinese-dict-search.exe` | The server (Windows x64). |
| `install.ps1` | Optional installer: copies the binary and generates an MCP client entry. |
| `mcp-config.example.json` | Template MCP client configuration with placeholders. |
| `INSTALL.md` | This guide. |
| `LICENSE` | MIT license for the code. |
| `SHA256SUMS.txt` | SHA-256 checksums for the bundle files. |

The dictionary archive, extracted Markdown, images, and any EPUB are deliberately absent.

## Install

1. Verify the checksums (optional but recommended):

   ```powershell
   Get-FileHash .\mcp-chinese-dict-search.exe -Algorithm SHA256
   Get-Content .\SHA256SUMS.txt
   ```

2. Either copy `mcp-chinese-dict-search.exe` wherever you keep local tools, or run the installer:

   ```powershell
   powershell -ExecutionPolicy Bypass -File .\install.ps1 -DataPath "C:\path\to\dictionary.zip"
   ```

   The installer copies the binary and `LICENSE` into
   `%LOCALAPPDATA%\mcp-chinese-dict-search` and writes `mcp-config.json` there. It does not copy
   the dictionary.

3. Confirm startup:

   ```powershell
   .\mcp-chinese-dict-search.exe --version
   .\mcp-chinese-dict-search.exe --help
   ```

## Prepare the linked copy (recommended)

The EPUB-to-Markdown conversion dropped the `id` attributes that the book's own links use, so only
`38-index.md` still maps anchors to entries. Build a linked copy once, at setup:

```powershell
.\mcp-chinese-dict-search.exe --data "C:\path\to\dictionary" `
  --build-linked-data "C:\mcp-dict-data"
```

The build copies every Markdown document, inserts one `<a id="…"></a>` before each entry,
unescapes the conversion's literal brackets so a math-enabled Markdown preview does not read the
monograph headings as LaTeX, verifies that every entry's text is unchanged, reports link coverage,
and exits without serving. On the real dictionary it reports 5,803 entries, 5,799 anchors inserted,
and 14,147 of 14,226 fragment links resolved (the remainder target the index, the table of
contents, the introduction, or a paragraph rather than an entry). Point `--data` at the copy; the
source archive or folder is never modified. The copy is derived from the copyrighted book: do not
commit it.

## Configure the MCP client

The server is a stdio process. Use absolute paths; a quoted argument may contain spaces.

For semantic retrieval, first build an index into a fixed cache directory (see
[Semantic index](#semantic-index-ollama)), then add these arguments to every client block:

```text
--cache-dir C:/path/to/mcp-dict-semantic
--embedding-model embeddinggemma:latest
--ollama-url http://127.0.0.1:11434
```

For Qwen3, replace the model and add the matching 1K profile:

```text
--embedding-model qwen3-embedding:0.6b
--embedding-profile C:/path/to/mcp-dict-semantic/qwen3-1k-profile.json
```

Add `--require-semantic` if the server should fail startup instead of falling back to lexical when
Ollama or the index is unavailable. Omit all semantic arguments for a purely lexical client.

**Claude Code** — user scope, then verify:

```powershell
claude mcp add --scope user chinese-medicine-dictionary -- `
  "C:/path/to/mcp-chinese-dict-search.exe" `
  --data "C:/path/to/mcp-dict-data" `
  --pages-dir "C:/path/to/mcp-dict-pages" `
  --cache-dir "C:/path/to/mcp-dict-semantic" `
  --embedding-model embeddinggemma:latest `
  --ollama-url "http://127.0.0.1:11434"
claude mcp list
```

`--scope project` writes `.mcp.json` in the repository instead; `--scope local` keeps the entry to
the current project for this user. Inside a session, `/mcp` lists the server and reconnects it. The
equivalent user entry is the top-level `mcpServers` object in `~/.claude.json`:

```json
{
  "mcpServers": {
    "chinese-medicine-dictionary": {
      "command": "C:/path/to/mcp-chinese-dict-search.exe",
      "args": [
        "--data", "C:/path/to/mcp-dict-data",
        "--pages-dir", "C:/path/to/mcp-dict-pages",
        "--cache-dir", "C:/path/to/mcp-dict-semantic",
        "--embedding-model", "embeddinggemma:latest",
        "--ollama-url", "http://127.0.0.1:11434"
      ]
    }
  }
}
```

**OpenCode** — current releases read `opencode.jsonc` in the project, or the global
`~/.config/opencode/opencode.jsonc`, and expect servers under `mcp.servers`:

```jsonc
{
  "$schema": "https://opencode.ai/config.json",
  "mcp": {
    "servers": {
      "chinese-medicine-dictionary": {
        "type": "local",
        "command": [
          "C:/path/to/mcp-chinese-dict-search.exe",
          "--data", "C:/path/to/mcp-dict-data",
          "--pages-dir", "C:/path/to/mcp-dict-pages",
          "--cache-dir", "C:/path/to/mcp-dict-semantic",
          "--embedding-model", "embeddinggemma:latest",
          "--ollama-url", "http://127.0.0.1:11434"
        ]
      }
    }
  }
}
```

The CLI writes the same entry; `--global` selects the global configuration:

```powershell
opencode mcp add chinese-medicine-dictionary -- `
  "C:/path/to/mcp-chinese-dict-search.exe" `
  --data "C:/path/to/mcp-dict-data" `
  --pages-dir "C:/path/to/mcp-dict-pages" `
  --cache-dir "C:/path/to/mcp-dict-semantic" `
  --embedding-model embeddinggemma:latest `
  --ollama-url "http://127.0.0.1:11434"
opencode mcp list
```

Inside the TUI, `/mcps` lists the server. Older OpenCode releases placed names directly under `mcp`
instead of `mcp.servers`.

**DeepSeek Harness** — add the block to `~/.dsh/profiles/<profile>/cordis.patch.yml` (or
`~/.dsh/cordis.patch.yml` for every profile). If the generated file contains `[]`, replace that
`[]` with the block rather than appending to it:

```yaml
- insert:
    - id: mcp-chinese-dict
      name: '@deepseek-ai/dsh-mcp-client'
      config:
        serverName: chinese-medicine-dictionary
        transport: stdio
        command: 'C:/path/to/mcp-chinese-dict-search.exe'
        args:
          - --data
          - 'C:/path/to/mcp-dict-data'
          - --pages-dir
          - 'C:/path/to/mcp-dict-pages'
          - --cache-dir
          - 'C:/path/to/mcp-dict-semantic'
          - --embedding-model
          - embeddinggemma:latest
          - --ollama-url
          - 'http://127.0.0.1:11434'
```

The tools register as `mcp__chinese-medicine-dictionary__search_dictionary`,
`mcp__chinese-medicine-dictionary__get_entry`, and
`mcp__chinese-medicine-dictionary__related_entries`,
`mcp__chinese-medicine-dictionary__render_graph`,
`mcp__chinese-medicine-dictionary__related_network`, and
`mcp__chinese-medicine-dictionary__render_network`. `dsh --profile <profile> --dump-config` shows whether
the entry was composed. DSH 0.2.x ships the client; do not install
`@deepseek-ai/dsh-mcp-client` separately. It supports tools and resources, but not MCP prompt
templates.

To make citations and suggestions clickable, add a page directory to the arguments — choose a path
without spaces, because VS Code's terminal link detection stops at a space:

```json
"args": [
  "--data", "C:/mcp-dict-data",
  "--pages-dir", "C:/mcp-dict-pages",
  "--cache-dir", "C:/path/to/mcp-dict-semantic",
  "--embedding-model", "embeddinggemma:latest",
  "--ollama-url", "http://127.0.0.1:11434"
]
```

Every `get_entry` then writes a page for the entry and names it in the result with
`Entry page: <absolute path>`; cross-references in the page link to sibling pages. A suggestion in a
failed lookup is likewise followed by its page path.

If your client supports environment-based configuration, this is equivalent:

```json
{
  "mcpServers": {
    "chinese-medicine-dictionary": {
      "command": "C:/path/to/mcp-chinese-dict-search.exe",
      "env": {
        "MCP_CHINESE_DICT_DATA": "C:/path/to/dictionary.zip",
        "MCP_CHINESE_DICT_PAGES_DIR": "C:/path/to/mcp-dict-pages",
        "MCP_CHINESE_DICT_CACHE_DIR": "C:/path/to/mcp-dict-semantic",
        "MCP_CHINESE_DICT_EMBEDDING_MODEL": "embeddinggemma:latest",
        "MCP_CHINESE_DICT_OLLAMA_URL": "http://127.0.0.1:11434"
      }
    }
  }
}
```

For Qwen3, add
`"MCP_CHINESE_DICT_EMBEDDING_PROFILE": "C:/path/to/qwen3-1k-profile.json"`.
For strict startup, add `"MCP_CHINESE_DICT_REQUIRE_SEMANTIC": "1"`.

## Tools

| Tool | Use |
|---|---|
| `search_dictionary` | `query` (required), `scope` (`all`, `dictionary`, `meridian`, `medicinal`, `formulas`, `weights`), `limit` (1–20). |
| `get_entry` | `term` (required), `source` (same values), `max_chars` (default 20,000). |
| `related_entries` | `term` (required), `direction` (`outgoing`, `incoming`, `both`), `depth` (1–3), `limit` (1–50). |
| `render_graph` | Same arguments as `related_entries`; returns a PNG image block plus an absolute file path. |
| `related_network` | `term` (required), `domain` (`formula`, `medicinal`, `point_combination`), `direction`, `depth` (1–3), `limit` (1–50). |
| `render_network` | Same arguments as `related_network`; returns a PNG image block plus an absolute file path. |

Examples to try in a client:

- `search_dictionary({"query":"abdominal fullness"})`
- `search_dictionary({"query":"ST-36"})`
- `get_entry({"term":"ST-36","source":"meridian"})`
- `search_dictionary({"query":"ma huang","scope":"medicinal"})`
- `search_dictionary({"query":"麻黄","scope":"medicinal"})`
- `get_entry({"term":"qi2"})`
- `related_entries({"term":"qi","direction":"both","depth":1})`
- `render_graph({"term":"spleen","direction":"outgoing","depth":2,"limit":20})`
- `related_network({"term":"Four Gentlemen Decoction","domain":"medicinal","depth":1})`
- `render_network({"term":"Four Gentlemen Decoction","domain":"formula","depth":1})`
- `search_dictionary({"query":"liang","scope":"weights"})`

When a query matches no index key and the ranked hits are weak, the lookup tools return near misses:
English headword typos (`abdomnal` → `abdominal fullness`) and pinyin typos
(`ma huag` → `ma huang (ephedra)`). Each suggestion carries the absolute path of a page written for
it when `--pages-dir` is configured. With a directory source, `get_entry` also names the exact
source line: `Source: C:\mcp-dict-data\10-c.md:3819 · anchor a1RMS`. A bare relative citation such
as `10-c.md#a1RMS` cannot be clicked, because a terminal resolves it against its working directory.

Entries are also MCP resources: `dictionary://about` and `dictionary://entry/<reference>` (anchor,
headword, pinyin, or Chinese), and the `citation` prompt returns one entry for a reference. Clients
that support resources and prompts can use these without a tool call.

## Lookup cache

The first startup writes a cache named
`<dictionary title>.lookup-cache-v2.<fingerprint>.json` beside the dictionary. Later startups reuse
it while the dictionary content, package version, and cache version match. A changed dictionary
produces a new fingerprint and a new cache; the old file is removed.

Use `--cache-dir <path>` or `MCP_CHINESE_DICT_CACHE_DIR` to keep the cache elsewhere, for example
when the dictionary is on a read-only volume. The same directory also holds the semantic index when
the semantic feature is enabled. The cache is derived from the copyrighted dictionary; do not copy
it into a public repository or into `dist/`.

To build the indices and write the cache without starting a client, run:

```powershell
.\mcp-chinese-dict-search.exe --build-indices --data "C:\path\to\dictionary.zip"
```

It prints the key counts and the cache path to stderr and exits; exit code `1` means the cache
could not be written. An ordinary startup after that is a cache hit.

## Semantic index (Ollama)

The Windows bundle includes the semantic feature but stays lexical until you supply a model. To
enable gated hybrid retrieval, install Ollama and a local embedding model. A source build uses the
default feature set; `--no-default-features` produces the minimal lexical-only binary.

```powershell
# Source builds only: semantic is a default feature in 0.6.0.
# cargo build --release

# embeddinggemma:latest is the CPU-friendly choice (2K context, 768 dimensions).
# qwen3-embedding:0.6b gives stronger non-exact Chinese separation but indexes much
# more slowly on CPU.
ollama pull embeddinggemma:latest

# Inspect the model, digest, dimensions, and index status. This never writes.
.\mcp-chinese-dict-search.exe --data "C:\path\to\mcp-dict-data" `
  --cache-dir "C:\path\to\mcp-dict-semantic" `
  --embedding-model embeddinggemma:latest --check-semantic

# Build the index. The path is digest-keyed; an older index is retained.
.\mcp-chinese-dict-search.exe --data "C:\path\to\mcp-dict-data" `
  --cache-dir "C:\path\to\mcp-dict-semantic" `
  --embedding-model embeddinggemma:latest --build-semantic-index
```

Then add `--cache-dir "C:\path\to\mcp-dict-semantic"`,
`--embedding-model embeddinggemma:latest`, and
`--ollama-url http://127.0.0.1:11434` to the client arguments and restart the server. For Qwen3,
also add `--embedding-profile` pointing at the same profile used to build its index.
Only loopback Ollama is accepted (`127.0.0.1`, `localhost`, `::1`); Ollama Cloud and remote hosts
are rejected. The default behavior if Ollama or the index is unavailable is lexical fallback with
a warning; add `--require-semantic` to fail startup instead. On a local 30-query English/mixed
set, hybrid recall@5 was 0.567 versus 0.233 lexical and MRR 0.455 versus 0.183, with 75 ms added
p95; the exact/pinyin/Chinese regression set was unchanged. The index is about 21 MB and the model
about 621 MB. These are local measurements, not a general quality claim. A Qwen3 evaluation at a
1K context override found higher recall@5 (0.633 vs 0.567) but lower MRR (0.429 vs 0.455) and
top-1 (0.30 vs 0.40), with a 5.6× longer build and a 27.5 MiB index; EmbeddingGemma remains the
default for this CPU.

The index is tied to the model name **and digest**. After `ollama pull` changes a tag:

1. Run `--check-semantic` and note the new digest.
2. Rebuild with `--build-semantic-index`; the old index remains usable for rollback.
3. Evaluate, switch the client configuration, and restart.
4. Remove only stale indexes with `--prune-semantic-index --yes`.

Never copy vectors between models; embeddings from different digests are not comparable. Keep the
last known-good model in the Ollama store and its index on disk until a replacement is accepted.
Do not copy the index or model weights into this repository or `dist/`.

### Copying a prebuilt index to another PC

The `.bin` index is portable. Copy it instead of rebuilding only when the server version, the
dictionary file names and content, the Ollama model name and full digest, and the embedding profile
all match. The target still needs the same model to embed queries; the index replaces only the
document-embedding pass.

On the source machine:

1. Run `--check-semantic` and record the model, digest, profile hash, and expected index path.
2. Copy the `.bin` file, the dictionary data (same file names and content), and, if used, the
   `--embedding-profile` JSON. The lookup-cache `.json` is optional; it rebuilds quickly.
3. Record the index checksum so you can verify the transfer:

   ```powershell
   Get-ChildItem "C:\path\to\semantic-cache\*.semantic-cache-*.bin" |
     Get-FileHash -Algorithm SHA256
   ```

On the target machine:

1. Install the same server version with the semantic feature and run
   `ollama pull embeddinggemma:latest`. Compare the model digest with the source; a changed tag
   digest invalidates the index.
2. Copy the dictionary data and the `.bin` file. Keep the index filename, place it in the cache
   directory, or point `--semantic-index` at its full path.
3. Run `--check-semantic` with the same model and profile. `status: active` means the copy is
   valid; `stale` names the mismatch and requires a rebuild.
4. Run one weak `search_dictionary` query, then update the client arguments with `--cache-dir`
   (and `--embedding-profile` for a non-default profile).

Do not commit the index or place it in `dist/`; it is derived data and is ignored by git.

## Offline evaluation

The bundle includes `evaluate-semantic.exe`, a Rust evaluator that loads the dictionary and
semantic index directly. It does not start the MCP server and does not require Python.

```powershell
.\evaluate-semantic.exe --data "C:\path\to\mcp-dict-data" `
  --cache-dir "C:\path\to\semantic-cache" `
  --queries "C:\path\to\queries.json" `
  --embedding-model embeddinggemma:latest `
  --per-query --repeat 3
```

The query file is a local JSON array of objects with `query`, `expected`, and optional `scope`
(`all`, `dictionary`, `meridian`, `medicinal`, `formulas`, `weights`). Keep a query file derived
from the dictionary out of git. `--mode lexical` runs without Ollama; omit `--embedding-model`
for a lexical-only baseline.

## Data path

`--data` (or `MCP_CHINESE_DICT_DATA`) is required; the server does not guess a filename. Use an
absolute path in the client configuration because a client may launch the process from another
directory. The citation title comes from `--title`, the archive file name, or the directory name.

## Troubleshooting

| Symptom | Fix |
|---|---|
| Exit code 2, “Cannot find the dictionary” | Pass `--data` with an absolute path, or set `MCP_CHINESE_DICT_DATA`. |
| Exit code 2, “Cannot read … as a zip archive” | The file is corrupt or not the expected archive; use the extracted `.md` directory instead. |
| Tools missing in the client | Check the client's MCP log; a malformed JSON entry or a stale executable path is the usual cause. |
| Wrong or empty results | Run with `--verbose --log-file server.log` and inspect the load report (counts, unresolved links, warnings). |
| Semantic search silently falls back to lexical | Run `--check-semantic`; confirm Ollama is on loopback, the model is pulled, and the index digest matches. |
| Ctrl+click on `10-c.md#a1RMS` opens Quick Open | That name is relative and exists only inside the data folder. Click the absolute `Source:` line, the `Entry page:` path, or a suggestion's page path instead. |
| Markdown headings vanish as LaTeX in a preview | The linked-copy build unescapes `\[`/`\]`; rebuild it and reopen the file. |
| Client hides stderr | Use `--log-file`; MCP clients usually discard a child process's stderr. |

No network access is used. The server reads local files only.
