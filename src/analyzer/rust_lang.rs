/// Rust 文件依赖分析器
/// 支持：use 语句、mod 声明、extern crate
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;

use super::{Analyzer, DepEntry, FileAnalysis};
use crate::error::Result;
use crate::graph::Language;

// ───────────────────────────── 正则定义 ─────────────────────────────

/// use 语句：use std::collections::HashMap;
static RE_USE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*(?:pub\s+)?use\s+([\w:{}*,\s]+)\s*;"#).unwrap());

/// mod 声明：mod foo; 或 pub mod foo;
static RE_MOD: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*(?:pub\s+)?mod\s+(\w+)\s*;"#).unwrap());

/// extern crate：extern crate serde;
static RE_EXTERN_CRATE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"^\s*extern\s+crate\s+(\w+)\s*;"#).unwrap());

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct RustAnalyzer;

impl RustAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 从单行提取 Rust 依赖信息
    fn extract_line(line: &str, line_num: usize) -> Vec<DepEntry> {
        let mut entries = Vec::new();

        // use 语句
        if let Some(cap) = RE_USE.captures(line) {
            entries.push(DepEntry {
                raw_path: cap[1].trim().to_string(),
                resolved: None,
                line: line_num,
                is_type_only: false,
            });
        }

        // mod 声明
        if let Some(cap) = RE_MOD.captures(line) {
            entries.push(DepEntry {
                raw_path: format!("mod::{}", &cap[1]),
                resolved: None,
                line: line_num,
                is_type_only: false,
            });
        }

        // extern crate
        if let Some(cap) = RE_EXTERN_CRATE.captures(line) {
            entries.push(DepEntry {
                raw_path: format!("extern::{}", &cap[1]),
                resolved: None,
                line: line_num,
                is_type_only: false,
            });
        }

        entries
    }

    /// 根据 mod 声明推断对应的文件路径
    /// 例如：src/foo.rs 中的 `mod bar` -> 尝试 src/bar.rs 或 src/bar/mod.rs
    fn resolve_mod(source_file: &Path, mod_name: &str) -> Option<PathBuf> {
        let base = source_file.parent()?;

        // 情况一：src/bar.rs
        let candidate1 = base.join(format!("{}.rs", mod_name));
        if candidate1.exists() {
            return Some(candidate1);
        }

        // 情况二：src/bar/mod.rs
        let candidate2 = base.join(mod_name).join("mod.rs");
        if candidate2.exists() {
            return Some(candidate2);
        }

        None
    }
}

impl Default for RustAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for RustAnalyzer {
    fn language(&self) -> Language {
        Language::Rust
    }

    fn can_handle(&self, path: &Path) -> bool {
        path.extension().and_then(|e| e.to_str()) == Some("rs")
    }

    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis> {
        let content = std::fs::read_to_string(path)?;

        let mut deps: Vec<DepEntry> = content
            .lines()
            .enumerate()
            .flat_map(|(i, line)| Self::extract_line(line, i + 1))
            .collect();

        // 对 mod 声明做路径解析
        for entry in deps.iter_mut() {
            if let Some(mod_name) = entry.raw_path.strip_prefix("mod::") {
                entry.resolved = Self::resolve_mod(path, mod_name);
            }
        }

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: Language::Rust,
            deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_use_statement() {
        let entries = RustAnalyzer::extract_line("use std::collections::HashMap;", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "std::collections::HashMap");
    }

    #[test]
    fn test_mod_declaration() {
        let entries = RustAnalyzer::extract_line("pub mod cli;", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "mod::cli");
    }

    #[test]
    fn test_extern_crate() {
        let entries = RustAnalyzer::extract_line("extern crate serde;", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "extern::serde");
    }

    #[test]
    fn test_pub_use() {
        let entries = RustAnalyzer::extract_line("pub use crate::error::Result;", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "crate::error::Result");
    }
}
