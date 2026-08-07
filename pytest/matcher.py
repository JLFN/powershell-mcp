"""Trigger matcher mirroring open-grok's skill-invocation surface.

open-grok renders each skill to the model as:

    - <name>: <description>
      Use when: <when-to-use>
      Absolute path: <path>

and the model decides whether to invoke. This module is a deterministic
proxy for that decision: a prompt "triggers" when it contains one of the
when-to-use phrases after normalization. It is a quality gate on the
trigger list (recall = PowerShell prompts are covered, precision =
unrelated prompts are not), not a guarantee of model behavior.

Normalization matches how a model reads the listing: lowercased, hyphens
count as spaces ("powershell-mcp" reads as "powershell mcp"), punctuation
is stripped, and light singularization folds plurals ("cmdlets" matches
the trigger "cmdlet", never "symptom" matching inside "asymptomatic").
"""

from __future__ import annotations

import re

__all__ = [
    "normalize",
    "singularize",
    "trigger_phrases",
    "matching_phrases",
    "triggers",
]


def normalize(text: str) -> str:
    """Lowercase, turn runs of non-alphanumerics (incl. hyphens, slashes,
    punctuation) into single spaces, collapse whitespace."""
    lowered = text.lower()
    spaced = re.sub(r"[^a-z0-9]+", " ", lowered)
    return re.sub(r"\s+", " ", spaced).strip()


def singularize(word: str) -> str:
    """Crude singular folding: ies -> y, ses -> s, trailing s stripped
    (cmdlets -> cmdlet, scripts -> script), except double-s endings.
    Both sides of the comparison get the same treatment, so asymmetries
    cancel."""
    if len(word) > 4 and word.endswith("ies"):
        return word[:-3] + "y"
    if len(word) > 4 and word.endswith("ses"):
        return word[:-2]
    if len(word) > 3 and word.endswith("es") and word[-3] in "sxz":
        return word[:-2]
    if len(word) > 3 and word.endswith("s") and not word.endswith("ss"):
        return word[:-1]
    return word


def trigger_phrases(when_to_use: str) -> list[str]:
    """Split the when-to-use frontmatter value into normalized phrases."""
    phrases: list[str] = []
    for raw in when_to_use.split(","):
        raw = raw.strip()
        if not raw:
            continue
        normalized = normalize(raw)
        if normalized:
            phrases.append(normalized)
    return phrases


def matching_phrases(prompt: str, phrases: list[str]) -> list[str]:
    """Which trigger phrases appear in the prompt. Single-word phrases match
    on singularized whole words (so 'shell' never matches inside
    'powershell', but 'cmdlets' matches 'cmdlet'). Multi-word phrases match
    with word boundaries and tolerate up to two interstitial words between
    phrase words ('powershell 7.4 scripting' still matches 'powershell
    scripting')."""
    normalized_prompt = normalize(prompt)
    prompt_singular = {singularize(w) for w in normalized_prompt.split()}
    hits: list[str] = []
    for phrase in phrases:
        words = phrase.split()
        if len(words) == 1:
            if singularize(words[0]) in prompt_singular:
                hits.append(phrase)
            continue
        # word (filler{0,2} word)*  -- whole-phrase, boundary-anchored.
        pattern = r"\b" + r"\s+(?:\S+\s+){0,2}".join(re.escape(w) for w in words) + r"\b"
        if re.search(pattern, normalized_prompt):
            hits.append(phrase)
    return hits


def triggers(prompt: str, phrases: list[str]) -> bool:
    return bool(matching_phrases(prompt, phrases))
