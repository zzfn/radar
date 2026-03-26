/// 函数调用图：节点为函数，边为调用关系
use std::collections::HashMap;
use std::path::PathBuf;

use petgraph::graph::{DiGraph, NodeIndex};
use serde::Serialize;

use crate::graph::Language;

/// 函数定义节点
#[derive(Debug, Clone, Serialize)]
pub struct FunctionNode {
    /// 函数名
    pub name: String,
    /// 所在文件
    pub file: PathBuf,
    /// 函数体起始行
    pub start_line: usize,
    /// 函数体结束行
    pub end_line: usize,
    /// 所属语言
    pub language: Language,
}

/// 函数调用边
#[derive(Debug, Clone, Serialize)]
pub struct CallEdge {
    /// 调用发生的行号
    pub line: usize,
    /// 原始被调函数名（未解析）
    pub raw_callee: String,
}

/// 函数调用图
pub struct FunctionGraph {
    pub graph: DiGraph<FunctionNode, CallEdge>,
    /// (文件路径, 函数名) -> 节点索引
    fn_map: HashMap<(PathBuf, String), NodeIndex>,
    /// 文件路径 -> 该文件内所有函数节点索引
    pub file_fns: HashMap<PathBuf, Vec<NodeIndex>>,
}

impl FunctionGraph {
    pub fn new() -> Self {
        Self {
            graph: DiGraph::new(),
            fn_map: HashMap::new(),
            file_fns: HashMap::new(),
        }
    }

    /// 添加函数节点，返回节点索引
    pub fn add_function(&mut self, node: FunctionNode) -> NodeIndex {
        let key = (node.file.clone(), node.name.clone());
        if let Some(&idx) = self.fn_map.get(&key) {
            return idx;
        }
        let file = node.file.clone();
        let idx = self.graph.add_node(node);
        self.fn_map.insert(key, idx);
        self.file_fns.entry(file).or_default().push(idx);
        idx
    }

    /// 添加调用边（避免重复）
    pub fn add_call(&mut self, from: NodeIndex, to: NodeIndex, edge: CallEdge) {
        let already = self
            .graph
            .edges_connecting(from, to)
            .any(|e| e.weight().raw_callee == edge.raw_callee);
        if !already {
            self.graph.add_edge(from, to, edge);
        }
    }

    /// 按 (文件, 函数名) 查找节点
    pub fn find_fn(&self, file: &PathBuf, name: &str) -> Option<NodeIndex> {
        self.fn_map.get(&(file.clone(), name.to_string())).copied()
    }

    /// 函数级影响分析：找出所有（直接+间接）调用了目标函数的函数
    /// max_depth = 0 表示不限深度
    pub fn fn_impact(
        &self,
        target_file: &PathBuf,
        target_fn: &str,
        max_depth: usize,
    ) -> FnImpactReport {
        use petgraph::Direction;
        use std::collections::VecDeque;

        let Some(start) = self.find_fn(target_file, target_fn) else {
            return FnImpactReport {
                target_file: target_file.clone(),
                target_function: target_fn.to_string(),
                callers: vec![],
                total_callers: 0,
            };
        };

        let mut visited: HashMap<NodeIndex, usize> = HashMap::new();
        let mut queue: VecDeque<(NodeIndex, usize, Vec<String>)> = VecDeque::new();
        let mut callers: Vec<FnCaller> = vec![];

        queue.push_back((start, 0, vec![target_fn.to_string()]));
        visited.insert(start, 0);

        while let Some((idx, depth, via)) = queue.pop_front() {
            if idx != start {
                let node = &self.graph[idx];
                callers.push(FnCaller {
                    function: node.name.clone(),
                    file: node.file.clone(),
                    depth,
                    via: via[..via.len().saturating_sub(1)].to_vec(),
                });
            }

            if max_depth > 0 && depth >= max_depth {
                continue;
            }

            for neighbor in self.graph.neighbors_directed(idx, Direction::Incoming) {
                if visited.contains_key(&neighbor) {
                    continue;
                }
                visited.insert(neighbor, depth + 1);
                let mut next_via = via.clone();
                next_via.push(self.graph[neighbor].name.clone());
                queue.push_back((neighbor, depth + 1, next_via));
            }
        }

        callers.sort_by_key(|c| c.depth);
        let total = callers.len();

        FnImpactReport {
            target_file: target_file.clone(),
            target_function: target_fn.to_string(),
            callers,
            total_callers: total,
        }
    }
}

impl Default for FunctionGraph {
    fn default() -> Self {
        Self::new()
    }
}

/// 函数级影响分析结果中的单个调用者
#[derive(Debug, Serialize)]
pub struct FnCaller {
    pub function: String,
    pub file: PathBuf,
    pub depth: usize,
    /// 调用链中的中间函数名列表（不含自身）
    pub via: Vec<String>,
}

/// 函数级影响分析结果
#[derive(Debug, Serialize)]
pub struct FnImpactReport {
    pub target_file: PathBuf,
    pub target_function: String,
    pub callers: Vec<FnCaller>,
    pub total_callers: usize,
}
