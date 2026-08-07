//! powershell-mcp binary entry point.
//!
//! With no arguments, serves MCP over stdio. Subcommands:
//!   build-index <corpus-root> [index-dir]  build the hybrid index
//!   query <question> [mode] [k]            CLI search over the active index
//!   index-info                             show index status
//!   files                                  list indexed files
//!   install <index-dir> [server-name]      install binary, register MCP, install skill

use std::io::IsTerminal;
use std::path::PathBuf;

use anyhow::{Context, Result};
use powershell_mcp::rag::{self, QueryMode};
use powershell_mcp::{embed, install, SERVER_NAME};
use rmcp::{transport::stdio, ServiceExt};

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("serve") => serve(),
        Some("build-index") => build_index(&args),
        Some("query") => query_cli(&args),
        Some("index-info") => index_info(),
        Some("files") => list_files(),
        Some("install") => install_cmd(&args),
        Some("--help") | Some("-h") => {
            print_help();
            Ok(())
        }
        Some(other) => anyhow::bail!("unknown command: {other} (try --help)"),
    }
}

fn print_help() {
    eprintln!("powershell-mcp — hybrid vector + BM25 RAG over the official PowerShell docs");
    eprintln!();
    eprintln!("Usage: powershell-mcp [COMMAND]");
    eprintln!();
    eprintln!("Commands:");
    eprintln!("  (none)                    Serve MCP over stdio (default)");
    eprintln!("  build-index <corpus> [d]  Build the hybrid index from a PowerShell-Docs checkout");
    eprintln!("  query <question> [m] [k]  CLI search (m: hybrid|vector|bm25, default 5)");
    eprintln!("  index-info                Show index status");
    eprintln!("  files                     List indexed files");
    eprintln!("  install <index-dir> [n]   Install binary, register MCP server, install skill");
    eprintln!();
    eprintln!("Environment:");
    eprintln!("  POWERSHELL_MCP_HOME         index directory (default <corpus>/.rag-index)");
    eprintln!("  POWERSHELL_MCP_EMBED_MODEL  bge-small (default), e5-multilingual, e5-base");
    eprintln!("  POWERSHELL_MCP_SERVER_NAME  MCP server name at install time (default powershell-mcp)");
}

fn serve() -> Result<()> {
    if std::io::stdin().is_terminal() {
        eprintln!(
            "powershell-mcp is an MCP server and is meant to be launched by your MCP \
             client, not run by hand. It will now wait silently for a client on stdin — \
             that is normal, not a hang. Press Ctrl-C to exit."
        );
    }

    let server = powershell_mcp::server::PwshServer::new()?;
    let rt = tokio::runtime::Runtime::new().context("create tokio runtime")?;
    rt.block_on(async move {
        let running = server.serve(stdio()).await?;
        running.waiting().await?;
        Ok(())
    })
}

fn build_index(args: &[String]) -> Result<()> {
    let corpus_root = args
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let root = match args.get(2) {
        Some(dir) => PathBuf::from(dir),
        None => std::env::var(rag::HOME_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|_| rag::default_rag_dir(&corpus_root)),
    };
    eprintln!(
        "[build-index] indexing {} into {}",
        corpus_root.display(),
        root.display()
    );
    let report = rag::build_index(&corpus_root, &root, &embed::embed_texts)?;
    eprintln!(
        "[build-index] done: {} chunks from {} files ({} dims, {})",
        report.chunks, report.files, report.dim, report.model
    );
    Ok(())
}

fn query_cli(args: &[String]) -> Result<()> {
    let question = args
        .get(1)
        .context("usage: powershell-mcp query <question> [mode] [k]")?;
    let mode = args
        .get(2)
        .map(|s| QueryMode::parse(s))
        .unwrap_or(QueryMode::Hybrid);
    let k = args
        .get(3)
        .and_then(|s| s.parse().ok())
        .unwrap_or(rag::DEFAULT_K);
    let root = rag::rag_dir();
    let hits = rag::search(&root, question, mode, k)?;
    let arr: Vec<serde_json::Value> = hits.iter().map(rag::scored_chunk_to_json).collect();
    println!("{}", serde_json::to_string_pretty(&arr)?);
    Ok(())
}

fn index_info() -> Result<()> {
    let root = rag::rag_dir();
    match rag::read_manifest(&root) {
        Ok(m) => {
            eprintln!("index:     {}", root.display());
            eprintln!("model:     {}", m.model);
            eprintln!("dim:       {}", m.dim);
            eprintln!("chunks:    {}", m.chunk_count);
            eprintln!("files:     {}", m.file_count);
            eprintln!("project:   {}", m.project_root);
            eprintln!("built at:  {}", m.built_at);
            eprintln!("fingerprint: {}", m.source_fingerprint);
            Ok(())
        }
        Err(e) => {
            eprintln!("no semantic index: {e}");
            Ok(())
        }
    }
}

fn list_files() -> Result<()> {
    let root = rag::rag_dir();
    let loaded = rag::load(&root)?;
    let mut files: Vec<&str> = loaded.chunks.iter().map(|c| c.file.as_str()).collect();
    files.sort_unstable();
    files.dedup();
    for f in files {
        println!("{f}");
    }
    Ok(())
}

fn install_cmd(args: &[String]) -> Result<()> {
    let rag_dir = match args.get(1) {
        Some(dir) => PathBuf::from(dir),
        None => std::env::var(rag::HOME_ENV)
            .map(PathBuf::from)
            .context(
                "usage: powershell-mcp install <index-dir> [server-name] (or set POWERSHELL_MCP_HOME)",
            )?,
    };
    let name = args
        .get(2)
        .cloned()
        .or_else(|| std::env::var("POWERSHELL_MCP_SERVER_NAME").ok())
        .unwrap_or_else(|| SERVER_NAME.to_string());
    install::install(&rag_dir, &name)
}
