/// 分析器模块：定义 Analyzer trait 和公共结构
pub mod file_dep;
pub mod fn_analyzer;
pub mod fn_builder;
pub mod go_lang;
pub mod java;
pub mod js_ts;
pub mod python;
pub mod rust_lang;
pub mod vue;

use std::path::{Path, PathBuf};

use globset::{Glob, GlobSet, GlobSetBuilder};

use crate::error::Result;
use crate::graph::{DependencyGraph, Language, Node, NodeKind};

/// 文件过滤选项（glob 模式）
#[derive(Debug, Default, Clone)]
pub struct FilterOpts {
    /// 只分析匹配这些 glob 的文件（空 = 全部）
    pub include: Vec<String>,
    /// 排除匹配这些 glob 的文件
    pub exclude: Vec<String>,
}

impl FilterOpts {
    /// 编译 include/exclude 为 GlobSet（编译失败的模式跳过）
    pub(crate) fn build_sets(&self) -> (Option<GlobSet>, Option<GlobSet>) {
        let include_set = if self.include.is_empty() {
            None
        } else {
            let mut b = GlobSetBuilder::new();
            for pat in &self.include {
                if let Ok(g) = Glob::new(pat) { b.add(g); }
            }
            b.build().ok()
        };
        let exclude_set = if self.exclude.is_empty() {
            None
        } else {
            let mut b = GlobSetBuilder::new();
            for pat in &self.exclude {
                if let Ok(g) = Glob::new(pat) { b.add(g); }
            }
            b.build().ok()
        };
        (include_set, exclude_set)
    }
}

/// 单个依赖条目（从文件中解析出的一条 import/use）
#[derive(Debug, Clone)]
pub struct DepEntry {
    /// 导入路径（原始字符串，如 `"../utils"` 或 `std::collections`）
    pub raw_path: String,
    /// 解析后的绝对路径（如果能解析）
    pub resolved: Option<PathBuf>,
    /// 导入发生的行号
    pub line: usize,
}

/// 文件分析结果
#[derive(Debug)]
pub struct FileAnalysis {
    /// 被分析的文件路径
    pub path: PathBuf,
    /// 检测到的语言
    pub language: Language,
    /// 解析出的所有依赖条目
    pub deps: Vec<DepEntry>,
}

/// 分析器 trait：所有语言分析器都实现此接口
pub trait Analyzer: Send + Sync {
    /// 判断是否能处理该文件
    fn can_handle(&self, path: &Path) -> bool;

    /// 分析单个文件，返回依赖列表
    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis>;

    /// 批量分析目录下的所有文件
    fn analyze_dir(&self, root: &Path, graph: &mut DependencyGraph, opts: &FilterOpts) -> Result<()> {
        use ignore::WalkBuilder;
        use rayon::prelude::*;

        let (include_set, exclude_set) = opts.build_sets();

        // 收集所有可处理的文件路径
        let files: Vec<PathBuf> = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.into_path())
            .filter(|p| self.can_handle(p))
            .filter(|p| {
                // 相对路径用于 glob 匹配
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

        // 并行分析所有文件
        let results: Vec<FileAnalysis> = files
            .par_iter()
            .filter_map(|p| self.analyze_file(p).ok())
            .collect();

        // 将分析结果合并进图
        for analysis in results {
            let source_node = Node {
                path: analysis.path.clone(),
                kind: NodeKind::File,
                language: analysis.language.clone(),
            };
            let source_idx = graph.add_node(source_node);

            for dep in analysis.deps {
                if let Some(resolved) = dep.resolved {
                    // 若 resolved 是目录（包），展开为目录内同语言的源文件，
                    // 建立文件到文件的精确边，使 impact 分析能定位到具体文件。
                    let target_paths: Vec<PathBuf> =
                        if resolved.is_dir() {
                            let ext = analysis.language.primary_extension();
                            match std::fs::read_dir(&resolved) {
                                Ok(rd) => rd
                                    .filter_map(|e| e.ok())
                                    .map(|e| e.path())
                                    .filter(|p| {
                                        p.is_file() && ext.map_or(true, |x| {
                                            p.extension().and_then(|e| e.to_str()) == Some(x)
                                        })
                                    })
                                    .collect(),
                                Err(_) => vec![resolved],
                            }
                        } else {
                            vec![resolved]
                        };

                    for target_path in target_paths {
                        let target_node = Node {
                            path: target_path,
                            kind: NodeKind::File,
                            language: analysis.language.clone(),
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

/// 根据语言枚举创建对应分析器
pub fn create_analyzer(lang: &Language) -> Box<dyn Analyzer> {
    match lang {
        Language::JavaScript | Language::TypeScript => {
            Box::new(js_ts::JsTsAnalyzer::new())
        }
        Language::Rust => Box::new(rust_lang::RustAnalyzer::new()),
        Language::Go => Box::new(go_lang::GoAnalyzer::new()),
        Language::Python => Box::new(python::PythonAnalyzer::new()),
        Language::Java => Box::new(java::JavaAnalyzer::new()),
        Language::Vue => Box::new(vue::VueAnalyzer::new()),
        _ => Box::new(file_dep::GenericAnalyzer::new()),
    }
}

/// 自动检测目录的主要语言（统计各扩展名的文件数量，取最多的已知语言）
pub fn detect_language(root: &Path) -> Language {
    use std::collections::HashMap;

    let mut ext_counts: HashMap<String, usize> = HashMap::new();
    for entry in ignore::Walk::new(root).flatten() {
        if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
            *ext_counts.entry(ext.to_lowercase()).or_insert(0) += 1;
        }
    }

    // 找出数量最多的已知语言扩展名
    let dominant = ext_counts
        .iter()
        .filter(|(ext, _)| {
            matches!(
                ext.as_str(),
                "rs" | "ts" | "tsx" | "js" | "jsx" | "py" | "go" | "java" | "vue"
            )
        })
        .max_by_key(|(_, &count)| count);

    match dominant.map(|(ext, _)| ext.as_str()) {
        Some("rs") => Language::Rust,
        Some("ts") | Some("tsx") => Language::TypeScript,
        Some("js") | Some("jsx") => Language::JavaScript,
        Some("py") => Language::Python,
        Some("go") => Language::Go,
        Some("java") => Language::Java,
        Some("vue") => Language::Vue,
        _ => Language::Unknown,
    }
}
