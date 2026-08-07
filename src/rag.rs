//! Hybrid vector + BM25 store for the PowerShell documentation corpus.
//!
//! The index is built offline (the build-index CLI) from a checkout of the
//! MicrosoftDocs/PowerShell-Docs repository and lives under a RAG data
//! directory (default <corpus>/.rag-index, overridable with
//! POWERSHELL_MCP_HOME). Storage is three flat files plus the tantivy
//! directory: manifest.json, chunks.jsonl, and a raw little-endian f32
//! embeddings blob. Hybrid retrieval fuses BM25 and cosine-similarity
//! rankings with reciprocal rank fusion.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::bm25;
use crate::chunk::{chunk_file, Chunk};
use crate::scan::{read_text, scan_project, ScannedFile};

pub const DEFAULT_K: usize = 5;
pub const MAX_K: usize = 20;

/// RAG data directory env override, used by the server and CLI query.
pub const HOME_ENV: &str = "POWERSHELL_MCP_HOME";

/// Query mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum QueryMode {
    Hybrid,
    Bm25,
    Vector,
}

impl QueryMode {
    /// "vector" | "semantic" -> Vector; "bm25" | "keyword" | "lexical" ->
    /// Bm25; anything else (including unset) -> Hybrid.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "vector" | "semantic" => QueryMode::Vector,
            "bm25" | "keyword" | "lexical" => QueryMode::Bm25,
            _ => QueryMode::Hybrid,
        }
    }
}

/// A chunk as persisted in chunks.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkMeta {
    /// Global index within the corpus.
    pub i: usize,
    /// Path relative to the project root.
    pub file: String,
    /// 1-based inclusive first source line.
    pub line_start: usize,
    /// 1-based inclusive last source line.
    pub line_end: usize,
    pub text: String,
}

/// Index manifest.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RagManifest {
    pub version: u32,
    /// Embedding model identifier; guards against serving stale vectors.
    pub model: String,
    pub dim: usize,
    pub chunk_count: usize,
    /// Files that produced at least one chunk.
    pub file_count: usize,
    pub built_at: String,
    /// Canonicalized project root the index was built from.
    pub project_root: String,
    /// SHA-256 over sorted "path<TAB>size" lines, so a changed project is
    /// detectable.
    pub source_fingerprint: String,
}

/// Summary of one build-index run.
#[derive(Debug, Clone)]
pub struct BuildReport {
    pub chunks: usize,
    pub files: usize,
    pub dim: usize,
    pub model: String,
}

/// Embedder callback injected for testability; the CLI passes the real
/// fastembed embedder.
pub type EmbedFn<'a> = &'a dyn Fn(&[String]) -> anyhow::Result<Vec<Vec<f32>>>;

/// Default RAG data directory for a project: <project>/.rag-index.
pub fn default_rag_dir(project_root: &Path) -> PathBuf {
    project_root.join(".rag-index")
}

/// Resolve the active RAG data directory for query/serve: $POWERSHELL_MCP_HOME
/// when set, else <cwd>/.rag-index.
pub fn rag_dir() -> PathBuf {
    if let Ok(p) = std::env::var(HOME_ENV) {
        if !p.trim().is_empty() {
            return PathBuf::from(p);
        }
    }
    std::env::current_dir()
        .map(|c| c.join(".rag-index"))
        .unwrap_or_else(|_| PathBuf::from(".rag-index"))
}

fn manifest_path(root: &Path) -> PathBuf {
    root.join("manifest.json")
}

fn chunks_path(root: &Path) -> PathBuf {
    root.join("chunks.jsonl")
}

fn embeddings_path(root: &Path) -> PathBuf {
    root.join("embeddings.bin")
}

pub fn tantivy_dir(root: &Path) -> PathBuf {
    root.join("tantivy")
}

// ---------------------------------------------------------------------------
// Index build
// ---------------------------------------------------------------------------

/// Build the hybrid index for a project directory into `root`. Scans the
/// project, chunks every indexable text file, embeds the chunks, and writes
/// the manifest, chunks, embedding blob, and tantivy index.
pub fn build_index(project_root: &Path, root: &Path, embed: EmbedFn) -> Result<BuildReport> {
    std::fs::create_dir_all(root).with_context(|| format!("create rag dir {}", root.display()))?;

    let files = scan_project(project_root).context("scan project")?;
    if files.is_empty() {
        anyhow::bail!(
            "no indexable files under {} (check DEFAULT_EXTS / EXCLUDED_DIRS in src/scan.rs)",
            project_root.display()
        );
    }

    let mut chunks: Vec<Chunk> = Vec::new();
    let mut indexed_files = 0usize;
    for f in &files {
        let full = project_root.join(&f.rel_path);
        let Some(text) = read_text(&full) else {
            continue;
        };
        let file_chunks = chunk_file(chunks.len(), &f.rel_path, &text);
        if !file_chunks.is_empty() {
            indexed_files += 1;
        }
        chunks.extend(file_chunks);
    }
    if chunks.is_empty() {
        anyhow::bail!("no chunks produced (all files empty, unreadable, or binary)");
    }

    let model = crate::embed::current_model();
    let prefix = if model.uses_prefixes() {
        crate::embed::PASSAGE_PREFIX
    } else {
        ""
    };
    let texts: Vec<String> = chunks
        .iter()
        .map(|c| format!("{prefix}{}", c.text))
        .collect();
    eprintln!("[rag] embedding {} chunks...", texts.len());
    let vectors = embed(&texts).context("embed chunks")?;
    let dim = vectors.first().map(|v| v.len()).unwrap_or(0);
    if dim == 0 || vectors.len() != chunks.len() {
        anyhow::bail!(
            "embedding dimension/count mismatch: {} vectors, dim {}",
            vectors.len(),
            dim
        );
    }

    let manifest = RagManifest {
        version: 1,
        model: model.id().to_string(),
        dim,
        chunk_count: chunks.len(),
        file_count: indexed_files,
        built_at: chrono::Utc::now().to_rfc3339(),
        project_root: project_root
            .canonicalize()
            .unwrap_or_else(|_| project_root.to_path_buf())
            .display()
            .to_string(),
        source_fingerprint: fingerprint_files(&files),
    };

    serde_json::to_writer_pretty(
        std::io::BufWriter::new(std::fs::File::create(manifest_path(root))?),
        &manifest,
    )
    .context("write manifest")?;

    let mut chunks_out = String::new();
    for c in &chunks {
        let meta = ChunkMeta {
            i: c.index,
            file: c.file.clone(),
            line_start: c.line_start,
            line_end: c.line_end,
            text: c.text.clone(),
        };
        chunks_out.push_str(&serde_json::to_string(&meta).context("serialize chunk")?);
        chunks_out.push('\n');
    }
    std::fs::write(chunks_path(root), chunks_out).context("write chunks.jsonl")?;

    let mut blob: Vec<u8> = Vec::with_capacity(vectors.len() * dim * 4);
    for v in &vectors {
        for f in v {
            blob.extend_from_slice(&f.to_le_bytes());
        }
    }
    std::fs::write(embeddings_path(root), blob).context("write embeddings.bin")?;

    bm25::index_chunks(&tantivy_dir(root), &chunks).context("build tantivy index")?;

    Ok(BuildReport {
        chunks: chunks.len(),
        files: indexed_files,
        dim,
        model: manifest.model,
    })
}

/// First 12 hex chars of the SHA-256 over sorted "path<TAB>size" lines.
pub fn fingerprint_files(files: &[ScannedFile]) -> String {
    let mut digest = Sha256::new();
    for f in files {
        digest.update(f.rel_path.as_bytes());
        digest.update(b"\t");
        digest.update(f.bytes.to_le_bytes());
        digest.update(b"\n");
    }
    let out = digest.finalize();
    out[..6].iter().map(|b| format!("{b:02x}")).collect()
}

// ---------------------------------------------------------------------------
// Load and query
// ---------------------------------------------------------------------------

/// Read the manifest, erroring with a helpful message when the index is
/// missing (i.e. build-index has not been run).
pub fn read_manifest(root: &Path) -> Result<RagManifest> {
    let path = manifest_path(root);
    let text = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "no semantic index at {} (run `powershell-mcp build-index <corpus-root>` first)",
            root.display()
        )
    })?;
    serde_json::from_str(&text).context("parse rag manifest")
}

fn load_chunks(root: &Path) -> Result<Vec<ChunkMeta>> {
    let path = chunks_path(root);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut out = Vec::new();
    for line in text.lines() {
        if line.trim().is_empty() {
            continue;
        }
        out.push(serde_json::from_str(line).context("parse chunk record")?);
    }
    Ok(out)
}

fn load_embeddings(root: &Path, count: usize, dim: usize) -> Result<Vec<Vec<f32>>> {
    let path = embeddings_path(root);
    let bytes = std::fs::read(&path).with_context(|| format!("read {}", path.display()))?;
    let expected = count * dim * 4;
    if bytes.len() != expected {
        anyhow::bail!(
            "embedding blob size mismatch: expected {} bytes, found {}",
            expected,
            bytes.len()
        );
    }
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let base = i * dim;
        let mut v = Vec::with_capacity(dim);
        for j in 0..dim {
            let offset = (base + j) * 4;
            let b = [
                bytes[offset],
                bytes[offset + 1],
                bytes[offset + 2],
                bytes[offset + 3],
            ];
            v.push(f32::from_le_bytes(b));
        }
        out.push(v);
    }
    Ok(out)
}

/// Loaded index: manifest + chunks + embeddings (heavy parts loaded lazily
/// by the server and cached).
#[derive(Debug, Clone)]
pub struct LoadedRag {
    pub manifest: RagManifest,
    pub chunks: Vec<ChunkMeta>,
    pub embeddings: Vec<Vec<f32>>,
}

pub fn load(root: &Path) -> Result<LoadedRag> {
    let manifest = read_manifest(root)?;
    let chunks = load_chunks(root)?;
    let embeddings = load_embeddings(root, manifest.chunk_count, manifest.dim)?;
    Ok(LoadedRag {
        manifest,
        chunks,
        embeddings,
    })
}

/// Error when the on-disk index was built with a different embedding model
/// than the one this binary embeds with (stale vectors must not be served).
pub fn ensure_model_matches(loaded: &LoadedRag) -> Result<()> {
    let expected = crate::embed::current_model().id();
    if loaded.manifest.model != expected {
        anyhow::bail!(
            "semantic index was built with model '{}' but this binary uses '{}'; run `powershell-mcp build-index` to rebuild",
            loaded.manifest.model,
            expected
        );
    }
    Ok(())
}

/// A scored retrieval hit.
#[derive(Debug, Clone)]
pub struct ScoredChunk {
    pub chunk: ChunkMeta,
    pub score: f32,
    pub vector_score: Option<f32>,
    pub bm25_score: Option<f32>,
}

/// Reciprocal rank fusion: score(id) = sum over lists of 1 / (k + rank).
fn reciprocal_rank_fusion(lists: &[Vec<(usize, f32)>], top_k: usize) -> Vec<(usize, f32)> {
    const K: f32 = 60.0;
    let mut scores: HashMap<usize, f32> = HashMap::new();
    for list in lists {
        for (rank, (idx, _)) in list.iter().enumerate() {
            *scores.entry(*idx).or_insert(0.0) += 1.0 / (K + rank as f32 + 1.0);
        }
    }
    let mut ranked: Vec<(usize, f32)> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    ranked.truncate(top_k);
    ranked
}

pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let an = norm(a);
    let bn = norm(b);
    if an > 0.0 && bn > 0.0 {
        dot(a, b) / (an * bn)
    } else {
        0.0
    }
}

fn norm(v: &[f32]) -> f32 {
    v.iter().map(|x| x * x).sum::<f32>().sqrt()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Run a hybrid/BM25/vector query over a loaded index. `query_embedding`
/// may be None in BM25 mode; it is required for vector and hybrid modes.
pub fn retrieve(
    loaded: &LoadedRag,
    root: &Path,
    question: &str,
    mode: QueryMode,
    k: usize,
    query_embedding: Option<&[f32]>,
) -> Result<Vec<ScoredChunk>> {
    let k = k.clamp(1, MAX_K);
    let tantivy_dir_path = tantivy_dir(root);
    let chunks = &loaded.chunks;

    match mode {
        QueryMode::Bm25 => {
            let hits = bm25::search_chunks(&tantivy_dir_path, question, k)?;
            let mut out = Vec::with_capacity(hits.len());
            for h in hits {
                if let Some(chunk) = chunks.get(h.chunk_index) {
                    out.push(ScoredChunk {
                        chunk: chunk.clone(),
                        score: h.score,
                        vector_score: None,
                        bm25_score: Some(h.score),
                    });
                }
            }
            Ok(out)
        }
        QueryMode::Vector => {
            let q = query_embedding.ok_or_else(|| anyhow::anyhow!("query embedding required"))?;
            let mut scored: Vec<(f32, usize)> = loaded
                .embeddings
                .iter()
                .enumerate()
                .map(|(idx, v)| (cosine_similarity(q, v), idx))
                .collect();
            scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            scored.truncate(k);
            let mut out = Vec::with_capacity(scored.len());
            for (sim, idx) in scored {
                if let Some(chunk) = chunks.get(idx) {
                    out.push(ScoredChunk {
                        chunk: chunk.clone(),
                        score: sim,
                        vector_score: Some(sim),
                        bm25_score: None,
                    });
                }
            }
            Ok(out)
        }
        QueryMode::Hybrid => {
            let q = query_embedding.ok_or_else(|| anyhow::anyhow!("query embedding required"))?;
            let pool = k * 4;
            let bm = bm25::search_chunks(&tantivy_dir_path, question, pool)?;
            let mut vec_scored: Vec<(f32, usize)> = loaded
                .embeddings
                .iter()
                .enumerate()
                .map(|(idx, v)| (cosine_similarity(q, v), idx))
                .collect();
            vec_scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
            vec_scored.truncate(pool);

            let bm_list: Vec<(usize, f32)> = bm.iter().map(|h| (h.chunk_index, h.score)).collect();
            let vec_list: Vec<(usize, f32)> = vec_scored.iter().map(|(s, i)| (*i, *s)).collect();
            let vmap: HashMap<usize, f32> = vec_scored.iter().map(|(s, i)| (*i, *s)).collect();
            let bmap: HashMap<usize, f32> = bm.iter().map(|h| (h.chunk_index, h.score)).collect();

            let ranked = reciprocal_rank_fusion(&[bm_list, vec_list], k);
            let mut out = Vec::with_capacity(ranked.len());
            for (idx, rrf) in ranked {
                if let Some(chunk) = chunks.get(idx) {
                    out.push(ScoredChunk {
                        chunk: chunk.clone(),
                        score: rrf,
                        vector_score: vmap.get(&idx).copied(),
                        bm25_score: bmap.get(&idx).copied(),
                    });
                }
            }
            Ok(out)
        }
    }
}

/// Full query entry point used by the CLI: loads the index, embeds the
/// question with the local model (vector/hybrid modes), and retrieves.
pub fn search(root: &Path, question: &str, mode: QueryMode, k: usize) -> Result<Vec<ScoredChunk>> {
    let loaded = load(root)?;
    let query_embedding: Option<Vec<f32>> = match mode {
        QueryMode::Bm25 => None,
        QueryMode::Vector | QueryMode::Hybrid => {
            ensure_model_matches(&loaded)?;
            let model = crate::embed::current_model();
            let prefix = if model.uses_prefixes() {
                crate::embed::QUERY_PREFIX
            } else {
                ""
            };
            let texts = vec![format!("{prefix}{question}")];
            let embedded = crate::embed::embed_texts(&texts)?;
            embedded.first().cloned()
        }
    };
    retrieve(&loaded, root, question, mode, k, query_embedding.as_deref())
}

/// Every chunk belonging to one file, in source order.
pub fn file_context<'a>(loaded: &'a LoadedRag, file: &str) -> Vec<&'a ChunkMeta> {
    loaded.chunks.iter().filter(|c| c.file == file).collect()
}

/// Render a ScoredChunk as the tool's result object.
pub fn scored_chunk_to_json(hit: &ScoredChunk) -> Value {
    json!({
        "i": hit.chunk.i,
        "file": hit.chunk.file,
        "line_start": hit.chunk.line_start,
        "line_end": hit.chunk.line_end,
        "text": hit.chunk.text,
        "score": hit.score,
        "bm25_score": hit.bm25_score,
        "vector_score": hit.vector_score,
    })
}
