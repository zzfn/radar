/// Java 文件依赖分析器
/// 支持：普通 import、static import、通配符 import，以及包名→文件路径解析
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;

use super::{Analyzer, DepEntry, FileAnalysis};
use crate::error::Result;
use crate::graph::Language;

// ───────────────────────────── 正则定义 ─────────────────────────────

/// 匹配 package 声明：package com.example;
static RE_PACKAGE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*package\s+([\w.]+)\s*;").unwrap()
});

/// 匹配普通 import（含通配符）：import com.example.Foo; 或 import com.example.*;
static RE_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*import\s+([\w.]+(?:\.\*)?)\s*;").unwrap()
});

/// 匹配 static import：import static com.example.Foo.method;
static RE_STATIC_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*import\s+static\s+([\w.]+(?:\.\*)?)\s*;").unwrap()
});

// ─────────────────────────── 路径解析 ───────────────────────────

/// 是否为 JDK/Android/Kotlin 标准库包（不在项目源码树中）
fn is_stdlib(pkg: &str) -> bool {
    matches!(
        pkg.split('.').next().unwrap_or(""),
        "java" | "javax" | "sun" | "com.sun" | "android" | "kotlin" | "org.w3c" | "org.xml"
    ) || pkg.starts_with("com.sun.")
        || pkg.starts_with("org.w3c.")
        || pkg.starts_with("org.xml.")
}

/// 在给定的多个 source root 中查找 Java 包路径对应的文件/目录。
/// 依次尝试每个 root，第一个命中即返回。
///
/// - `com.example.Foo`  → `{root}/com/example/Foo.java`（文件）
/// - `com.example.*`   → `{root}/com/example/`（目录）
/// - `com.example.Foo.method`（static import）→ 先尝试 Foo.java，再尝试目录
fn resolve_java_import_in(root: &Path, import_path: &str) -> Option<PathBuf> {
    // 通配符 import：com.example.* → com/example/
    if import_path.ends_with(".*") {
        let pkg = &import_path[..import_path.len() - 2];
        let dir = root.join(pkg.replace('.', "/"));
        return if dir.is_dir() { Some(dir) } else { None };
    }

    // 普通 import：将点路径转为斜线，尝试 .java 文件
    let rel_path = import_path.replace('.', "/");
    let candidate = root.join(format!("{rel_path}.java"));
    if candidate.is_file() {
        return Some(candidate);
    }

    // static import：com.example.Foo.method → 取掉最后一段再试
    let parts: Vec<&str> = import_path.split('.').collect();
    if parts.len() >= 2 {
        let without_last = parts[..parts.len() - 1].join("/");
        let fallback = root.join(format!("{without_last}.java"));
        if fallback.is_file() {
            return Some(fallback);
        }
        // 还可能是内部类：com.example.Outer.Inner → Outer.java
        if parts.len() >= 3 {
            let without_two = parts[..parts.len() - 2].join("/");
            let fallback2 = root.join(format!("{without_two}.java"));
            if fallback2.is_file() {
                return Some(fallback2);
            }
        }
    }

    None
}

/// 在多个 source root 中解析 Java import，跳过 stdlib，依次尝试每个 root。
fn resolve_java_import(roots: &[PathBuf], import_path: &str) -> Option<PathBuf> {
    if is_stdlib(import_path) {
        return None;
    }
    roots.iter().find_map(|root| resolve_java_import_in(root, import_path))
}

/// 自动发现项目下所有 Maven/Gradle 风格的 Java source root（`src/main/java`）。
/// 如果给定路径本身不是标准 source root，也将其作为候选加入。
fn discover_java_roots(start: &Path) -> Vec<PathBuf> {
    use std::collections::HashSet;
    let mut roots: Vec<PathBuf> = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();

    // 从 start 向上找到项目根（含 pom.xml / build.gradle 的最高祖先目录）
    let project_root = {
        let mut cur = start;
        let mut found = start;
        loop {
            if cur.join("pom.xml").exists() || cur.join("build.gradle").exists() || cur.join("settings.gradle").exists() {
                found = cur;
            }
            match cur.parent() {
                Some(p) => cur = p,
                None => break,
            }
        }
        found
    };

    // 在项目根下递归搜索所有 src/main/java 目录（最多 6 层）
    fn walk(dir: &Path, depth: u32, roots: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>) {
        if depth == 0 { return; }
        let src_main_java = dir.join("src/main/java");
        if src_main_java.is_dir() {
            if seen.insert(src_main_java.clone()) {
                roots.push(src_main_java);
            }
            return; // 找到了，不再向子目录递归
        }
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_dir() {
                    walk(&p, depth - 1, roots, seen);
                }
            }
        }
    }

    walk(project_root, 6, &mut roots, &mut seen);

    // 如果没找到任何 src/main/java，退回到 start 本身
    if roots.is_empty() {
        roots.push(start.to_path_buf());
    }

    roots
}

// ─────────────────────────── 解析逻辑 ───────────────────────────

/// 解析单行 Java 源码，返回依赖条目（package 声明不作为依赖）
fn parse_line(line: &str, line_num: usize) -> Option<(String, usize)> {
    // package 声明不记录为依赖
    if RE_PACKAGE.is_match(line) {
        return None;
    }

    // static import 优先匹配（避免被普通 import 正则误吞）
    if let Some(cap) = RE_STATIC_IMPORT.captures(line) {
        return Some((cap[1].to_string(), line_num));
    }

    // 普通 import（含通配符）
    if let Some(cap) = RE_IMPORT.captures(line) {
        return Some((cap[1].to_string(), line_num));
    }

    None
}

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct JavaAnalyzer {
    /// 所有可用的 Java source root（支持多模块项目）
    roots: Vec<PathBuf>,
}

impl JavaAnalyzer {
    pub fn new() -> Self {
        Self { roots: vec![] }
    }

    /// 指定起始目录，自动发现项目下所有 source root
    pub fn with_root(root: &Path) -> Self {
        Self {
            roots: discover_java_roots(root),
        }
    }

    /// 解析 Java 源码内容，返回所有 import 条目
    pub fn parse_imports(content: &str, roots: &[PathBuf]) -> Vec<DepEntry> {
        content
            .lines()
            .enumerate()
            .filter_map(|(i, line)| parse_line(line, i + 1))
            .map(|(raw_path, line)| {
                let resolved = resolve_java_import(roots, &raw_path);
                DepEntry { raw_path, resolved, line }
            })
            .collect()
    }
}

impl Default for JavaAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for JavaAnalyzer {
    fn can_handle(&self, path: &Path) -> bool {
        matches!(path.extension().and_then(|e| e.to_str()), Some("java"))
    }

    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis> {
        let content = std::fs::read_to_string(path)?;
        let deps = Self::parse_imports(&content, &self.roots);

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: Language::Java,
            deps,
        })
    }

    /// 覆盖默认实现：在遍历目录前先用带 root 的实例初始化
    fn analyze_dir(
        &self,
        root: &Path,
        graph: &mut crate::graph::DependencyGraph,
        opts: &crate::analyzer::FilterOpts,
    ) -> Result<()> {
        let analyzer = JavaAnalyzer::with_root(root);
        // 复用父 trait 默认逻辑的实现，通过带 root 的新实例调用
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
                    if !inc.is_match(rel) {
                        return false;
                    }
                }
                if let Some(ref exc) = exclude_set {
                    if exc.is_match(rel) {
                        return false;
                    }
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
                language: Language::Java,
            };
            let source_idx = graph.add_node(source_node);

            for dep in analysis.deps {
                if let Some(resolved) = dep.resolved {
                    // 通配符 import 解析到目录，展开为目录内所有 .java 文件
                    let target_paths: Vec<PathBuf> = if resolved.is_dir() {
                        match std::fs::read_dir(&resolved) {
                            Ok(rd) => rd
                                .filter_map(|e| e.ok())
                                .map(|e| e.path())
                                .filter(|p| {
                                    p.is_file()
                                        && p.extension().and_then(|x| x.to_str()) == Some("java")
                                })
                                .collect(),
                            Err(_) => vec![resolved],
                        }
                    } else {
                        vec![resolved]
                    };

                    for target_path in target_paths {
                        let target_node = crate::graph::Node {
                            path: target_path,
                            kind: crate::graph::NodeKind::File,
                            language: Language::Java,
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

    fn parse(content: &str) -> Vec<DepEntry> {
        JavaAnalyzer::parse_imports(content, &[])
    }

    #[test]
    fn test_normal_import() {
        let imports = parse("import com.example.Foo;");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "com.example.Foo");
    }

    #[test]
    fn test_static_import() {
        let imports = parse("import static com.example.Foo.method;");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "com.example.Foo.method");
    }

    #[test]
    fn test_wildcard_import() {
        let imports = parse("import com.example.*;");
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "com.example.*");
    }

    #[test]
    fn test_package_declaration_not_imported() {
        let imports = parse("package com.example;");
        assert!(imports.is_empty());
    }

    #[test]
    fn test_full_java_file() {
        let content = r#"
package com.example.app;

import java.util.List;
import java.util.ArrayList;
import static java.util.Collections.sort;
import com.example.model.*;

public class Main {}
"#;
        let imports = parse(content);
        assert_eq!(imports.len(), 4);
        assert_eq!(imports[0].raw_path, "java.util.List");
        assert_eq!(imports[1].raw_path, "java.util.ArrayList");
        assert_eq!(imports[2].raw_path, "java.util.Collections.sort");
        assert_eq!(imports[3].raw_path, "com.example.model.*");
    }

    #[test]
    fn test_line_numbers() {
        let content =
            "package com.example;\nimport com.example.Foo;\nimport com.example.Bar;";
        let imports = parse(content);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].line, 2);
        assert_eq!(imports[1].line, 3);
    }

    #[test]
    fn test_stdlib_not_resolved() {
        // java.* / javax.* 不应解析为本地文件
        assert!(is_stdlib("java.util.List"));
        assert!(is_stdlib("javax.servlet.http.HttpServlet"));
        assert!(!is_stdlib("com.example.Foo"));
    }

    #[test]
    fn test_resolve_wildcard_nonexistent() {
        // 不存在的目录应返回 None
        let result = resolve_java_import(&[PathBuf::from("/nonexistent")], "com.example.*");
        assert!(result.is_none());
    }

    #[test]
    fn test_resolve_file_nonexistent() {
        let result = resolve_java_import(&[PathBuf::from("/nonexistent")], "com.example.Foo");
        assert!(result.is_none());
    }
}
