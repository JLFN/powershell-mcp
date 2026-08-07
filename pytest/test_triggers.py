"""Trigger quality: recall over PowerShell prompts, precision over
unrelated ones. The goal is a when-to-use list that covers every
realistic PowerShell phrasing (recall) while containing no generic word
that leaks into unrelated conversations (precision)."""

from __future__ import annotations

import pytest

from cases import NEGATIVE_CASES, POSITIVE_CASES
from matcher import matching_phrases, singularize, trigger_phrases


@pytest.mark.parametrize(
    "prompt", [prompt for prompt, _ in POSITIVE_CASES], ids=range(len(POSITIVE_CASES))
)
def test_powershell_prompt_triggers(prompt: str, phrases: list[str]) -> None:
    hits = matching_phrases(prompt, phrases)
    assert hits, f"no when-to-use phrase matched a PowerShell prompt: {prompt!r}"


@pytest.mark.parametrize("prompt", NEGATIVE_CASES, ids=range(len(NEGATIVE_CASES)))
def test_unrelated_prompt_does_not_trigger(prompt: str, phrases: list[str]) -> None:
    hits = matching_phrases(prompt, phrases)
    assert not hits, f"over-trigger: {prompt!r} matched {hits}"


@pytest.mark.parametrize(
    "prompt,expected", POSITIVE_CASES, ids=range(len(POSITIVE_CASES))
)
def test_expected_trigger_phrases_are_documented(
    prompt: str, expected: list[str], phrases: list[str]
) -> None:
    # The expected list documents intent; every listed phrase must be
    # covered by a trigger (singular-folded, so "cmdlets" is covered by
    # the trigger "cmdlet"). Protects against typos when the list is
    # edited.
    for phrase in expected:
        folded = singularize(phrase)
        assert any(
            singularize(p) == folded for p in phrases
        ), f"case expects trigger {phrase!r} but it is not in when-to-use"


def test_trigger_list_has_no_duplicates_or_empty_entries(when_to_use: str) -> None:
    raw = [p.strip() for p in when_to_use.split(",")]
    assert all(raw), "empty entry in when-to-use (double comma?)"
    normalized = trigger_phrases(when_to_use)
    assert len(normalized) == len(set(normalized)), (
        "duplicate trigger phrases: "
        f"{[p for p in normalized if normalized.count(p) > 1]}"
    )
