/// 从多个文件的函数分析结果构建 FunctionGraph
use std::path::Path;

use crate::analyzer::fn_analyzer::{analyze_file_functions, FileFunctions};
use crate::error::Result;
use crate::function_graph::{CallEdge, FunctionGraph, FunctionNode};

/// 将 FileFunctions 的集合转换为 FunctionGraph
pub fn build_function_graph(files: Vec<FileFunctions>) -> FunctionGraph {
    let mut fg = FunctionGraph::new();

    // 第一遍：添加所有函数定义节点
    for file in &files {
        for def in &file.defs {
            fg.add_function(FunctionNode {
                name: def.name.clone(),
                file: file.path.clone(),
                start_line: def.start_line,
                end_line: def.end_line,
                language: file.language.clone(),
            });
        }
    }

    // 构建全局名称索引：函数名 -> [(文件, NodeIndex)]（用于跨文件解析）
    let mut global_name_map: std::collections::HashMap<
        String,
        Vec<(std::path::PathBuf, petgraph::graph::NodeIndex)>,
    > = std::collections::HashMap::new();
    for idx in fg.graph.node_indices() {
        let node = &fg.graph[idx];
        global_name_map
            .entry(node.name.clone())
            .or_default()
            .push((node.file.clone(), idx));
    }

    // 第二遍：添加调用边
    for file in &files {
        for call in &file.calls {
            // 找到包含此调用点的函数定义（按字节范围判断）
            let caller_def = file.defs.iter().find(|d| {
                call.byte_offset >= d.start_byte && call.byte_offset < d.end_byte
            });

            let Some(caller_def) = caller_def else {
                continue;
            };

            let Some(caller_idx) = fg.find_fn(&file.path, &caller_def.name) else {
                continue;
            };

            // 跨文件解析：优先同文件，其次全局唯一匹配
            let callee_idx = if let Some(idx) = fg.find_fn(&file.path, &call.callee_name) {
                // 同文件调用
                Some(idx)
            } else if let Some(candidates) = global_name_map.get(&call.callee_name) {
                if candidates.len() == 1 {
                    // 全局唯一匹配
                    Some(candidates[0].1)
                } else {
                    // 歧义：暂不解析
                    None
                }
            } else {
                None
            };

            if let Some(callee_idx) = callee_idx {
                if caller_idx != callee_idx {
                    fg.add_call(
                        caller_idx,
                        callee_idx,
                        CallEdge {
                            line: call.line,
                            raw_callee: call.callee_name.clone(),
                        },
                    );
                }
            }
        }
    }

    fg
}

/// 扫描目录，构建函数调用图
pub fn analyze_dir_functions(root: &Path) -> Result<FunctionGraph> {
    use ignore::WalkBuilder;
    use rayon::prelude::*;

    let supported_exts = ["rs", "ts", "tsx", "js", "jsx", "go", "py", "vue"];

    let files: Vec<std::path::PathBuf> = WalkBuilder::new(root)
        .hidden(false)
        .git_ignore(true)
        .build()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().map(|t| t.is_file()).unwrap_or(false))
        .map(|e| e.into_path())
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .map(|ext| supported_exts.contains(&ext))
                .unwrap_or(false)
        })
        .collect();

    let results: Vec<FileFunctions> = files
        .par_iter()
        .filter_map(|p| {
            analyze_file_functions(p).map_err(|e| {
                eprintln!("警告: 跳过 {}: {}", p.display(), e);
            }).ok()
        })
        .collect();

    Ok(build_function_graph(results))
}
