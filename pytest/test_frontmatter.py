"""Frontmatter integrity: the skill must parse cleanly and carry the
fields open-grok needs (name, description, when-to-use, user-invocable)."""

from __future__ import annotations

import re


def test_name_is_present_and_valid(frontmatter: dict) -> None:
    name = frontmatter.get("name")
    assert name, "missing name"
    assert re.fullmatch(r"[a-z0-9-]{1,64}", str(name)), f"invalid skill name: {name}"
    assert str(name) == "powershell-mcp"


def test_description_is_present_and_substantial(frontmatter: dict) -> None:
    description = str(frontmatter.get("description", "")).strip()
    assert len(description) > 80, "description too short to guide invocation"
    assert "PowerShell" in description, "description should mention PowerShell"
    assert "cmdlet" in description, "description should mention cmdlet reference"


def test_when_to_use_is_present(frontmatter: dict) -> None:
    value = frontmatter.get("when-to-use") or frontmatter.get("when_to_use")
    assert value, "when-to-use missing — the model has no trigger guidance"


def test_user_invocable_not_disabled(frontmatter: dict) -> None:
    assert frontmatter.get("user-invocable", True) is not False, (
        "user-invocable must stay true so /powershell-mcp works"
    )


def test_model_invocation_not_disabled(frontmatter: dict) -> None:
    assert frontmatter.get("disable-model-invocation", False) is False, (
        "disable-model-invocation must stay false for automatic triggering"
    )


def test_argument_hint_present(frontmatter: dict) -> None:
    assert frontmatter.get("argument-hint"), "missing argument-hint for the slash command"


def test_no_emojis_in_frontmatter(skill_text: str) -> None:
    assert not re.search(r"[\U0001F300-\U0001FAFF\u2600-\u27BF]", skill_text), (
        "emoji in the skill file"
    )
