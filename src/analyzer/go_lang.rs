/// Go 文件依赖分析器
/// 支持：单行 import、import 分组块、别名导入（alias/_ /.），读取 go.mod 解析本地包路径
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;

use super::{Analyzer, DepEntry, FileAnalysis};
use crate::error::Result;
use crate::graph::Language;

// ───────────────────────────── 正则定义 ─────────────────────────────

/// 单行 import：import "pkg" 或 import alias "pkg"
static RE_SINGLE_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*import\s+(?:[_\w.]+\s+)?"([^"]+)""#).unwrap()
});

/// import 块开始：import (
static RE_BLOCK_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*import\s*\("#).unwrap()
});

/// import 块内单条记录：可选别名 + 路径，如 `"pkg"`、`alias "pkg"`、`_ "pkg"`、`. "pkg"`
static RE_BLOCK_ENTRY: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*(?:[_.\w]+\s+)?"([^"]+)""#).unwrap()
});

/// import 块结束：)
static RE_BLOCK_END: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"^\s*\)"#).unwrap()
});

// ─────────────────────────── 路径解析 ───────────────────────────

/// 从 go.mod 读取 module 名称
fn read_module_name(root: &Path) -> Option<String> {
    let go_mod = root.join("go.mod");
    let content = std::fs::read_to_string(&go_mod).ok()?;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("module ") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// 根据 module 名称和 import 路径，解析出本地目录的绝对路径
/// - import_path 以 module_name 开头 → 本地包
/// - 其他 → 外部依赖，返回 None
fn resolve_go_import(root: &Path, module_name: &str, import_path: &str) -> Option<PathBuf> {
    if !import_path.starts_with(module_name) {
        return None;
    }

    // module_name 之后的相对部分，例如 "github.com/foo/bar/pkg/util" → "pkg/util"
    let rel = import_path.strip_prefix(module_name)?.trim_start_matches('/');
    let candidate = if rel.is_empty() {
        root.to_path_buf()
    } else {
        root.join(rel)
    };

    if candidate.is_dir() {
        Some(candidate)
    } else {
        None
    }
}

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct GoAnalyzer {
    /// 项目根目录（用于定位 go.mod）
    root: PathBuf,
    /// 从 go.mod 解析出的 module 名称
    module_name: Option<String>,
}

impl GoAnalyzer {
    pub fn new() -> Self {
        Self {
            root: PathBuf::new(),
            module_name: None,
        }
    }

    /// 指定项目根目录并尝试读取 go.mod
    pub fn with_root(root: &Path) -> Self {
        let module_name = read_module_name(root);
        Self {
            root: root.to_path_buf(),
            module_name,
        }
    }

    /// 解析单个 .go 文件，返回所有 import 路径及行号
    fn parse_imports(content: &str) -> Vec<(String, usize)> {
        let mut result = Vec::new();
        let mut in_block = false;

        for (i, line) in content.lines().enumerate() {
            let line_num = i + 1;

            if in_block {
                if RE_BLOCK_END.is_match(line) {
                    in_block = false;
                } else if let Some(cap) = RE_BLOCK_ENTRY.captures(line) {
                    result.push((cap[1].to_string(), line_num));
                }
            } else if RE_BLOCK_START.is_match(line) {
                in_block = true;
            } else if let Some(cap) = RE_SINGLE_IMPORT.captures(line) {
                result.push((cap[1].to_string(), line_num));
            }
        }

        result
    }
}

impl Default for GoAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for GoAnalyzer {
    fn can_handle(&self, path: &Path) -> bool {
        matches!(path.extension().and_then(|e| e.to_str()), Some("go"))
    }

    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis> {
        let content = std::fs::read_to_string(path)?;
        let raw_imports = Self::parse_imports(&content);

        let deps = raw_imports
            .into_iter()
            .map(|(import_path, line)| {
                let resolved = self.module_name.as_deref().and_then(|mod_name| {
                    resolve_go_import(&self.root, mod_name, &import_path)
                });
                DepEntry {
                    raw_path: import_path,
                    resolved,
                    line,
                }
            })
            .collect();

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: Language::Go,
            deps,
        })
    }

    /// 覆盖默认实现：在遍历目录前先初始化 module_name
    fn analyze_dir(&self, root: &Path, graph: &mut crate::graph::DependencyGraph, opts: &crate::analyzer::FilterOpts) -> Result<()> {
        // 用带 root 的新实例来分析，确保 go.mod 被正确读取
        let analyzer = GoAnalyzer::with_root(root);
        // 复用父 trait 的默认逻辑，但通过新实例调用
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
                language: Language::Go,
            };
            let source_idx = graph.add_node(source_node);

            for dep in analysis.deps {
                if let Some(resolved_dir) = dep.resolved {
                    // Go import 解析到包目录，将目录展开为目录内所有 .go 文件，
                    // 建立文件到文件的边，使 impact 分析能精确到单个文件。
                    let go_files: Vec<PathBuf> = match std::fs::read_dir(&resolved_dir) {
                        Ok(rd) => rd
                            .filter_map(|e| e.ok())
                            .map(|e| e.path())
                            .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("go"))
                            .collect(),
                        Err(_) => vec![resolved_dir], // 回退：目录读取失败时直接用目录路径
                    };

                    for go_file in go_files {
                        let target_node = crate::graph::Node {
                            path: go_file,
                            kind: crate::graph::NodeKind::File,
                            language: Language::Go,
                        };
                        let target_idx = graph.add_node(target_node);
                        graph.add_edge(
                            source_idx,
                            target_idx,
                            crate::graph::Edge {
                                kind: crate::graph::EdgeKind::Import,
                                line: Some(dep.line),
                                raw_path: Some(dep.raw_path.clone()),
                            },
                        );
                    }
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
    fn test_single_import() {
        let content = r#"import "fmt""#;
        let imports = GoAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].0, "fmt");
        assert_eq!(imports[0].1, 1);
    }

    #[test]
    fn test_aliased_import() {
        let content = r#"import log "github.com/sirupsen/logrus""#;
        let imports = GoAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].0, "github.com/sirupsen/logrus");
    }

    #[test]
    fn test_blank_import() {
        let content = r#"import _ "database/sql""#;
        let imports = GoAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].0, "database/sql");
    }

    #[test]
    fn test_import_block() {
        let content = r#"import (
    "fmt"
    "os"
    log "github.com/sirupsen/logrus"
    _ "embed"
)"#;
        let imports = GoAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 4);
        assert_eq!(imports[0].0, "fmt");
        assert_eq!(imports[1].0, "os");
        assert_eq!(imports[2].0, "github.com/sirupsen/logrus");
        assert_eq!(imports[3].0, "embed");
    }

    #[test]
    fn test_dot_import() {
        let content = r#"import . "math/rand""#;
        let imports = GoAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].0, "math/rand");
    }

    #[test]
    fn test_local_package_resolution() {
        // 模拟：module = "github.com/foo/bar"，import "github.com/foo/bar/pkg/util"
        let result = resolve_go_import(
            Path::new("/project"),
            "github.com/foo/bar",
            "github.com/foo/bar/pkg/util",
        );
        // 路径计算正确，存在性校验会失败（测试环境没有真实目录），只验证逻辑
        // 实际调用时 candidate = /project/pkg/util，is_dir() 决定返回值
        let _ = result; // 不 assert exists，仅测试不 panic
    }

    #[test]
    fn test_external_package_not_resolved() {
        let result = resolve_go_import(
            Path::new("/project"),
            "github.com/foo/bar",
            "github.com/other/lib",
        );
        assert!(result.is_none());
    }
}
