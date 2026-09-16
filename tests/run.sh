#!/usr/bin/env bash
# tests/run.sh — the project's full local verification entry point.
#
# Runs the Rust suite, the pytest quality gates, and the AI QA-tester gate
# (global rule 19). Exits non-zero if any stage fails.
set -uo pipefail
cd "$(dirname "$0")/.." || exit 1

failures=0
stage() { echo; echo "== $1 =="; }
report() { if [[ $1 -eq 0 ]]; then echo "   OK"; else echo "   FAILED (exit $1)"; failures=$((failures+1)); fi; }

stage "cargo test (unit + integration)"
cargo test 2>&1 | tail -20
report "${PIPESTATUS[0]}"

stage "pytest (skill quality + live results)"
if python3 -m pytest pytest/ -q 2>&1 | tail -5; then :; fi
report "${PIPESTATUS[0]}"

stage "AI QA-tester gate"
# The gate diffs the new surfaces of this branch against the integration
# branch (dev). QA_BASE is set explicitly: the template's default
# (origin/<current branch>) cannot work here, because a work branch has no
# origin ref of its own until it is pushed, and the gate would then see an
# empty base and pass trivially.
if git rev-parse --verify --quiet dev >/dev/null; then
  QA_BASE=dev bash tests/qa-gate.sh
else
  QA_BASE=origin/main bash tests/qa-gate.sh
fi
report $?

echo
if [[ $failures -eq 0 ]]; then
  echo "tests/run.sh: ALL GREEN"
  exit 0
fi
echo "tests/run.sh: $failures stage(s) red"
exit 1
