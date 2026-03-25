/// Mermaid 格式输出
/// 生成可嵌入 Markdown 的 Mermaid 流程图
use std::io::Write;

use crate::error::Result;
use crate::graph::{DependencyGraph, EdgeKind};
use crate::output::OutputFormat;

pub struct MermaidOutput {
    /// 图的方向：LR（左到右）或 TD（上到下）
    pub direction: MermaidDirection,
    /// 最大节点数（超出则截断，避免图太大）
    pub max_nodes: usize,
}

#[derive(Debug, Clone)]
pub enum MermaidDirection {
    /// 从左到右
    LeftRight,
    /// 从上到下
    TopDown,
}

impl MermaidDirection {
    fn as_str(&self) -> &'static str {
        match self {
            Self::LeftRight => "LR",
            Self::TopDown => "TD",
        }
    }
}

impl MermaidOutput {
    pub fn new() -> Self {
        Self {
            direction: MermaidDirection::LeftRight,
            max_nodes: 100,
        }
    }
}

impl Default for MermaidOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl OutputFormat for MermaidOutput {
    fn name(&self) -> &'static str {
        "mermaid"
    }

    fn write<W: Write + ?Sized>(&self, graph: &DependencyGraph, writer: &mut W) -> Result<()> {
        writeln!(writer, "```mermaid")?;
        writeln!(writer, "graph {}", self.direction.as_str())?;
        writeln!(writer, "    %% 由 radar 自动生成，节点数: {}", graph.node_count())?;
        writeln!(writer)?;

        // 节点定义
        let node_count = graph.graph.node_count().min(self.max_nodes);
        writeln!(writer, "    %% 节点")?;
        for idx in graph.graph.node_indices().take(node_count) {
            let node = &graph.graph[idx];
            let label = node
                .path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("?");
            let id = format!("n{}", idx.index());
            // Mermaid 节点：不同类型用不同形状
            let node_def = match &node.kind {
                crate::graph::NodeKind::File => format!("{}[{}]", id, escape_mermaid(label)),
                crate::graph::NodeKind::Function(name) => {
                    format!("{}(({}::{}))", id, escape_mermaid(label), escape_mermaid(name))
                }
                crate::graph::NodeKind::Component(name) => {
                    format!("{}>{}<]", id, escape_mermaid(name))
                }
                crate::graph::NodeKind::Module(name) => {
                    format!("{}[/{}\\]", id, escape_mermaid(name))
                }
            };
            writeln!(writer, "    {}", node_def)?;
        }

        writeln!(writer)?;

        // 边定义
        writeln!(writer, "    %% 依赖关系")?;
        for edge_idx in graph.graph.edge_indices() {
            let (src, dst) = graph.graph.edge_endpoints(edge_idx).unwrap();
            // 跳过超出节点限制的边
            if src.index() >= node_count || dst.index() >= node_count {
                continue;
            }
            let edge = &graph.graph[edge_idx];
            let arrow = match &edge.kind {
                EdgeKind::Import => "-->",
                EdgeKind::Call => "-.->",
                EdgeKind::Render => "==>",
                EdgeKind::Use => "-->",
                EdgeKind::ReExport => "-->>",
            };
            let label = edge
                .line
                .map(|l| format!("|line:{}|", l))
                .unwrap_or_default();
            writeln!(
                writer,
                "    n{} {}{}n{}",
                src.index(),
                arrow,
                label,
                dst.index()
            )?;
        }

        // 节点样式
        writeln!(writer)?;
        writeln!(writer, "    %% 样式")?;
        writeln!(writer, "    classDef fileNode fill:#ddeeff,stroke:#4477aa;")?;
        writeln!(writer, "    classDef funcNode fill:#ffe0cc,stroke:#aa7744;")?;
        writeln!(writer, "    classDef compNode fill:#ccffcc,stroke:#44aa44;")?;

        if graph.node_count() > self.max_nodes {
            writeln!(
                writer,
                "    %% 注意：图太大，仅显示前 {} 个节点（共 {} 个）",
                self.max_nodes,
                graph.node_count()
            )?;
        }

        writeln!(writer, "```")?;
        Ok(())
    }
}

/// 转义 Mermaid 标签中的特殊字符
fn escape_mermaid(s: &str) -> String {
    s.replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('[', "(")
        .replace(']', ")")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph_mermaid() {
        let graph = DependencyGraph::new();
        let output = MermaidOutput::new();
        let result = output.to_string(&graph).unwrap();
        assert!(result.contains("```mermaid"));
        assert!(result.contains("graph LR"));
        assert!(result.contains("```"));
    }

    #[test]
    fn test_escape_mermaid() {
        assert_eq!(escape_mermaid("foo[bar]"), "foo(bar)");
    }
}
