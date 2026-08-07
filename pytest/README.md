Pytest suite for powershell-mcp

Quality gates for the server and its shipped skill, mirroring the
health-second-opinion-mcp pytest convention.

Tests

- test_frontmatter.py — the SKILL.md frontmatter must parse as strict
  YAML and carry the fields open-grok needs (name, description,
  when-to-use, user-invocable, argument-hint, no emojis).
- test_triggers.py — recall and precision of the when-to-use trigger
  list: realistic PowerShell prompts must fire, unrelated prompts must
  not, and the documented case expectations must match the list.
- test_mcp.py — live result-quality tests over the real binary and index:
  index_info, hybrid/vector/bm25 search with provenance and scores, and
  file_context round-trips.

Setup

    pip install -r requirements.txt

Run

    pytest pytest/ -v

The live MCP tests spawn bin/powershell-mcp (build it first with
build/linux/build.sh) against the index at index/ (build it first with
`bin/powershell-mcp build-index corpus/PowerShell-Docs index`). They are
skipped automatically when the index is missing. Override the paths with
POWERSHELL_MCP_BIN, POWERSHELL_MCP_HOME, and POWERSHELL_SKILL_PATH.
