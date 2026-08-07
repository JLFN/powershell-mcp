---
name: powershell-mcp
description: >
  Answer questions about PowerShell using the official Microsoft
  documentation through the powershell-mcp MCP server: search the hybrid
  BM25 + vector index of the PowerShell-Docs corpus (cmdlet reference,
  about topics, and conceptual guides) with file and line provenance,
  pull the full text of one doc file, and check index state. Use when the
  user asks about PowerShell cmdlets, syntax, parameters, modules,
  scripting concepts, or version-specific behavior, or runs
  /powershell-mcp.
argument-hint: "[question]"
metadata:
  short-description: "PowerShell documentation search (cmdlet reference, about topics, conceptual guides)"
---

# PowerShell Docs (powershell-mcp)

Hybrid retrieval over the official PowerShell documentation. The corpus is
a checkout of the MicrosoftDocs/PowerShell-Docs repository — the same
markdown that powers learn.microsoft.com/powershell — covering cmdlet
reference pages, about topics, and conceptual guides for PowerShell
versions 5.1, 7.4, 7.5, 7.6, and 7.7. Embeddings run locally (fastembed
ONNX, bge-small-en-v1.5); nothing leaves the machine.

The server powershell-mcp provides three tools, used as
powershell-mcp__search, powershell-mcp__file_context, and
powershell-mcp__index_info.

## The flow

1. If search errors with "no semantic index", the index has not been
   built or POWERSHELL_MCP_HOME points at the wrong place. Run
   `powershell-mcp build-index <PowerShell-Docs checkout> <index-dir>` from
   the project, then `powershell-mcp install <index-dir>` and restart the
   MCP client.
2. For a PowerShell question, call powershell-mcp__search with the
   question. Use mode hybrid by default; switch to bm25 for exact cmdlet
   names, parameters, or keywords, and vector for paraphrase-level concept
   matching. k defaults to 5, max 20.
3. Each hit is a chunk with file, line_start, line_end, text, and scores
   (score, bm25_score, vector_score). File paths carry the PowerShell
   version and module, for example
   reference/7.4/Microsoft.PowerShell.Core/Get-Command.md. Answer from the
   text and cite the file and lines, including the version.
4. When the answer needs the whole of one document (for example the full
   parameter table of a cmdlet, or an entire about topic), call
   powershell-mcp__file_context with the exact file path from a search
   hit.
5. To check whether the index is stale (the corpus changed since it was
   built), call powershell-mcp__index_info and compare source_fingerprint
   with a fresh build; refresh the checkout with
   `git -C <corpus> pull` and rebuild with
   `powershell-mcp build-index <corpus> <index-dir>`.

## Rules

- search returns ranked chunks, not answers. Read the text and synthesize.
- Always cite file, line range, and the PowerShell version from the hits;
  do not invent paths or cmdlets.
- The docs are the source of truth for syntax, parameters, and behavior.
  If a search returns nothing for a term, try bm25 mode (exact keyword)
  before concluding the docs lack the information.
- file_context needs the exact relative path from a search result
  (forward slashes).
- bm25 mode needs no embedding model; vector and hybrid refuse to run
  against an index built with a different model (stale-index guard) until
  it is rebuilt.

## Worked example

Question: "How do I use Get-Command to find cmdlets that take a
Credential parameter?"

Call powershell-mcp__search with query "Get-Command find cmdlets
Credential parameter", mode hybrid, k 5. Hits point at
reference/7.4/Microsoft.PowerShell.Core/Get-Command.md (syntax and
ParameterName parameter, with the -ParameterName example) and the about
topic for common parameters. Answer: Get-Command accepts -ParameterName
Credential to filter by parameter, cite the file and lines, and open
file_context on the Get-Command.md file if the full parameter table is
needed.
