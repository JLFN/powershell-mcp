//! Corpus scanning — walk the PowerShell-Docs checkout and collect the
//! markdown files that should be indexed, with relative paths and sizes.
//!
//! The corpus is the official MicrosoftDocs/PowerShell-Docs repository
//! (reference/ cmdlet pages, about topics, and docs-conceptual guides).
//! Only markdown is indexed; supporting directories (media, mapping,
//! breadcrumbs, CI, tests) and contributor-facing root documents are
//! skipped.

use anyhow::{Context, Result};
use std::path::Path;

/// Extensions indexed by default. The PowerShell corpus is markdown only.
pub const DEFAULT_EXTS: &[&str] = &["md", "markdown"];

/// Directories never scanned, at any depth. Generic noise plus the
/// PowerShell-Docs directories that carry no retrievable prose.
pub const EXCLUDED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
    "dist",
    "build",
    "out",
    ".idea",
    ".vscode",
    ".devcontainer",
    ".opengrok",
    ".rag-index",
    "graphify-out",
    ".cache",
    "vendor",
    "bin",
    "artifacts",
    "coverage",
    ".next",
    ".nuxt",
    ".svelte-kit",
    "Pods",
    ".terraform",
    // PowerShell-Docs specific: CI, tests, redirect stubs, media, and
    // navigation support files hold no documentation prose.
    ".github",
    "tests",
    "assets",
    "redir",
    "media",
    "bread",
    "mapping",
];

/// Lockfiles, generated manifests, and contributor-facing root documents
/// that add noise without retrieval value.
pub const EXCLUDED_FILES: &[&str] = &[
    "Cargo.lock",
    "package-lock.json",
    "yarn.lock",
    "pnpm-lock.yaml",
    "poetry.lock",
    "Pipfile.lock",
    "uv.lock",
    "composer.lock",
    "Gemfile.lock",
    "go.sum",
    // PowerShell-Docs contributor-facing documents, not PowerShell
    // reference material.
    "CODE_OF_CONDUCT.md",
    "CONTRIBUTING.md",
    "SECURITY.md",
    "ThirdPartyNotices.md",
    "LICENSE.md",
    "LICENSE-CODE.md",
];

/// Files larger than this are skipped (generated bundles, dumps).
pub const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// One file the scanner wants to index.
#[derive(Debug, Clone)]
pub struct ScannedFile {
    /// Path relative to the project root, forward slashes.
    pub rel_path: String,
    /// File size in bytes (used for the source fingerprint).
    pub bytes: u64,
}

/// Walk a project root and return the indexable files, sorted by path so
/// indices are reproducible.
pub fn scan_project(root: &Path) -> Result<Vec<ScannedFile>> {
    let mut files = Vec::new();
    scan_dir(root, root, &mut files)?;
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(files)
}

fn scan_dir(root: &Path, dir: &Path, out: &mut Vec<ScannedFile>) -> Result<()> {
    let entries = std::fs::read_dir(dir).with_context(|| format!("read dir {}", dir.display()))?;
    for entry in entries {
        let entry = entry.with_context(|| format!("read entry in {}", dir.display()))?;
        let path = entry.path();
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        if path.is_dir() {
            if name_str.starts_with('.') || EXCLUDED_DIRS.contains(&name_str.as_ref()) {
                continue;
            }
            scan_dir(root, &path, out)?;
        } else if path.is_file() {
            if name_str.starts_with('.') || EXCLUDED_FILES.contains(&name_str.as_ref()) {
                continue;
            }
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if !DEFAULT_EXTS.contains(&ext) {
                continue;
            }
            let meta = entry
                .metadata()
                .with_context(|| format!("metadata {}", path.display()))?;
            if meta.len() > MAX_FILE_BYTES {
                continue;
            }
            let rel = path
                .strip_prefix(root)
                .with_context(|| format!("strip prefix for {}", path.display()))?;
            out.push(ScannedFile {
                rel_path: rel.to_string_lossy().to_string(),
                bytes: meta.len(),
            });
        }
    }
    Ok(())
}

/// Read a file as lossy UTF-8, returning None when it is unreadable or
/// contains NUL bytes in the first 8 KiB (a cheap binary check).
pub fn read_text(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return None;
    }
    Some(String::from_utf8_lossy(&bytes).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A minimal PowerShell-Docs-shaped fixture: reference markdown,
    /// about topics, conceptual guides, and noise that must be skipped.
    fn docs_fixture(dir: &Path) {
        std::fs::create_dir_all(dir.join("reference/7.4/Microsoft.PowerShell.Core/About")).unwrap();
        std::fs::create_dir_all(dir.join("reference/docs-conceptual/learn")).unwrap();
        std::fs::create_dir_all(dir.join("tests")).unwrap();
        std::fs::create_dir_all(dir.join(".github")).unwrap();
        std::fs::create_dir_all(dir.join("media")).unwrap();
        std::fs::create_dir_all(dir.join("redir")).unwrap();
        std::fs::create_dir_all(dir.join("reference/bread")).unwrap();
        std::fs::create_dir_all(dir.join("reference/mapping")).unwrap();
        std::fs::write(
            dir.join("reference/7.4/Microsoft.PowerShell.Core/Get-Command.md"),
            "---\ntitle: Get-Command\n---\n\n# Get-Command\n\nGets all commands.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("reference/7.4/Microsoft.PowerShell.Core/About/about_Arrays.md"),
            "# about_Arrays\n\nDescribes arrays.\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("reference/docs-conceptual/learn/learn-powershell.md"),
            "# Learn PowerShell\n\nA tutorial.\n",
        )
        .unwrap();
        std::fs::write(dir.join("README.md"), "# PowerShell Docs\n").unwrap();
        // Noise the scanner must skip.
        std::fs::write(dir.join("CODE_OF_CONDUCT.md"), "conduct\n").unwrap();
        std::fs::write(dir.join("SECURITY.md"), "security\n").unwrap();
        std::fs::write(dir.join("tests/unit.md"), "test docs\n").unwrap();
        std::fs::write(dir.join(".github/pull_request_template.md"), "pr\n").unwrap();
        std::fs::write(dir.join("media/logo.svg"), "<svg/>\n").unwrap();
        std::fs::write(dir.join("reference/bread/toc.yml"), "items:\n").unwrap();
        std::fs::write(dir.join("reference/mapping/monikerMapping.json"), "{}\n").unwrap();
        std::fs::write(dir.join("redir/redirect.md"), "redirect\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "plain text\n").unwrap();
    }

    #[test]
    fn scanner_finds_reference_markdown_and_skips_noise() {
        let dir = tempfile::tempdir().expect("tempdir");
        let root = dir.path();
        docs_fixture(root);

        let files = scan_project(root).unwrap();
        let paths: Vec<&str> = files.iter().map(|f| f.rel_path.as_str()).collect();
        assert_eq!(
            paths,
            vec![
                "README.md",
                "reference/7.4/Microsoft.PowerShell.Core/About/about_Arrays.md",
                "reference/7.4/Microsoft.PowerShell.Core/Get-Command.md",
                "reference/docs-conceptual/learn/learn-powershell.md",
            ],
            "only docs markdown is indexed; CI, tests, media, redirects, non-md files, and \
             contributor documents must be skipped"
        );
    }

    #[test]
    fn read_text_rejects_binary() {
        let dir = tempfile::tempdir().expect("tempdir");
        let p = dir.path().join("blob.md");
        let mut bytes = b"# heading\n".to_vec();
        bytes.push(0);
        std::fs::write(&p, bytes).unwrap();
        assert!(read_text(&p).is_none());
    }
}
