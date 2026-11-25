# mcp-chinese-dict-search

An [MCP](https://modelcontextprotocol.io) stdio server, written in Rust, for **A Practical
Dictionary of Chinese Medicine** and its four appendices. It parses the Markdown dictionary
locally, resolves the book's cross-references through its index, ranks results, and exposes two
tools:

- `search_dictionary` — ranked hits across headwords, Chinese characters, pinyin, definitions,
  acupuncture points, herbs, formulas, and weights/measures;
- `get_entry` — one exact entry with aliases, full text, resolved cross-references, and related
  acupuncture-point monographs.

## Data and copyright

This repository **does not contain dictionary data**. You supply a dictionary archive (`.zip`) or
its extracted Markdown directory in the layout described in [`PLAN.md`](PLAN.md). The data may be
copyrighted; do not commit the archive, extracted Markdown, images, or an EPUB to a public
repository. The server reads local files only and makes no network requests.

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
| Tests | 100 unit + 26 integration tests pass; `clippy -D warnings` clean |
| Performance (Windows, development machine) | measured on the reference corpus; representative tool calls are sub-second |

The layout was validated against a reference conversion; no dictionary text is in the repository.

## Requirements

- Rust and Cargo **1.99.0** (edition 2024).
- The dictionary archive or its extracted directory.
- No network access, daemon, API key, or external service.

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

The tools register as `mcp__chinese-medicine-dictionary__search_dictionary` and
`mcp__chinese-medicine-dictionary__get_entry`. `dsh --profile <profile> --dump-config` shows whether
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

- call `chinese-medicine-dictionary`'s `search_dictionary` / `get_entry` instead of reading the
  dictionary folder with the client's own `Read`, `rg`, or `grep` tools;
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
| `--cache-dir <path>` | `MCP_CHINESE_DICT_CACHE_DIR` | beside the dictionary | Where to write the generated lookup cache. |
| `--pages-dir <path>` | `MCP_CHINESE_DICT_PAGES_DIR` | off | Write a Markdown page per `get_entry` result here and name it in the result, so citations can be clicked. Use a path without spaces. |
| `--build-indices` | — | off | Rebuild the pinyin and Han lookup indices, write the cache, and exit without serving stdio. |
| `--build-linked-data <dir>` | — | off | Copy the Markdown set into `<dir>`, insert the index anchors the conversion dropped, verify entries and links, and exit without serving. Point `--data` at the copy. |
| `--verbose` | — | off | Log request handling, timing, and the load report. |
| `-h`, `--help`, `--version` | — | — | Printed before an MCP client is attached. |

stdout carries JSON-RPC only; every diagnostic goes to stderr. The server negotiates the legacy
protocol era (`2025-11-25`), and answers `server/discover` with `-32601` so 2026-era clients fall
back to `initialize`.

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
                                                └── ranking + link resolution
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
- Output is bounded: 5 hits by default, 20,000 characters per entry by default.
- No network, database, embedding model, or external service is used.

## Project layout

```text
mcp-chinese-dict-search/
├── src/
│   ├── main.rs / lib.rs
│   ├── cli.rs / config.rs / error.rs / logging.rs
│   ├── protocol.rs / server.rs / transport.rs / tools.rs
│   └── dictionary/
│       ├── mod.rs / model.rs / normalize.rs / source.rs / parse.rs
│       ├── index.rs / appendix.rs / search.rs / pinyin.rs / lookup.rs
├── tests/
│   ├── support/mod.rs
│   ├── support/fixture.rs       # invented corpus, written at test time
│   ├── stdio_contract.rs / tools_e2e.rs / real_archive.rs / dist_bundle.rs
├── tools/                       # read-only source-data analysis scripts
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
```

The tests need no network. `tests/real_archive.rs` runs the golden counts against the real archive
when it is present (repository root or `MCP_CHINESE_DICT_DATA`); otherwise it prints a skip reason.
The fixture suite is hermetic.

## Known limitations

- 4 entries have no anchor in either `38-index.md` or the EPUB: `dancing of the arms and legs`,
  `food diarrhea2`, `provisioning aspect`, and `provisioning, defense, qi4, and blood`. They are
  searchable and cite `file:line`.
- Appendix-4 output renders tables as text rows, not as a formatted table.
- Additional books and a research/RAG mode remain deferred; see `PLAN.md` §15.
- A toneless pinyin query can be ambiguous (`qi` matches qì and qí); exact entries are ranked
  first, and the caller can add the tone number to disambiguate.

## License

MIT for the code. The dictionary data is not included and remains the property of its rights
holders.
