/// JS/TS 文件依赖分析器
/// 支持：ESM import/export、CommonJS require、动态 import()、JSX 组件引用
use std::path::Path;

use once_cell::sync::Lazy;
use regex::Regex;

use super::{Analyzer, DepEntry, FileAnalysis};
use crate::analyzer::file_dep::resolve_relative;
use crate::error::Result;
use crate::graph::Language;

// ───────────────────────────── 正则定义 ─────────────────────────────

/// ESM 静态 import（含 type import）
/// 匹配：import foo from '...', import { a } from '...', import * as x from '...'
static RE_ESM_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*import\s+(?:type\s+)?(?:.*?from\s+)?['"]([^'"]+)['"]"#).unwrap()
});

/// ESM 动态 import()
static RE_DYNAMIC_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:import|require)\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap()
});

/// CommonJS require()
static RE_REQUIRE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?:^|=\s*)require\s*\(\s*['"]([^'"]+)['"]\s*\)"#).unwrap()
});

/// ESM re-export: export { x } from '...'
static RE_EXPORT_FROM: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*export\s+(?:type\s+)?(?:\*|\{[^}]*\})\s+from\s+['"]([^'"]+)['"]"#).unwrap()
});

/// JSX 组件引用（大写字母开头的标签）
/// 简化匹配：<ComponentName 或 <ComponentName/>
static RE_JSX_COMPONENT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"<([A-Z][A-Za-z0-9]*)[/\s>]"#).unwrap()
});

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct JsTsAnalyzer;

impl JsTsAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 判断是否是 type-only import
    fn is_type_import(line: &str) -> bool {
        line.contains("import type ") || line.contains("import type{")
    }

    /// 从一行中提取所有导入路径
    fn extract_line(line: &str, line_num: usize) -> Vec<DepEntry> {
        let mut entries = Vec::new();
        let is_type = Self::is_type_import(line);

        // ESM import
        for cap in RE_ESM_IMPORT.captures_iter(line) {
            entries.push(DepEntry {
                raw_path: cap[1].to_string(),
                resolved: None,
                line: line_num,
                is_type_only: is_type,
            });
        }

        // 如果 ESM 没匹配到，再尝试 require
        if entries.is_empty() {
            for cap in RE_REQUIRE.captures_iter(line) {
                entries.push(DepEntry {
                    raw_path: cap[1].to_string(),
                    resolved: None,
                    line: line_num,
                    is_type_only: false,
                });
            }
        }

        // 动态 import（可能与 ESM 重复，需去重）
        for cap in RE_DYNAMIC_IMPORT.captures_iter(line) {
            let path = cap[1].to_string();
            if !entries.iter().any(|e| e.raw_path == path) {
                entries.push(DepEntry {
                    raw_path: path,
                    resolved: None,
                    line: line_num,
                    is_type_only: false,
                });
            }
        }

        // export ... from '...'
        for cap in RE_EXPORT_FROM.captures_iter(line) {
            let path = cap[1].to_string();
            if !entries.iter().any(|e| e.raw_path == path) {
                entries.push(DepEntry {
                    raw_path: path,
                    resolved: None,
                    line: line_num,
                    is_type_only: is_type,
                });
            }
        }

        entries
    }

    /// 对解析出的 DepEntry 列表做路径解析
    fn resolve_entries(entries: &mut Vec<DepEntry>, source_file: &Path) {
        for entry in entries.iter_mut() {
            entry.resolved = resolve_relative(source_file, &entry.raw_path);
        }
    }
}

impl Default for JsTsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for JsTsAnalyzer {
    fn language(&self) -> Language {
        Language::TypeScript // 同时处理 JS，以 TS 为代表
    }

    fn can_handle(&self, path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts")
        )
    }

    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis> {
        let content = std::fs::read_to_string(path)?;

        let lang = match path.extension().and_then(|e| e.to_str()) {
            Some("ts" | "tsx" | "mts" | "cts") => Language::TypeScript,
            _ => Language::JavaScript,
        };

        let mut deps: Vec<DepEntry> = content
            .lines()
            .enumerate()
            .flat_map(|(i, line)| Self::extract_line(line, i + 1))
            .collect();

        // 路径解析
        Self::resolve_entries(&mut deps, path);

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: lang,
            deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_esm_import() {
        let entries = JsTsAnalyzer::extract_line("import React from 'react'", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "react");
    }

    #[test]
    fn test_type_import() {
        let entries = JsTsAnalyzer::extract_line("import type { Foo } from './foo'", 1);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].is_type_only);
        assert_eq!(entries[0].raw_path, "./foo");
    }

    #[test]
    fn test_require() {
        let entries = JsTsAnalyzer::extract_line("const path = require('path')", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "path");
    }

    #[test]
    fn test_export_from() {
        let entries = JsTsAnalyzer::extract_line("export { default } from './Button'", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "./Button");
    }
}
