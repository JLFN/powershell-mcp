Setup — PowerShell docs RAG for Open Grok

This guide walks through getting the corpus, building the index,
registering the MCP server, and keeping the index fresh. It assumes Linux
and Open Grok; the same steps apply elsewhere with the matching builder
script.

1. Build

   cd /data/powershell-mcp
   bash build/linux/build.sh

The canonical builder compiles a release binary to bin/powershell-mcp and
removes target/. For iterative work you may use cargo build directly so
the incremental cache survives; always produce the final binary with the
embedded builder.

2. Get the corpus

The source of truth is the official MicrosoftDocs/PowerShell-Docs
repository. A shallow clone keeps it small (about 90 MB):

   git clone --depth 1 https://github.com/MicrosoftDocs/PowerShell-Docs corpus/PowerShell-Docs

3. Index the corpus

   bin/powershell-mcp build-index corpus/PowerShell-Docs index

The first run downloads the embedding model (bge-small-en-v1.5, about
130 MB) into ~/.cache/fastembed; later runs use the cache. The index is
written to index/. A full build embeds roughly 21,000 chunks and takes
about 30 minutes on this machine (the ONNX runs saturate all cores); it
is a background operation, run it with a generous timeout. To put the
index elsewhere, set POWERSHELL_MCP_HOME or pass the directory as the
second argument.

4. Register with Open Grok

   bin/powershell-mcp install index

This does three things:

- Copies the binary to ~/.local/bin/powershell-mcp.
- Adds the [mcp_servers.powershell-mcp] block to ~/.opengrok/config.toml
  with command, enabled, and an env table setting POWERSHELL_MCP_HOME to
  your index. An existing block for the same server name is replaced;
  nothing else in the config changes.
- Installs the skill to ~/.opengrok/skills/powershell-mcp/SKILL.md.

5. Restart and verify

Restart Open Grok or press r in the /mcps screen, then check health:

   open-grok mcp doctor powershell-mcp

You can also verify the index directly:

   bin/powershell-mcp index-info
   bin/powershell-mcp query "how does Get-Command work" hybrid 5

6. Refresh after upstream changes

The index is a snapshot of the checkout. To update it:

   git -C corpus/PowerShell-Docs pull
   bin/powershell-mcp build-index corpus/PowerShell-Docs index

The manifest records a source fingerprint (SHA-256 over sorted file
paths and sizes), so you can detect staleness by comparing index-info
fingerprints over time. index_info exposes it over MCP too.

7. Switching embedding models

Set POWERSHELL_MCP_EMBED_MODEL to e5-multilingual or e5-base before
build-index and before serving queries. The stale-index guard refuses to
serve vector or hybrid queries against an index built with a different
model until you rebuild; bm25 queries are unaffected because they never
use the model.

8. Tuning what gets indexed

Edit src/scan.rs and rebuild: DEFAULT_EXTS controls which extensions are
indexed (markdown only for this corpus), EXCLUDED_DIRS and EXCLUDED_FILES
control what is skipped (CI, tests, media, redirects, contributor docs),
and MAX_FILE_BYTES caps file size. The chunking policy (target size,
paragraph alignment) is in src/chunk.rs and documented in
docs/chunking.md.

Uninstalling

Remove the [mcp_servers.powershell-mcp] and
[mcp_servers.powershell-mcp.env] blocks from ~/.opengrok/config.toml,
delete ~/.local/bin/powershell-mcp, and delete
~/.opengrok/skills/powershell-mcp/. The index directory and the corpus
checkout can be deleted freely; they are not referenced by the installed
binary.
