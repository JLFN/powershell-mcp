# QA evidence — powershell-mcp dependency security remediation

- Unit: fix(deps): clear four OSV advisories via tantivy/fastembed major bumps
- Branch: feature/security-deps (base dev = e06bdcd)
- Commit under test: b0322a1
- QA model: self (operator-directed; the project records no QA model in its
  AGENTS.md, and the operator chose self-verification for this unit)
- Date: 2026-09-16
- Covered surfaces: ALL NEW SURFACES

## What was verified

The unit changes no tool signature, no CLI subcommand, and no output schema;
it changes dependency versions and the two call sites those versions touch
(src/bm25.rs collector + document-value reads, src/embed.rs embedder
ownership). The blanket token ALL NEW SURFACES is claimed on that basis:
every user-visible surface was re-exercised live rather than only compiled.

## Evidence

1. Build, canonical builder (global rule 5):
   bash /data/build/linux/build.sh -p /data/powershell-mcp
   -> bin/powershell-mcp, 37,465,560 bytes, release profile, exit 0.
   Sha256 37029721dd0e5230afac9644d9290da06c5b40e4b27b93c10ca6f42d1ea4249d.

2. Install: bin/powershell-mcp install /data/powershell-mcp/index
   -> copied to ~/.local/bin/powershell-mcp, sha256 identical to the built
   binary (verified with sha256sum on both paths). The registered server
   therefore runs the patched crates.

3. cargo test on the feature branch: 16 passed, 0 failed.
   - 11 unit tests (chunk, scan, install, embed)
   - 1 live MCP round-trip driving the real binary over stdio
   - 4 integration tests (rag build/query, stale-manifest guard, error paths)
   cargo check --all-targets: exit 0 with zero warnings.

4. Live MCP E2E over stdio against the real 21,340-chunk index, driven by an
   independent JSON-RPC client (not the crate's own test harness):
   - initialize -> serverInfo powershell-mcp 0.1.1
   - tools/list -> exactly [file_context, index_info, search]
   - search mode=hybrid -> 3 hits, top Get-ChildItem.md L1-53, rrf 0.0320
   - search mode=vector -> 3 hits, top Get-ChildItem.md L1-53, cosine 0.8396
   - search mode=bm25   -> 3 hits, top Get-ChildItem.md L1-53, bm25 15.8431
     (all modes returned file + line_start + line_end + text provenance)
   - index_info -> bge-small-en-v1.5, 384 dims, 21340 chunks, 3090 files
   - file_context reference/7.4/.../Get-ChildItem.md -> 21 chunks, all from
     that file, first chunk L1-53
   This is the operative proof: BM25 exercises the tantivy 0.26 collector and
   CompactDocValue paths that this unit rewrote, and vector/hybrid exercise the
   fastembed 7 embedder that this unit rewrote.

5. Index compatibility: the existing index was built 2026-08-07 by tantivy
   0.22. tantivy 0.26 reads it unchanged (INDEX_FORMAT_OLDEST_SUPPORTED_VERSION
   = 4); the manifest still reports 21340 chunks / 3090 files, fingerprint
   8674d5fa7ebc. No reindex was required or performed.

6. pytest quality suite: 57 passed in 0.44s (frontmatter, trigger
   recall/precision, live result quality).

7. Post-fix OSV rescan over Cargo.lock: 354 packages, 2 findings, both
   documented accepted risk with written reasoning (lru 0.16.4 pinned by
   tantivy's ^0.16.3 requirement with no patched 0.16.x; paste unmaintained,
   compile-time-only proc-macro, unreachable at runtime). The four other
   advisories are cleared: h2 and rustls upgraded past their fixes, instant and
   number_prefix removed from the tree entirely.

## Gaps and accepted risk

- The two remaining advisories are unfixable from this crate today and are
  recorded in the commit body with reachability analysis. Re-check on the next
  tantivy and fastembed releases.
- The server is listed in disabled_mcp_servers in ~/.opengrok/config.toml.
  That predates this unit (confirmed against a pre-install backup of the
  config) and is an operator setting; the binary install and registration are
  correct and the server answers correctly when launched directly.

verdict: overall PASS
