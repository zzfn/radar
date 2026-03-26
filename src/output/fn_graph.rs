/// 函数调用图输出格式：DOT / Mermaid / JSON / Tree
use std::collections::BTreeMap;
use std::io::Write;

use crate::error::Result;
use crate::function_graph::FunctionGraph;

/// DOT 格式输出
pub fn write_dot<W: Write + ?Sized>(fg: &FunctionGraph, writer: &mut W) -> Result<()> {
    writeln!(writer, "digraph function_graph {{")?;
    writeln!(writer, "    rankdir=LR;")?;
    writeln!(
        writer,
        "    node [shape=ellipse, style=filled, fillcolor=\"#ffe0cc\", fontname=\"Helvetica\"];"
    )?;
    writeln!(writer, "    edge [fontname=\"Helvetica\", fontsize=10];")?;
    writeln!(writer)?;

    for idx in fg.graph.node_indices() {
        let n = &fg.graph[idx];
        let file_name = n
            .file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("?");
        let label = format!("{}\\n({})", escape_dot(&n.name), escape_dot(file_name));
        let tooltip = format!("{}:{}-{}", n.file.display(), n.start_line, n.end_line);
        writeln!(
            writer,
            "    n{} [label=\"{}\", tooltip=\"{}\"];",
            idx.index(),
            label,
            escape_dot(&tooltip)
        )?;
    }

    writeln!(writer)?;

    for eidx in fg.graph.edge_indices() {
        let (src, dst) = fg.graph.edge_endpoints(eidx).unwrap();
        let edge = &fg.graph[eidx];
        writeln!(
            writer,
            "    n{} -> n{} [label=\"L{}\"];",
            src.index(),
            dst.index(),
            edge.line
        )?;
    }

    writeln!(writer, "}}")?;
    Ok(())
}

/// Mermaid 格式输出
pub fn write_mermaid<W: Write + ?Sized>(fg: &FunctionGraph, writer: &mut W) -> Result<()> {
    writeln!(writer, "graph LR")?;

    for idx in fg.graph.node_indices() {
        let n = &fg.graph[idx];
        let file_name = n
            .file
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("?");
        writeln!(
            writer,
            "    n{}[\"{}<br/>{}\"]",
            idx.index(),
            escape_mermaid(&n.name),
            escape_mermaid(file_name)
        )?;
    }

    writeln!(writer)?;

    for eidx in fg.graph.edge_indices() {
        let (src, dst) = fg.graph.edge_endpoints(eidx).unwrap();
        let edge = &fg.graph[eidx];
        writeln!(
            writer,
            "    n{} -->|\"L{}\"|n{}",
            src.index(),
            edge.line,
            dst.index()
        )?;
    }

    Ok(())
}

/// JSON 格式输出（nodes + edges）
pub fn write_json<W: Write + ?Sized>(fg: &FunctionGraph, writer: &mut W) -> Result<()> {
    let nodes: Vec<serde_json::Value> = fg
        .graph
        .node_indices()
        .map(|idx| {
            let n = &fg.graph[idx];
            serde_json::json!({
                "id": idx.index(),
                "name": n.name,
                "file": n.file,
                "start_line": n.start_line,
                "end_line": n.end_line,
                "language": format!("{:?}", n.language),
            })
        })
        .collect();

    let edges: Vec<serde_json::Value> = fg
        .graph
        .edge_indices()
        .map(|eidx| {
            let (src, dst) = fg.graph.edge_endpoints(eidx).unwrap();
            let e = &fg.graph[eidx];
            serde_json::json!({
                "from": src.index(),
                "to": dst.index(),
                "line": e.line,
                "callee": e.raw_callee,
            })
        })
        .collect();

    let out = serde_json::json!({ "nodes": nodes, "edges": edges });
    writeln!(writer, "{}", serde_json::to_string_pretty(&out)?)?;
    Ok(())
}

/// 树形文本输出：按文件分组列出所有函数定义
pub fn write_tree<W: Write + ?Sized>(fg: &FunctionGraph, writer: &mut W) -> Result<()> {
    let mut by_file: BTreeMap<String, Vec<(String, usize, usize)>> = BTreeMap::new();

    for idx in fg.graph.node_indices() {
        let n = &fg.graph[idx];
        let file = n.file.display().to_string();
        by_file
            .entry(file)
            .or_default()
            .push((n.name.clone(), n.start_line, n.end_line));
    }

    let total_fns: usize = by_file.values().map(|v| v.len()).sum();
    writeln!(
        writer,
        "函数列表（共 {} 个函数，{} 个文件）",
        total_fns,
        by_file.len()
    )?;
    writeln!(writer, "{}", "─".repeat(50))?;

    let files: Vec<_> = by_file.iter().collect();
    for (fi, (file, fns)) in files.iter().enumerate() {
        let is_last_file = fi == files.len() - 1;
        let file_connector = if is_last_file { "└── " } else { "├── " };
        writeln!(writer, "{}{}", file_connector, file)?;

        let child_prefix = if is_last_file { "    " } else { "│   " };
        let mut sorted_fns: Vec<(String, usize, usize)> = fns.to_vec();
        sorted_fns.sort_by_key(|(_, start, _)| *start);

        for (fi2, (name, start, end)) in sorted_fns.iter().enumerate() {
            let is_last_fn = fi2 == sorted_fns.len() - 1;
            let fn_connector = if is_last_fn { "└── " } else { "├── " };
            writeln!(
                writer,
                "{}{}{} (L{}-{})",
                child_prefix, fn_connector, name, start, end
            )?;
        }
    }

    Ok(())
}

fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn escape_mermaid(s: &str) -> String {
    s.replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}
