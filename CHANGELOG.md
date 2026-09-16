# Changelog

All notable changes to this project are documented in this file. The format
follows Keep a Changelog, and the project adheres to Semantic Versioning.

## [0.1.2] - 2026-09-16

### Security

- Cleared four OSV advisories by moving the two dependency trees that pinned
  them. `tantivy` 0.22 to 0.26 and `fastembed` 4 to 7 remove the `instant`
  and `number_prefix` crates from the graph entirely and carry `lru` past its
  `IterMut` soundness fix. `h2` moved to 0.4.16 and `rustls` to 0.23.45 for
  RUSTSEC-2026-0258 and RUSTSEC-2026-0285.
- `fastembed` default features are disabled, dropping the unused
  `image-models` feature and with it roughly sixty crates of image-decoding
  machinery this text-only server never invokes.
- Two findings remain as documented accepted risk, both unreachable here:
  `lru` 0.16.4, whose fix (0.18.2) is excluded by tantivy's `^0.16.3`
  requirement, and `paste` 1.0.15, a maintenance-status notice reached only
  through `tokenizers` as a compile-time-only proc macro.

### Changed

- `src/bm25.rs`: tantivy 0.26 API. The collector is
  `TopDocs::with_limit(n).order_by_score()` and document values are read as
  `CompactDocValue` through the `Value` trait.
- `src/embed.rs`: fastembed 7 API. `embed` takes `&mut self`, so the
  process-global embedder is held behind a `Mutex`; `InitOptions` is now
  `TextInitOptions`.

### Added

- `tests/qa-gate.sh` and `tests/run.sh`: the local verification chain, with
  the AI QA-tester gate wired for this project's surfaces.
- `qa-evidence/`: QA reports for the dependency remediation and for the
  delivered build's usage evidence.

### Notes

- The index format is unchanged. An index built by tantivy 0.22 is read
  unchanged by 0.26, so no rebuild is required after upgrading.
- No public API change: the affected crates are not exposed in any public
  signature.

## [0.1.1] - 2026-08-07

### Added

- Repository and documentation metadata for crates.io, and README badges.

### Fixed

- Live MCP tests skip cleanly when the server binary is missing.

## [0.1.0] - 2026-08-07

### Added

- Initial release: hybrid BM25 + vector RAG MCP server over the official
  PowerShell documentation, with `search`, `file_context`, and `index_info`
  tools, a `build-index` CLI, an install command, and a companion skill.
