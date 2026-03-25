/// JSON 格式输出
/// 将依赖图序列化为结构化 JSON，便于工具链集成
use std::io::Write;

use serde::Serialize;

use crate::error::Result;
use crate::graph::DependencyGraph;
use crate::output::OutputFormat;

/// JSON 输出的节点结构
#[derive(Serialize)]
struct JsonNode {
    id: usize,
    path: String,
    kind: String,
    language: String,
}

/// JSON 输出的边结构
#[derive(Serialize)]
struct JsonEdge {
    from: usize,
    to: usize,
    kind: String,
    line: Option<usize>,
    raw_path: Option<String>,
}

/// JSON 输出的完整图结构
#[derive(Serialize)]
struct JsonGraph {
    /// 元数据
    meta: JsonMeta,
    nodes: Vec<JsonNode>,
    edges: Vec<JsonEdge>,
}

#[derive(Serialize)]
struct JsonMeta {
    node_count: usize,
    edge_count: usize,
    generated_at: String,
}

pub struct JsonOutput {
    /// 是否美化输出（缩进）
    pub pretty: bool,
}

impl JsonOutput {
    pub fn new(pretty: bool) -> Self {
        Self { pretty }
    }
}

impl Default for JsonOutput {
    fn default() -> Self {
        Self::new(true)
    }
}

impl OutputFormat for JsonOutput {
    fn name(&self) -> &'static str {
        "json"
    }

    fn write<W: Write + ?Sized>(&self, graph: &DependencyGraph, writer: &mut W) -> Result<()> {
        // 构建节点列表
        let nodes: Vec<JsonNode> = graph
            .graph
            .node_indices()
            .map(|idx| {
                let node = &graph.graph[idx];
                JsonNode {
                    id: idx.index(),
                    path: node.path.display().to_string(),
                    kind: format!("{:?}", node.kind),
                    language: format!("{:?}", node.language),
                }
            })
            .collect();

        // 构建边列表
        let edges: Vec<JsonEdge> = graph
            .graph
            .edge_indices()
            .map(|eidx| {
                let (src, dst) = graph.graph.edge_endpoints(eidx).unwrap();
                let edge = &graph.graph[eidx];
                JsonEdge {
                    from: src.index(),
                    to: dst.index(),
                    kind: format!("{:?}", edge.kind),
                    line: edge.line,
                    raw_path: edge.raw_path.clone(),
                }
            })
            .collect();

        let json_graph = JsonGraph {
            meta: JsonMeta {
                node_count: graph.node_count(),
                edge_count: graph.edge_count(),
                generated_at: chrono_now(),
            },
            nodes,
            edges,
        };

        if self.pretty {
            serde_json::to_writer_pretty(writer, &json_graph)?;
        } else {
            serde_json::to_writer(writer, &json_graph)?;
        }

        Ok(())
    }
}

/// 获取当前时间字符串（简化实现，不引入 chrono 依赖）
fn chrono_now() -> String {
    // 使用系统时间的简单格式化
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("unix:{}", secs)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph_json() {
        let graph = DependencyGraph::new();
        let output = JsonOutput::new(true);
        let result = output.to_string(&graph).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["meta"]["node_count"], 0);
        assert_eq!(parsed["meta"]["edge_count"], 0);
        assert!(parsed["nodes"].as_array().unwrap().is_empty());
        assert!(parsed["edges"].as_array().unwrap().is_empty());
    }
}
