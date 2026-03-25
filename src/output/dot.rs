/// Graphviz DOT 格式输出
/// 生成可用 `dot -Tsvg -o output.svg` 渲染的 DOT 文件
use std::io::Write;

use crate::error::Result;
use crate::graph::{DependencyGraph, EdgeKind, NodeKind};
use crate::output::OutputFormat;

pub struct DotOutput {
    /// 图的标题
    pub title: String,
    /// 是否使用集群（按语言分组）
    pub cluster: bool,
}

impl DotOutput {
    pub fn new() -> Self {
        Self {
            title: "dependency_graph".to_string(),
            cluster: false,
        }
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }
}

impl Default for DotOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputFormat for DotOutput {
    fn name(&self) -> &'static str {
        "dot"
    }

    fn write<W: Write + ?Sized>(&self, graph: &DependencyGraph, writer: &mut W) -> Result<()> {
        // DOT 文件头
        writeln!(writer, "digraph {} {{", sanitize_id(&self.title))?;
        writeln!(writer, "    // 由 radar 自动生成")?;
        writeln!(writer, "    rankdir=LR;")?;
        writeln!(writer, "    node [shape=box, style=filled, fillcolor=\"#f0f4ff\", fontname=\"Helvetica\"];")?;
        writeln!(writer, "    edge [fontname=\"Helvetica\", fontsize=10];")?;
        writeln!(writer)?;

        // 输出所有节点
        writeln!(writer, "    // === 节点 ===")?;
        for idx in graph.graph.node_indices() {
            let node = &graph.graph[idx];
            let label = node.path.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            let tooltip = node.path.display().to_string();
            let (shape, color) = match &node.kind {
                NodeKind::File => ("box", "#ddeeff"),
                NodeKind::Function(_) => ("ellipse", "#ffe0cc"),
                NodeKind::Component(_) => ("component", "#ccffcc"),
                NodeKind::Module(_) => ("folder", "#fff0cc"),
            };
            writeln!(
                writer,
                "    n{} [label=\"{}\", tooltip=\"{}\", shape={}, fillcolor=\"{}\"];",
                idx.index(),
                escape_dot(label),
                escape_dot(&tooltip),
                shape,
                color
            )?;
        }

        writeln!(writer)?;

        // 输出所有边
        writeln!(writer, "    // === 边 ===")?;
        for edge_idx in graph.graph.edge_indices() {
            let (src, dst) = graph.graph.edge_endpoints(edge_idx).unwrap();
            let edge = &graph.graph[edge_idx];
            let (style, color, label) = match &edge.kind {
                EdgeKind::Import => ("solid", "#4477aa", "import"),
                EdgeKind::Call => ("dashed", "#aa7744", "call"),
                EdgeKind::Render => ("dotted", "#44aa44", "render"),
                EdgeKind::Use => ("solid", "#7744aa", "use"),
                EdgeKind::ReExport => ("bold", "#aa4444", "re-export"),
            };
            let line_label = edge.line
                .map(|l| format!("{}:{}", label, l))
                .unwrap_or_else(|| label.to_string());
            writeln!(
                writer,
                "    n{} -> n{} [label=\"{}\", style={}, color=\"{}\"];",
                src.index(),
                dst.index(),
                escape_dot(&line_label),
                style,
                color
            )?;
        }

        writeln!(writer, "}}")?;
        Ok(())
    }
}

/// 转义 DOT 标签中的特殊字符
fn escape_dot(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

/// 将任意字符串转为合法的 DOT 标识符
fn sanitize_id(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_dot() {
        assert_eq!(escape_dot(r#"foo "bar""#), r#"foo \"bar\""#);
    }

    #[test]
    fn test_empty_graph() {
        let graph = DependencyGraph::new();
        let output = DotOutput::new();
        let result = output.to_string(&graph).unwrap();
        assert!(result.contains("digraph"));
        assert!(result.contains('}'));
    }
}
