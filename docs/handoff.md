---
project: powershell-mcp
plan_start_commit: 23bfee6
last_updated_commit: 23bfee6
branch: main
remote: https://github.com/JLFN/powershell-mcp.git
handoff_written_at_context_usage: ~5% (fresh session; signals.json not reliably written)
handoff_written_at: 2026-08-11T22:51:00+02:00
session_id: 019ff28c-9a8f-72c2-8f59-55e0a2e2f008 (parent orchestrator session)
---

# Handoff + executable plan — powershell-mcp (2026-08-11)

## 0. Rules for writing THIS document (read before editing, not just before reading)

- Only write or replace this file at a clean, verified, committed, pushed boundary. Never checkpoint mid-work.
- Start drafting once context usage crosses ~25%. Finalize and commit before 40% — do not wait for the threshold to begin writing. The writing is a summarization task and degrades like any other task under context pressure. Treat 25-35 as the drafting target and 40 as the hard cap.
- Every claim in "Current state" (section 2) must be re-verified by running the relevant command in this session, not copied or paraphrased from a previous handoff. If you cannot re-run it, mark it unverified rather than asserting it.
- Hard caps: "Current state" max 15 lines. "Guardrails and open items" max 15 lines. "Next unit plan" has no cap — it is the actual work spec. If a section would exceed its cap, cut detail; do not compress into denser prose (denser prose is where drift hides).
- Do NOT: replay prior units' logs (git log is the record), re-explain global rules already in ~/.opengrok/AGENTS.md, restate anything the staleness check in section 6 already covers, or record acceptance evidence you did not personally observe this session.
- The handoff-hooks binary (~/.opengrok/hooks/bin/handoff-hooks) scaffolds the frontmatter at SessionEnd/PreCompact, refreshes the progress/staleness readout at SessionStart, and warns when a unit's final commit lacks the "Unit: N complete" trailer. It is passive and fails open; keep the document in the template shape so the automation can manage it.

## 1. Session protocol (global rule 9)

This project is a sequence of units (feature commits / rounds); each unit ends with a verified, pushed commit set. On completing a unit: do not stop at the boundary. Begin drafting the next unit's plan text (section 3) once usage crosses ~30%; the 40% mark is when you cut over, not when you start writing.

1. Context usage: the /context gauge is the authority; signals.json is not reliably written on this machine. If signals.json exists, read contextTokensUsed / contextWindowTokens from the newest session dir under ~/.opengrok/sessions/; otherwise use the gauge. Treat a bytes/4 estimate of chat_history.jsonl as a hard LOWER BOUND that undercounts badly (injected AGENTS.md + skill list + tool schemas are missed); add a large safety margin.
2. If usage < 40%: continue immediately — write the next unit's plan into section 3 below, execute it, verify, commit, push, re-check usage, repeat.
3. At the ~40% threshold: do not pause or ask. Finish the write-up of this handoff (started per section 0's drafting rule), then launch a fresh session via ~/.local/bin/open-grok-handoff.

Standing rules currently in effect:
- Backup (global rule 7, yolo mode): runs at commit time whenever a commit changes open-grok config. No confirmation asked.
- Graphify rebuild (rule 12): at commit time.
- repo-rag reindex (rule 11): on demand only — no commit-time step; rebuild only when search must reflect new code.

## 2. Current state (2026-08-07 — initial delivery COMPLETE, verified)

- Newest unit outcome: initial delivery complete. 7 commits, all 2026-08-07, all on main (HEAD 23bfee6, working tree clean, no tags): feat "hybrid RAG MCP server over official PowerShell docs" (cf524c1) through the test-skip fix (23bfee6); version 0.1.1. First handoff for this project — no prior handoff or plan file existed and there is no dedicated project memory entry.
- What the repo is: hybrid RAG MCP server over the official PowerShell documentation (MicrosoftDocs/PowerShell-Docs). Tools: search (hybrid/vector/bm25), file_context, index_info. BM25 (tantivy) fused with local fastembed ONNX vectors (bge-small-en-v1.5, no API key) via reciprocal rank fusion. Also a build-index CLI, an install command that registers the server with Open Grok, and a companion agent skill.
- Index state (verified this session from index/manifest.json): bge-small-en-v1.5, 384 dims, 21340 chunks / 3090 files, built 2026-08-07T07:55:51Z, source fingerprint 8674d5fa7ebc. The index is a snapshot of the corpus checkout — compare the fingerprint after a corpus pull (index_info reports it).
- Corpus: corpus/PowerShell-Docs is a shallow git checkout with origin https://github.com/MicrosoftDocs/PowerShell-Docs (git pull to refresh); not committed to this repo.
- Tests: cargo suite (tests/rag.rs with a fake embedder, tests/live_mcp.rs real round-trip that runs only when bge-small is cached); pytest/ quality gates (frontmatter, trigger recall/precision, live result quality; live tests skip without an index).

## 3. Next unit plan — none documented

No active plan documented. Derive the next unit from git log and open issues; do not invent work. The repo states no next step (no TODO/roadmap markers in README.md, docs/, pytest/, or skills/); the most recent commits prepared the 0.1.1 crates.io/docs.rs metadata, so the only genuinely documented candidate is verifying that publication actually landed (section 5 open item), which is a check, not in-repo work.

Trailer convention: each unit's final commit carries the trailer "Unit: <N> complete" as its last body line, so a fresh session can reconstruct unit boundaries with git log --grep instead of reading diffs. When a unit is planned, write its concrete plan HERE before executing it (goal and verified ground truth; phases with files, commands, and per-phase acceptance checks; the verify/package procedure with the expected counts; notes/decisions). When the unit completes: replace this entire section with the next unit's plan, and fold this unit's outcome into section 2. Do not keep both.

## 4. Project facts a fresh session cannot re-derive

- What exists: Rust crate (edition 2021, rust-version 1.88, Apache-2.0). src/scan.rs, src/chunk.rs, src/embed.rs, src/bm25.rs, src/rag.rs, src/server.rs, src/install.rs, src/main.rs; tests/rag.rs and tests/live_mcp.rs; pytest/ skill-quality + live gates (see pytest/README.md); skills/powershell-mcp/SKILL.md; build/ (embedded standard builder from /data/build); docs/setup.md, docs/chunking.md, docs/sources.md; corpus/PowerShell-Docs (not committed); index/ (built RAG index, not committed).
- Repositories: the main repo's only remote is github.com/JLFN/powershell-mcp.git (branch main). corpus/PowerShell-Docs is its own git checkout with origin https://github.com/MicrosoftDocs/PowerShell-Docs (shallow clone).
- Installed / registered: ~/.local/bin/powershell-mcp (Aug 7 build); [mcp_servers.powershell-mcp] in ~/.opengrok/config.toml with POWERSHELL_MCP_HOME=/data/powershell-mcp/index (enabled); skill synced to ~/.opengrok/skills/powershell-mcp/SKILL.md; embedding model bge-small-en-v1.5 cached under ~/.cache/fastembed (POWERSHELL_MCP_EMBED_MODEL switchable — changing it invalidates stored vectors and demands a full rebuild); a running MCP server keeps its startup state (inode) until restarted.
- README advertises crates.io and docs.rs badges for 0.1.1; the publication status was NOT verified this session (check crates.io before relying on it).
- Verification: no counts recorded here. Run the project suite; the expected numbers belong in the next unit's verify phase (section 3).

## 5. Guardrails and open items

- Global rules in effect (from ~/.opengrok/AGENTS.md): yolo mode, no pause-and-ask (rules 6/7/8/9/10); canonical builder for deliverables, target/ removed by design (5); OSV scan before push (10); graphify rebuild at commit time (12); repo-rag on demand (11); never publish private repos, visibility check (gh repo view) before any remote action (6); no emojis / plain text / conventional commits with QA bodies (1-3); web fetch fallback chain with rustwright (4); browser automation only via MCP servers (8).
- Project rules (from /data/powershell-mcp/AGENTS.md): build with bash build/linux/build.sh, install with bin/powershell-mcp install index; never point anything at target/; the index is a snapshot of the corpus checkout — compare source_fingerprint after git -C corpus/PowerShell-Docs pull; re-indexing is a full rebuild (~21,000 chunks, about 30 minutes — run as a background job with a generous timeout); cite every search result with the returned file path and line range; try bm25 mode before concluding the docs lack a term.
- OPEN items: none known beyond the two below. Verify the crates.io/docs.rs publication status of 0.1.1 (README advertises it; not re-verified this session). Optionally create a dedicated project memory entry under ~/.opengrok/memory — this project currently has none (the handoff is the only cross-session artifact).

## 6. Progress and staleness check (run this first, every session)

Progress meter (how far through the plan we are — the diff you run after a crash or stop):
    cd /data/powershell-mcp
    git log --oneline 23bfee6..HEAD
    git diff --stat 23bfee6..HEAD
    git log --grep "Unit: .* complete" 23bfee6..HEAD
The diff shows what changed; the grep lists completed unit boundaries without reading diffs (each unit's final commit carries the "Unit: N complete" trailer, per section 3). Trust git over the plan headings: continue with the first unit whose work is NOT in the diff. plan_start_commit equals HEAD, so all three commands are empty until the first unit lands.

Staleness check (does this handoff predate newer work):
    cd /data/powershell-mcp && git log --oneline 23bfee6..HEAD
- Any output means this handoff predates newer commits. Read the log and diffs before trusting sections 2-5. Update last_updated_commit in the frontmatter when done.
- Branch or remote in the frontmatter differs from the actual repo: stop and reconcile before continuing; do not proceed on a guess.

This document is COMPLETE and self-contained: a new session can execute the project end to end from this file alone. It carries the plan ahead, the facts git cannot provide, and the guardrails. History is deliberately absent — git log is the historical record, and the git diff from plan_start_commit is the progress meter.
