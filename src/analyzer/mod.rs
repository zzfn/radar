/// 分析器模块：定义 Analyzer trait 和公共结构
pub mod file_dep;
pub mod js_ts;
pub mod rust_lang;

use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::graph::{DependencyGraph, Language, Node, NodeKind};

/// 单个依赖条目（从文件中解析出的一条 import/use）
#[derive(Debug, Clone)]
pub struct DepEntry {
    /// 导入路径（原始字符串，如 `"../utils"` 或 `std::collections`）
    pub raw_path: String,
    /// 解析后的绝对路径（如果能解析）
    pub resolved: Option<PathBuf>,
    /// 导入发生的行号
    pub line: usize,
    /// 是否是类型导入（TS 中的 `import type`）
    pub is_type_only: bool,
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
    /// 返回该分析器支持的语言
    fn language(&self) -> Language;

    /// 判断是否能处理该文件
    fn can_handle(&self, path: &Path) -> bool;

    /// 分析单个文件，返回依赖列表
    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis>;

    /// 批量分析目录下的所有文件
    fn analyze_dir(&self, root: &Path, graph: &mut DependencyGraph) -> Result<()> {
        use ignore::WalkBuilder;
        use rayon::prelude::*;

        // 收集所有可处理的文件路径
        let files: Vec<PathBuf> = WalkBuilder::new(root)
            .hidden(false)
            .git_ignore(true)
            .build()
            .filter_map(|entry| entry.ok())
            .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
            .map(|e| e.into_path())
            .filter(|p| self.can_handle(p))
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
                    let target_node = Node {
                        path: resolved,
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
                            raw_path: Some(dep.raw_path),
                        },
                    );
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
        .filter(|(ext, _)| matches!(ext.as_str(), "rs" | "ts" | "tsx" | "js" | "jsx" | "py"))
        .max_by_key(|(_, &count)| count);

    match dominant.map(|(ext, _)| ext.as_str()) {
        Some("rs") => Language::Rust,
        Some("ts") | Some("tsx") => Language::TypeScript,
        Some("js") | Some("jsx") => Language::JavaScript,
        Some("py") => Language::Python,
        _ => Language::Unknown,
    }
}
