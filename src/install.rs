//! Install helper: copy the binary to ~/.local/bin, register the MCP
//! server (with POWERSHELL_MCP_HOME pointing at the index directory) in
//! ~/.opengrok/config.toml, and install the embedded skill, baking in the
//! index directory.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

/// The skill shipped with the server, embedded so the installed binary is
/// self-contained. When you rename the server, update the skill content.
pub const SKILL_MD: &str = include_str!("../skills/powershell-mcp/SKILL.md");

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(PathBuf::from))
}

/// $OPENGROK_HOME or ~/.opengrok, on any OS.
fn open_grok_home() -> Result<PathBuf> {
    if let Some(h) = std::env::var_os("OPENGROK_HOME") {
        return Ok(PathBuf::from(h));
    }
    home_dir()
        .map(|h| h.join(".opengrok"))
        .context("cannot determine home directory (set OPENGROK_HOME)")
}

/// $POWERSHELL_MCP_INSTALL_DIR or ~/.local/bin, on any OS.
fn default_install_dir() -> Result<PathBuf> {
    if let Some(d) = std::env::var_os("POWERSHELL_MCP_INSTALL_DIR") {
        return Ok(PathBuf::from(d));
    }
    home_dir()
        .map(|h| h.join(".local").join("bin"))
        .context("cannot determine home directory (set POWERSHELL_MCP_INSTALL_DIR)")
}

/// Install this binary under `server_name`: copy itself into the install
/// dir, register the MCP server in the Open Grok config with
/// POWERSHELL_MCP_HOME set to `rag_dir`, and install the skill.
pub fn install(rag_dir: &Path, server_name: &str) -> Result<()> {
    let exe = std::env::current_exe().context("locate own binary")?;
    let install_dir = default_install_dir()?;
    std::fs::create_dir_all(&install_dir)?;
    let dest = install_dir.join(server_name);
    if exe != dest {
        std::fs::copy(&exe, &dest).with_context(|| format!("copy to {}", dest.display()))?;
        println!("installed binary: {}", dest.display());
    } else {
        println!("binary already in place: {}", dest.display());
    }

    register_mcp_server(&dest, server_name, rag_dir)?;

    let skill_dir = open_grok_home()?.join("skills").join(server_name);
    std::fs::create_dir_all(&skill_dir)?;
    let skill_path = skill_dir.join("SKILL.md");
    std::fs::write(&skill_path, SKILL_MD)?;
    println!("installed skill: {}", skill_path.display());

    println!(
        "next: restart open-grok (or /mcps + r to refresh), then verify with:\n  open-grok mcp doctor {server_name}"
    );
    Ok(())
}

/// Register (or update) the [mcp_servers.<name>] and
/// [mcp_servers.<name>.env] blocks in the Open Grok config.toml. Existing
/// blocks for the name are replaced; everything else is preserved.
fn register_mcp_server(bin: &Path, name: &str, rag_dir: &Path) -> Result<()> {
    let config_path = open_grok_home()?.join("config.toml");
    if let Some(parent) = config_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create {}", parent.display()))?;
    }
    let mut content = if config_path.exists() {
        std::fs::read_to_string(&config_path)
            .with_context(|| format!("read {}", config_path.display()))?
    } else {
        String::new()
    };

    content = remove_section(&content, &format!("[mcp_servers.{name}]"));
    content = remove_section(&content, &format!("[mcp_servers.{name}.env]"));

    if !content.is_empty() && !content.ends_with('\n') {
        content.push('\n');
    }
    if !content.is_empty() {
        content.push('\n');
    }
    content.push_str(&format!(
        "[mcp_servers.{name}]\ncommand = {:?}\nenabled = true\n\n[mcp_servers.{name}.env]\nPOWERSHELL_MCP_HOME = {:?}\n",
        bin.display().to_string(),
        rag_dir.display().to_string(),
    ));
    std::fs::write(&config_path, content)
        .with_context(|| format!("write {}", config_path.display()))?;
    println!(
        "registered MCP server '{name}' in {}",
        config_path.display()
    );
    Ok(())
}

/// Drop every line from the first occurrence of `header` through the next
/// line that starts a new TOML table.
fn remove_section(content: &str, header: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            skipping = trimmed == header;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remove_section_drops_only_the_target_block() {
        let content = concat!(
            "[mcp_servers.other]\ncommand = \"x\"\nenabled = true\n\n",
            "[mcp_servers.powershell-mcp]\ncommand = \"a\"\nenabled = true\n\n",
            "[mcp_servers.powershell-mcp.env]\nPOWERSHELL_MCP_HOME = \"/old\"\n\n",
            "[mcp_servers.yet-another]\ncommand = \"y\"\nenabled = true\n",
        );
        let out = remove_section(content, "[mcp_servers.powershell-mcp]");
        let out = remove_section(&out, "[mcp_servers.powershell-mcp.env]");
        assert!(out.contains("mcp_servers.other"));
        assert!(out.contains("mcp_servers.yet-another"));
        assert!(!out.contains("powershell-mcp"));
    }

    #[test]
    fn remove_section_handles_missing_block() {
        let content = "[mcp_servers.a]\ncommand = \"x\"\n";
        assert_eq!(remove_section(content, "[mcp_servers.b]"), content);
    }
}
