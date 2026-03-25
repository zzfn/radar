/// 通用文件级依赖分析器
/// 通过正则匹配常见的 import/require/include 语句
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;

use super::{Analyzer, DepEntry, FileAnalysis};
use crate::error::Result;
use crate::graph::Language;

/// 通用 import 模式：匹配多种语言的导入语句
static IMPORT_PATTERNS: Lazy<Vec<Regex>> = Lazy::new(|| {
    vec![
        // Python: import foo, from foo import bar
        Regex::new(r#"^\s*(?:from\s+([\w.]+)\s+import|import\s+([\w.,\s]+))"#).unwrap(),
        // C/C++: #include "foo.h" or #include <foo.h>
        Regex::new(r#"^\s*#include\s+["<]([^">]+)[">]"#).unwrap(),
        // 通用 require("...") 或 require('...')
        Regex::new(r#"require\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap(),
    ]
});

/// 通用文件分析器（用于不特定支持的语言，或作为兜底）
pub struct GenericAnalyzer;

impl GenericAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 从单行文本中提取依赖路径
    fn extract_deps_from_line(line: &str, line_num: usize) -> Vec<DepEntry> {
        let mut entries = Vec::new();
        for pattern in IMPORT_PATTERNS.iter() {
            for cap in pattern.captures_iter(line) {
                // 取第一个非空捕获组
                let raw = (1..cap.len())
                    .find_map(|i| cap.get(i))
                    .map(|m| m.as_str().trim().to_string());

                if let Some(raw_path) = raw {
                    entries.push(DepEntry {
                        raw_path,
                        resolved: None, // 通用分析器不做路径解析
                        line: line_num,
                        is_type_only: false,
                    });
                }
            }
        }
        entries
    }
}

impl Default for GenericAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for GenericAnalyzer {
    fn language(&self) -> Language {
        Language::Unknown
    }

    fn can_handle(&self, _path: &Path) -> bool {
        // 作为兜底分析器，接受所有文本文件
        true
    }

    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis> {
        let content = std::fs::read_to_string(path)?;
        let lang = path
            .extension()
            .and_then(|e| e.to_str())
            .map(Language::from_extension)
            .unwrap_or(Language::Unknown);

        let deps = content
            .lines()
            .enumerate()
            .flat_map(|(i, line)| Self::extract_deps_from_line(line, i + 1))
            .collect();

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: lang,
            deps,
        })
    }
}

/// 解析相对路径（相对于当前文件所在目录）
pub fn resolve_relative(source_file: &Path, import_path: &str) -> Option<PathBuf> {
    // 只处理相对路径（以 ./ 或 ../ 开头）
    if !import_path.starts_with('.') {
        return None;
    }

    let base_dir = source_file.parent()?;
    let candidate = base_dir.join(import_path);

    // 尝试直接路径
    if candidate.exists() {
        return Some(candidate);
    }

    // 尝试常见扩展名
    for ext in &["js", "ts", "tsx", "jsx", "rs", "py"] {
        let with_ext = candidate.with_extension(ext);
        if with_ext.exists() {
            return Some(with_ext);
        }
    }

    // 尝试 index 文件
    for ext in &["js", "ts", "tsx", "jsx"] {
        let index = candidate.join(format!("index.{}", ext));
        if index.exists() {
            return Some(index);
        }
    }

    None
}
