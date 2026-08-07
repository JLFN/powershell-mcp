//! MCP server handler — the three query tools, wired to the RAG store.
//!
//! Tool results are returned as pretty-printed JSON in a text content
//! block, or isError with "Error executing <name>: ..." on failure. The
//! index is loaded once per process and cached.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rmcp::{
    handler::server::wrapper::Parameters,
    model::{CallToolResult, ContentBlock, ServerCapabilities, ServerInfo},
    schemars, tool, tool_handler, tool_router, ServerHandler,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::rag::{self, LoadedRag, QueryMode};
use crate::SERVER_NAME;

/// Wrap a tool result into an MCP response: success is a pretty-printed
/// JSON text block; failure is isError with the exact message.
fn tool_result(name: &str, result: Result<Value, String>) -> CallToolResult {
    match result {
        Ok(v) => {
            let text = serde_json::to_string_pretty(&v).unwrap_or_else(|_| "{}".into());
            CallToolResult::success(vec![ContentBlock::text(text)])
        }
        Err(message) => CallToolResult::error(vec![ContentBlock::text(format!(
            "Error executing {}: {}",
            name, message
        ))]),
    }
}

/// MCP server. Holds the resolved RAG data directory and a lazily-loaded
/// index cache.
pub struct PwshServer {
    rag_root: PathBuf,
    rag_cache: Arc<Mutex<Option<Arc<LoadedRag>>>>,
}

impl PwshServer {
    pub fn new() -> anyhow::Result<Self> {
        Ok(Self {
            rag_root: rag::rag_dir(),
            rag_cache: Arc::new(Mutex::new(None)),
        })
    }

    /// Load (once) and return the semantic index, erroring when it has not
    /// been built yet.
    fn loaded_rag(&self) -> Result<Arc<LoadedRag>, String> {
        let mut cache = self.rag_cache.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(loaded) = cache.as_ref() {
            return Ok(loaded.clone());
        }
        let loaded = rag::load(&self.rag_root).map_err(|e| e.to_string())?;
        let arc = Arc::new(loaded);
        *cache = Some(arc.clone());
        Ok(arc)
    }

    fn do_search(&self, query: &str, mode: QueryMode, k: usize) -> Result<Value, String> {
        let loaded = self.loaded_rag()?;
        let query_embedding: Option<Vec<f32>> = match mode {
            QueryMode::Bm25 => None,
            QueryMode::Vector | QueryMode::Hybrid => {
                rag::ensure_model_matches(&loaded).map_err(|e| e.to_string())?;
                let model = crate::embed::current_model();
                let prefix = if model.uses_prefixes() {
                    crate::embed::QUERY_PREFIX
                } else {
                    ""
                };
                let texts = vec![format!("{prefix}{query}")];
                let embedded = crate::embed::embed_texts(&texts).map_err(|e| e.to_string())?;
                embedded.first().cloned()
            }
        };
        let hits = rag::retrieve(
            &loaded,
            &self.rag_root,
            query,
            mode,
            k,
            query_embedding.as_deref(),
        )
        .map_err(|e| e.to_string())?;
        Ok(json!(hits
            .iter()
            .map(rag::scored_chunk_to_json)
            .collect::<Vec<_>>()))
    }

    fn do_file_context(&self, file: &str) -> Result<Value, String> {
        let loaded = self.loaded_rag()?;
        let chunks: Vec<Value> = rag::file_context(&loaded, file)
            .iter()
            .map(|c| {
                json!({
                    "i": c.i,
                    "file": c.file,
                    "line_start": c.line_start,
                    "line_end": c.line_end,
                    "text": c.text,
                })
            })
            .collect();
        Ok(json!(chunks))
    }

    fn do_index_info(&self) -> Result<Value, String> {
        let loaded = self.loaded_rag()?;
        Ok(json!({
            "root": self.rag_root.display().to_string(),
            "version": loaded.manifest.version,
            "model": loaded.manifest.model,
            "dim": loaded.manifest.dim,
            "chunk_count": loaded.manifest.chunk_count,
            "file_count": loaded.manifest.file_count,
            "built_at": loaded.manifest.built_at,
            "project_root": loaded.manifest.project_root,
            "source_fingerprint": loaded.manifest.source_fingerprint,
        }))
    }
}

// ---------------------------------------------------------------------------
// Request types (serde + schemars, doc comments on every field)
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SearchRequest {
    /// The question or keywords to search the PowerShell documentation for.
    query: String,
    /// hybrid (default, BM25 + vector fused), vector (semantic only), or
    /// bm25 (exact keyword, no model needed).
    mode: Option<String>,
    /// Number of results to return, default 5, max 20.
    k: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct FileContextRequest {
    /// Path relative to the corpus root, exactly as returned by search
    /// results (forward slashes).
    file: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct NoParams {}

// ---------------------------------------------------------------------------
// Tools
// ---------------------------------------------------------------------------

#[tool_router]
impl PwshServer {
    #[tool(
        description = "Search the PowerShell documentation corpus (cmdlet reference, about topics, and conceptual guides from MicrosoftDocs/PowerShell-Docs) with a hybrid BM25 + vector index and return the most relevant chunks with file and line provenance. mode: hybrid (default), vector, or bm25. k: number of results, default 5, max 20. Use for questions about PowerShell cmdlets, syntax, parameters, concepts, or behavior."
    )]
    async fn search(
        &self,
        Parameters(SearchRequest { query, mode, k }): Parameters<SearchRequest>,
    ) -> CallToolResult {
        let mode = mode
            .as_deref()
            .map(QueryMode::parse)
            .unwrap_or(QueryMode::Hybrid);
        let k = k.map(|v| v as usize).unwrap_or(rag::DEFAULT_K);
        tool_result("search", self.do_search(&query, mode, k))
    }

    #[tool(
        description = "Return every indexed chunk of one documentation file, in source order, with line ranges. file is the path relative to the corpus root, exactly as returned by search results (for example reference/7.4/Microsoft.PowerShell.Core/Get-Command.md)."
    )]
    async fn file_context(
        &self,
        Parameters(FileContextRequest { file }): Parameters<FileContextRequest>,
    ) -> CallToolResult {
        tool_result("file_context", self.do_file_context(&file))
    }

    #[tool(
        description = "Report the state of the semantic index this server serves: model, dimensions, chunk and file counts, built-at time, corpus root, and source fingerprint. Errors clearly when no index has been built yet."
    )]
    async fn index_info(&self, Parameters(NoParams {}): Parameters<NoParams>) -> CallToolResult {
        tool_result("index_info", self.do_index_info())
    }
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

#[tool_handler]
impl ServerHandler for PwshServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(rmcp::model::Implementation::new(
                SERVER_NAME,
                env!("CARGO_PKG_VERSION"),
            ))
            .with_instructions(
                "Hybrid RAG over the official PowerShell documentation: search returns the \
                 most relevant chunks (BM25 + vector fused) with file and line provenance, \
                 file_context returns every chunk of one file, index_info reports the index \
                 state. Run `powershell-mcp build-index <corpus-root>` first if search errors.",
            )
    }
}
