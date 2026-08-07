#!/usr/bin/env python3
"""Smoke test for powershell-mcp: MCP handshake plus one query over stdio.

Usage:
  python3 smoke.py [POWERSHELL_MCP_HOME] [BIN]

Requires an existing index in POWERSHELL_MCP_HOME (run
`powershell-mcp build-index` first). Exits non-zero on any assertion
failure.
"""

import json
import os
import subprocess
import sys

def main() -> None:
    home = sys.argv[1] if len(sys.argv) > 1 else os.environ.get("POWERSHELL_MCP_HOME", ".rag-index")
    bin_path = sys.argv[2] if len(sys.argv) > 2 else os.environ.get("POWERSHELL_MCP_BIN", "powershell-mcp")

    env = dict(os.environ)
    env["POWERSHELL_MCP_HOME"] = home
    proc = subprocess.Popen(
        [bin_path],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        env=env,
    )
    assert proc.stdin is not None and proc.stdout is not None

    next_id = 0

    def send(obj: dict) -> None:
        proc.stdin.write(json.dumps(obj) + "\n")
        proc.stdin.flush()

    def wait_for(rid: int) -> dict:
        while True:
            line = proc.stdout.readline()
            if not line:
                raise RuntimeError("server closed stdout while waiting for response")
            frame = json.loads(line)
            if frame.get("id") == rid:
                return frame

    def request(method: str, params: dict) -> dict:
        nonlocal next_id
        next_id += 1
        send({"jsonrpc": "2.0", "id": next_id, "method": method, "params": params})
        return wait_for(next_id)

    def call_tool(name: str, args: dict) -> dict:
        frame = request("tools/call", {"name": name, "arguments": args})
        if "error" in frame:
            raise RuntimeError(f"tools/call {name} failed: {frame['error']}")
        result = frame["result"]
        text = "".join(
            item.get("text", "")
            for item in result.get("content", [])
            if item.get("type") == "text"
        )
        return json.loads(text)

    init = request(
        "initialize",
        {
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "smoke", "version": "0.0.1"},
        },
    )
    info = init["result"]["serverInfo"]
    assert info["name"] == "powershell-mcp", info

    tools = request("tools/list", {})["result"]["tools"]
    names = {t["name"] for t in tools}
    assert {"search", "file_context", "index_info"} <= names, names

    info = call_tool("index_info", {})
    assert info["chunk_count"] > 0, info

    hits = call_tool("search", {"query": "Get-Command parameter", "mode": "hybrid", "k": 3})
    assert isinstance(hits, list), hits
    print(json.dumps(hits, indent=2)[:2000])

    proc.kill()
    proc.wait()
    print("smoke OK: handshake, tools, index_info, and search all answered")


if __name__ == "__main__":
    main()
