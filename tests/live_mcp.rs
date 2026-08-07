//! Live MCP round-trip test.
//!
//! Builds a small index with the real embedder (the bge-small model,
//! cached under ~/.cache/fastembed), then spawns the server binary and
//! drives the MCP handshake over stdio: initialize, tools/list, search,
//! file_context, index_info.
//!
//! The test is skipped when the embedding model is not yet cached, so a
//! plain `cargo test` never triggers the download. Build an index once
//! (`powershell-mcp build-index <dir>`) and the live test then runs fully.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc::{channel, Receiver};
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};

const RESPONSE_TIMEOUT: Duration = Duration::from_secs(240);

/// A minimal MCP client speaking newline-delimited JSON framing over stdio.
struct McpClient {
    child: Child,
    writer: ChildStdin,
    frames: Receiver<String>,
    next_id: u64,
}

impl McpClient {
    fn spawn(bin: &Path, home: &Path) -> Self {
        let mut child = Command::new(bin)
            .env("POWERSHELL_MCP_HOME", home)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn powershell-mcp");
        let writer = child.stdin.take().expect("child stdin");
        let stdout: ChildStdout = child.stdout.take().expect("child stdout");
        let (tx, rx) = channel();
        thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            let mut line = String::new();
            loop {
                line.clear();
                match reader.read_line(&mut line) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {
                        if tx.send(line.clone()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Self {
            child,
            writer,
            frames: rx,
            next_id: 1,
        }
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        let msg = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        self.send(&msg);
        self.wait_for(id)
    }

    fn notify(&mut self, method: &str) {
        let msg = json!({"jsonrpc": "2.0", "method": method});
        self.send(&msg);
    }

    fn call_tool(&mut self, name: &str, arguments: Value) -> Value {
        let frame = self.request("tools/call", json!({"name": name, "arguments": arguments}));
        let result = frame
            .get("result")
            .unwrap_or_else(|| panic!("tools/call {name} failed: {frame}"));
        if let Some(err) = frame.get("error") {
            panic!("tools/call {name} returned error: {err}");
        }
        result.clone()
    }

    fn tool_text(&self, result: &Value) -> Value {
        let content = result
            .get("content")
            .and_then(|c| c.as_array())
            .expect("result has content array");
        let text: String = content
            .iter()
            .filter_map(|item| {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    item.get("text").and_then(|t| t.as_str())
                } else {
                    None
                }
            })
            .collect();
        serde_json::from_str(&text)
            .unwrap_or_else(|e| panic!("tool result is not JSON ({e}): {text}"))
    }

    fn send(&mut self, msg: &Value) {
        writeln!(self.writer, "{msg}").expect("write to server stdin");
        self.writer.flush().expect("flush server stdin");
    }

    fn wait_for(&mut self, id: u64) -> Value {
        let deadline = std::time::Instant::now() + RESPONSE_TIMEOUT;
        while std::time::Instant::now() < deadline {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match self.frames.recv_timeout(remaining) {
                Ok(line) => {
                    let frame: Value = serde_json::from_str(&line)
                        .unwrap_or_else(|e| panic!("bad frame from server: {e}: {line}"));
                    if frame.get("id").and_then(|v| v.as_u64()) == Some(id) {
                        return frame;
                    }
                }
                Err(_) => break,
            }
        }
        panic!("timed out waiting for response id {id}");
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        let _ = self.writer.flush();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn fixture_corpus(dir: &Path) {
    std::fs::create_dir_all(dir.join("reference/7.4/Microsoft.PowerShell.Core")).unwrap();
    std::fs::write(
        dir.join("reference/7.4/Microsoft.PowerShell.Core/Get-Command.md"),
        "---\ntitle: Get-Command\n---\n\n# Get-Command\n\nGets all commands. It mentions zebra crossings and the word personuppgifter for the live test.\n\n## SYNTAX\n\n```\nGet-Command [-Verb <String[]>] [<CommonParameters>]\n```\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("README.md"),
        "# Live fixture\n\nUsed by the live MCP test to prove the binary works end to end.\n",
    )
    .unwrap();
}

#[test]
fn live_mcp_roundtrip() {
    if !powershell_mcp::embed::active_model_cached() {
        eprintln!(
            "skipping live MCP test: bge-small embedding model not cached yet \
             (run `powershell-mcp build-index <dir>` once to download it)"
        );
        return;
    }

    let tmp = tempfile::tempdir().expect("tempdir");
    let corpus = tmp.path().join("corpus");
    fixture_corpus(&corpus);
    let home = tmp.path().join("index");

    let bin = std::env::var("POWERSHELL_MCP_BIN")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_BIN_EXE_powershell-mcp")));

    // Build the index via the CLI with the real embedder.
    let status = Command::new(&bin)
        .arg("build-index")
        .arg(&corpus)
        .env("POWERSHELL_MCP_HOME", &home)
        .status()
        .expect("run build-index");
    assert!(status.success(), "build-index failed");

    // Handshake: initialize -> notifications/initialized.
    let mut client = McpClient::spawn(&bin, &home);
    let init = client.request(
        "initialize",
        json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": {"name": "live-test", "version": "0.0.1"}
        }),
    );
    let server_info = &init["result"]["serverInfo"];
    assert_eq!(server_info["name"], "powershell-mcp");
    assert_eq!(server_info["version"], env!("CARGO_PKG_VERSION"));
    client.notify("notifications/initialized");

    // tools/list: the three tools are advertised.
    let tools = client.request("tools/list", json!({}));
    let names: Vec<String> = tools["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|t| t["name"].as_str().map(str::to_string))
        .collect();
    for expected in ["search", "file_context", "index_info"] {
        assert!(
            names.iter().any(|n| n == expected),
            "missing tool {expected} in {names:?}"
        );
    }
    assert_eq!(names.len(), 3);

    // Hybrid search returns hits with provenance.
    let search = client.call_tool(
        "search",
        json!({"query": "personuppgifter", "mode": "hybrid", "k": 3}),
    );
    let hits = client.tool_text(&search);
    let hits = hits.as_array().expect("hits array");
    assert!(!hits.is_empty(), "search returned no hits");
    let first = &hits[0];
    assert!(first["file"].as_str().is_some());
    assert!(first["line_start"].as_u64().is_some());
    assert!(first["text"].as_str().is_some());
    assert!(first["score"].as_f64().is_some());

    // file_context returns every chunk of a known file.
    let ctx = client.call_tool("file_context", json!({"file": "README.md"}));
    let chunks = client.tool_text(&ctx);
    let chunks = chunks.as_array().expect("chunks array");
    assert!(!chunks.is_empty());
    assert_eq!(chunks[0]["file"], "README.md");

    // index_info reports the built index.
    let info = client.call_tool("index_info", json!({}));
    let info = client.tool_text(&info);
    assert_eq!(info["model"], powershell_mcp::embed::current_model().id());
    assert!(info["chunk_count"].as_u64().unwrap_or(0) >= 2);
    assert!(info["file_count"].as_u64().unwrap_or(0) >= 2);
}
