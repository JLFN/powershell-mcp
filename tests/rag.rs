//! RAG integration test: build a small index with synthetic embeddings and
//! run hybrid/BM25/vector queries and file-context lookups through the real
//! scan + chunk + storage pipeline. The real embedding model is exercised
//! by the live MCP test.

use std::path::Path;

use powershell_mcp::rag::{self, QueryMode};

/// Fake embedder: deterministic 8-dim vectors derived from the first bytes
/// of each text, so identical texts embed identically.
fn fake_embed(texts: &[String]) -> anyhow::Result<Vec<Vec<f32>>> {
    Ok(texts
        .iter()
        .map(|t| {
            let mut v = vec![0.0f32; 8];
            for (i, b) in t.bytes().take(8).enumerate() {
                v[i] = b as f32 / 255.0;
            }
            v
        })
        .collect())
}

/// A tiny PowerShell-Docs-shaped fixture: reference cmdlet pages, an about
/// topic, a conceptual guide, plus noise that the scanner must ignore.
fn fixture_corpus(dir: &Path) {
    std::fs::create_dir_all(dir.join("reference/7.4/Microsoft.PowerShell.Core/About")).unwrap();
    std::fs::create_dir_all(dir.join("reference/docs-conceptual/learn")).unwrap();
    std::fs::create_dir_all(dir.join("tests")).unwrap();
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    std::fs::write(
        dir.join("reference/7.4/Microsoft.PowerShell.Core/Get-Command.md"),
        "---\ntitle: Get-Command\nModule Name: Microsoft.PowerShell.Core\n---\n\n# Get-Command\n\nGets all commands. It mentions zebra crossings so BM25 has a distinctive token.\n\n## SYNTAX\n\n```\nGet-Command [-Verb <String[]>] [<CommonParameters>]\n```\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("reference/7.4/Microsoft.PowerShell.Core/About/about_Arrays.md"),
        "# about_Arrays\n\nDescribes arrays, which store a collection of items. The word personuppgifter appears only here.\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("reference/docs-conceptual/learn/learn-powershell.md"),
        "# Learn PowerShell\n\nA tutorial for getting started with the PowerShell language.\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("README.md"),
        "# PowerShell Docs\n\nA fixture corpus for the powershell-mcp tests.\n",
    )
    .unwrap();
    // Noise the scanner must skip.
    std::fs::write(dir.join("CODE_OF_CONDUCT.md"), "conduct\n").unwrap();
    std::fs::write(dir.join("Cargo.lock"), "lockfile\n".repeat(50)).unwrap();
    std::fs::write(dir.join("tests/unit.md"), "test docs\n").unwrap();
    std::fs::write(dir.join(".git/config"), "git\n").unwrap();
    std::fs::write(dir.join("notes.txt"), "not markdown\n").unwrap();
}

#[test]
fn build_and_query_small_index() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let corpus = tmp.path().join("corpus");
    fixture_corpus(&corpus);
    let root = tmp.path().join("index");

    let report = rag::build_index(&corpus, &root, &fake_embed).unwrap();
    assert_eq!(report.model, powershell_mcp::embed::current_model().id());
    assert_eq!(report.dim, 8);
    assert!(
        report.chunks >= 4,
        "expected at least 4 chunks, got {}",
        report.chunks
    );
    assert_eq!(
        report.files, 4,
        "lockfile, tests, .git, non-md files, and contributor docs must be skipped"
    );

    // Manifest + chunks + embeddings readable.
    let loaded = rag::load(&root).unwrap();
    assert_eq!(loaded.manifest.dim, 8);
    assert_eq!(loaded.manifest.chunk_count, report.chunks);
    assert_eq!(loaded.manifest.file_count, 4);
    assert_eq!(
        loaded.manifest.model,
        powershell_mcp::embed::current_model().id(),
        "manifest records the embedding model"
    );
    rag::ensure_model_matches(&loaded).unwrap();
    assert_eq!(
        loaded.chunks[0].file, "README.md",
        "sorted scan order puts README first"
    );

    // BM25 query: the fixture mentions a distinctive token.
    let hits = rag::retrieve(&loaded, &root, "personuppgifter", QueryMode::Bm25, 5, None).unwrap();
    assert!(!hits.is_empty());
    assert!(hits[0].bm25_score.is_some());
    assert!(hits[0].vector_score.is_none());

    // BM25 multi-word query with a hyphenated cmdlet name: requires
    // positions on the body field, a regression guard for the tantivy
    // schema (cmdlet names like Get-Command must be searchable).
    let hits = rag::retrieve(
        &loaded,
        &root,
        "Get-Command Verb syntax",
        QueryMode::Bm25,
        5,
        None,
    )
    .unwrap();
    assert!(!hits.is_empty(), "hyphenated multi-word bm25 query must return hits");
    assert!(hits[0].chunk.text.contains("Get-Command"));

    // Vector query with the embedding of a text present in the corpus.
    let q = fake_embed(&["Gets all commands".to_string()]).unwrap();
    let hits = rag::retrieve(&loaded, &root, "x", QueryMode::Vector, 3, Some(&q[0])).unwrap();
    assert!(!hits.is_empty());
    assert!(hits[0].vector_score.is_some());
    assert!(hits[0].bm25_score.is_none());

    // Hybrid fuses both arms and reports both scores.
    let hits = rag::retrieve(
        &loaded,
        &root,
        "personuppgifter",
        QueryMode::Hybrid,
        5,
        Some(&q[0]),
    )
    .unwrap();
    assert!(!hits.is_empty());

    // Scored chunk renders the tool's JSON shape with provenance.
    let json = rag::scored_chunk_to_json(&hits[0]);
    assert!(json.get("file").is_some());
    assert!(json.get("line_start").is_some());
    assert!(json.get("line_end").is_some());
    assert!(json.get("text").is_some());
    assert!(json.get("score").is_some());

    // file_context returns every chunk of one file in source order.
    let ctx = rag::file_context(&loaded, "reference/7.4/Microsoft.PowerShell.Core/Get-Command.md");
    assert!(!ctx.is_empty());
    assert_eq!(ctx[0].line_start, 1);
    assert!(ctx
        .iter()
        .all(|c| c.file == "reference/7.4/Microsoft.PowerShell.Core/Get-Command.md"));

    // The full search entry point works too (BM25 needs no embedder).
    let hits = rag::search(&root, "zebra", QueryMode::Bm25, 3).unwrap();
    assert!(!hits.is_empty());
    assert!(hits[0].chunk.text.contains("zebra"));

    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn missing_index_is_a_clear_error() {
    let root = std::env::temp_dir().join(format!("rag-missing-{}", std::process::id()));
    let err = rag::read_manifest(&root).unwrap_err();
    assert!(err.to_string().contains("build-index"), "{err}");
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn stale_model_manifest_is_rejected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let corpus = tmp.path().join("corpus");
    fixture_corpus(&corpus);
    let root = tmp.path().join("index");

    rag::build_index(&corpus, &root, &fake_embed).unwrap();

    // Rewrite the manifest to claim a different model, as an old index
    // built with another embedder would.
    let manifest_path = root.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&manifest_path).unwrap()).unwrap();
    manifest["model"] = serde_json::Value::String("multilingual-e5-small".into());
    std::fs::write(
        &manifest_path,
        serde_json::to_string_pretty(&manifest).unwrap(),
    )
    .unwrap();

    let loaded = rag::load(&root).unwrap();
    let err = rag::ensure_model_matches(&loaded).unwrap_err();
    assert!(
        err.to_string().contains("build-index"),
        "stale index must demand a rebuild: {err}"
    );
    std::fs::remove_dir_all(&root).ok();
}

#[test]
fn build_index_fails_helpfully_without_indexable_files() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let empty = tmp.path().join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let root = tmp.path().join("index");
    let err = rag::build_index(&empty, &root, &fake_embed).unwrap_err();
    assert!(err.to_string().contains("no indexable files"), "{err}");
}
