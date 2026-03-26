/// Vue 单文件组件（SFC）依赖分析器
/// 支持：提取 <script>/<script setup> 块交给 JsTsAnalyzer 解析，
///       识别 <template> 中大写字母开头的组件标签作为软依赖
/// v0.3 新增：@/ 别名解析（惯例映射到 src/）及 tsconfig.json paths 支持
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;

use super::js_ts::{read_tsconfig_paths, resolve_alias, JsTsAnalyzer};
use super::{Analyzer, DepEntry, FileAnalysis};
use crate::analyzer::file_dep::resolve_relative;
use crate::error::Result;
use crate::graph::Language;

// ───────────────────────────── 正则定义 ─────────────────────────────

/// 匹配 <script> 或 <script setup ...> 开始标签
static RE_SCRIPT_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)<script(\s[^>]*)?>").unwrap()
});

/// 匹配 </script> 结束标签
static RE_SCRIPT_END: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)</script>").unwrap()
});

/// 匹配 <template> 开始标签
static RE_TEMPLATE_START: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)<template(\s[^>]*)?>").unwrap()
});

/// 匹配 </template> 结束标签
static RE_TEMPLATE_END: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)</template>").unwrap()
});

/// 匹配 <template> 中大写字母开头的组件标签
/// 例如：<MyComponent />、<MyComponent>、</MyComponent>
static RE_COMPONENT_TAG: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"</?([A-Z][A-Za-z0-9]*)(?:\s[^>]*)?>").unwrap()
});

// ─────────────────────────── 解析逻辑 ───────────────────────────

/// 从 Vue SFC 内容中提取 script 块文本
fn extract_script_block(content: &str) -> Option<(String, usize)> {
    let mut in_script = false;
    let mut script_lines = Vec::new();
    let mut start_line = 0;

    for (i, line) in content.lines().enumerate() {
        if !in_script && RE_SCRIPT_START.is_match(line) {
            in_script = true;
            start_line = i + 1; // 1-based 行号
            continue; // 跳过 <script> 标签行本身
        }

        if in_script {
            if RE_SCRIPT_END.is_match(line) {
                break;
            }
            script_lines.push(line);
        }
    }

    if script_lines.is_empty() {
        None
    } else {
        Some((script_lines.join("\n"), start_line))
    }
}

/// 从 Vue SFC 内容中提取 template 块中的组件引用
/// 返回 (组件名, 行号) 列表
fn extract_template_components(content: &str) -> Vec<(String, usize)> {
    let mut in_template = false;
    let mut components = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for (i, line) in content.lines().enumerate() {
        let line_num = i + 1;

        if !in_template && RE_TEMPLATE_START.is_match(line) {
            in_template = true;
            continue;
        }

        if in_template {
            if RE_TEMPLATE_END.is_match(line) {
                in_template = false;
                continue;
            }

            // 提取所有大写开头的组件标签
            for cap in RE_COMPONENT_TAG.captures_iter(line) {
                let name = cap[1].to_string();
                // 去重，每个组件只记录首次出现
                if seen.insert(name.clone()) {
                    components.push((name, line_num));
                }
            }
        }
    }

    components
}

/// 尝试解析 @/ 开头的路径（先查 tsconfig paths，再 fallback 到 src/ 惯例）
fn resolve_at_alias(
    root: &Path,
    alias_map: &HashMap<String, Vec<String>>,
    import_path: &str,
) -> Option<PathBuf> {
    // 先尝试 tsconfig paths 别名
    if !alias_map.is_empty() {
        if let Some(p) = resolve_alias(root, alias_map, import_path) {
            return Some(p);
        }
    }

    // Fallback：惯例 @/ → src/
    if let Some(rel) = import_path.strip_prefix("@/") {
        let candidate = root.join("src").join(rel);
        if candidate.exists() {
            return Some(candidate);
        }
        for ext in &["vue", "ts", "tsx", "js", "jsx"] {
            let with_ext = candidate.with_extension(ext);
            if with_ext.exists() { return Some(with_ext); }
        }
        for ext in &["vue", "ts", "tsx", "js", "jsx"] {
            let index = candidate.join(format!("index.{}", ext));
            if index.exists() { return Some(index); }
        }
    }

    None
}

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct VueAnalyzer {
    /// tsconfig.json paths 别名映射
    alias_map: HashMap<String, Vec<String>>,
    /// 项目根目录
    root: PathBuf,
}

impl VueAnalyzer {
    pub fn new() -> Self {
        Self { alias_map: HashMap::new(), root: PathBuf::new() }
    }

    pub fn with_root(root: &Path) -> Self {
        let alias_map = read_tsconfig_paths(root);
        Self { alias_map, root: root.to_path_buf() }
    }

    /// 解析 Vue SFC 内容，返回依赖条目列表（仅测试使用）
    #[cfg(test)]
    pub fn parse_deps(content: &str, source_file: &Path) -> Vec<DepEntry> {
        // 无 root 时使用空 alias_map 的简化版本
        let analyzer = VueAnalyzer::new();
        analyzer.parse_deps_with_root(content, source_file)
    }

    /// 解析 Vue SFC 内容（支持别名）
    fn parse_deps_with_root(&self, content: &str, source_file: &Path) -> Vec<DepEntry> {
        let mut deps = Vec::new();

        // 1. 提取 script 块，交给 JS/TS 逻辑解析 import
        if let Some((script_content, _start_line)) = extract_script_block(content) {
            let script_deps: Vec<DepEntry> = script_content
                .lines()
                .enumerate()
                .flat_map(|(i, line)| {
                    let mut entries = JsTsAnalyzer::extract_line(line, i + 1);
                    // 路径解析：相对路径、别名路径
                    for entry in &mut entries {
                        if entry.raw_path.starts_with('.') {
                            entry.resolved = resolve_relative(source_file, &entry.raw_path);
                        } else if entry.raw_path.starts_with('@') {
                            entry.resolved = resolve_at_alias(&self.root, &self.alias_map, &entry.raw_path);
                        } else if !self.alias_map.is_empty() {
                            entry.resolved = resolve_alias(&self.root, &self.alias_map, &entry.raw_path);
                        }
                    }
                    entries
                })
                .collect();

            deps.extend(script_deps);
        }

        // 2. 提取 template 中的组件引用（软依赖，resolved = None）
        for (component_name, line_num) in extract_template_components(content) {
            deps.push(DepEntry {
                raw_path: component_name,
                resolved: None, // 无法直接解析组件路径
                line: line_num,
            });
        }

        deps
    }
}

impl Default for VueAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for VueAnalyzer {
    fn can_handle(&self, path: &Path) -> bool {
        matches!(path.extension().and_then(|e| e.to_str()), Some("vue"))
    }

    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis> {
        let content = std::fs::read_to_string(path)?;
        let deps = self.parse_deps_with_root(&content, path);

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: Language::Vue,
            deps,
        })
    }

    /// 覆盖默认实现：先读取 tsconfig.json paths，再分析文件
    fn analyze_dir(&self, root: &Path, graph: &mut crate::graph::DependencyGraph, opts: &crate::analyzer::FilterOpts) -> Result<()> {
        let analyzer = VueAnalyzer::with_root(root);
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
                language: Language::Vue,
            };
            let source_idx = graph.add_node(source_node);

            for dep in analysis.deps {
                if let Some(resolved) = dep.resolved {
                    let target_node = crate::graph::Node {
                        path: resolved,
                        kind: crate::graph::NodeKind::File,
                        language: Language::Vue,
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
    use std::path::Path;

    /// 辅助：解析 Vue SFC 内容
    fn parse(content: &str) -> Vec<DepEntry> {
        VueAnalyzer::parse_deps(content, Path::new("/project/src/App.vue"))
    }

    #[test]
    fn test_script_setup_import() {
        let content = r#"
<template>
  <div>Hello</div>
</template>

<script setup>
import { ref } from 'vue'
import MyButton from './components/MyButton.vue'
</script>
"#;
        let deps = parse(content);
        // 应包含 vue 和 ./components/MyButton.vue 两个 import
        let paths: Vec<&str> = deps.iter().map(|d| d.raw_path.as_str()).collect();
        assert!(paths.contains(&"vue"), "应包含 vue import");
        assert!(
            paths.contains(&"./components/MyButton.vue"),
            "应包含 MyButton.vue import"
        );
    }

    #[test]
    fn test_template_component_references() {
        let content = r#"
<template>
  <div>
    <MyButton @click="handleClick" />
    <UserCard :user="user" />
    <div class="wrapper">普通 HTML 标签不应被识别</div>
  </div>
</template>

<script setup>
import MyButton from './MyButton.vue'
import UserCard from './UserCard.vue'
</script>
"#;
        let deps = parse(content);

        // template 中识别到的组件
        let component_deps: Vec<&DepEntry> =
            deps.iter().filter(|d| d.resolved.is_none() && !d.raw_path.starts_with('.')).collect();
        let component_names: Vec<&str> =
            component_deps.iter().map(|d| d.raw_path.as_str()).collect();

        assert!(
            component_names.contains(&"MyButton"),
            "应识别 MyButton 组件"
        );
        assert!(
            component_names.contains(&"UserCard"),
            "应识别 UserCard 组件"
        );
    }

    #[test]
    fn test_no_lowercase_tags_in_template() {
        let content = r#"
<template>
  <div>
    <span>text</span>
    <input type="text" />
    <MyComponent />
  </div>
</template>

<script setup>
</script>
"#;
        let deps = parse(content);
        let names: Vec<&str> = deps.iter().map(|d| d.raw_path.as_str()).collect();

        // 小写标签不应被识别
        assert!(!names.contains(&"div"), "div 不应被识别为组件");
        assert!(!names.contains(&"span"), "span 不应被识别为组件");
        assert!(!names.contains(&"input"), "input 不应被识别为组件");
        // 大写开头的应被识别
        assert!(names.contains(&"MyComponent"), "MyComponent 应被识别");
    }

    #[test]
    fn test_script_without_setup() {
        let content = r#"
<script>
import axios from 'axios'
export default {
  name: 'MyComp'
}
</script>
"#;
        let deps = parse(content);
        let paths: Vec<&str> = deps.iter().map(|d| d.raw_path.as_str()).collect();
        assert!(paths.contains(&"axios"), "应识别普通 script 块中的 import");
    }

    #[test]
    fn test_component_deduplication() {
        let content = r#"
<template>
  <div>
    <MyButton />
    <MyButton />
    <MyButton />
  </div>
</template>

<script setup>
</script>
"#;
        let deps = parse(content);
        let button_count = deps
            .iter()
            .filter(|d| d.raw_path == "MyButton")
            .count();
        assert_eq!(button_count, 1, "重复出现的组件只记录一次");
    }
}
