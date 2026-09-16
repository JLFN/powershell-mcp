//! Local embedding model (fastembed, ONNX, no API key).
//!
//! Default model is BAAI/bge-small-en-v1.5 (384 dims, English prose,
//! cached on this machine in ~/.cache/fastembed). The PowerShell
//! documentation is English technical prose, which is exactly what
//! bge-small is tuned for; switch with the POWERSHELL_MCP_EMBED_MODEL
//! environment variable. E5 models are trained with instruction prefixes
//! ("query: " / "passage: "); bge is not. Sliced batching keeps peak
//! memory bounded.

use anyhow::{bail, Context, Result};
use fastembed::{EmbeddingModel, TextEmbedding, TextInitOptions};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

/// Number of texts handed to the model per call, bounding peak memory.
/// The slice is split into batch-sized chunks that fastembed runs
/// concurrently on its rayon pool, so a 256-text slice means four 64-text
/// ONNX runs in flight — a good wall-clock/memory trade-off on machines
/// with ~16 GB+ RAM.
pub const EMBED_SLICE: usize = 256;

/// Batch size passed to each fastembed call.
pub const EMBED_BATCH: usize = 64;

/// E5 instruction prefixes. Applied only when the active model needs them.
pub const QUERY_PREFIX: &str = "query: ";
pub const PASSAGE_PREFIX: &str = "passage: ";

/// The embedding models this server knows how to run locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Model {
    /// BAAI/bge-small-en-v1.5, 384 dims. English prose, fastest. Default.
    BgeSmall,
    /// intfloat/multilingual-e5-small, 384 dims. Multilingual, heavier.
    E5Multilingual,
    /// intfloat/multilingual-e5-base, 768 dims. Better quality, heaviest.
    E5Base,
}

impl Model {
    /// Resolve the active model from POWERSHELL_MCP_EMBED_MODEL; anything
    /// unknown or unset means the default (bge-small).
    pub fn from_env() -> Self {
        match std::env::var("POWERSHELL_MCP_EMBED_MODEL").as_deref() {
            Ok("e5-multilingual") | Ok("multilingual-e5-small") => Model::E5Multilingual,
            Ok("e5-base") | Ok("multilingual-e5-base") => Model::E5Base,
            _ => Model::BgeSmall,
        }
    }

    fn fastembed(self) -> EmbeddingModel {
        match self {
            Model::BgeSmall => EmbeddingModel::BGESmallENV15,
            Model::E5Multilingual => EmbeddingModel::MultilingualE5Small,
            Model::E5Base => EmbeddingModel::MultilingualE5Base,
        }
    }

    /// Identifier recorded in the index manifest. Must stay stable so the
    /// stale-index guard can compare it across builds.
    pub fn id(self) -> &'static str {
        match self {
            Model::BgeSmall => "bge-small-en-v1.5",
            Model::E5Multilingual => "multilingual-e5-small",
            Model::E5Base => "multilingual-e5-base",
        }
    }

    /// Embedding dimension of the model.
    pub fn dim(self) -> usize {
        match self {
            Model::BgeSmall => 384,
            Model::E5Multilingual => 384,
            Model::E5Base => 768,
        }
    }

    /// Whether indexed passages and queries need the E5 instruction
    /// prefixes.
    pub fn uses_prefixes(self) -> bool {
        matches!(self, Model::E5Multilingual | Model::E5Base)
    }
}

/// The active model for this process.
pub fn current_model() -> Model {
    Model::from_env()
}

static EMBEDDER: OnceLock<Mutex<TextEmbedding>> = OnceLock::new();

/// Embed texts into fixed-size vectors with bounded peak memory. The input
/// is split into small sequential slices so the model never sees the whole
/// corpus at once.
pub fn embed_texts(texts: &[String]) -> Result<Vec<Vec<f32>>> {
    let emb = embedder()?;
    let mut out = Vec::with_capacity(texts.len());
    for slice in texts.chunks(EMBED_SLICE) {
        let vectors = emb
            .lock()
            .map_err(|_| anyhow::anyhow!("embedder mutex poisoned"))?
            .embed(slice.to_vec(), Some(EMBED_BATCH))
            .context("embed texts")?;
        if vectors.len() != slice.len() {
            bail!(
                "embedding returned {} vectors for {} texts",
                vectors.len(),
                slice.len()
            );
        }
        out.extend(vectors);
    }
    Ok(out)
}

/// The directory fastembed caches model binaries in: $FASTEMBED_CACHE, or
/// ~/.cache/fastembed. Public so tests and tooling can detect whether a
/// model is already downloaded.
pub fn model_cache_dir() -> Option<PathBuf> {
    std::env::var("FASTEMBED_CACHE")
        .map(PathBuf::from)
        .ok()
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".cache").join("fastembed"))
        })
}

/// Whether the active model is already present in the cache (used by
/// tests to skip live embedding work without triggering a download).
pub fn active_model_cached() -> bool {
    let Some(cache) = model_cache_dir() else {
        return false;
    };
    let Ok(entries) = std::fs::read_dir(&cache) else {
        return false;
    };
    entries.flatten().any(|e| {
        e.file_name()
            .to_string_lossy()
            .contains(current_model().id())
    })
}

fn embedder() -> Result<&'static Mutex<TextEmbedding>> {
    if let Some(e) = EMBEDDER.get() {
        return Ok(e);
    }
    let mut init = TextInitOptions::default();
    init.model_name = current_model().fastembed();
    init.show_download_progress = true;
    init.cache_dir = model_cache_dir().unwrap_or_else(|| PathBuf::from(".fastembed_cache"));
    let emb = TextEmbedding::try_new(init)
        .context("init embedding model (first run downloads the model binary)")?;
    // Race-safe: if another thread won, keep its instance.
    let _ = EMBEDDER.set(Mutex::new(emb));
    Ok(EMBEDDER.get().expect("embedder set"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bge_small_is_the_default() {
        std::env::remove_var("POWERSHELL_MCP_EMBED_MODEL");
        assert_eq!(current_model(), Model::BgeSmall);
        assert_eq!(Model::BgeSmall.id(), "bge-small-en-v1.5");
        assert_eq!(Model::BgeSmall.dim(), 384);
        assert!(!Model::BgeSmall.uses_prefixes());
    }

    #[test]
    fn e5_models_use_prefixes() {
        assert!(Model::E5Multilingual.uses_prefixes());
        assert!(Model::E5Base.uses_prefixes());
    }
}
