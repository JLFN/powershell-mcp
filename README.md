# powershell-mcp — hybrid RAG over the official PowerShell documentation

[![license](https://img.shields.io/badge/license-Apache--2.0-blue?style=for-the-badge)](LICENSE)
[![github](https://img.shields.io/badge/github-JLFN_powershell_mcp-8da0cb?style=for-the-badge&labelColor=555555&logo=github)](https://github.com/JLFN/powershell-mcp)

powershell-mcp is an MCP server that answers questions about PowerShell
from the official documentation. The corpus is the MicrosoftDocs/
PowerShell-Docs repository — the same markdown that powers
learn.microsoft.com/powershell — covering cmdlet reference pages, about
topics, and conceptual guides for PowerShell versions 5.1, 7.4, 7.5,
7.6, and 7.7.

The server is built from the rag-template recipe used by se-law-mcp and
health-second-opinion-mcp: a local hybrid index that fuses BM25 full-text
search with vector embeddings via reciprocal rank fusion. Everything runs
on this machine — embeddings use fastembed ONNX (bge-small-en-v1.5) and
nothing leaves it.

What you get

- Local retrieval only: embeddings run on this machine, no API key.
- Hybrid search: BM25 (exact cmdlet names, parameters, keywords) and
  vector similarity (concepts) fused with reciprocal rank fusion.
- Provenance on every hit: file path (which carries the PowerShell
  version and module), 1-based line range, and the text.
- Paragraph-aligned chunking that never splits a function body, a code
  block, or a prose paragraph.
- A three-tool MCP server registered with Open Grok: search,
  file_context, index_info.
- A build-index CLI, an install command that registers the server with
  the index baked in, and a companion agent skill.
- A stale-index guard: switching embedding models refuses to serve old
  vectors until you rebuild.

Tools

- search(query, mode, k): the top-k most relevant chunks for a question
  about PowerShell. mode is hybrid (default), vector, or bm25; k
  defaults to 5, max 20.
- file_context(file): every chunk of one documentation file, in source
  order, with line ranges.
- index_info(): model, dimensions, chunk and file counts, built-at time,
  corpus root, and source fingerprint. Errors clearly when no index has
  been built yet.

CLI

- powershell-mcp build-index <corpus-root> [index-dir]
- powershell-mcp query <question> [mode] [k]
- powershell-mcp index-info
- powershell-mcp files
- powershell-mcp install <index-dir> [server-name]
- powershell-mcp serve (default when no arguments; MCP over stdio)

Installation

1. Build with the embedded standard builder from the repo root:

   bash build/linux/build.sh

   The release binary lands in bin/powershell-mcp (target/ is removed).

2. Get the corpus (a checkout of the official docs repo):

   git clone --depth 1 https://github.com/MicrosoftDocs/PowerShell-Docs corpus/PowerShell-Docs

3. Index the corpus:

   bin/powershell-mcp build-index corpus/PowerShell-Docs index

   The first run downloads the embedding model (bge-small-en-v1.5, about
   130 MB, cached under ~/.cache/fastembed); later runs use the cache.

4. Install and register with Open Grok:

   bin/powershell-mcp install index

   This copies the binary to ~/.local/bin, adds the
   [mcp_servers.powershell-mcp] block to ~/.opengrok/config.toml with
   POWERSHELL_MCP_HOME pointing at your index, and installs the skill.

5. Restart Open Grok (or press r in /mcps), then verify:

   open-grok mcp doctor powershell-mcp

Configuration

The server serves whichever index POWERSHELL_MCP_HOME points at (the
install command bakes this in). Environment variables:

- POWERSHELL_MCP_HOME: index directory. Default for build-index is
  <corpus>/.rag-index; default for query/serve is <cwd>/.rag-index.
- POWERSHELL_MCP_EMBED_MODEL: bge-small (default), e5-multilingual, or
  e5-base. Changing it invalidates an existing index; rebuild it.
- POWERSHELL_MCP_SERVER_NAME: MCP server name used at install time
  (default powershell-mcp).
- POWERSHELL_MCP_INSTALL_DIR: where install copies the binary (default
  ~/.local/bin).
- FASTEMBED_CACHE: where embedding models are cached (default
  ~/.cache/fastembed).

Architecture

The index lives in a RAG data directory as three flat files plus the BM25
index: manifest.json (model, dim, counts, corpus fingerprint), chunks.jsonl
(one JSON record per chunk with provenance), embeddings.bin (raw
little-endian f32 vectors), and tantivy/ (the full-text index). The server
loads the index once per process and caches it. A query in hybrid mode
runs BM25 and cosine-similarity retrieval in parallel, then fuses the two
rankings with reciprocal rank fusion; bm25 mode needs no model at all and
vector mode is pure semantic search.

Which files are indexed: markdown under the corpus (reference/ cmdlet
pages, About topics, and docs-conceptual guides), with CI, tests, media,
redirects, and contributor-facing documents skipped (src/scan.rs). Hidden
entries are skipped, files over 2 MB are skipped, and files containing NUL
bytes are treated as binary and skipped.

Refreshing the corpus

The source of truth is the upstream repo. To update:

   git -C corpus/PowerShell-Docs pull
   bin/powershell-mcp build-index corpus/PowerShell-Docs index

index_info reports a source fingerprint (SHA-256 over sorted file paths
and sizes), so you can detect staleness by comparing fingerprints over
time.

Testing

cargo test runs the unit and integration tests. The RAG integration test
uses a fake embedder and never downloads a model. The live MCP test builds
a real index with the real embedder and drives the full MCP handshake over
stdio; it runs only when the bge-small model is already cached, so a plain
cargo test never triggers the download. Build an index once and the live
test then runs fully.

python3 -m pytest pytest/ runs the skill quality gates (frontmatter,
trigger recall and precision) and live result-quality tests over the real
binary and index (index_info, hybrid/vector/bm25 search, file_context).
The live tests skip automatically when the index is missing.

Project layout

- src/scan.rs: corpus walking, markdown-only extension rule, exclusions.
- src/chunk.rs: paragraph-aligned chunking with line provenance.
- src/embed.rs: local fastembed model wrapper with sliced batching.
- src/bm25.rs: tantivy full-text index build and search.
- src/rag.rs: manifest/chunks/embeddings persistence, hybrid retrieval,
  reciprocal rank fusion, stale-index guard.
- src/server.rs: the MCP server and its three tools.
- src/install.rs: binary install, Open Grok registration, skill install.
- src/main.rs: CLI and stdio entry point.
- tests/rag.rs: integration tests with a fake embedder.
- tests/live_mcp.rs: end-to-end MCP round trip with the real embedder.
- pytest/: skill frontmatter and trigger quality gates plus live MCP
  result-quality tests (see pytest/README.md).
- skills/powershell-mcp/SKILL.md: the companion agent skill.
- build/: the embedded standard builder from /data/build.
- corpus/PowerShell-Docs: the docs checkout (not committed).
- index/: the built RAG index (not committed).

Documentation

- docs/setup.md: full setup, re-indexing, and refresh workflow.
- docs/chunking.md: how documents are split into retrieval units.
- docs/sources.md: the source-of-truth corpus and its licensing.

Known limitations

- The default bge-small model downloads about 130 MB on first index.
- Re-indexing is a full rebuild, not incremental.
- Chunks are capped at 512 tokens by the embedder's truncation; very long
  single lines are kept whole as their own chunk.
- The Open Grok registration is global; one server name per index.

License

Apache-2.0. The bundled corpus checkout is Microsoft documentation,
licensed under the Creative Commons Attribution 4.0 International license
(see corpus/PowerShell-Docs/LICENSE.md), and is not part of this
repository.
