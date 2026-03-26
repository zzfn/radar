/// 基于 tree-sitter 的函数级分析器
use std::path::{Path, PathBuf};

use tree_sitter::{Language as TsLanguage, Parser, Query, QueryCursor, Tree};

use crate::error::Result;
use crate::graph::Language;

/// 单个文件中提取的函数定义
#[derive(Debug, Clone)]
pub struct FnDef {
    pub name: String,
    pub start_line: usize,
    pub end_line: usize,
    /// 节点在源码中的字节范围（用于判断调用位于哪个函数内部）
    pub start_byte: usize,
    pub end_byte: usize,
}

/// 单个文件中提取的调用点
#[derive(Debug, Clone)]
pub struct CallSite {
    /// 被调函数名
    pub callee_name: String,
    /// 调用发生的字节偏移（用于定位所在函数）
    pub byte_offset: usize,
    /// 调用发生的行号
    pub line: usize,
}

/// 单文件分析结果
#[derive(Debug)]
pub struct FileFunctions {
    pub path: PathBuf,
    pub language: Language,
    pub defs: Vec<FnDef>,
    pub calls: Vec<CallSite>,
}

/// 解析单个文件，返回函数定义和调用点
pub fn analyze_file_functions(path: &Path) -> Result<FileFunctions> {
    let lang = detect_lang(path);
    let source = std::fs::read_to_string(path)?;

    match lang {
        Language::Rust => parse_rust(path, &source),
        Language::JavaScript => parse_js_ts(path, &source, &Language::JavaScript),
        Language::TypeScript => parse_js_ts(path, &source, &Language::TypeScript),
        Language::Go => parse_go(path, &source),
        Language::Python => parse_python(path, &source),
        _ => Ok(FileFunctions {
            path: path.to_path_buf(),
            language: lang,
            defs: vec![],
            calls: vec![],
        }),
    }
}

fn detect_lang(path: &Path) -> Language {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => Language::Rust,
        Some("ts" | "tsx" | "mts") => Language::TypeScript,
        Some("js" | "jsx" | "mjs" | "cjs") => Language::JavaScript,
        Some("go") => Language::Go,
        Some("py") => Language::Python,
        _ => Language::Unknown,
    }
}

fn parse_rust(path: &Path, source: &str) -> Result<FileFunctions> {
    let ts_lang = tree_sitter_rust::language();
    let mut parser = Parser::new();
    parser.set_language(&ts_lang)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter 解析失败: {}", path.display()))?;

    // 查询函数定义
    let def_query = Query::new(
        &ts_lang,
        r#"(function_item name: (identifier) @fn_name) @fn_def"#,
    )?;

    // 查询函数调用
    let call_query = Query::new(
        &ts_lang,
        r#"[
          (call_expression function: (identifier) @callee)
          (call_expression function: (scoped_identifier name: (identifier) @callee))
          (call_expression function: (field_expression field: (field_identifier) @callee))
        ]"#,
    )?;

    extract_defs_and_calls(path, source, &ts_lang, &tree, &def_query, &call_query, Language::Rust)
}

fn parse_go(path: &Path, source: &str) -> Result<FileFunctions> {
    let ts_lang = tree_sitter_go::language();
    let mut parser = Parser::new();
    parser.set_language(&ts_lang)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter 解析失败: {}", path.display()))?;

    let def_query = Query::new(
        &ts_lang,
        r#"[
          (function_declaration name: (identifier) @fn_name) @fn_def
          (method_declaration name: (field_identifier) @fn_name) @fn_def
        ]"#,
    )?;

    let call_query = Query::new(
        &ts_lang,
        r#"[
          (call_expression function: (identifier) @callee)
          (call_expression function: (selector_expression field: (field_identifier) @callee))
        ]"#,
    )?;

    extract_defs_and_calls(path, source, &ts_lang, &tree, &def_query, &call_query, Language::Go)
}

fn parse_js_ts(path: &Path, source: &str, lang: &Language) -> Result<FileFunctions> {
    let ts_lang: TsLanguage = match lang {
        Language::TypeScript => tree_sitter_typescript::language_typescript(),
        _ => tree_sitter_javascript::language(),
    };

    let mut parser = Parser::new();
    parser.set_language(&ts_lang)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter 解析失败: {}", path.display()))?;

    let def_query = Query::new(
        &ts_lang,
        r#"[
          (function_declaration name: (identifier) @fn_name) @fn_def
          (method_definition key: (property_identifier) @fn_name) @fn_def
          (variable_declarator
            name: (identifier) @fn_name
            value: [(function_expression) (arrow_function)]) @fn_def
        ]"#,
    )?;

    let call_query = Query::new(
        &ts_lang,
        r#"[
          (call_expression function: (identifier) @callee)
          (call_expression function: (member_expression property: (property_identifier) @callee))
        ]"#,
    )?;

    extract_defs_and_calls(path, source, &ts_lang, &tree, &def_query, &call_query, lang.clone())
}

fn parse_python(path: &Path, source: &str) -> Result<FileFunctions> {
    let ts_lang = tree_sitter_python::language();
    let mut parser = Parser::new();
    parser.set_language(&ts_lang)?;
    let tree = parser
        .parse(source, None)
        .ok_or_else(|| anyhow::anyhow!("tree-sitter 解析失败: {}", path.display()))?;

    let def_query = Query::new(
        &ts_lang,
        r#"(function_definition name: (identifier) @fn_name) @fn_def"#,
    )?;

    let call_query = Query::new(
        &ts_lang,
        r#"[
          (call function: (identifier) @callee)
          (call function: (attribute attribute: (identifier) @callee))
        ]"#,
    )?;

    extract_defs_and_calls(
        path,
        source,
        &ts_lang,
        &tree,
        &def_query,
        &call_query,
        Language::Python,
    )
}

/// 公共提取函数：从已解析的语法树中提取函数定义和调用点
fn extract_defs_and_calls(
    path: &Path,
    source: &str,
    _language: &TsLanguage,
    tree: &Tree,
    def_query: &Query,
    call_query: &Query,
    lang: Language,
) -> Result<FileFunctions> {
    let src_bytes = source.as_bytes();
    let root = tree.root_node();

    let def_name_idx = def_query.capture_index_for_name("fn_name").unwrap();
    let def_node_idx = def_query.capture_index_for_name("fn_def").unwrap();
    let callee_idx = call_query.capture_index_for_name("callee").unwrap();

    // 提取函数定义
    let mut defs: Vec<FnDef> = vec![];
    let mut cursor = QueryCursor::new();
    for m in cursor.matches(def_query, root, src_bytes) {
        let mut name: Option<String> = None;
        let mut start_line = 0usize;
        let mut end_line = 0usize;
        let mut start_byte = 0usize;
        let mut end_byte = 0usize;
        for cap in m.captures {
            if cap.index == def_name_idx {
                if let Ok(t) = cap.node.utf8_text(src_bytes) {
                    name = Some(t.to_string());
                }
            }
            if cap.index == def_node_idx {
                start_line = cap.node.start_position().row + 1;
                end_line = cap.node.end_position().row + 1;
                start_byte = cap.node.start_byte();
                end_byte = cap.node.end_byte();
            }
        }
        if let Some(n) = name {
            defs.push(FnDef {
                name: n,
                start_line,
                end_line,
                start_byte,
                end_byte,
            });
        }
    }

    // 提取调用点
    let mut calls: Vec<CallSite> = vec![];
    let mut cursor2 = QueryCursor::new();
    for m in cursor2.matches(call_query, root, src_bytes) {
        for cap in m.captures {
            if cap.index == callee_idx {
                if let Ok(t) = cap.node.utf8_text(src_bytes) {
                    calls.push(CallSite {
                        callee_name: t.to_string(),
                        byte_offset: cap.node.start_byte(),
                        line: cap.node.start_position().row + 1,
                    });
                }
            }
        }
    }

    Ok(FileFunctions {
        path: path.to_path_buf(),
        language: lang,
        defs,
        calls,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn 提取rust简单函数定义() {
        let source = r#"
fn foo() {
    bar();
}

fn bar() {}
"#;
        // 用临时路径模拟
        let path = std::path::Path::new("test.rs");
        let result = parse_rust(path, source).expect("解析失败");
        let names: Vec<&str> = result.defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"foo"), "未提取到 foo，实际: {:?}", names);
        assert!(names.contains(&"bar"), "未提取到 bar，实际: {:?}", names);
    }

    #[test]
    fn 提取rust函数调用点() {
        let source = r#"
fn caller() {
    callee();
}

fn callee() {}
"#;
        let path = std::path::Path::new("test.rs");
        let result = parse_rust(path, source).expect("解析失败");
        let callee_names: Vec<&str> = result.calls.iter().map(|c| c.callee_name.as_str()).collect();
        assert!(
            callee_names.contains(&"callee"),
            "未提取到调用 callee，实际: {:?}",
            callee_names
        );
    }
}
