/// 输出格式模块
pub mod dot;
pub mod fn_graph;
pub mod json;
pub mod mermaid;

use std::io::Write;

use crate::error::Result;
use crate::graph::DependencyGraph;

/// 输出格式 trait：所有输出器实现此接口
pub trait OutputFormat {
    /// 将依赖图写入 writer（可以是 File 或 stdout）
    fn write<W: Write + ?Sized>(&self, graph: &DependencyGraph, writer: &mut W) -> Result<()>;
}

/// 树形格式（终端友好的文本输出）
pub struct TreeOutput;

impl TreeOutput {
    pub fn new() -> Self {
        Self {}
    }
}

impl OutputFormat for TreeOutput {
    fn write<W: Write + ?Sized>(&self, graph: &DependencyGraph, writer: &mut W) -> Result<()> {
        use petgraph::Direction;

        writeln!(writer, "依赖树（节点数: {}, 边数: {}）", graph.node_count(), graph.edge_count())?;
        writeln!(writer, "{}", "─".repeat(50))?;

        // 找出所有入度为 0 的根节点（没有人依赖它的节点）
        let roots: Vec<_> = graph
            .graph
            .node_indices()
            .filter(|&idx| {
                graph
                    .graph
                    .neighbors_directed(idx, Direction::Incoming)
                    .count()
                    == 0
            })
            .collect();

        if roots.is_empty() {
            writeln!(writer, "（所有节点均有入边，可能存在循环依赖）")?;
            // 降级：直接列出所有节点
            for idx in graph.graph.node_indices() {
                let node = &graph.graph[idx];
                writeln!(writer, "  {}", node.path.display())?;
            }
            return Ok(());
        }

        let mut visited = std::collections::HashSet::new();
        for root in roots {
            print_subtree(graph, root, writer, "", true, &mut visited)?;
        }

        Ok(())
    }
}

/// 递归打印子树，visited 用于检测循环依赖，防止无限递归
fn print_subtree<W: Write + ?Sized>(
    graph: &DependencyGraph,
    idx: petgraph::graph::NodeIndex,
    writer: &mut W,
    prefix: &str,
    is_last: bool,
    visited: &mut std::collections::HashSet<petgraph::graph::NodeIndex>,
) -> Result<()> {
    use petgraph::Direction;

    let node = &graph.graph[idx];
    let connector = if is_last { "└── " } else { "├── " };

    if visited.contains(&idx) {
        writeln!(writer, "{}{}{}  (循环引用)", prefix, connector, node.path.display())?;
        return Ok(());
    }
    visited.insert(idx);

    writeln!(writer, "{}{}{}", prefix, connector, node.path.display())?;

    let children: Vec<_> = graph
        .graph
        .neighbors_directed(idx, Direction::Outgoing)
        .collect();

    let new_prefix = format!("{}{}   ", prefix, if is_last { " " } else { "│" });

    for (i, child) in children.iter().enumerate() {
        let is_last_child = i == children.len() - 1;
        print_subtree(graph, *child, writer, &new_prefix, is_last_child, visited)?;
    }

    visited.remove(&idx);

    Ok(())
}
