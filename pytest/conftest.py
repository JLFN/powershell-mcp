"""Shared fixtures for the powershell-mcp skill and server tests.

Points POWERSHELL_SKILL_PATH at the SKILL.md to test (defaults to the repo
copy). The installed copy can be tested with:

    POWERSHELL_SKILL_PATH=~/.opengrok/skills/powershell-mcp/SKILL.md

The live MCP client spawns the repo's release binary
(bin/powershell-mcp, overridable with POWERSHELL_MCP_BIN) with
POWERSHELL_MCP_HOME pointing at the built index (default <repo>/index).
"""

from __future__ import annotations

import json
import os
import pathlib
import queue
import subprocess
import threading
import time

import pytest
import yaml

REPO_ROOT = pathlib.Path(__file__).resolve().parent.parent
DEFAULT_SKILL = REPO_ROOT / "skills" / "powershell-mcp" / "SKILL.md"
SKILL_PATH = pathlib.Path(
    os.environ.get("POWERSHELL_SKILL_PATH", str(DEFAULT_SKILL))
).expanduser()

BINARY = pathlib.Path(
    os.environ.get("POWERSHELL_MCP_BIN", str(REPO_ROOT / "bin" / "powershell-mcp"))
).expanduser()
INDEX_HOME = pathlib.Path(
    os.environ.get("POWERSHELL_MCP_HOME", str(REPO_ROOT / "index"))
).expanduser()


@pytest.fixture(scope="session")
def skill_path() -> pathlib.Path:
    assert SKILL_PATH.exists(), f"SKILL.md not found at {SKILL_PATH}"
    return SKILL_PATH


@pytest.fixture(scope="session")
def skill_text(skill_path: pathlib.Path) -> str:
    return skill_path.read_text(encoding="utf-8")


@pytest.fixture(scope="session")
def frontmatter(skill_text: str) -> dict:
    if not skill_text.startswith("---"):
        pytest.fail("SKILL.md must start with YAML frontmatter delimiters")
    parts = skill_text.split("---", 2)
    if len(parts) < 3:
        pytest.fail("malformed frontmatter: missing closing ---")
    meta = yaml.safe_load(parts[1])
    if not isinstance(meta, dict):
        pytest.fail("frontmatter must parse to a mapping")
    return meta


@pytest.fixture(scope="session")
def body(skill_text: str) -> str:
    parts = skill_text.split("---", 2)
    return parts[2] if len(parts) >= 3 else ""


@pytest.fixture(scope="session")
def when_to_use(frontmatter: dict) -> str:
    value = frontmatter.get("when-to-use") or frontmatter.get("when_to_use")
    if not value:
        pytest.fail("when-to-use frontmatter field is missing")
    return str(value)


@pytest.fixture(scope="session")
def phrases(when_to_use: str) -> list[str]:
    from matcher import trigger_phrases

    return trigger_phrases(when_to_use)


@pytest.fixture(scope="session")
def description(frontmatter: dict) -> str:
    value = frontmatter.get("description")
    if not value:
        pytest.fail("description frontmatter field is missing")
    return str(value)


# ---------------------------------------------------------------------------
# Live MCP client over stdio (JSON-RPC), for result-quality tests
# ---------------------------------------------------------------------------


class McpClient:
    """Minimal MCP client speaking newline-delimited JSON over stdio."""

    def __init__(self, binary: pathlib.Path, home: pathlib.Path) -> None:
        env = dict(os.environ)
        env["POWERSHELL_MCP_HOME"] = str(home)
        self.proc = subprocess.Popen(
            [str(binary)],
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            bufsize=1,
            env=env,
        )
        self.q: "queue.Queue[str]" = queue.Queue()
        self.next_id = 1
        threading.Thread(target=self._reader, daemon=True).start()

    def _reader(self) -> None:
        for line in self.proc.stdout:
            self.q.put(line)

    def _request(self, method: str, params: dict, timeout: float = 240.0) -> dict:
        ident = self.next_id
        self.next_id += 1
        self.proc.stdin.write(
            json.dumps({"jsonrpc": "2.0", "id": ident, "method": method, "params": params})
            + "\n"
        )
        self.proc.stdin.flush()
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            frame = json.loads(self.q.get(timeout=timeout))
            if frame.get("id") == ident:
                return frame
        raise TimeoutError(f"no response for {method}")

    def initialize(self) -> None:
        self._request(
            "initialize",
            {
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": {"name": "pytest", "version": "0"},
            },
            timeout=30,
        )
        self._request("notifications/initialized", {}, timeout=30)

    def call_tool(self, name: str, arguments: dict, timeout: float = 240.0) -> dict:
        frame = self._request("tools/call", {"name": name, "arguments": arguments}, timeout)
        result = frame.get("result", {})
        if result.get("isError"):
            pytest.fail(f"tool {name} error: {result}")
        text = "".join(c.get("text", "") for c in result.get("content", []))
        return json.loads(text)

    def close(self) -> None:
        try:
            self.proc.stdin.close()
        finally:
            self.proc.kill()
            self.proc.wait(timeout=5)


@pytest.fixture(scope="session")
def mcp() -> "McpClient":
    assert BINARY.exists(), f"binary not found at {BINARY} (build with build/linux/build.sh)"
    client = McpClient(BINARY, INDEX_HOME)
    client.initialize()
    yield client
    client.close()


def _index_available() -> bool:
    if not BINARY.exists():
        return False
    client = McpClient(BINARY, INDEX_HOME)
    try:
        client.initialize()
        info = client.call_tool("index_info", {}, timeout=30)
        return bool(info.get("chunk_count", 0) > 0)
    except Exception:
        return False
    finally:
        client.close()


requires_index = pytest.mark.skipif(
    not _index_available(),
    reason="no built index; run `powershell-mcp build-index <corpus> index` first",
)
