# CLAUDE.md — working rules for this repository

This file is loaded into every session. It defines the project, the mandated behaviour rules, the
research protocol, and the coding guidelines. Keep it short enough to read and precise enough to
enforce.

## Project in one paragraph

`mcp-chinese-dict-search` is a Rust **MCP stdio server** that searches a user-supplied Chinese
medicine dictionary in Markdown and its four appendices (meridians/acupuncture points, medicinal
agents, herbal formulas, weights and measures). It loads a `.zip` or extracted directory, parses
the main and appendix entries, resolves the dictionary's internal links through its index file,
and exposes six tools: `search_dictionary`, `get_entry`, `related_entries`, `render_graph`,
`related_network`, and `render_network`, plus the optional surfaces
`dictionary://entry/<anchor>` resources and a `citation` prompt, and optional per-entry Markdown
pages so a citation can be clicked. `related_entries` follows the resolved cross-reference graph
in either direction; `render_graph` draws it as a PNG. `related_network` covers formula families,
formula-medicinal composition, and named point combinations; `render_network` draws those. An
optional `semantic` Cargo feature adds gated hybrid retrieval over a loopback-only Ollama
embedding model; the default build, configuration, and dependency set stay lexical-only.
See [`PLAN.md`](PLAN.md) for the full design and [`CHECKLIST.md`](CHECKLIST.md) for live status,
and [`ENHANCEMENT-PLAN.md`](ENHANCEMENT-PLAN.md) for the semantic design and phases.

## Architecture, in one glance

`main.rs` starts `cli.rs`; `config.rs` discovers the data path; `dictionary/` loads and parses the
archive; `transport.rs` reads newline-delimited JSON-RPC from stdin and spawns one task per
message; `protocol.rs` classifies it; `server.rs` dispatches `initialize`, `ping`, `tools/list`,
`tools/call`, `resources/list`, `resources/templates/list`, `resources/read`, `prompts/list`,
`prompts/get`, and `logging/setLevel`; `tools.rs` validates arguments, calls `dictionary::search`
or `dictionary::related` / `dictionary::related_network`, renders model-facing text, and uses
`graph_image.rs` for PNG output;
`resources.rs` and `prompts.rs` serve the same entry text as addressable resources and the
`citation` prompt, and `pages.rs` writes the clickable citation page
when a page directory is configured. `semantic/` (feature-gated) holds the Ollama client, model
profiles, flat index, gated fusion, and serving engine. `schema.rs` derives each tool's `outputSchema` from the
serialized response type, so the advertised contract and the payload cannot drift.
`dictionary/linked.rs` builds the one-time linked copy (`--build-linked-data`) that restores the
anchors the conversion dropped. The dictionary is immutable after startup.

## Behavior Guidelines

- Always be precise and technical.
- Do not speculate; if unsure, state what information is missing.
- Follow code style guidelines exactly.
- If you know code is buggy, you should not perform a git commit.
- CRITICAL: DO NOT USE sed tool, IT CORRUPTS FILES in Windows OS Environment.

## Tool use

- Do not use Claude Code's built-in Web Search or Web Fetch tools; they do not connect to the
  Claude server. Use the `web_search` and `web_fetch` MCP tools when web research is required.
- Use dedicated file, search, and edit tools rather than shell text manipulation.
- Never run `sed`. Use the Edit tool for exact replacements.

## CRITICAL: RESEARCH BEFORE GIVING UP

**When encountering a limitation or error, do NOT assume it's a dead end without thorough
research.**

### Required Research Steps

1. **USE WEB SEARCH** — Search for solutions, workarounds, and similar problems.
   - Search the relevant crate or API documentation.
   - Search Stack Overflow, GitHub issues, and upstream changelogs.
   - Search for "<API> limitation workaround" when a direct route fails.
   - *(The source guideline used Office.js as its example; for this project, search the
     documentation of the crate or protocol involved.)*
2. **THINK CREATIVELY** — Consider alternative approaches:
   - Can the problem be solved differently?
   - Are there workarounds using different APIs?
   - Can the data flow be restructured?
3. **VERIFY LIMITATIONS** — Do not assume something is impossible without proof:
   - Document what you tried.
   - Document the exact error received.
   - Only call it a limitation after exhaustive research.
4. **PROPOSE WORKAROUNDS** — Even if a direct solution isn't possible:
   - Can intermediate steps solve it?
   - Can helper functions or data reshaping help?
   - Can the approach be inverted?

### Example of What NOT To Do

```text
❌ "This is an API limitation" (without research)
✅ "Let me search for workarounds to this issue"
✅ "I found several approaches others have used..."
```

### Before Declaring a Limitation

You MUST:

1. Search the web for solutions.
2. Try at least 2–3 different approaches.
3. Document what was attempted.
4. Only then propose a workaround.

## PROBLEM SOLVING GUIDELINES

### When Stuck on a Bug or Issue

**DO NOT GIVE UP EASILY.** Follow these steps before accepting a limitation:

1. **Research First** — Use web search to find whether others have solved the same problem.
   - Search for the specific error or API limitation.
   - Check Stack Overflow, official docs, and GitHub issues.
   - Look for workarounds others have used.
2. **Think Creatively** — Consider alternative approaches:
   - Can the problem be solved differently?
   - Is there a workaround using different APIs?
   - Can the input be transformed to make it work?
3. **Test Assumptions** — Do not assume a limitation without proof:
   - Try simple test cases to isolate the issue.
   - Verify the exact error message.
   - Check for caching or configuration issues.
4. **Document What Was Tried** — Even if you cannot solve it, document:
   - Which approaches were attempted.
   - What the error messages were.
   - What might work as a solution.

### Example of NOT Giving Up

**Bad response:** "This is a library limitation, it can't be done."
**Good response:** "The direct approach doesn't work, but let me try a different parser shape as a
workaround."

## Coding guidelines (andrej-karpathy-skills)

Derived from the MIT-licensed *Karpathy-Inspired Claude Code Guidelines*
(`forrestchang/andrej-karpathy-skills`). These bias toward caution over speed; for trivial tasks,
use judgment.

### 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

- State assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them; do not pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what is confusing. Ask.

### 2. Simplicity First

**Minimum code that solves the problem. Nothing speculative.**

- No features beyond what was asked.
- No abstractions for single-use code.
- No "flexibility" or "configurability" that was not requested.
- No error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.

Ask: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

### 3. Surgical Changes

**Touch only what you must. Clean up only your own mess.**

When editing existing code:

- Do not "improve" adjacent code, comments, or formatting.
- Do not refactor things that are not broken.
- Match existing style, even if you would do it differently.
- If you notice unrelated dead code, mention it; do not delete it.

When your changes create orphans, remove imports, variables, or functions that **your** changes
made unused. Do not remove pre-existing dead code unless asked.

**The test:** every changed line should trace directly to the user's request.

### 4. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass."
- "Fix the bug" → "Write a test that reproduces it, then make it pass."
- "Refactor X" → "Ensure tests pass before and after."

For multi-step tasks, state a brief plan:

```text
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let the work proceed independently. Weak criteria ("make it work")
require constant clarification.

## Project-specific rules

### Data and copyright

- The archive and its extracted text are **user-supplied copyrighted material**. Never commit the
  archive, extracted Markdown, or images. Never ship them in `dist/`.
- The EPUB is the source work; the Markdown is its derivative, and the linked copy is a further
  derivative. None of the three may enter the repository, including test fixtures. Integration
  tests use an invented corpus written at test time by `tests/support/fixture.rs`; unit-test
  samples must be invented too. `tests/fixtures/dictionary/` is ignored to prevent an accidental
  re-add.
- Treat the archive as untrusted input. Parse it; never execute scripts from it. Keep analysis
  scripts in `tools/` and pass the archive path as an argument. Run Python with `-I`.
- The server reads local files only. Do not add network access, telemetry, or remote fetches.
- `.gitignore` must cover the archive, `target/`, logs, and local data directories.

### Protocol and output

- **stdout carries JSON-RPC only.** Every diagnostic goes to stderr. No `println!` outside
  `--help` / `--version`.
- Keep the deliberate legacy protocol era: `initialize` answers `2025-11-25`, and
  `server/discover` answers `-32601` so 2026-era clients fall back.
- Tool schemas stay inside the narrow subset: `type`, `properties`, `required`,
  `additionalProperties`, `items`, `enum`, `const`, `description`, `title`, `default`,
  `examples`. A wider schema can drop the server's entire tool set in a harness.
- Input schemas are written by hand and validated by tests. **Output schemas are derived** from
  the response types in `src/dictionary/model.rs` through `src/schema.rs`; never hand-edit an
  `outputSchema`, and never add a response field without the `Serde`/`JsonSchema` derives. A
  contract test validates real `structuredContent` against the advertised schema, so a payload
  field that is missing from the schema fails `cargo test`.
- Bounds the schema cannot express are stated in prose and enforced in code.
- Results are bounded: 5 hits by default (max 20), 20,000 characters per entry by default
  (max 100,000). Never return an unbounded entry body.

### Parser rules

- In the Markdown archive, anchors exist only in `38-index.md`; never expect anchor definitions in
  the letter files. The source EPUB defines them on entry containers, and a small table verified
  against the EPUB fills the index's few omissions. Never invent an anchor from Markdown content.
- Treat `#a1XXXX` anchors as opaque source-EPUB element IDs. Resolve them through the index plus
  that verified table; never parse them, sort by them, or infer page/entry meaning from them.
- An entry boundary is a line whose **first byte** is `\` and whose second byte is `*`.
  `\*` also occurs mid-line in alias lines.
- Normalize for matching only: NFKD, strip combining marks, fold NBSP, unescape Markdown,
  lowercase, collapse whitespace. Preserve display text verbatim.
- Tolerate the known anomalies: duplicate headwords, an empty entry, missing alias lines, `⁑`,
  nested parentheses, escaped brackets, `No Match` suffixes, and old appendix filenames.
- Malformed source data must produce warnings and a degraded result, never a panic or a startup
  crash after the archive is found.

### Surfaces that must move together

A change is not done when the code works. It is done when every surface that describes the code
agrees with it:

- `src/tools.rs` tool names, descriptions, input schemas
- `src/schema.rs` output schemas, derived from `src/dictionary/model.rs` response types
- `src/server.rs` `SERVER_INSTRUCTIONS`, error messages, version
- `src/config.rs` `usage()` and the flag/environment behaviour it advertises
- `README.md` configuration table, tool tables, data requirements
- `BUILDING.md` target commands, artifact names, platform notes
- `src/error.rs` stable codes and messages
- `PLAN.md` behaviour claims and measured counts
- `CHECKLIST.md` task status and progress log
- `tests/fixtures/tools-list.json` — the reviewed `tools/list` snapshot
- `tests/` — the assertion that keeps the surfaces in step

### Completion gates

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
```

`cargo test --all-targets` includes the schema contract: real payloads are validated against the
advertised `outputSchema`, and `tests/fixtures/tools-list.json` is compared with `tools/list`. An
intentional schema or description change must be reviewed as a diff and accepted with:

```sh
MCP_UPDATE_TOOLS_LIST=1 cargo test --test schema_contract
```

Do not commit while a known bug exists. Do not mark a checklist task `[x]` without running the
verification named beside it. Append the evidence to the progress log in `CHECKLIST.md` in the
same turn.

### Windows and portability

- Target Windows first; keep the Rust code portable.
- Handle CRLF, non-breaking spaces, and paths with spaces (the archive filename has spaces).
- Use absolute paths in MCP client configuration examples because clients may launch the server
  from another working directory.
- Never use `sed`.

## Diagnostics

When startup or a tool call fails and the cause is not obvious:

1. Run the binary with `--verbose --log-file <path>`; an MCP client usually discards the child's
   stderr.
2. Check the load report: main entries, appendix entries, resolved/unresolved links, header
   failures.
3. Reproduce with a piped stdio session:
   `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | <binary> --data <path>`.
4. Isolate the data path from the protocol path: load the same archive in a unit test or with the
   inspection script.
