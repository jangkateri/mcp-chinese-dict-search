# AGENTS.md — dictionary lookups and citations

This repository contains `mcp-chinese-dict-search`, an MCP stdio server for a local Markdown
dictionary and its appendices. Repository working rules live in [`CLAUDE.md`](CLAUDE.md); this file
governs how an agent must query the dictionary and cite what it returns.

## Use the MCP tools, not the raw files

- For any dictionary question, call the MCP server `chinese-medicine-dictionary`:
  - `search_dictionary` — OpenCode exposes it as
    `chinese-medicine-dictionary_search_dictionary`; other clients as
    `mcp__chinese-medicine-dictionary__search_dictionary`.
  - `get_entry` — `chinese-medicine-dictionary_get_entry` or
    `mcp__chinese-medicine-dictionary__get_entry`.
  - `related_entries` — follow resolved cross-references from a main entry; `direction` is
    `outgoing`, `incoming`, or `both`; `depth` is 1–3.
  - `render_graph` — draw that neighborhood as a PNG and return an absolute file path.
  - `related_network` — formula, medicinal, or point-combination appendix relationships;
    `domain` is `formula`, `medicinal`, or `point_combination`.
  - `render_network` — render an appendix network as a PNG.
- Use `related_network` for formula derivation, formula ingredients, or named point combinations.
  It does not expose arbitrary co-occurrence in prose as a relationship.
- Do **not** open, read, grep, or ripgrep the dictionary data directory for content. The server
  resolves aliases, pinyin, Chinese characters, anchors, redirects, and appendix identifiers; a raw
  read bypasses all of that and produces unclickable citations.
- Use `related_entries` for "what connects to this entry" questions. It covers main dictionary
  entries, not appendix entries; use `get_entry` for an appendix monograph.
- If the MCP tools are unavailable, say so. Do not fall back to reading the data directory.

## Copy the citation the server returns

A successful `get_entry` result begins with lines shaped like:

```
Source: C:\ProgramData\mcp-dict-data\35-appendix-2.md:35
Entry page: C:\ProgramData\mcp-dict-pages\medicinal-01.01.02.md
Clickable link: [open the entry](file:///C:/ProgramData/mcp-dict-pages/medicinal-01.01.02.md)
```

In the answer:

- Reproduce the `Source: <absolute path>:<line>` line **verbatim**. That absolute path is what a
  terminal can open.
- Include the `Entry page: <absolute path>` when the result provides one.
- Do not emit `file:///` links in prose; VS Code's terminal and Markdown preview ignore them.
- Never abbreviate a citation to a bare file name such as `35-appendix-2.md`, and never write
  `35-appendix-2.md#a1XXXX`. A terminal resolves those against its working directory, fails, and
  falls back to the command palette.
- Never leave a citation anchor unresolved. Pass it to `get_entry` and include the returned entry.

## Misspellings and suggestions

- A failed lookup returns near-miss suggestions, each with a `term` and, when pages are enabled, an
  absolute `page` path.
- Reissue `get_entry` with the suggestion's `term`; quote the page path in the answer.
- Do not invent a headword, and do not read the raw files to guess what the misspelling meant.

## Repository changes

- Follow [`CLAUDE.md`](CLAUDE.md). The completion gates are `cargo fmt --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all-targets`.
- Never add the dictionary archive, extracted Markdown, images, the EPUB, the linked copy, pages,
  lookup caches, or any fixture derived from them to the repository.
