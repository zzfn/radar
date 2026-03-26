/// 基于 tree-sitter 的函数级分析器
use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;
use regex::Regex;
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
        Language::Vue => parse_vue(path, &source),
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
        Some("vue") => Language::Vue,
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
          (method_definition name: (property_identifier) @fn_name) @fn_def
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

// ─────────────────────────── Vue SFC ───────────────────────────

/// 解析 Vue 单文件组件：提取 `<script>` 中的函数定义/调用，以及 `<template>` 中的调用引用。
/// `<template>` 的所有引用统一归属到合成函数节点 `$template`。
fn parse_vue(path: &Path, source: &str) -> Result<FileFunctions> {
    let mut defs: Vec<FnDef> = vec![];
    let mut calls: Vec<CallSite> = vec![];

    // --- script 块 ---
    if let Some((script, line_off, byte_off, is_ts)) = extract_script_info(source) {
        let lang = if is_ts { &Language::TypeScript } else { &Language::JavaScript };
        if let Ok(sf) = parse_js_ts(path, &script, lang) {
            for mut d in sf.defs {
                d.start_line += line_off;
                d.end_line += line_off;
                d.start_byte += byte_off;
                d.end_byte += byte_off;
                defs.push(d);
            }
            for mut c in sf.calls {
                c.line += line_off;
                c.byte_offset += byte_off;
                calls.push(c);
            }
        }
    }

    // --- template 块 ---
    if let Some((tmpl, tmpl_line_off, tmpl_byte_off)) = extract_template_info(source) {
        let tmpl_end_byte = tmpl_byte_off + tmpl.len();
        let tmpl_end_line = tmpl_line_off + tmpl.lines().count();

        // 合成 $template 节点，作为所有 template 调用的 caller
        defs.push(FnDef {
            name: "$template".to_string(),
            start_line: tmpl_line_off + 1,
            end_line: tmpl_end_line,
            start_byte: tmpl_byte_off,
            end_byte: tmpl_end_byte,
        });

        calls.extend(scan_template_calls(&tmpl, tmpl_line_off, tmpl_byte_off));
    }

    Ok(FileFunctions {
        path: path.to_path_buf(),
        language: Language::Vue,
        defs,
        calls,
    })
}

/// 提取 `<script>` 内容及其在原文件中的偏移。
/// 返回 `(内容, 行偏移, 字节偏移, 是否TypeScript)`。
/// 行偏移为内容第一行在原文件中的 0-based 行号（加到 tree-sitter row+1 即得原文件行号）。
fn extract_script_info(source: &str) -> Option<(String, usize, usize, bool)> {
    static RE_START: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)<script(\s[^>]*)?>").unwrap());
    static RE_END: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)</script>").unwrap());

    let m = RE_START.find(source)?;
    let tag_attrs = &source[m.start()..m.end()];
    let is_ts = tag_attrs.contains(r#"lang="ts""#) || tag_attrs.contains("lang='ts'");

    // 跳过紧跟标签的换行符，让内容从下一行开始（与 tree-sitter row=0 对齐）
    let after_tag = m.end();
    let content_start = if source[after_tag..].starts_with('\n') {
        after_tag + 1
    } else {
        after_tag
    };

    let end_m = RE_END.find(&source[content_start..])?;
    let content_end = content_start + end_m.start();
    let content = source[content_start..content_end].to_string();

    // 行偏移 = content_start 之前的换行数
    let line_offset = source[..content_start].chars().filter(|&c| c == '\n').count();

    Some((content, line_offset, content_start, is_ts))
}

/// 提取 `<template>` 内容及其在原文件中的偏移。
/// 返回 `(内容, 行偏移, 字节偏移)`。
fn extract_template_info(source: &str) -> Option<(String, usize, usize)> {
    static RE_START: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"(?i)<template(\s[^>]*)?>").unwrap());
    static RE_END: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)</template>").unwrap());

    let m = RE_START.find(source)?;
    let after_tag = m.end();
    let content_start = if source[after_tag..].starts_with('\n') {
        after_tag + 1
    } else {
        after_tag
    };

    let end_m = RE_END.find(&source[content_start..])?;
    let content_end = content_start + end_m.start();
    let content = source[content_start..content_end].to_string();
    let line_offset = source[..content_start].chars().filter(|&c| c == '\n').count();

    Some((content, line_offset, content_start))
}

/// 扫描 template 内容，提取函数调用点：
/// - 事件处理器：`@click="fn"` / `v-on:click="fn"`
/// - 插值表达式：`{{ fn() }}`
fn scan_template_calls(template: &str, line_offset: usize, byte_offset: usize) -> Vec<CallSite> {
    // 匹配事件处理器属性值中的函数名（取第一个标识符）
    static RE_EVENT: Lazy<Regex> = Lazy::new(|| {
        Regex::new(r#"(?:@|v-on:)[\w.\-:]+\s*=\s*["']([a-zA-Z_$][\w$]*)"#).unwrap()
    });
    // 匹配调用表达式中的函数名（用于 {{ }} 内）
    static RE_CALL: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"\b([a-zA-Z_$][\w$]*)\s*\(").unwrap());

    // 跳过 JS 关键字，避免误报
    const SKIP: &[&str] = &[
        "if", "for", "while", "switch", "return", "typeof", "instanceof", "new", "delete",
        "void", "throw", "catch", "function", "true", "false", "null", "undefined",
    ];

    let mut result = vec![];
    let mut cur_byte = byte_offset;

    for (i, line) in template.lines().enumerate() {
        let line_num = line_offset + i + 1;

        // 1. 事件处理器
        for cap in RE_EVENT.captures_iter(line) {
            if let Some(m) = cap.get(1) {
                result.push(CallSite {
                    callee_name: m.as_str().to_string(),
                    byte_offset: cur_byte + m.start(),
                    line: line_num,
                });
            }
        }

        // 2. {{ expr }} 插值中的函数调用
        let mut pos = 0;
        while let Some(open) = line[pos..].find("{{") {
            let expr_start = pos + open + 2;
            if let Some(close) = line[expr_start..].find("}}") {
                let expr = &line[expr_start..expr_start + close];
                for cap in RE_CALL.captures_iter(expr) {
                    if let Some(m) = cap.get(1) {
                        let name = m.as_str();
                        if !SKIP.contains(&name) {
                            result.push(CallSite {
                                callee_name: name.to_string(),
                                byte_offset: cur_byte + expr_start + m.start(),
                                line: line_num,
                            });
                        }
                    }
                }
                pos = expr_start + close + 2;
            } else {
                break;
            }
        }

        cur_byte += line.len() + 1; // +1 for '\n'
    }

    result
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

    #[test]
    fn vue_extract_script_info_basic() {
        let source = "<template>\n<div/>\n</template>\n<script>\nfunction foo() {}\n</script>\n";
        let r = extract_script_info(source);
        assert!(r.is_some(), "extract_script_info 返回 None");
        let (content, line_off, _byte_off, is_ts) = r.unwrap();
        assert!(!is_ts, "不应识别为 TS");
        assert!(content.contains("function foo"), "script 内容: {:?}", content);
        assert_eq!(line_off, 4, "行偏移应为 4，实际: {}", line_off);
    }

    #[test]
    fn vue_parse_js_script_直接调用() {
        let source = "function foo() {}\n";
        let path = std::path::Path::new("test.js");
        let r = parse_js_ts(path, source, &Language::JavaScript).expect("parse_js_ts 失败");
        let names: Vec<&str> = r.defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"foo"), "defs: {:?}", names);
    }

    #[test]
    fn vue_提取script函数定义() {
        let source = r#"<template>
  <button @click="handleClick">点击</button>
  <span>{{ formatName() }}</span>
</template>
<script>
function handleClick() {
  formatName();
}
function formatName() {}
</script>
"#;
        let path = std::path::Path::new("test.vue");
        let result = parse_vue(path, source).expect("Vue 解析失败");

        let def_names: Vec<&str> = result.defs.iter().map(|d| d.name.as_str()).collect();
        assert!(def_names.contains(&"handleClick"), "未提取到 handleClick，实际: {:?}", def_names);
        assert!(def_names.contains(&"formatName"), "未提取到 formatName，实际: {:?}", def_names);
        assert!(def_names.contains(&"$template"), "未提取到 $template，实际: {:?}", def_names);

        // 函数行号应偏移到原文件（script 内容从第6行开始）
        let hc = result.defs.iter().find(|d| d.name == "handleClick").unwrap();
        assert_eq!(hc.start_line, 6, "handleClick 行号应为 6，实际: {}", hc.start_line);
    }

    #[test]
    fn vue_提取template事件处理器() {
        let source = r#"<template>
  <button @click="handleClick">点击</button>
  <span>{{ formatName() }}</span>
</template>
<script>
function handleClick() {}
function formatName() {}
</script>
"#;
        let path = std::path::Path::new("test.vue");
        let result = parse_vue(path, source).expect("Vue 解析失败");

        // template 中应有来自 @click 和 {{ }} 的调用
        let call_names: Vec<&str> = result.calls.iter().map(|c| c.callee_name.as_str()).collect();
        assert!(
            call_names.contains(&"handleClick"),
            "未从 @click 提取到 handleClick，实际: {:?}",
            call_names
        );
        assert!(
            call_names.contains(&"formatName"),
            "未从 {{{{ }}}} 提取到 formatName，实际: {:?}",
            call_names
        );
    }
}
