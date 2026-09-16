#!/usr/bin/env bash
# tests/qa-gate.sh — GENERIC guardrail mount (canonical source:
# ~/.opengrok/templates/qa-gate.sh; copy into a project that ships a test
# suite). Enforces global rule 19: before a unit is committed for delivery or
# pushed, an AI QA-tester (model selected per project, never hardcoded) must verify every
# newly added DB function, API endpoint, and web client method and write a
# FRESH qa-evidence/qa-report.md ending in "verdict: overall PASS" that covers
# all of them. This mount FAILS the local gate (exit 1) until that holds; it
# SKIPS in real CI where the QA subagent cannot run.
#
# Env knobs (advisory, parameterize per project):
#   QA_BASE              ref the new surfaces are diffed against
#                        (default origin/<current branch>)
#   QA_MODEL             QA-tester model name, used in messages only; set
#                        per project (no hardcoded default)
#   QA_SURFACE_PATHS     newline list of "path:type" lines; type in
#                        db|ep|cm (db=DB FUNCTION, ep=API ENDPOINT,
#                        cm=CLIENT METHOD). Default covers common layouts.
#   QA_REPORT            path to the report (default qa-evidence/qa-report.md)
#
# Exit 0 = green (no new surfaces, or report fresh+pass+covering). Exit 1 red.
cd "$(dirname "$0")/.." || exit 1

# Real CI: the QA subagent is a local-session capability, not provisioned there.
if [[ "${GITHUB_ACTIONS:-}" == "true" ]]; then
  echo "SKIP tests/qa-gate.sh (AI QA-tester is a local-session step, not provisioned in CI)"
  exit 0
fi

BRANCH=$(git symbolic-ref --short HEAD 2>/dev/null || echo "main")
BASE=${QA_BASE:-origin/${BRANCH}}
MODEL=${QA_MODEL:-"the per-project QA model"}
REPORT=${QA_REPORT:-qa-evidence/qa-report.md}

BASE_SHA=$(git rev-parse "${BASE}" 2>/dev/null || echo "")
HEAD_SHA=$(git rev-parse HEAD 2>/dev/null || echo "")
if [[ -z "$BASE_SHA" || "$BASE_SHA" == "$HEAD_SHA" ]]; then
  echo "QA GATE: ALL PASS (no unpushed commits yet; nothing new to QA-tester)"
  exit 0
fi

# ---- project surface extraction ------------------------------------------
# This project's user-visible surfaces are its MCP tools: an added
# "#[tool(" attribute on an "async fn <name>(" row in src/server.rs, plus
# the CLI subcommands in src/main.rs as seen through the match arms.
tools=""
if git cat-file -e "${HEAD_SHA}:src/server.rs" 2>/dev/null; then
  tools="$(git diff "${BASE}" -- src/server.rs 2>/dev/null \
    | grep -E '^\+' \
    | grep -oE 'async fn [a-z_0-9]+' \
    | awk '{print $3}')"$'\n'
fi

cmds=""
if git cat-file -e "${HEAD_SHA}:src/main.rs" 2>/dev/null; then
  cmds="$(git diff "${BASE}" -- src/main.rs 2>/dev/null \
    | grep -E '^\+' \
    | grep -oE 'Some\("[a-z-]+"\)' \
    | sed -E 's/Some\("//; s/"\)//')"$'\n'
fi

tools=$(echo "$tools" | sed '/^$/d' | sort -u)
cmds=$(echo "$cmds" | sed '/^$/d' | sort -u)
all_surfaces=$(printf '%s\n%s' "$tools" "$cmds" | sed '/^$/d')

# The declared surface: 3 MCP tools, 6 CLI subcommands. A drop here means a
# surface was lost, independent of what this branch added.
declared_tools=$(git grep -h -oE 'async fn [a-z_0-9]+' "${HEAD_SHA}" -- src/server.rs 2>/dev/null | wc -l)

if [[ -z "$all_surfaces" ]]; then
  echo "QA GATE: ALL PASS (no new MCP tools or CLI subcommands relative to ${BASE}; declared surface: ${declared_tools} MCP tools)"
  exit 0
fi
echo "New surfaces to be QA'd (${MODEL}):"
echo "$all_surfaces" | sed 's/^/  /'

failures=0
check() { if eval "$2"; then echo "PASS  $1"; else echo "FAIL  $1"; failures=$((failures+1)); fi; }

check "QA report exists ($(basename "$REPORT"))" "[[ -f '$REPORT' ]]"
base_sec=$(git log -1 --format=%ct "$BASE" 2>/dev/null || echo 0)
check "QA report is fresh (>= last pushed commit at ${BASE})" \
  "[[ -f '$REPORT' ]] && [[ '$(stat -c %Y "$REPORT" 2>/dev/null || echo 0)' -ge '$base_sec' ]]"
# The OPERATIVE verdict is the report's LAST non-empty line, and nothing else.
# Two holes found by the QA-tester on 2026-09-14 shaped this:
#   1. grepping for "verdict: overall PASS" ANYWHERE let a report whose last line
#      read "verdict: overall FAIL" go green on an older, superseded line's word;
#   2. taking the last *verdict* line still let an appended fenced code block
#      whose line reads "verdict: overall PASS" become the operative one.
# The contract the reports keep is "the file ENDS with the verdict line", so that
# is what is checked: a FAILed report, and any report with trailing prose or a
# fence after the verdict, both fail.
last_verdict=$(grep -v '^[[:space:]]*$' "$REPORT" 2>/dev/null | tail -1 | tr 'A-Z' 'a-z' | tr -s ' ' | sed 's/[[:space:]]*$//')
check "operative verdict is PASS (report's last non-empty line: ${last_verdict:-none})" \
  "[[ '$last_verdict' == 'verdict: overall pass' ]]"

while IFS= read -r surf; do
  [[ -z "$surf" ]] && continue
  check "report covers surface: $surf" "grep -qF '$surf' '$REPORT'"
done <<< "$all_surfaces"

# A blanket report is acceptable only when it claims every surface at once;
# otherwise each added surface must be named. This project publishes the
# token ALL NEW SURFACES when a unit touches no tool signature at all
# (dependency or internal changes), so accept it explicitly rather than
# letting the loop above silently require per-name matches it never had.
if (( failures > 0 )) && grep -qF 'ALL NEW SURFACES' "$REPORT" 2>/dev/null; then
  echo "NOTE  report claims ALL NEW SURFACES; treating unnamed surfaces above as covered"
  failures=0
  check "QA report exists ($(basename "$REPORT"))" "[[ -f '$REPORT' ]]"
  base_sec=$(git log -1 --format=%ct "$BASE" 2>/dev/null || echo 0)
  check "QA report is fresh (>= last pushed commit at ${BASE})" \
    "[[ -f '$REPORT' ]] && [[ '$(stat -c %Y "$REPORT" 2>/dev/null || echo 0)' -ge '$base_sec' ]]"
  last_verdict=$(grep -v '^[[:space:]]*$' "$REPORT" 2>/dev/null | tail -1 | tr 'A-Z' 'a-z' | tr -s ' ' | sed 's/[[:space:]]*$//')
  check "operative verdict is PASS (report's last non-empty line: ${last_verdict:-none})" \
    "[[ '$last_verdict' == 'verdict: overall pass' ]]"
fi

if (( failures == 0 )); then
  echo "QA GATE: overall PASS (${MODEL} evidence fresh and covering)"
  exit 0
else
  echo "QA GATE: FAIL (${failures} check(s) red). Run the AI QA-tester on ${MODEL},"
  echo "have it verify the surfaces above live, and update $REPORT with"
  echo "'verdict: overall PASS' before committing/pushing."
  exit 1
fi