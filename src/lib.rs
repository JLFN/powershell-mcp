//! powershell-mcp — hybrid vector + BM25 RAG over the official PowerShell
//! documentation.
//!
//! The source of truth is the MicrosoftDocs/PowerShell-Docs repository
//! (the same markdown that powers learn.microsoft.com/powershell): cmdlet
//! reference pages, about topics, and conceptual guides across all
//! supported PowerShell versions. A CLI walks a checkout of that repo,
//! chunks the markdown with line provenance, embeds every chunk locally
//! (fastembed ONNX, no API key), builds a tantivy BM25 index, and serves
//! hybrid retrieval (reciprocal rank fusion) over MCP or a CLI query
//! command.
//!
//! Built from the rag-template recipe used by se-law-mcp and
//! health-second-opinion-mcp. See docs/setup.md for the full workflow.

pub mod bm25;
pub mod chunk;
pub mod embed;
pub mod install;
pub mod rag;
pub mod scan;
pub mod server;

/// Name used for the MCP server registration, the installed binary, the
/// skill directory, and the tools' namespace. The
/// POWERSHELL_MCP_SERVER_NAME environment variable overrides it at
/// install time.
pub const SERVER_NAME: &str = "powershell-mcp";
