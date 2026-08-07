"""Positive and negative trigger cases.

Positive cases are realistic prompts a user or agent would type that
should activate the powershell-mcp skill. Each carries the expected
trigger phrases (informational; the test only asserts that at least one
trigger matches).

Negative cases are unrelated prompts that must NOT activate the skill.
They are the over-trigger guard: a generic word in when-to-use that also
appears here is a precision leak.

Deliberate trade-offs (kept triggers with rare non-PowerShell uses,
excluded from the negative set): script and pipeline are intentionally
absent from when-to-use so generic scripting and CI talk does not fire.

When tuning when-to-use, add a positive case for any phrasing you want
to guarantee and a negative case for any context that must not fire.
"""

from __future__ import annotations

POSITIVE_CASES: list[tuple[str, list[str]]] = [
    # Cmdlet and syntax phrasing
    ("how do I use Get-Process to list running processes in PowerShell?", ["powershell"]),
    ("what is the syntax of ForEach-Object in PowerShell?", ["powershell"]),
    ("powershell Get-Command to find cmdlets by parameter", ["powershell", "cmdlet"]),
    ("how do I create a PowerShell module?", ["powershell module", "powershell"]),
    ("difference between Write-Host and Write-Output in PowerShell", ["powershell"]),
    ("explain about_CommonParameters in PowerShell", ["powershell"]),
    # PowerShell-specific concepts
    ("how does splatting work with hashtables in PowerShell?", ["powershell", "splatting"]),
    ("what does $_ mean inside a PowerShell pipeline?", ["powershell"]),
    ("powerShell version 7.4 behavior changes", ["powershell version", "powershell"]),
    ("cmdlet reference for Get-Item", ["cmdlet reference", "cmdlet"]),
    ("which cmdlets can I use to manage services?", ["cmdlets", "cmdlet"]),
    # Slash command and server name phrasing
    ("/powershell-mcp about_Arrays", ["powershell mcp"]),
    ("PowerShell MCP: what is the automatic pipeline variable?", ["powershell mcp", "powershell"]),
    ("how do I sign a .ps1 script?", ["ps1", "powershell"]),
    ("powershell scripting best practices for error handling", ["powershell scripting", "powershell"]),
]

NEGATIVE_CASES: list[str] = [
    "refactor this rust function to reduce complexity",
    "what does this python code do",
    "how do I set up a new MCP server?",
    "deploy to production",
    "review this pull request",
    "write a unit test for this function",
    "set up a CI pipeline for this repo",
    "the power shell of this machine is failing to boot",
    "debug this bash script for backups",
    "what's the weather today",
    "explain this API design to me",
    "the script is failing during deployment",
]
