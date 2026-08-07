Sources — the PowerShell documentation corpus

powershell-mcp serves one corpus: the official PowerShell documentation
repository maintained by Microsoft.

Repository

- Owner/repo: MicrosoftDocs/PowerShell-Docs
- URL: https://github.com/MicrosoftDocs/PowerShell-Docs
- Default branch: main
- Snapshot: shallow clone (--depth 1) kept under corpus/PowerShell-Docs in
  this project, not committed to this repository.

What gets indexed

The scanner (src/scan.rs) indexes markdown only, which for this repo
means:

- reference/<version>/<Module>/<Cmdlet>.md — the cmdlet reference pages
  (the primary source of truth for syntax, parameters, outputs, and
  examples).
- reference/<version>/Microsoft.PowerShell.Core/About/about_*.md — the
  about topics (concepts such as about_Arrays, about_CommonParameters,
  about_Splatting).
- reference/docs-conceptual/**/*.md — conceptual and learning guides.
- reference/includes/*.md — shared snippets such as support matrices.

Versions present at index time: 5.1, 7.4, 7.5, 7.6, 7.7. The file path
carries the version and module, so every retrieval hit states which
PowerShell version a behavior belongs to.

What is skipped

- CI and contribution scaffolding: .github/, tests/, .devcontainer/.
- Navigation and build support: reference/bread/, reference/mapping/,
  redir/, media/, assets/.
- Contributor-facing documents at the repo root: CODE_OF_CONDUCT.md,
  CONTRIBUTING.md, SECURITY.md, ThirdPartyNotices.md, LICENSE.md.
- Hidden entries, files over 2 MB, and binary files.

Licensing

The corpus checkout is Microsoft documentation, released under the
Creative Commons Attribution 4.0 International license (see
corpus/PowerShell-Docs/LICENSE.md and LICENSE-CODE.md). It is a data
snapshot for the index, not part of this repository's source. The
powershell-mcp code itself is Apache-2.0.

Refreshing

Pull upstream and rebuild the index; see docs/setup.md section 6. The
index manifest's source_fingerprint changes whenever the corpus does, so
index_info makes staleness detectable.
