# Usage evidence — powershell-mcp, built from the delivered HEAD

- Date: 2026-09-16
- Commit built: 8244f14 (dev == main == origin, the merge of PR #4)
- Binary: /data/powershell-mcp/bin/powershell-mcp
- sha256: 37029721dd0e5230afac9644d9290da06c5b40e4b27b93c10ca6f42d1ea4249d
- Installed copy: ~/.local/bin/powershell-mcp, same sha256
- Index served: bge-small-en-v1.5, 384 dims, 21340 chunks, 3090 files,
  fingerprint 8674d5fa7ebc

## Provenance of the binary

The binary was rebuilt from scratch from the delivered commit: the previous
bin/powershell-mcp was deleted, then `bash /data/build/linux/build.sh -p
/data/powershell-mcp` (the canonical builder, global rule 5) produced a new
one. The resulting sha256 is identical to the binary verified earlier in the
same session, so the delivered tree builds reproducibly.

## What was exercised

A real MCP session over stdio, driven by an independent JSON-RPC client, with
five questions a PowerShell user would actually ask. Every mode is covered.
For each question the TOP citation was then verified against the corpus file
on disk, independently of the server: the cited path must exist, the cited
line range must fall inside the file, and the returned chunk text must
actually appear at those lines.

| Question | Mode | Top citation | Top score | Verified |
|---|---|---|---|---|
| How do I copy a file to a remote computer reliably? | hybrid | reference/5.1/.../Copy-Item.md L176-214 | 0.0301 | yes |
| What is the difference between -Filter and -Include on Get-ChildItem? | hybrid | reference/5.1/.../Get-ChildItem.md L157-198 | 0.0164 | yes |
| How do I handle errors in a script with try catch? | hybrid | .../learn/deep-dives/everything-about-exceptions.md L586-635 | 0.0309 | yes |
| How do I create and use a PowerShell credential object? | vector | reference/7.4/.../Get-Credential.md L56-105 | 0.8748 | yes |
| Get-Service dependent services | bm25 | .../samples/Managing-Services.md L48-92 | 17.0228 | yes |

Result: 5 citations checked, 5 verified on disk, 0 failures.

`file_context` was also exercised (Copy-Item.md -> 15 chunks, all from that
file). `index_info` reported the index state shown above.

## Why this is meaningful

The questions are not keyword echoes of the retrieved text: the credential
question in vector mode returns the Get-Credential reference without the word
"credential object" appearing in the query, and the error-handling question
resolves to the exceptions deep-dive plus about_Try_Catch_Finally. Both
retrieval halves are therefore doing real work — BM25 through tantivy 0.26
(the version this unit moved to), and vectors through fastembed 7 (also moved
by this unit). The line ranges and text were checked against the corpus, not
taken on the server's word.

## Scope and limits

- This is a usage demonstration, not an exhaustive correctness test; it does
  not prove the ranking is optimal, only that both retrieval paths return
  correct, traceable citations for representative questions.
- The index is a snapshot of the corpus checkout from 2026-08-07. The
  fingerprint above should be compared after any `git -C corpus/PowerShell-Docs
  pull` to detect staleness.
- The server is registered in ~/.opengrok/config.toml but listed in
  disabled_mcp_servers, so open-grok sessions do not load it; the binary and
  registration are correct and the server answers correctly when launched
  directly, as shown here.
