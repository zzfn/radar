/// Python 文件依赖分析器
/// 支持：import foo、from foo import bar、相对导入（from . import utils）等
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;

use super::{Analyzer, DepEntry, FileAnalysis};
use crate::error::Result;
use crate::graph::Language;

// ───────────────────────────── 正则定义 ─────────────────────────────

/// 匹配 `import foo` 或 `import foo.bar.baz`
static RE_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*import\s+([\w.]+)").unwrap()
});

/// 匹配 `from foo import bar` 或 `from . import bar` 或 `from ..utils import helper`
/// 捕获组 1：点前缀（相对层级），捕获组 2：模块名（可为空），捕获组 3：导入的符号列表
static RE_FROM_IMPORT: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*from\s+(\.+)([\w.]*)\s+import\s+(.+)").unwrap()
});

/// 匹配绝对 `from foo import bar`
static RE_FROM_IMPORT_ABS: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\s*from\s+([\w][\w.]*)\s+import\s+(.+)").unwrap()
});

// ─────────────────────────── 路径解析 ───────────────────────────

/// 将相对导入转换为绝对路径
/// - dots：`.` 表示当前目录，`..` 表示上级目录
/// - module：点后面的模块名，如 `from ..utils import foo` 中的 `utils`
/// - source_file：当前文件路径
fn resolve_relative_import(source_file: &Path, dots: &str, module: &str) -> Option<PathBuf> {
    let base_dir = source_file.parent()?;

    // 根据点的数量确定起始目录
    // `.` = 当前包目录（文件所在目录），`..` = 上级目录
    let levels = dots.len().saturating_sub(1);
    let mut start_dir = base_dir.to_path_buf();
    for _ in 0..levels {
        start_dir = start_dir.parent()?.to_path_buf();
    }

    if module.is_empty() {
        // `from . import foo` —— 返回包目录本身
        return Some(start_dir);
    }

    // 将 module 中的 `.` 替换为路径分隔符
    let rel_path = module.replace('.', std::path::MAIN_SEPARATOR_STR);
    let candidate = start_dir.join(&rel_path);

    // 尝试目录（包）
    if candidate.is_dir() {
        return Some(candidate);
    }

    // 尝试 .py 文件
    let with_py = candidate.with_extension("py");
    if with_py.exists() {
        return Some(with_py);
    }

    // 路径不存在则返回 None，避免将幽灵路径插入依赖图
    None
}

// ─────────────────────────── 解析逻辑 ───────────────────────────

/// 解析单行 Python 源码，返回 (raw_path, resolved, line_num) 列表
fn parse_line(line: &str, line_num: usize, source_file: &Path) -> Vec<DepEntry> {
    let mut entries = Vec::new();

    // 相对导入：from . import foo 或 from ..utils import bar
    if let Some(cap) = RE_FROM_IMPORT.captures(line) {
        let dots = &cap[1];
        let module = cap[2].trim();
        let symbols_raw = cap[3].trim();

        // raw_path 用模块名（含点前缀表示层级）
        let raw_path = if module.is_empty() {
            format!("{}", dots)
        } else {
            format!("{}{}", dots, module)
        };

        let resolved = resolve_relative_import(source_file, dots, module);

        // 每个导入符号单独一条 entry，但 resolved 指向同一模块
        // 这里仅为每行记录一条 entry（模块级别）
        let symbols: Vec<&str> = symbols_raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        for sym in &symbols {
            entries.push(DepEntry {
                raw_path: format!("{} -> {}", raw_path, sym),
                resolved: resolved.clone(),
                line: line_num,
            });
        }

        // 若没解析到符号，至少记录模块本身
        if entries.is_empty() {
            entries.push(DepEntry {
                raw_path,
                resolved,
                line: line_num,
            });
        }

        return entries;
    }

    // 绝对 from import：from foo import bar
    if let Some(cap) = RE_FROM_IMPORT_ABS.captures(line) {
        let module = cap[1].trim().to_string();
        let symbols_raw = cap[2].trim();

        let symbols: Vec<&str> = symbols_raw
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .collect();

        for sym in &symbols {
            entries.push(DepEntry {
                raw_path: format!("{} -> {}", module, sym),
                resolved: None, // 绝对导入不做路径解析
                line: line_num,
            });
        }

        if entries.is_empty() {
            entries.push(DepEntry {
                raw_path: module,
                resolved: None,
                line: line_num,
            });
        }

        return entries;
    }

    // 普通 import：import foo 或 import foo.bar.baz
    if let Some(cap) = RE_IMPORT.captures(line) {
        let module = cap[1].trim().to_string();
        entries.push(DepEntry {
            raw_path: module,
            resolved: None, // 绝对导入不做路径解析
            line: line_num,
        });
    }

    entries
}

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct PythonAnalyzer;

impl PythonAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 解析 Python 源码内容，返回所有依赖条目
    pub fn parse_imports(content: &str, source_file: &Path) -> Vec<DepEntry> {
        content
            .lines()
            .enumerate()
            .flat_map(|(i, line)| parse_line(line, i + 1, source_file))
            .collect()
    }
}

impl Default for PythonAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl Analyzer for PythonAnalyzer {
    fn can_handle(&self, path: &Path) -> bool {
        matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("py" | "pyw")
        )
    }

    fn analyze_file(&self, path: &Path) -> Result<FileAnalysis> {
        let content = std::fs::read_to_string(path)?;
        let deps = Self::parse_imports(&content, path);

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: Language::Python,
            deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    /// 辅助函数：用虚拟路径解析单行
    fn parse(line: &str) -> Vec<DepEntry> {
        parse_line(line, 1, Path::new("/project/src/foo.py"))
    }

    #[test]
    fn test_import_os() {
        let entries = parse("import os");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "os");
        assert!(entries[0].resolved.is_none());
    }

    #[test]
    fn test_from_pathlib_import_path() {
        let entries = parse("from pathlib import Path");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "pathlib -> Path");
        assert!(entries[0].resolved.is_none());
    }

    #[test]
    fn test_from_dot_import_utils() {
        // from . import utils —— 相对导入，指向当前目录
        let entries = parse("from . import utils");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, ". -> utils");
        // resolved 应指向 /project/src（当前目录）
        let resolved = entries[0].resolved.as_ref().unwrap();
        assert_eq!(resolved, Path::new("/project/src"));
    }

    #[test]
    fn test_from_dotdot_base_import_base() {
        // from ..base import Base —— 相对导入，路径在测试文件系统中不存在，resolved 应为 None
        let entries = parse("from ..base import Base");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "..base -> Base");
        assert!(entries[0].resolved.is_none());
    }

    #[test]
    fn test_import_dotted_path() {
        // import foo.bar.baz —— raw_path 为 "foo.bar.baz"
        let entries = parse("import foo.bar.baz");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, "foo.bar.baz");
        assert!(entries[0].resolved.is_none());
    }

    #[test]
    fn test_from_import_multiple_symbols() {
        // from os.path import join, exists
        let entries = parse("from os.path import join, exists");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].raw_path, "os.path -> join");
        assert_eq!(entries[1].raw_path, "os.path -> exists");
    }

    #[test]
    fn test_relative_import_with_module() {
        // from .utils import helper —— 路径在测试文件系统中不存在，resolved 应为 None
        let entries = parse("from .utils import helper");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].raw_path, ".utils -> helper");
        assert!(entries[0].resolved.is_none());
    }
}
