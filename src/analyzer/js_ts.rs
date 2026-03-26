/// JS/TS 文件依赖分析器
/// 支持：ESM import/export、CommonJS require、动态 import()、JSX 组件引用
/// v0.3 新增：tsconfig.json paths 别名解析
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

// ─────────────────────────── tsconfig paths 解析 ───────────────────────────

/// 尝试读取 tsconfig.json 中的 paths 别名配置
/// 返回 Map<别名前缀, 替换路径列表>，例如 {"@/*": ["src/*"]}
pub fn read_tsconfig_paths(root: &Path) -> HashMap<String, Vec<String>> {
    let tsconfig_path = root.join("tsconfig.json");
    let content = match std::fs::read_to_string(&tsconfig_path) {
        Ok(c) => c,
        Err(_) => return HashMap::new(),
    };
    // 移除注释（tsconfig 允许注释，serde_json 不支持）
    let stripped = strip_json_comments(&content);
    let v: serde_json::Value = match serde_json::from_str(&stripped) {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    let mut result = HashMap::new();
    if let Some(paths) = v["compilerOptions"]["paths"].as_object() {
        for (alias, targets) in paths {
            let target_list: Vec<String> = targets
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .filter_map(|t| t.as_str().map(String::from))
                .collect();
            result.insert(alias.clone(), target_list);
        }
    }
    result
}

/// 简单剥离行注释（// ...）和块注释（/* ... */）
fn strip_json_comments(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    let mut in_string = false;
    let mut escape = false;
    while let Some(c) = chars.next() {
        if escape {
            escape = false;
            result.push(c);
            continue;
        }
        if c == '\\' && in_string {
            escape = true;
            result.push(c);
            continue;
        }
        if c == '"' {
            in_string = !in_string;
            result.push(c);
            continue;
        }
        if in_string {
            result.push(c);
            continue;
        }
        if c == '/' {
            match chars.peek() {
                Some('/') => {
                    // 行注释：跳到行尾
                    for nc in chars.by_ref() {
                        if nc == '\n' { result.push('\n'); break; }
                    }
                    continue;
                }
                Some('*') => {
                    // 块注释
                    chars.next();
                    loop {
                        match chars.next() {
                            Some('*') if chars.peek() == Some(&'/') => { chars.next(); break; }
                            Some('\n') => result.push('\n'),
                            None => break,
                            _ => {}
                        }
                    }
                    continue;
                }
                _ => {}
            }
        }
        result.push(c);
    }
    result
}

/// 尝试将 import 路径通过 tsconfig paths 别名解析为实际路径
/// alias_map: 从 tsconfig.json 读取的 paths 配置
/// root: 项目根目录
/// import_path: 原始 import 路径，如 "@/utils/helper"
pub fn resolve_alias(root: &Path, alias_map: &HashMap<String, Vec<String>>, import_path: &str) -> Option<PathBuf> {
    for (alias, targets) in alias_map {
        // 支持两种模式：精确匹配 "alias" 和通配符 "alias/*"
        let (alias_prefix, wildcard) = if alias.ends_with("/*") {
            (alias.trim_end_matches("/*"), true)
        } else {
            (alias.as_str(), false)
        };

        let rest = if wildcard {
            if let Some(r) = import_path.strip_prefix(&format!("{}/", alias_prefix)) {
                Some(r)
            } else if import_path == alias_prefix {
                Some("")
            } else {
                None
            }
        } else if import_path == alias_prefix {
            Some("")
        } else {
            None
        };

        if let Some(rest) = rest {
            for target in targets {
                let target_rel = if target.ends_with("/*") {
                    let base = target.trim_end_matches("/*");
                    if rest.is_empty() { base.to_string() } else { format!("{}/{}", base, rest) }
                } else {
                    target.clone()
                };
                // 从项目根目录解析
                let candidate = root.join(&target_rel);
                if candidate.exists() {
                    return Some(candidate);
                }
                // 尝试加扩展名
                for ext in &["ts", "tsx", "js", "jsx"] {
                    let with_ext = candidate.with_extension(ext);
                    if with_ext.exists() { return Some(with_ext); }
                }
                // 尝试 index 文件
                for ext in &["ts", "tsx", "js", "jsx"] {
                    let index = candidate.join(format!("index.{}", ext));
                    if index.exists() { return Some(index); }
                }
            }
        }
    }
    None
}

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct JsTsAnalyzer {
    /// tsconfig.json paths 别名映射（key: 别名前缀, value: 实际路径列表）
    alias_map: HashMap<String, Vec<String>>,
    /// 项目根目录
    root: PathBuf,
}

impl JsTsAnalyzer {
    pub fn new() -> Self {
        Self { alias_map: HashMap::new(), root: PathBuf::new() }
    }

    pub fn with_root(root: &Path) -> Self {
        let alias_map = read_tsconfig_paths(root);
        Self { alias_map, root: root.to_path_buf() }
    }

    /// 从一行中提取所有导入路径
    pub fn extract_line(line: &str, line_num: usize) -> Vec<DepEntry> {
        let mut entries = Vec::new();

        // ESM import
        for cap in RE_ESM_IMPORT.captures_iter(line) {
            entries.push(DepEntry {
                raw_path: cap[1].to_string(),
                resolved: None,
                line: line_num,
            });
        }

        // 如果 ESM 没匹配到，再尝试 require
        if entries.is_empty() {
            for cap in RE_REQUIRE.captures_iter(line) {
                entries.push(DepEntry {
                    raw_path: cap[1].to_string(),
                    resolved: None,
                    line: line_num,
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
                });
            }
        }

        entries
    }

    /// 对解析出的 DepEntry 列表做路径解析（支持别名）
    fn resolve_entries(&self, entries: &mut Vec<DepEntry>, source_file: &Path) {
        for entry in entries.iter_mut() {
            if entry.raw_path.starts_with('.') {
                entry.resolved = resolve_relative(source_file, &entry.raw_path);
            } else if !self.alias_map.is_empty() {
                entry.resolved = resolve_alias(&self.root, &self.alias_map, &entry.raw_path);
            }
        }
    }
}

impl Default for JsTsAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for JsTsAnalyzer {
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

        // 路径解析（含别名）
        self.resolve_entries(&mut deps, path);

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: lang,
            deps,
        })
    }

    /// 覆盖默认实现：先读取 tsconfig.json paths，再分析文件
    fn analyze_dir(&self, root: &Path, graph: &mut crate::graph::DependencyGraph, opts: &crate::analyzer::FilterOpts) -> Result<()> {
        let analyzer = JsTsAnalyzer::with_root(root);
        use ignore::WalkBuilder;
        use rayon::prelude::*;

        let (include_set, exclude_set) = opts.build_sets();

        let files: Vec<PathBuf> = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.into_path())
            .filter(|p| analyzer.can_handle(p))
            .filter(|p| {
                let rel = p.strip_prefix(root).unwrap_or(p);
                if let Some(ref inc) = include_set {
                    if !inc.is_match(rel) { return false; }
                }
                if let Some(ref exc) = exclude_set {
                    if exc.is_match(rel) { return false; }
                }
                true
            })
            .collect();

        let results: Vec<FileAnalysis> = files
            .par_iter()
            .filter_map(|p| analyzer.analyze_file(p).ok())
            .collect();

        for analysis in results {
            let source_node = crate::graph::Node {
                path: analysis.path.clone(),
                kind: crate::graph::NodeKind::File,
                language: analysis.language.clone(),
            };
            let source_idx = graph.add_node(source_node);

            for dep in analysis.deps {
                if let Some(resolved) = dep.resolved {
                    let target_node = crate::graph::Node {
                        path: resolved,
                        kind: crate::graph::NodeKind::File,
                        language: analysis.language.clone(),
                    };
                    let target_idx = graph.add_node(target_node);
                    graph.add_edge(
                        source_idx,
                        target_idx,
                        crate::graph::Edge {
                            kind: crate::graph::EdgeKind::Import,
                            line: Some(dep.line),
                            raw_path: Some(dep.raw_path),
                        },
                    );
                }
            }
        }

        Ok(())
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
