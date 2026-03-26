/// Java 文件依赖分析器
/// 支持：普通 import、static import、通配符 import
/// 不支持路径解析（Java 包路径无法直接映射到文件系统）
use std::path::Path;

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

// ─────────────────────────── 解析逻辑 ───────────────────────────

/// 解析单行 Java 源码，返回依赖条目（package 声明不作为依赖）
fn parse_line(line: &str, line_num: usize) -> Option<DepEntry> {
    // package 声明不记录为依赖
    if RE_PACKAGE.is_match(line) {
        return None;
    }

    // static import 优先匹配（避免被普通 import 正则误吞）
    if let Some(cap) = RE_STATIC_IMPORT.captures(line) {
        return Some(DepEntry {
            raw_path: cap[1].to_string(),
            resolved: None,
            line: line_num,
        });
    }

    // 普通 import（含通配符）
    if let Some(cap) = RE_IMPORT.captures(line) {
        return Some(DepEntry {
            raw_path: cap[1].to_string(),
            resolved: None,
            line: line_num,
        });
    }

    None
}

// ─────────────────────────── 分析器实现 ───────────────────────────

pub struct JavaAnalyzer;

impl JavaAnalyzer {
    pub fn new() -> Self {
        Self
    }

    /// 解析 Java 源码内容，返回所有 import 条目
    pub fn parse_imports(content: &str) -> Vec<DepEntry> {
        content
            .lines()
            .enumerate()
            .filter_map(|(i, line)| parse_line(line, i + 1))
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
        let deps = Self::parse_imports(&content);

        Ok(FileAnalysis {
            path: path.to_path_buf(),
            language: Language::Java,
            deps,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_import() {
        let content = "import com.example.Foo;";
        let imports = JavaAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "com.example.Foo");
        assert!(imports[0].resolved.is_none());
    }

    #[test]
    fn test_static_import() {
        let content = "import static com.example.Foo.method;";
        let imports = JavaAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "com.example.Foo.method");
        assert!(imports[0].resolved.is_none());
    }

    #[test]
    fn test_wildcard_import() {
        let content = "import com.example.*;";
        let imports = JavaAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 1);
        assert_eq!(imports[0].raw_path, "com.example.*");
    }

    #[test]
    fn test_package_declaration_not_imported() {
        // package 声明不应被识别为 import
        let content = "package com.example;";
        let imports = JavaAnalyzer::parse_imports(content);
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
        let imports = JavaAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 4);
        assert_eq!(imports[0].raw_path, "java.util.List");
        assert_eq!(imports[1].raw_path, "java.util.ArrayList");
        assert_eq!(imports[2].raw_path, "java.util.Collections.sort");
        assert_eq!(imports[3].raw_path, "com.example.model.*");
    }

    #[test]
    fn test_line_numbers() {
        let content = "package com.example;\nimport com.example.Foo;\nimport com.example.Bar;";
        let imports = JavaAnalyzer::parse_imports(content);
        assert_eq!(imports.len(), 2);
        assert_eq!(imports[0].line, 2);
        assert_eq!(imports[1].line, 3);
    }
}
