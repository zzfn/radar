/// 依赖图数据结构
/// 基于 petgraph 构建，节点为文件/函数/组件，边为依赖关系
use std::collections::HashMap;
use std::path::PathBuf;

use petgraph::graph::{DiGraph, NodeIndex};
use serde::{Deserialize, Serialize};

/// 节点类型
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    /// 文件节点
    File,
    /// 函数节点，携带函数名
    Function(String),
    /// 前端组件节点，携带组件名
    Component(String),
    /// 模块节点，携带模块名
    Module(String),
}

/// 支持的编程语言
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    JavaScript,
    TypeScript,
    Rust,
    Python,
    /// 未知或未检测到的语言
    Unknown,
}

impl Language {
    /// 根据文件扩展名推断语言
    pub fn from_extension(ext: &str) -> Self {
        match ext.to_lowercase().as_str() {
            "js" | "jsx" | "mjs" | "cjs" => Self::JavaScript,
            "ts" | "tsx" | "mts" | "cts" => Self::TypeScript,
            "rs" => Self::Rust,
            "py" | "pyw" => Self::Python,
            _ => Self::Unknown,
        }
    }
}

/// 图节点：表示一个可被依赖的单元
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    /// 文件路径（相对于分析根目录）
    pub path: PathBuf,
    /// 节点类型
    pub kind: NodeKind,
    /// 所属语言
    pub language: Language,
}

/// 边的类型（依赖关系类型）
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EdgeKind {
    /// import / require 导入
    Import,
    /// 函数调用
    Call,
    /// 组件渲染（JSX 中的 <Component />）
    Render,
    /// use / mod 引用（Rust 风格）
    Use,
    /// 重新导出
    ReExport,
}

/// 图边：表示两个节点之间的依赖关系
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Edge {
    /// 依赖关系类型
    pub kind: EdgeKind,
    /// 发生依赖的源码行号（可选）
    pub line: Option<usize>,
    /// 原始导入路径字符串（如 `"../utils/helper"`）
    pub raw_path: Option<String>,
}

/// 依赖图：核心数据结构
pub struct DependencyGraph {
    /// petgraph 有向图，节点为 Node，边为 Edge
    pub graph: DiGraph<Node, Edge>,
    /// 路径 -> 节点索引的快速查找 map
    node_map: HashMap<PathBuf, NodeIndex>,
}

impl DependencyGraph {
    /// 创建空图
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            node_map: HashMap::new(),
        }
    }

    /// 添加节点，如果已存在则返回已有索引
    pub fn add_node(&mut self, node: Node) -> NodeIndex {
        if let Some(&idx) = self.node_map.get(&node.path) {
            return idx;
        }
        let path = node.path.clone();
        let idx = self.graph.add_node(node);
        self.node_map.insert(path, idx);
        idx
    }

    /// 添加依赖边（from -> to 表示 from 依赖 to）
    pub fn add_edge(&mut self, from: NodeIndex, to: NodeIndex, edge: Edge) {
        // 避免重复添加相同类型的边
        if !self
            .graph
            .edges_connecting(from, to)
            .any(|e| e.weight().kind == edge.kind)
        {
            self.graph.add_edge(from, to, edge);
        }
    }

    /// 根据路径查找节点索引
    pub fn find_node(&self, path: &PathBuf) -> Option<NodeIndex> {
        self.node_map.get(path).copied()
    }

    /// 获取节点总数
    pub fn node_count(&self) -> usize {
        self.graph.node_count()
    }

    /// 获取边总数
    pub fn edge_count(&self) -> usize {
        self.graph.edge_count()
    }

    /// 检测循环依赖，返回所有循环路径
    pub fn detect_cycles(&self) -> Vec<Vec<PathBuf>> {
        use petgraph::algo::kosaraju_scc;

        // 强连通分量中大小 > 1 的即为循环依赖
        kosaraju_scc(&self.graph)
            .into_iter()
            .filter(|scc| scc.len() > 1)
            .map(|scc| {
                scc.iter()
                    .map(|&idx| self.graph[idx].path.clone())
                    .collect()
            })
            .collect()
    }

    /// 获取指定节点的直接依赖（出边邻居）
    pub fn direct_deps(&self, idx: NodeIndex) -> Vec<NodeIndex> {
        use petgraph::Direction;
        self.graph
            .neighbors_directed(idx, Direction::Outgoing)
            .collect()
    }

    /// 获取依赖指定节点的节点（入边邻居，即"谁依赖了我"）
    pub fn dependents(&self, idx: NodeIndex) -> Vec<NodeIndex> {
        use petgraph::Direction;
        self.graph
            .neighbors_directed(idx, Direction::Incoming)
            .collect()
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// 分析结果摘要
#[derive(Debug, Serialize)]
pub struct GraphSummary {
    /// 节点总数
    pub node_count: usize,
    /// 边总数
    pub edge_count: usize,
    /// 循环依赖数量
    pub cycle_count: usize,
    /// 最大出度（依赖最多的节点）
    pub max_out_degree: usize,
    /// 最大入度（被依赖最多的节点）
    pub max_in_degree: usize,
}
