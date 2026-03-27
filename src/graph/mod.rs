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
    Go,
    Java,
    Vue,
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
            "go" => Self::Go,
            "java" => Self::Java,
            "vue" => Self::Vue,
            _ => Self::Unknown,
        }
    }

    /// 返回该语言源文件的主扩展名，用于目录展开时过滤文件
    pub fn primary_extension(&self) -> Option<&'static str> {
        match self {
            Self::Python => Some("py"),
            Self::Go => Some("go"),
            Self::Rust => Some("rs"),
            Self::Java => Some("java"),
            Self::JavaScript => Some("js"),
            Self::TypeScript => Some("ts"),
            Self::Vue => Some("vue"),
            Self::Unknown => None,
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

    /// 影响范围分析：从 target 出发，沿反向边（谁依赖了我）BFS，
    /// 返回所有受影响的节点及其影响链。
    /// max_depth = 0 表示不限深度。
    pub fn impact(&self, target: &PathBuf, max_depth: usize) -> ImpactReport {
        use petgraph::Direction;
        use std::collections::{HashMap, VecDeque};

        let has_cycles = !self.detect_cycles().is_empty();

        let Some(&start) = self.node_map.get(target) else {
            return ImpactReport {
                target: target.clone(),
                affected: vec![],
                total_affected: 0,
                has_cycles,
            };
        };

        // BFS：(节点索引, 当前深度, 影响链)
        let mut queue: VecDeque<(NodeIndex, usize, Vec<PathBuf>)> = VecDeque::new();
        // visited 记录已访问节点，避免循环依赖时死循环
        let mut visited: HashMap<NodeIndex, usize> = HashMap::new();
        let mut affected: Vec<AffectedNode> = Vec::new();

        queue.push_back((start, 0, vec![target.clone()]));
        visited.insert(start, 0);

        while let Some((idx, depth, via)) = queue.pop_front() {
            // 跳过 start 本身，只收集它的依赖者
            if idx != start {
                affected.push(AffectedNode {
                    path: self.graph[idx].path.clone(),
                    depth,
                    via: via[..via.len() - 1].to_vec(), // via 不含自身
                });
            }

            // 达到最大深度则不再展开
            if max_depth > 0 && depth >= max_depth {
                continue;
            }

            // 遍历反向边（谁导入了 idx）
            for neighbor in self.graph.neighbors_directed(idx, Direction::Incoming) {
                if visited.contains_key(&neighbor) {
                    continue;
                }
                visited.insert(neighbor, depth + 1);
                let mut next_via = via.clone();
                next_via.push(self.graph[neighbor].path.clone());
                queue.push_back((neighbor, depth + 1, next_via));
            }
        }

        // 按深度升序排序
        affected.sort_by_key(|n| n.depth);
        let total = affected.len();

        ImpactReport {
            target: target.clone(),
            affected,
            total_affected: total,
            has_cycles,
        }
    }

    /// 生成依赖图的统计摘要
    pub fn summary(&self) -> GraphSummary {
        use petgraph::Direction;

        let cycle_count = self.detect_cycles().len();

        let mut max_out: Option<(PathBuf, usize)> = None;
        let mut max_in: Option<(PathBuf, usize)> = None;
        let mut isolated_count = 0;

        for idx in self.graph.node_indices() {
            let out_deg = self.graph.neighbors_directed(idx, Direction::Outgoing).count();
            let in_deg = self.graph.neighbors_directed(idx, Direction::Incoming).count();

            if out_deg == 0 && in_deg == 0 {
                isolated_count += 1;
            }
            if max_out.as_ref().map_or(true, |(_, d)| out_deg > *d) {
                max_out = Some((self.graph[idx].path.clone(), out_deg));
            }
            if max_in.as_ref().map_or(true, |(_, d)| in_deg > *d) {
                max_in = Some((self.graph[idx].path.clone(), in_deg));
            }
        }

        GraphSummary {
            node_count: self.node_count(),
            edge_count: self.edge_count(),
            cycle_count,
            max_out_degree: max_out,
            max_in_degree: max_in,
            isolated_count,
        }
    }

    /// 提取以 root 为起点的正向子图（出向 BFS，max_depth=0 表示不限深度）
    /// 返回只包含可达节点和对应边的新 DependencyGraph
    pub fn subgraph(&self, root: &PathBuf, max_depth: usize) -> DependencyGraph {
        use petgraph::visit::EdgeRef;
        use petgraph::Direction;
        use std::collections::{HashMap, VecDeque};

        let Some(&start) = self.node_map.get(root) else {
            return DependencyGraph::new();
        };

        // BFS 收集可达节点
        let mut visited: HashMap<NodeIndex, usize> = HashMap::new();
        let mut queue: VecDeque<(NodeIndex, usize)> = VecDeque::new();
        queue.push_back((start, 0));
        visited.insert(start, 0);

        while let Some((idx, depth)) = queue.pop_front() {
            if max_depth > 0 && depth >= max_depth {
                continue;
            }
            for neighbor in self.graph.neighbors_directed(idx, Direction::Outgoing) {
                if !visited.contains_key(&neighbor) {
                    visited.insert(neighbor, depth + 1);
                    queue.push_back((neighbor, depth + 1));
                }
            }
        }

        // 构建子图
        let mut sub = DependencyGraph::new();
        for (&old_idx, _) in &visited {
            sub.add_node(self.graph[old_idx].clone());
        }
        for (&old_idx, _) in &visited {
            for edge_ref in self.graph.edges_directed(old_idx, Direction::Outgoing) {
                if visited.contains_key(&edge_ref.target()) {
                    let src_path = &self.graph[old_idx].path;
                    let dst_path = &self.graph[edge_ref.target()].path;
                    if let (Some(&si), Some(&di)) = (sub.node_map.get(src_path), sub.node_map.get(dst_path)) {
                        sub.graph.add_edge(si, di, edge_ref.weight().clone());
                    }
                }
            }
        }
        sub
    }

    /// 未引用文件检测：返回 in-degree == 0 的文件列表
    /// skip_entry: 跳过常见入口文件名（main.rs、index.ts 等）
    pub fn unused_files(&self, skip_entry: bool) -> Vec<UnusedFile> {
        use petgraph::Direction;

        let mut result = Vec::new();
        for idx in self.graph.node_indices() {
            let in_deg = self
                .graph
                .neighbors_directed(idx, Direction::Incoming)
                .count();
            if in_deg > 0 {
                continue;
            }
            let node = &self.graph[idx];
            if skip_entry && is_likely_entry_point(&node.path) {
                continue;
            }
            let out_deg = self
                .graph
                .neighbors_directed(idx, Direction::Outgoing)
                .count();
            result.push(UnusedFile {
                path: node.path.clone(),
                out_degree: out_deg,
                language: node.language.clone(),
            });
        }
        result.sort_by(|a, b| a.path.cmp(&b.path));
        result
    }

    /// hotspot 分析：按 in-degree 降序返回最受依赖的文件列表
    pub fn hotspots(&self, top_n: usize) -> Vec<HotspotFile> {
        use petgraph::Direction;

        let mut result: Vec<HotspotFile> = self
            .graph
            .node_indices()
            .map(|idx| {
                let node = &self.graph[idx];
                let in_deg = self
                    .graph
                    .neighbors_directed(idx, Direction::Incoming)
                    .count();
                let out_deg = self
                    .graph
                    .neighbors_directed(idx, Direction::Outgoing)
                    .count();
                HotspotFile {
                    path: node.path.clone(),
                    in_degree: in_deg,
                    out_degree: out_deg,
                }
            })
            .collect();

        result.sort_by(|a, b| b.in_degree.cmp(&a.in_degree).then(a.path.cmp(&b.path)));
        if top_n > 0 {
            result.truncate(top_n);
        }
        result
    }

    /// 路径查找：BFS 找从 from 到 to 的最短依赖路径
    /// 返回 None 表示无路径
    pub fn find_path(&self, from: &PathBuf, to: &PathBuf) -> Option<Vec<PathBuf>> {
        use petgraph::Direction;
        use std::collections::{HashMap, VecDeque};

        let start = *self.node_map.get(from)?;
        let end = *self.node_map.get(to)?;

        // BFS，记录每个节点的前驱
        let mut prev: HashMap<NodeIndex, NodeIndex> = HashMap::new();
        let mut queue: VecDeque<NodeIndex> = VecDeque::new();
        queue.push_back(start);
        prev.insert(start, start);

        while let Some(idx) = queue.pop_front() {
            if idx == end {
                // 回溯路径
                let mut path = Vec::new();
                let mut cur = end;
                loop {
                    path.push(self.graph[cur].path.clone());
                    let p = prev[&cur];
                    if p == cur {
                        break;
                    }
                    cur = p;
                }
                path.reverse();
                return Some(path);
            }
            for neighbor in self.graph.neighbors_directed(idx, Direction::Outgoing) {
                if !prev.contains_key(&neighbor) {
                    prev.insert(neighbor, idx);
                    queue.push_back(neighbor);
                }
            }
        }
        None
    }
}

impl Default for DependencyGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// 依赖图统计摘要
#[derive(Debug, Serialize)]
pub struct GraphSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub cycle_count: usize,
    /// 出度最大的节点路径及其出度值
    pub max_out_degree: Option<(PathBuf, usize)>,
    /// 入度最大的节点路径及其入度值
    pub max_in_degree: Option<(PathBuf, usize)>,
    /// 孤立节点数（无任何边）
    pub isolated_count: usize,
}

/// impact 分析中的单个受影响节点
#[derive(Debug, Serialize)]
pub struct AffectedNode {
    /// 受影响的文件路径
    pub path: PathBuf,
    /// 距离目标的跳数（1 = 直接依赖者）
    pub depth: usize,
    /// 影响链：target → ... → 此文件的中间路径（含 target，不含自身）
    pub via: Vec<PathBuf>,
}

/// impact 分析结果
#[derive(Debug, Serialize)]
pub struct ImpactReport {
    /// 被修改的目标文件
    pub target: PathBuf,
    /// 所有受影响的文件（按深度升序）
    pub affected: Vec<AffectedNode>,
    /// 受影响文件总数
    pub total_affected: usize,
    /// 影响链中是否存在循环依赖
    pub has_cycles: bool,
}

/// unused 分析中的单个未引用文件
#[derive(Debug, Serialize)]
pub struct UnusedFile {
    pub path: PathBuf,
    /// 该文件自身依赖的文件数（出度）
    pub out_degree: usize,
    pub language: Language,
}

/// hotspot 分析中的单个高被依赖文件
#[derive(Debug, Serialize)]
pub struct HotspotFile {
    pub path: PathBuf,
    /// 被多少文件依赖（入度）
    pub in_degree: usize,
    /// 自身依赖多少文件（出度）
    pub out_degree: usize,
}

/// 判断文件路径是否为常见入口文件（不应算作"未引用"）
fn is_likely_entry_point(path: &PathBuf) -> bool {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(
        name.as_str(),
        // Rust
        "main.rs" | "lib.rs" | "build.rs"
        // JS/TS
        | "index.js" | "index.ts" | "index.jsx" | "index.tsx"
        | "index.mjs" | "index.cjs"
        | "main.js" | "main.ts" | "main.jsx" | "main.tsx"
        | "app.js" | "app.ts" | "app.jsx" | "app.tsx"
        | "vite.config.ts" | "vite.config.js"
        | "next.config.js" | "next.config.ts"
        // Python
        | "__init__.py" | "__main__.py" | "setup.py" | "conftest.py"
        | "manage.py" | "wsgi.py" | "asgi.py"
        // Go
        | "main.go"
        // Java
        | "main.java"
    )
}

