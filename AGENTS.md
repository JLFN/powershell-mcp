# AGENTS.md — powershell-mcp

## What this is

A local hybrid RAG MCP server over the official PowerShell documentation
(MicrosoftDocs/PowerShell-Docs). Tools: search, file_context, index_info
(used as powershell-mcp__<tool>). The index fuses BM25 (tantivy) with
local vector embeddings (fastembed ONNX, no API key) via reciprocal rank
fusion.

## Current state of this machine's install

- Project: /data/powershell-mcp (source, builder, docs, skill).
- Binary: ~/.local/bin/powershell-mcp (built with the embedded build/
  standard builder; target/ is removed by design — never point anything
  at target/).
- Corpus: /data/powershell-mcp/corpus/PowerShell-Docs — a shallow clone
  of the upstream docs repo (git pull to refresh).
- Index: /data/powershell-mcp/index (built with
  `powershell-mcp build-index corpus/PowerShell-Docs index`). Registered
  in ~/.opengrok/config.toml as [mcp_servers.powershell-mcp] with the
  .env block setting POWERSHELL_MCP_HOME.
- Embedding model: bge-small-en-v1.5 (cached in ~/.cache/fastembed),
  switchable via POWERSHELL_MCP_EMBED_MODEL; changing it invalidates the
  stored vectors and demands a rebuild.
- Skill: ~/.opengrok/skills/powershell-mcp/SKILL.md.

## Data state

- The index is a snapshot of the corpus checkout. Check staleness with
  index_info: compare source_fingerprint against a fresh build after
  `git -C corpus/PowerShell-Docs pull`.
- Re-indexing is a full rebuild, not incremental.

## Rebuild guidance

- After pulling upstream docs: run
  `bin/powershell-mcp build-index corpus/PowerShell-Docs index`, then
  reinstall only if the binary itself changed
  (`bin/powershell-mcp install index`).
- A full build embeds ~21,000 chunks and takes about 30 minutes on this
  machine; treat it as a background job with a generous timeout.

## Rules for agents using it

- Cite every claim with the returned file path and line range; the path
  carries the PowerShell version and module.
- search returns ranked chunks, not answers — synthesize from the text.
- If a term returns nothing, try bm25 mode (exact keyword) before
  concluding the docs lack the information.

## Memory and handoff

- The complete project handoff + executable plan is at
  /data/powershell-mcp/powershell-mcp-handoff.md (the hook-managed
  handoff per global rule 9 — read it first for this project's state,
  open items, and acceptance criteria; the handoff-hooks binary refreshes
  its frontmatter and injects the progress/staleness readout). The
  docs/handoff.md file in this repo is a one-line redirect to it.
- The project memory entry lives in
  ~/.opengrok/memory/powershell-mcp-aec8143f/MEMORY.md. If anything is
  unclear, run memory_search for "powershell-mcp" or read that file
  before guessing.
- Branches: dev is the integration branch (the local working line) and
  main is production. Work branches fork dev and merge back via a PR;
  dev reaches main only via a PR. Run `bash tests/run.sh` before any
  commit for delivery or push — it runs the cargo suite, the pytest
  gates, and the rule-19 AI QA-tester gate (tests/qa-gate.sh).
- Launch sessions with `open-grok --cwd /data/powershell-mcp`: the hooks
  fail open outside a git workspace, so a session started from /data
  produces no hook-managed handoff.
