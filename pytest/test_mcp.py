"""Live MCP result-quality tests: the server must return usable, citable
results for representative PowerShell questions. These drive the real
binary over stdio (JSON-RPC), exactly like a client would.
"""

from __future__ import annotations

from conftest import requires_index


@requires_index
def test_index_info_reports_index(mcp) -> None:
    info = mcp.call_tool("index_info", {})
    assert info.get("model") == "bge-small-en-v1.5"
    assert (info.get("chunk_count") or 0) > 0
    assert (info.get("file_count") or 0) > 0
    assert info.get("dim") == 384
    assert "PowerShell-Docs" in (info.get("project_root") or "")
    assert info.get("source_fingerprint")


@requires_index
def test_search_hybrid_returns_provenance(mcp) -> None:
    result = mcp.call_tool(
        "search",
        {"query": "how does splatting work with hashtables", "mode": "hybrid", "k": 3},
    )
    assert isinstance(result, list) and result, "hybrid search returned no hits"
    hit = result[0]
    assert hit.get("file"), "hit must carry a file path"
    assert hit.get("line_start") is not None
    assert hit.get("line_end") is not None
    assert len(hit.get("text", "")) > 30, "hit must carry substantial text"
    assert hit.get("score") is not None
    assert hit.get("bm25_score") is not None, "hybrid must report the bm25 arm"
    assert hit.get("vector_score") is not None, "hybrid must report the vector arm"


@requires_index
def test_search_hybrid_surfaces_hashtable_doc(mcp) -> None:
    result = mcp.call_tool(
        "search",
        {"query": "how does splatting work with hashtables", "mode": "hybrid", "k": 5},
    )
    files = [h.get("file", "") for h in result]
    assert any("hashtable" in f.lower() for f in files), (
        "splatting question must surface the hashtable deep dive, got {files}"
    )


@requires_index
def test_search_bm25_hyphenated_cmdlet(mcp) -> None:
    # Hyphenated cmdlet names must not error in bm25 mode (positions are
    # indexed) and must surface the cmdlet's own reference page.
    result = mcp.call_tool(
        "search",
        {"query": "Get-Process", "mode": "bm25", "k": 8},
    )
    assert isinstance(result, list) and result, "bm25 search returned no hits"
    for hit in result:
        assert hit.get("bm25_score") is not None
        assert hit.get("vector_score") is None, "bm25 must not report a vector arm"
    files = [h.get("file", "") for h in result]
    assert any("Get-Process.md" in f for f in files), (
        "bm25 Get-Process must surface its reference page, got {files}"
    )


@requires_index
def test_search_vector_has_vector_scores_only(mcp) -> None:
    result = mcp.call_tool(
        "search",
        {"query": "pass objects from one command to the next", "mode": "vector", "k": 3},
    )
    assert isinstance(result, list) and result, "vector search returned no hits"
    assert result[0].get("vector_score") is not None
    assert result[0].get("bm25_score") is None, "vector mode must not report a bm25 arm"


@requires_index
def test_file_context_returns_every_chunk_of_one_file(mcp) -> None:
    known = "reference/7.4/Microsoft.PowerShell.Core/Get-Command.md"
    chunks = mcp.call_tool("file_context", {"file": known})
    assert isinstance(chunks, list) and chunks, f"file_context returned nothing for {known}"
    assert all(c.get("file") == known for c in chunks)
    # Chunks are in source order with non-overlapping ranges.
    for prev, cur in zip(chunks, chunks[1:]):
        assert prev["line_end"] <= cur["line_start"]
    assert chunks[0]["line_start"] == 1


@requires_index
def test_file_context_roundtrip_from_search(mcp) -> None:
    hits = mcp.call_tool(
        "search",
        {"query": "automatic variable for the last item in a pipeline", "mode": "hybrid", "k": 2},
    )
    assert hits, "search returned no hits to round-trip"
    file = hits[0]["file"]
    chunks = mcp.call_tool("file_context", {"file": file})
    assert chunks, f"file_context returned nothing for the search hit {file}"
    # The hit's line range must fall inside one of the file's chunks.
    assert any(
        c["line_start"] <= hits[0]["line_start"] <= c["line_end"] for c in chunks
    ), "search line provenance must map into file_context chunks"
