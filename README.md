# mcp-chinese-dict-search

An [MCP](https://modelcontextprotocol.io) stdio server, written in Rust, for **A Practical
Dictionary of Chinese Medicine** and its four appendices. It parses the Markdown dictionary
locally, resolves the book's cross-references through its index, ranks results, and exposes three
tools:

- `search_dictionary` — ranked hits across headwords, Chinese characters, pinyin, definitions,
  acupuncture points, herbs, formulas, and weights/measures;
- `get_entry` — one exact entry with aliases, full text, resolved cross-references, and related
  acupuncture-point monographs;
- `related_entries` — follow the resolved cross-references from one entry, in either direction, up
  to three hops;
- `render_graph` — draw that neighborhood as a PNG and return its absolute path;
- `related_network` — formula families, formula-medicinal composition, or named point
  combinations;
- `render_network` — draw an appendix network as a PNG.

## Data and copyright

This repository **does not contain dictionary data**. You supply a dictionary archive (`.zip`) or
its extracted Markdown directory in the layout described in [`PLAN.md`](PLAN.md). The data may be
copyrighted; do not commit the archive, extracted Markdown, images, or an EPUB to a public
repository. The default build includes the semantic feature but makes no network request until
`--embedding-model` is supplied; when enabled, it calls only the configured loopback Ollama server.

The generated lookup cache is also derived from the dictionary data. It is written beside the dictionary
(or into `--cache-dir`) and matched by `.gitignore`; keep it out of git and `dist/`.

## Status

| Area | State |
|---|---|
| MCP stdio transport, `initialize` / `ping` / `tools/list` / `tools/call` | Implemented |
| Main dictionary parsing | Implemented; tolerates the reference conversion's anomalies |
| Appendices | Implemented (points, medicinals, formulas, weights) |
| Index-based anchor and cross-reference resolution | Implemented; unresolved fragments are reported, never fatal |
| Generated pinyin and Han lookup indices | Cached by dictionary title + content fingerprint; tone-numbered, tone-marked, toneless, simplified, and traditional forms |
| `search_dictionary`, `get_entry` | Implemented |
| `related_entries` graph traversal | Implemented over resolved cross-references; depth 1–3, limit 1–50 |
| `render_graph` PNG rendering | Implemented; writes to `--pages-dir` or the system temp directory |
| Appendix networks | `related_network` / `render_network`: formula derivation, formula ingredients, named point combinations |
| Semantic fallback | Included in the default build; gated hybrid retrieval over a local Ollama embedding model; inactive unless `--embedding-model` is supplied, and loopback-only |
| Tests | Unit and integration tests pass; `clippy -D warnings` clean |
| Performance (Windows, development machine) | measured on the reference corpus; representative tool calls are sub-second |

The layout was validated against a reference conversion; no dictionary text is in the repository.

## Requirements

- Rust and Cargo **1.99.0** (edition 2024).
- The dictionary archive or its extracted directory.
- No daemon, API key, or external service unless semantic retrieval is enabled. The default build
  includes the semantic feature; enabling it requires a local **Ollama** server on loopback and an
  embedding model such as `embeddinggemma:latest` (CPU-friendly) or `qwen3-embedding:0.6b`
  (stronger non-exact Chinese separation on CPU-tested pairs). A minimal lexical-only build is
  available with `--no-default-features`.

## Quick start

```sh
cargo build --release

# The archive may stay anywhere; pass its absolute path.
./target/release/mcp-chinese-dict-search --data "C:/path/to/dictionary.zip"

# Show options and the published tools
./target/release/mcp-chinese-dict-search --help
```

On Windows the executable is `target\release\mcp-chinese-dict-search.exe`.

`--data` (or `MCP_CHINESE_DICT_DATA`) is required; the server does not guess a filename. Because
an MCP client may launch the process from another directory, use an absolute path. The display
name in citations becomes the archive file name, the extracted directory name, or `--title`.

On first startup the server writes
`<dictionary title>.lookup-cache-v2.<fingerprint>.json` beside the archive, or into `--cache-dir`.
Later startups reuse it when the dictionary title, content fingerprint, package version, and cache
version match. A changed dictionary produces a new fingerprint and a new cache; the old file is
pruned. Delete the cache file to force a rebuild, or run `--build-indices`, which rebuilds the
indices, writes the cache, and exits without waiting for a client.

### A complete handshake and lookup

```sh
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-11-25","clientInfo":{"name":"demo"}}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_dictionary","arguments":{"query":"your term","limit":3}}}' \
  '{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"get_entry","arguments":{"term":"your term","max_chars":2000}}}' \
  | ./target/release/mcp-chinese-dict-search --data "C:/path/to/dictionary.zip"
```

## Client configuration

The server is a stdio process: the client starts the executable and passes the data path. Use
absolute paths, because a client may launch it from another working directory, and use forward
slashes or escaped backslashes in JSON. The examples point `--data` at the linked copy made once by
`--build-linked-data` (see above); the `.zip` or the extracted directory also works. A quoted
argument may contain spaces.

### Claude Code

Add the entry at user scope, then verify it:

```sh
claude mcp add --scope user chinese-medicine-dictionary -- \
  "C:/path/to/mcp-chinese-dict-search.exe" \
  --data "C:/path/to/mcp-dict-data" \
  --pages-dir "C:/path/to/mcp-dict-pages"

claude mcp list
```

`--scope project` writes `.mcp.json` in the repository instead, and `--scope local` keeps the entry
to the current project for this user. Inside a session, `/mcp` lists the server; reconnect after
changing the executable or its arguments.

The equivalent user-scope JSON is the top-level `mcpServers` object in `~/.claude.json`:

```json
{
  "mcpServers": {
    "chinese-medicine-dictionary": {
      "command": "C:/path/to/mcp-chinese-dict-search.exe",
      "args": [
        "--data", "C:/path/to/mcp-dict-data",
        "--pages-dir", "C:/path/to/mcp-dict-pages"
      ]
    }
  }
}
```

A project `.mcp.json` uses the same `mcpServers` object.

### OpenCode

Current OpenCode reads `opencode.jsonc` in the project, or the global
`~/.config/opencode/opencode.jsonc`, and expects servers under `mcp.servers`. A local server takes
the command as an array:

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
          "--pages-dir", "C:/path/to/mcp-dict-pages"
        ]
      }
    }
  }
}
```

The CLI writes the same entry:

```sh
opencode mcp add chinese-medicine-dictionary -- \
  "C:/path/to/mcp-chinese-dict-search.exe" --data "C:/path/to/mcp-dict-data"
```

Add `--global` to write the global configuration instead of the project one, and run
`opencode mcp list` to check the connection, or `/mcps` inside the TUI. Older OpenCode releases
placed server names directly under `mcp` instead of `mcp.servers`; if a current release rejects the
entry, check which shape your version expects.

### DeepSeek Harness (`dsh`)

DSH configures MCP servers through `@deepseek-ai/dsh-mcp-client` in a YAML patch. Add this block to
`~/.dsh/profiles/<profile>/cordis.patch.yml` (or `~/.dsh/cordis.patch.yml` for every profile). If the
generated file contains `[]`, replace that `[]` with the block rather than appending to it:

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
```

The tools register as `mcp__chinese-medicine-dictionary__search_dictionary`,
`mcp__chinese-medicine-dictionary__get_entry`,
`mcp__chinese-medicine-dictionary__related_entries`,
`mcp__chinese-medicine-dictionary__render_graph`,
`mcp__chinese-medicine-dictionary__related_network`, and
`mcp__chinese-medicine-dictionary__render_network`. `dsh --profile <profile> --dump-config` shows whether
the entry was composed. DSH 0.2.x ships the client; do not install `@deepseek-ai/dsh-mcp-client`
separately. DSH supports MCP tools and resources, but not prompt templates, so the `citation` prompt
is unavailable there; the server instruction `tcm citation <anchor>` still maps to `get_entry`.

### Other clients

Any MCP client that supports stdio servers can launch the binary:

```json
{
  "mcpServers": {
    "chinese-medicine-dictionary": {
      "command": "C:/path/to/mcp-chinese-dict-search.exe",
      "args": [
        "--data", "C:/path/to/mcp-dict-data",
        "--pages-dir", "C:/path/to/mcp-dict-pages"
      ]
    }
  }
}
```

## Tools

### `search_dictionary`

| Argument | Type | Default | Notes |
|---|---|---|---|
| `query` | string, required | — | English term; pinyin in tone-numbered (`qi4`), tone-marked (`qì`), or toneless (`qi`) form; simplified (`气`) or traditional (`氣`) Chinese; point ID (`ST-36`); herb; formula; measurement; or citation anchor (`a1TAK`). |
| `scope` | string enum, optional | `all` | `all`, `dictionary`, `meridian`, `medicinal`, `formulas`, `weights`. |
| `limit` | integer, optional | 5 | 1–20; out-of-range values are rejected, not clamped. |

Returns ranked hits as text plus `structuredContent`:

```json
{
  "query": "abdominal fullness",
  "scope": "all",
  "hits": [
    {
      "source": "dictionary",
      "term": "abdominal fullness",
      "id": null,
      "snippet": "A subjective feeling of fullness …",
      "matched_in": "headword",
      "score": 1000,
      "file": "08-a.md",
      "anchor": "a1S5W",
      "redirect": false
    }
  ],
  "total": 1
}
```

Appendix bodies are searched when `scope` names that appendix. In `all`, appendix hits come from
term, identifier, and alias matches; this keeps common words from flooding dictionary results.
Point IDs, herb names, formula names, and measurement terms therefore bring in the relevant
appendix entry without a separate call.

A citation anchor returns one exact hit: `search_dictionary("a1TAK")` returns the `qi4` entry with
its full text in the hit, up to 20,000 characters. This lets a citation tag be fed straight back
into search.

Pinyin and Chinese queries use generated lookup indices. Tone-numbered, tone-marked, and toneless
forms resolve to the same entry: `qi4`, `qì`, and `qi` all reach the `qi4` air/energy entry, while
`qi2` and `qí` reach the `Qi4` dynasty entry. Simplified and traditional forms are both indexed:
`气` and `氣` both reach `qi4`; `麻黄` and `麻黃` both reach the medicinal `ephedra` entry. Exact
pinyin and Han keys answer from the index; English and non-exact text still use the ranked search.

**Misspelling suggestions.** When a query matches no index key and the ranked hits are weak (or
`get_entry` finds nothing), the result lists up to five near misses:

- English: optimal string alignment over headwords, appendix terms, and weights titles. A
  single-word query may match one word of a multi-word term, so `alpha sweling` suggests
  `alpha swelling`; `constipaton` suggests `constipation`.
- Pinyin: the same distance over toneless pinyin keys, including the key with its word spaces
  removed, so `jia3 zhon3` suggests `jia zhong (alpha swelling)` and the space-free `jiage`
  suggests `jia gen (jia root)`. A compact match is ranked ahead of an equally distant shorter
  key, so a squashed two-word query is not pushed out by a short fragment.

Distance 1 is allowed for queries up to six characters, distance 2 above that; adjacent
transpositions count as one edit. Suggestions do not change exact lookup, are not applied to
high-confidence results, and are advisory: the caller reissues the query. A query that is a prefix
of an alias still resolves through the ordinary fallback instead of being suggested, for example
`xue4 ha` reaches `xue4 han2` (blood cold).

When `--pages-dir` is configured, each suggestion also names the page written for it, so the
suggestion itself is clickable:

```text
Did you mean: "ma huang (ephedra)" — C:\ProgramData\mcp-dict-pages\medicinal-01.01.01.md, ...
```

A click opens that entry's page, with its full text and its cross-reference links. The page is
overwritten by later lookups. Without a page directory, the suggestion is text only, because a
terminal click cannot run the lookup.

### `get_entry`

| Argument | Type | Default | Notes |
|---|---|---|---|
| `term` | string, required | — | Exact headword; pinyin in tone-numbered (`qi4`), tone-marked (`qì`), or toneless (`qi`) form; simplified or traditional Chinese; point ID; herb/formula name or ID; weights section; or citation anchor such as `a1TAK`. |
| `source` | string enum, optional | `all` | Restrict to one part of the book. |
| `max_chars` | integer, optional | 20000 | 1–100000. |

Returns the entry's Chinese/pinyin aliases, content, resolved cross-references, related
acupuncture-point summaries, a citation, and truncation metadata. A `See …` redirect is followed
one hop and disclosed as `redirected_from`.

A citation anchor resolves to its exact entry, even when two entries share a headword. For
example, `get_entry("a1TAK")` returns the `qi4` air/energy entry, while `get_entry("a1VJ4")` returns
the `Qi4` dynasty entry. The rendered entry ends with
`To reopen this citation, ask: tcm citation a1TAK`; the server instruction maps that prompt to
`get_entry`.

Pinyin and Chinese aliases resolve the same way: `get_entry("qi")` and `get_entry("氣")` return
`qi4`, while `get_entry("qi2")` returns the `Qi4` dynasty; `get_entry("zu3 san1 li3",
source="meridian")` and `get_entry("足三里", source="meridian")` both return `ST-36`.

If no entry matches, the tool returns an `isError` result with up to five suggestions; malformed
arguments are JSON-RPC `-32602` errors.

### `related_entries`

| Argument | Type | Default | Notes |
|---|---|---|---|
| `term` | string, required | — | Main dictionary headword, alias, pinyin, Chinese, or citation anchor. |
| `direction` | string enum, optional | `both` | `outgoing` (entries this entry references), `incoming` (entries that reference it), or `both`. |
| `depth` | integer, optional | 1 | 1–3 hops. |
| `limit` | integer, optional | 20 | 1–50 returned entries. |

The graph contains main dictionary entries and the cross-references the parser resolved. Appendix
entries are not graph nodes; use `get_entry` for those. Each returned node carries its distance,
edge direction, the `via` entry, the visible link text, and the same canonical and absolute
citations as the other tools. Cycles are visited once, and results are ordered by distance then
book order.

```json
{
  "query": "qi",
  "term": "qi4",
  "direction": "both",
  "depth": 1,
  "nodes": [
    {
      "source": "dictionary",
      "term": "channel qi4",
      "anchor": "a1PDH",
      "file": "23-p.md",
      "citation": "Dictionary, 23-p.md#a1PDH",
      "distance": 1,
      "edge_direction": "outgoing",
      "via": "qi4",
      "link_text": "channel qi4",
      "redirect": false
    }
  ],
  "total": 1,
  "truncated": false
}
```

`truncated` is true when `limit` or the internal 10,000-node safety cap cuts the result.

### `render_graph`

The same arguments as `related_entries`, rendered as a PNG:

```json
{"term":"spleen","direction":"outgoing","depth":2,"limit":20}
```

The result contains:

- a text summary and an absolute `Image: <path>` line;
- an MCP `image` block containing the PNG;
- `structuredContent` with the graph, image path, MIME type, width, and height.

The file is written to `--pages-dir` when configured, otherwise to the system temporary directory.
The image uses a simple column-per-hop layout: gold start node, green outgoing edges, blue incoming
edges, and labels from the 8x8 bitmap font. Long terms are truncated; use `get_entry` for full text.
PNG is used because text and thin edges remain readable; JPEG would introduce compression artifacts.

### `related_network`

| Argument | Type | Default | Notes |
|---|---|---|---|
| `term` | string, required | — | Formula, medicinal, point, or named point-combination entry. |
| `domain` | string enum, required | — | `formula`, `medicinal`, or `point_combination`. |
| `direction` | string enum, optional | `both` | `outgoing`, `incoming`, `both`. |
| `depth` | integer, optional | 1 | 1–3. |
| `limit` | integer, optional | 20 | 1–50. |

Domains:

- **`formula`** — formula-to-formula derivation from explicit prose:
  “This is X with the addition of…”, “combined with…”, “variant of…”.
- **`medicinal`** — formula ingredient lines (`□`) mapped to appendix-2 medicinal entries.
- **`point_combination`** — named `combining … points` entries parsed into combination records.
  Each record lists its point IDs/roles and indication; point→combination and method→combination
  edges are available.

Every returned node has a `node_type`, `relation`, `label`, absolute `Source:` line when available,
and a citation. Edges are book relationships, not clinical equivalence.

### `render_network`

Same arguments as `related_network`; writes a PNG to `--pages-dir` or the system temporary directory,
returns an MCP image block, an absolute `Image:` line, and structured network metadata.

## Resources and prompt

Every entry is also addressable as an **MCP resource**, so a citation can be read without a tool
call:

| URI | Content |
|---|---|
| `dictionary://about` | What the server searches, the measured counts, the tools, and the URI scheme. |
| `dictionary://entry/<reference>` | One entry, by citation anchor (`a1TAK`), headword, pinyin (`qi4`, `ma2 huang2`), or Chinese (`气`). Percent-encode spaces and non-ASCII characters. |

`resources/list` publishes `dictionary://about`; `resources/templates/list` publishes
`dictionary://entry/{reference}`. An unknown URI, or one naming no entry, is JSON-RPC `-32002`.

A **prompt** named `citation` returns the same text for one reference. Clients that expose MCP
prompts as commands list it as `/mcp__chinese-medicine-dictionary__citation` (Claude Code shows it
in the `/` menu as `/chinese-medicine-dictionary:citation`). Arguments are split on whitespace, so
use an anchor or a single word. DeepSeek Harness, Codex, and pi do not support MCP prompts; they
still get the tools and resources.

## Clickable citations

No MCP client turns a hyperlink into a tool call, so a clickable citation has to point at content
that already exists. With `--pages-dir` (or `MCP_CHINESE_DICT_PAGES_DIR`), every `get_entry` result
is also written as a Markdown page in that directory, and the result ends with two lines:

```text
Entry page: C:\mcp-dict-pages\formulas-01.01.01.md
Clickable link: [open the entry](file:///C:/mcp-dict-pages/formulas-01.01.01.md)
```

- The bare path is what VS Code's integrated terminal linkifies: Ctrl+click opens the page in the
  editor, local or over Remote-SSH/WSL. This is the reliable path in VS Code, which does not
  always follow `file://` or custom-scheme links.
- The `file://` link covers plain terminals that follow OSC 8 hyperlinks (Windows Terminal,
  iTerm2, Kitty, WezTerm, GNOME Terminal).
- Inside the page, every resolved cross-reference is a link to a **sibling page** written by the
  same call (`dictionary-a1RRD.md` and so on), so the link works in VS Code's Markdown preview,
  which resolves a relative target against the page's folder. Those target pages keep their own
  cross-references as plain text; the walk stops after one hop.
- The source citation is shown as an inline code span (`10-c.md#a1RMS`). A bare relative name would be
  resolved against the page's folder by the preview, and when it is not there VS Code falls back to
  a web address (`https://10-c.md/#a1RMS`). VS Code's built-in preview does not follow `file:///`
  links either ([microsoft/vscode#182870](https://github.com/microsoft/vscode/issues/182870)), so
  code is the honest rendering.
- Page names are `<source>-<id>.md`: `dictionary-a1TAK.md`, `meridian-ST-36.md`,
  `medicinal-01.01.01.md`, `formulas-01.01.01.md`, `weights-21.md`. Pages are overwritten on each
  lookup, so the directory stays small.
- Use a page directory **without spaces** and outside the repository. Spaces can stop VS Code's
  path detection; the server logs a warning when the directory contains one. The pages are derived
  from the copyrighted book and must not be committed.
- A page that cannot be written degrades to a note in the result; the entry itself is still
  returned.

### Keep model-written citations clickable

The server returns an absolute `Source: <path>:<line>` and `Entry page: <absolute path>`, but the
**assistant writes the final answer** and can abbreviate them to `10-c.md` or `10-c.md#a1RMS`. A
terminal cannot resolve that relative name; Ctrl+click falls back to Quick Open. This is neither an
MCP nor a client defect — only text the model copied from the tool result is clickable.

Tell the model, in its instruction file or in the prompt, to:

- call `chinese-medicine-dictionary`'s `search_dictionary`, `get_entry`, or `related_entries`
  instead of reading the dictionary folder with the client's own `Read`, `rg`, or `grep` tools;
- copy the tool's `Source:` and `Entry page:` lines verbatim, as absolute paths;
- never write a bare relative citation, and never emit `file:///` links in prose (VS Code ignores
  them).

This repository ships the rule in `AGENTS.md` (OpenCode) and `CLAUDE.md` (Claude Code), which
apply when the workspace is this repository. Nothing needs to be added globally for the server to
work. If you also want the same citation behavior in another workspace, use that project's
instruction file, add the rule to the clients' global files (`~/.config/opencode/AGENTS.md`,
`~/.claude/CLAUDE.md`), or include this sentence in the prompt:

> Use the chinese-medicine-dictionary tools, and copy the tool's `Source:` line and `Entry page:`
> path exactly as returned.

Without that instruction the tools still work; only the citations may come back shortened and
unclickable.

## One-time linked copy

If the Markdown conversion dropped the `id` attributes that the dictionary's own links
(`10-c.md#a1RMS`) target, the index file is the only anchor-to-entry map. Build a linked copy once,
at setup, the way `--build-indices` is run once:

```sh
mcp-chinese-dict-search --data "<archive-or-folder>" --build-linked-data "C:/mcp-dict-data"
```

The build copies the Markdown documents, inserts one `<a id="…"></a>` line before each indexed
entry, re-loads the copy, and verifies that every entry's headword, aliases, and body are unchanged
and that the fragment links resolve. It then exits without serving and reports file, entry, anchor,
and link coverage; unresolved fragments target the index, table of contents, introduction, or a
paragraph rather than an entry. Point `--data` at the copy:

```json
{
  "command": "C:/path/to/mcp-chinese-dict-search.exe",
  "args": ["--data", "C:/mcp-dict-data", "--pages-dir", "C:/mcp-dict-pages"]
}
```

With a directory source, tool output also becomes terminal-clickable: an entry header reads

```text
dictionary · Example Dictionary
Source: C:\mcp-dict-data\10-c.md:3819 · anchor a1RMS
```

and a search hit ends with `— dictionary · C:\mcp-dict-data\10-c.md:3819 · anchor a1RMS`. A terminal
opens that absolute file at the entry's line on Ctrl+click, where a bare `10-c.md#a1RMS` would be
resolved against the working directory and fail. Cross-reference lines in tool text name only the
target anchor, for example `- difficult defecation → difficult defecation (anchor a1RRD)`, so no
dead path is printed; pass that anchor back to `get_entry` to read the target. Zip sources have no
file on disk and keep the canonical citation.

The build also unescapes the conversion's literal brackets (`\[01.01.01\]` becomes `[01.01.01]`).
In a Markdown preview with math enabled, the escaped form is read as LaTeX display math, which
hides monograph headings; with the backslashes removed the headings render as ordinary text. The
parser accepts either form, and the server's own output is unchanged because it already unescaped
them.

The source archive or folder is never modified. The copy is derived from the dictionary data:
keep it out of git and `dist/`. Running the build again from an already linked copy inserts
nothing; rebuild it after changing the source data. A linked copy also carries a
`dictionary-title.txt` sidecar so citations keep the source title.

## Configuration

| Flag | Environment | Default | Meaning |
|---|---|---|---|
| `--data <path>` | `MCP_CHINESE_DICT_DATA` | required | Dictionary `.zip` or extracted directory. |
| `--title <text>` | `MCP_CHINESE_DICT_TITLE` | derived from the data path | Display name used in citations. |
| `--log-file <path>` | `MCP_CHINESE_DICT_LOG_FILE` | off | Append diagnostics to a file as well as stderr. |
| `--cache-dir <path>` | `MCP_CHINESE_DICT_CACHE_DIR` | beside the dictionary | Where to write the generated lookup cache and semantic index. |
| `--pages-dir <path>` | `MCP_CHINESE_DICT_PAGES_DIR` | off | Write a Markdown page per `get_entry` result here and name it in the result, so citations can be clicked. Use a path without spaces. |
| `--build-indices` | — | off | Rebuild the pinyin and Han lookup indices, write the cache, and exit without serving stdio. |
| `--build-linked-data <dir>` | — | off | Copy the Markdown set into `<dir>`, insert the index anchors the conversion dropped, verify entries and links, and exit without serving. Point `--data` at the copy. |
| `--ollama-url <url>` | `MCP_CHINESE_DICT_OLLAMA_URL` | `http://127.0.0.1:11434` | Loopback Ollama base URL; remote hosts and Ollama Cloud are rejected. |
| `--embedding-model <name>` | `MCP_CHINESE_DICT_EMBEDDING_MODEL` | off | Ollama embedding model; supplying it enables gated hybrid retrieval. |
| `--embedding-profile <path>` | `MCP_CHINESE_DICT_EMBEDDING_PROFILE` | built-in | Optional JSON override for query/document prefixes, instruction, `min_similarity`, `num_ctx`, and input guard. |
| `--semantic-index <path>` | `MCP_CHINESE_DICT_SEMANTIC_INDEX` | digest-keyed beside the cache | Override the semantic index path. |
| `--build-semantic-index` | — | off | Build a digest-keyed semantic index and exit. Requires `--embedding-model`. |
| `--check-semantic` | — | off | Print model, digest, dimensions, expected index, record count, status, and remediation. Never writes. |
| `--prune-semantic-index` | — | off | List semantic indexes that do not match the current model and digest; with `--yes`, remove them. |
| `--require-semantic` | `MCP_CHINESE_DICT_REQUIRE_SEMANTIC` | off | Fail startup if Ollama or the index is unavailable; default is lexical fallback. |
| `--yes` | — | off | Confirm pruning. |
| `--verbose` | — | off | Log request handling, timing, and the load report. |
| `-h`, `--help`, `--version` | — | — | Printed before an MCP client is attached. |

stdout carries JSON-RPC only; every diagnostic goes to stderr. The server negotiates the legacy
protocol era (`2025-11-25`), and answers `server/discover` with `-32601` so 2026-era clients fall
back to `initialize`.

## Semantic retrieval

The default build includes the semantic feature and the loopback HTTP client, but semantic
retrieval stays inactive until `--embedding-model` is supplied. A minimal lexical-only build is
available with `--no-default-features`. When enabled, it adds a gated fallback for paraphrases that
share no token with the entry:

1. Lexical search runs first and alone.
2. The embedding model is consulted only when the best lexical score is **below 300**; exact
   anchors, exact pinyin, exact Han, and strong lexical hits never embed.
3. Pure pinyin queries stay on the pinyin/fuzzy path and never embed.
4. Semantic candidates are merged with reciprocal-rank fusion (`k = 60`); semantic-only hits are
   labelled `semantic` and score 200–299. Lexical hits at ≥600 are pinned ahead.
5. If Ollama is down, the model is missing, the index is stale, or an embedding fails, the response
   is the normal lexical response (unless `--require-semantic` made startup fail).

Only loopback Ollama is accepted (`127.0.0.1`, `localhost`, `::1`). Ollama Cloud and other remote
hosts are rejected. The MCP process holds no model weights; Ollama owns the model session.

Build and serve:

```sh
# Default build: semantic-capable, lexical until a model is configured.
cargo build --release

# Minimal lexical-only build (no HTTP client).
cargo build --release --no-default-features

# Confirm the model, digest, dimensions, and index status; never writes.
mcp-chinese-dict-search --data "C:/mcp-dict-data" \
  --embedding-model embeddinggemma:latest --check-semantic

# Build the digest-keyed index (the old index is retained).
mcp-chinese-dict-search --data "C:/mcp-dict-data" \
  --embedding-model embeddinggemma:latest --build-semantic-index

# Serve with the fallback enabled.
mcp-chinese-dict-search --data "C:/mcp-dict-data" \
  --embedding-model embeddinggemma:latest
```

The index path is derived from the dictionary content fingerprint, model name, and the first 16
hex characters of the model digest. A new digest never overwrites the last known-good index. Use
`--check-semantic` after `ollama pull`, rebuild, evaluate, switch the configuration, and only then
`--prune-semantic-index --yes`. Vectors are never reused across model digests.

A model already pulled into the local Ollama store remains usable even if the registry withdraws
it. Do not `ollama rm` the last known-good model or delete its index until a replacement has been
built and accepted. If the local model is gone and the registry no longer serves it, choose a
replacement, build a new digest-keyed index, evaluate, then update the client configuration. There
is no vector-conversion path; a model backup, if operationally required, belongs outside this
repository.

Model profiles pin query formatting and `num_ctx`: Qwen3 uses its instruction and 4K context;
`embeddinggemma` uses 2K; `bge-m3` uses 8K; `nomic-embed-text` uses its query/document prefixes.
On CPU-only hosts, `embeddinggemma` indexes substantially faster than Qwen3; the local evaluation
used `embeddinggemma:latest` for that reason.
`--embedding-profile` can override the prefix, instruction, `min_similarity`, `num_ctx`, and input
guard. `min_similarity` does not require re-embedding; changing a prefix, instruction, or context
does.

### Copying a prebuilt index to another PC

The semantic index is a single portable `.bin` file. Copying it avoids the expensive document
embedding pass; it is valid only when all of the following match on both machines:

- the same server version (the index header records the package version);
- the same dictionary source files (the fingerprint hashes file names and content, not absolute
  paths);
- the same Ollama model name and full digest;
- the same embedding profile (the built-in default, or the same `--embedding-profile` JSON).

The target still needs the same Ollama model: the index stores document vectors, but every query
must still be embedded by the model.

On the source machine:

```powershell
# Shows the model, digest, profile hash, and exact expected index path.
mcp-chinese-dict-search.exe --data C:/ProgramData/mcp-dict-data `
  --cache-dir C:/ProgramData/mcp-dict-semantic `
  --embedding-model embeddinggemma:latest --check-semantic

# Record the checksum of the index you are about to copy.
Get-ChildItem "C:/ProgramData/mcp-dict-semantic/*.semantic-cache-*.bin" |
  Get-FileHash -Algorithm SHA256
```

Copy the `.bin` file, the dictionary data (same file names and content), and, if you used one, the
`--embedding-profile` JSON. The lookup-cache `.json` is optional; it rebuilds quickly and is not
required for semantic retrieval.

On the target machine:

1. Install the same server version with the `semantic` feature and pull the model
   (`ollama pull embeddinggemma:latest`). Verify the model digest matches the source; a changed
   tag digest invalidates the index.
2. Put the dictionary data anywhere; keep the file names and content identical.
3. Put the `.bin` in the cache directory under its original filename, or point
   `--semantic-index` at it directly.
4. Run `--check-semantic` with the same model and profile. `status: active` means the copy is
   valid. If it says `stale`, the remediation names the mismatch; rebuild rather than converting
   vectors.
5. Run one weak `search_dictionary` query, then point the MCP client at the target binary with
   `--cache-dir` (and `--embedding-profile` for a non-default profile).

Do not commit the index or place it in `dist/`; it is derived from the dictionary and is ignored
by git.

On a local 30-query English/mixed paraphrase set with `embeddinggemma:latest`, hybrid recall@5
rose from 0.23 to 0.57 and MRR from 0.18 to 0.46; added p95 was 75 ms and the MCP working set grew
by 21 MB. The exact/pinyin/Chinese regression set stayed at top-1 1.0, because those queries close
the gate before embedding. These are measurements on one corpus and one query set, not a general
quality claim.

A Qwen3 evaluation at the practical 1K override found higher recall@5 (0.633 vs 0.567) but lower
MRR (0.429 vs 0.455) and top-1 (0.30 vs 0.40), with a 5.6× longer build (76 vs 14 minutes) and a
larger index (27.5 vs 20.7 MiB). EmbeddingGemma remains the default for this CPU; Qwen3 is the
recall-first or Chinese-heavy alternative.

For appendix-specific queries, set `scope` to `meridian`, `medicinal`, `formulas`, or `weights`.
The semantic gate uses the best lexical score within the requested scope; with the default `all`,
unrelated main-dictionary body matches can exceed 300 and close the gate before embedding runs. A
scoped 17-query point/formula probe found lexical recall@5 0.706 versus hybrid 0.588, although
hybrid had slightly better MRR and top-1. Scoped lexical retrieval is already strong for appendix
entries; semantic fusion is query-dependent and can demote correct scoped hits, so evaluate per
source before relying on it.

## Architecture

The crate is split into a library (`src/lib.rs`) and a thin binary (`src/main.rs`):

```text
stdin
  │  newline-delimited JSON-RPC
  ▼
transport::serve ──► protocol::classify_message ──► server::McpServer
                                                         │
                                                   tools::call_tool
                                                         │
                                              dictionary::Dictionary
                                                ├── main entries
                                                ├── index anchors
                                                ├── appendix entries
                                                ├── ranking + link resolution
                                                └── graph adjacency (resolved links)
                                                         │
stdout ◄── transport::write_message ◄── result_response / error_response
```

Key decisions:

- The archive is loaded once at startup into an immutable in-memory index.
- The Markdown files contain no anchor definitions; `38-index.md` is the authoritative
  `anchor → term` table, with a small EPUB-verified correction table for omitted anchors. See
  `PLAN.md` §3.3.1 for the analysis of the opaque `a1XXXX` IDs.
- Appendix routing is semantic, because only four links anywhere point at appendix files.
- Pinyin and Han lookup indices are generated from the archive's aliases on first startup and
  cached by dictionary title + content fingerprint; the cache stays out of git and `dist/`.
- The graph is built once from resolved `Entry.links`; it adds outgoing and incoming adjacency,
  not inferred relations.
- Output is bounded: 5 search hits, 20 graph nodes, 20,000 characters per entry by default.
- The default build makes no network request unless a semantic model is configured. When enabled,
  it calls only the configured loopback Ollama HTTP endpoint, from a blocking worker so the async
  transport is not stalled.

## Project layout

```text
mcp-chinese-dict-search/
├── src/
│   ├── main.rs / lib.rs
│   ├── cli.rs / config.rs / error.rs / logging.rs
│   ├── protocol.rs / server.rs / transport.rs / tools.rs
│   ├── graph_image.rs            # PNG rendering of a graph result
│   ├── dictionary/
│       ├── mod.rs / model.rs / normalize.rs / source.rs / parse.rs
│       ├── index.rs / appendix.rs / search.rs / pinyin.rs / lookup.rs
│       ├── graph.rs
│       └── network.rs            # formulas, medicinals, point combinations
│   └── semantic/                 # default semantic feature; inert until configured
│       ├── document.rs / embedder.rs / index.rs / fusion.rs
│       ├── profiles.rs / ollama.rs / build.rs / engine.rs / startup.rs
├── tests/
│   ├── support/mod.rs
│   ├── support/fixture.rs       # invented corpus, written at test time
│   ├── stdio_contract.rs / tools_e2e.rs / real_data.rs / dist_bundle.rs
├── tools/                       # read-only analysis and local evaluation scripts
├── dist/                        # Windows x64 deployment bundle
├── PLAN.md / CHECKLIST.md / CLAUDE.md
├── Cargo.toml / Cargo.lock / rustfmt.toml
└── LICENSE
```

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets

# Minimal lexical-only build; no model is needed for either suite.
cargo clippy --all-targets --no-default-features -- -D warnings
cargo test --all-targets --no-default-features
```

The default tests need no network or model. `tests/real_data.rs` checks the configured dictionary when
`MCP_CHINESE_DICT_DATA` is set; otherwise it prints a skip reason. `tests/semantic_gate.rs` and
`tests/semantic_real.rs` skip with a reason unless the Ollama environment variables are set; the
fixture suite is hermetic.

`tools/evaluate_semantic.py` drives the stdio server and reports lexical and hybrid recall@5, MRR,
top-1, and p50/p95 latency for a local JSON query set. It supports `--embedding-profile`,
`--mode`, and `--per-query`; see [`tools/README.md`](tools/README.md). Keep a query set derived from
the dictionary out of git; pass it by path:

```sh
python -I tools/evaluate_semantic.py \
  --exe target/release/mcp-chinese-dict-search.exe \
  --data "C:/ProgramData/mcp-dict-data" \
  --cache-dir "C:/ProgramData/mcp-dict-semantic" \
  --queries "C:/ProgramData/mcp-dict-semantic/queries.json" \
  --embedding-model embeddinggemma:latest
```

## Known limitations

- 4 entries have no anchor in either `38-index.md` or the EPUB: `dancing of the arms and legs`,
  `food diarrhea2`, `provisioning aspect`, and `provisioning, defense, qi4, and blood`. They are
  searchable and cite `file:line`.
- Appendix-4 output renders tables as text rows, not as a formatted table.
- `related_entries` covers main dictionary entries only; appendix entries are not graph nodes.
  It follows resolved cross-references and does not infer relations from prose. Links whose target
  cannot be mapped are excluded and counted as `graph_unmapped_links` in the `--verbose` load
  report (8 on the reference corpus).
- `render_graph` uses a simple layered layout and an ASCII bitmap font. Long labels are truncated,
  and dense depth-2 neighborhoods can require a high `limit` before second-hop nodes fit.
- Appendix network edges are extracted from explicit formula prose, ingredient lines, and named
  point-combination entries. Formula-family edges are the least structured: they are relation
  candidates with source lines, not manually reviewed facts. Formula ingredients include stated
  alternatives where one ingredient line offers a choice.
- Additional books and a research/RAG mode remain deferred; see `PLAN.md` §15.
- A toneless pinyin query can be ambiguous (`qi` matches qì and qí); exact entries are ranked
  first, and the caller can add the tone number to disambiguate.
- Semantic retrieval requires a separate feature build, a local Ollama server, an installed
  embedding model, and a built index. It is not a substitute for exact terminology, and pure
  pinyin is deliberately excluded because tested models did not separate related and unrelated
  pinyin pairs reliably.
- A semantic index is model- and digest-specific. Pulling an updated tag invalidates it; rebuild
  rather than migrate vectors. The server never silently reuses a stale index.

## License

MIT for the code. The dictionary data is not included and remains the property of its rights
holders.
