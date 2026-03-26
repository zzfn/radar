/// Radar - 项目依赖关系分析工具
/// 入口文件：解析 CLI 参数，分发到对应子命令处理逻辑
mod analyzer;
mod cli;
mod error;
mod function_graph;
mod graph;
mod output;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::analyzer::{create_analyzer, detect_language};
use crate::cli::{Cli, Commands, FunctionsArgs, ImpactArgs, Lang, OutputFormat};
use crate::error::Result;
use crate::graph::DependencyGraph;
use crate::output::{dot::DotOutput, json::JsonOutput, mermaid::MermaidOutput, OutputFormat as OutputFormatTrait, TreeOutput};

fn main() {
    let cli = Cli::parse();

    if let Err(e) = run(cli) {
        eprintln!("{} {}", "错误:".red().bold(), e);
        std::process::exit(1);
    }
}

/// 主逻辑分发
fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Commands::Analyze(args) => cmd_analyze(args),
        Commands::Graph(args) => cmd_graph(args),
        Commands::Cycles(args) => cmd_cycles(args),
        Commands::Impact(args) => cmd_impact(args),
        Commands::Functions(args) => cmd_functions(args),
    }
}

// ─────────────────────────── analyze 子命令 ───────────────────────────

fn cmd_analyze(args: crate::cli::AnalyzeArgs) -> Result<()> {
    let path = args.path.canonicalize()
        .unwrap_or(args.path.clone());

    println!(
        "{} {}",
        "分析目录:".cyan().bold(),
        path.display().to_string().yellow()
    );

    // 确定分析语言
    let lang = resolve_lang(&args.lang, &path);
    println!("{} {:?}", "检测语言:".cyan(), lang);

    // 构建图
    let mut graph = DependencyGraph::new();
    let analyzer = create_analyzer(&lang);

    // 进度条
    let pb = if args.progress {
        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} {msg}")
                .unwrap(),
        );
        pb.set_message("正在分析文件...");
        Some(pb)
    } else {
        None
    };

    let filter = crate::analyzer::FilterOpts {
        include: args.include.clone(),
        exclude: args.exclude.clone(),
    };
    analyzer.analyze_dir(&path, &mut graph, &filter)?;

    if let Some(pb) = pb {
        pb.finish_with_message(format!(
            "分析完成：{} 个节点，{} 条边",
            graph.node_count(),
            graph.edge_count()
        ));
    }

    // 如果指定了 focus 文件，过滤图为子图
    let graph = if let Some(focus) = &args.focus {
        let focus_abs = focus.canonicalize().unwrap_or(focus.clone());
        println!("{} {}", "聚焦文件:".cyan(), focus_abs.display());
        graph.subgraph(&focus_abs, args.depth)
    } else {
        graph
    };

    if args.summary {
        let s = graph.summary();
        println!("\n{}", "── 统计摘要 ──────────────────────────────".cyan());
        println!("  节点数: {}  边数: {}  循环依赖: {}", s.node_count, s.edge_count, s.cycle_count);
        if let Some((path, deg)) = &s.max_out_degree {
            println!("  最高出度: {} (依赖 {} 个文件)", path.display().to_string().yellow(), deg);
        }
        if let Some((path, deg)) = &s.max_in_degree {
            println!("  最高入度: {} (被 {} 个文件依赖)", path.display().to_string().yellow(), deg);
        }
        println!("  孤立节点: {}", s.isolated_count);
    }

    // 输出
    write_output(&graph, &args.output, args.out_file.as_deref())?;

    Ok(())
}

// ─────────────────────────── graph 子命令 ───────────────────────────

fn cmd_graph(args: crate::cli::GraphArgs) -> Result<()> {
    let path = args.path.canonicalize()
        .unwrap_or(args.path.clone());

    println!("{} {}", "生成依赖图:".cyan().bold(), path.display().to_string().yellow());

    let lang = resolve_lang(&args.lang, &path);
    let mut graph = DependencyGraph::new();
    let analyzer = create_analyzer(&lang);

    analyzer.analyze_dir(&path, &mut graph, &crate::analyzer::FilterOpts::default())?;

    println!(
        "{} {} 个节点，{} 条边",
        "完成:".green().bold(),
        graph.node_count(),
        graph.edge_count()
    );

    write_output(&graph, &args.output, args.out_file.as_deref())?;

    Ok(())
}

// ─────────────────────────── cycles 子命令 ───────────────────────────

fn cmd_cycles(args: crate::cli::CyclesArgs) -> Result<()> {
    let path = args.path.canonicalize()
        .unwrap_or(args.path.clone());

    println!("{} {}", "检测循环依赖:".cyan().bold(), path.display().to_string().yellow());

    let lang = resolve_lang(&args.lang, &path);
    let mut graph = DependencyGraph::new();
    let analyzer = create_analyzer(&lang);

    analyzer.analyze_dir(&path, &mut graph, &crate::analyzer::FilterOpts::default())?;

    let cycles = graph.detect_cycles();

    if cycles.is_empty() {
        println!("{}", "未发现循环依赖".green().bold());
        return Ok(());
    }

    println!(
        "{} 发现 {} 个循环依赖",
        "警告:".yellow().bold(),
        cycles.len()
    );

    if args.json {
        // JSON 输出
        let json = serde_json::to_string_pretty(&cycles)?;
        println!("{}", json);
    } else {
        // 文本输出
        for (i, cycle) in cycles.iter().enumerate() {
            println!("\n循环 {}:", i + 1);
            for path in cycle {
                println!("  {} {}", "→".red(), path.display().to_string().yellow());
            }
        }
    }

    Ok(())
}

// ─────────────────────────── impact 子命令 ───────────────────────────

fn cmd_impact(args: ImpactArgs) -> Result<()> {
    // 确定目标文件绝对路径
    let target = args.target.canonicalize().unwrap_or(args.target.clone());

    // 确定项目根目录：优先用 --root，否则用当前目录
    let root = match args.root {
        Some(r) => r.canonicalize().unwrap_or(r),
        None => std::env::current_dir()?,
    };

    // 函数级分析
    if let Some(ref fn_name) = args.function {
        let fg = crate::analyzer::fn_builder::analyze_dir_functions(&root)?;
        let report = fg.fn_impact(&target, fn_name, args.depth);
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }

    // 构建依赖图（原有文件级分析）
    let lang = resolve_lang(&args.lang, &root);
    let mut graph = DependencyGraph::new();
    let analyzer = create_analyzer(&lang);
    analyzer.analyze_dir(&root, &mut graph, &crate::analyzer::FilterOpts::default())?;

    // 执行影响范围分析
    let report = graph.impact(&target, args.depth);

    if args.text {
        // 人类可读的文本输出
        if report.total_affected == 0 {
            println!("{} 无文件依赖此目标，修改影响范围为零。", "✓".green().bold());
        } else {
            println!(
                "{} 修改 {} 将影响 {} 个文件：",
                "影响范围:".yellow().bold(),
                target.display().to_string().cyan(),
                report.total_affected
            );
            for node in &report.affected {
                let indent = "  ".repeat(node.depth);
                println!(
                    "{}{}  (depth={})",
                    indent,
                    node.path.display().to_string().yellow(),
                    node.depth
                );
            }
            if report.has_cycles {
                println!("{} 依赖链中存在循环依赖，影响范围可能不完整", "⚠".red());
            }
        }
    } else {
        // JSON 输出（AI 调用默认格式）
        let json = serde_json::to_string_pretty(&report)?;
        println!("{}", json);
    }

    Ok(())
}

// ─────────────────────────── functions 子命令 ───────────────────────────

fn cmd_functions(args: FunctionsArgs) -> Result<()> {
    let path = args.path.canonicalize().unwrap_or(args.path.clone());
    let fg = crate::analyzer::fn_builder::analyze_dir_functions(&path)?;

    let mut all_fns: Vec<serde_json::Value> = fg
        .graph
        .node_indices()
        .map(|idx| {
            let n = &fg.graph[idx];
            serde_json::json!({
                "name": n.name,
                "file": n.file,
                "start_line": n.start_line,
                "end_line": n.end_line,
                "language": format!("{:?}", n.language),
            })
        })
        .collect();

    // 按文件排序
    all_fns.sort_by(|a, b| {
        a["file"]
            .as_str()
            .unwrap_or("")
            .cmp(b["file"].as_str().unwrap_or(""))
    });

    println!("{}", serde_json::to_string_pretty(&all_fns)?);
    Ok(())
}

// ─────────────────────────── 工具函数 ───────────────────────────

/// 将 CLI lang 参数转换为内部 Language 枚举
fn resolve_lang(cli_lang: &Lang, path: &Path) -> crate::graph::Language {
    use crate::graph::Language;
    match cli_lang {
        Lang::Auto => detect_language(path),
        Lang::Js => Language::JavaScript,
        Lang::Ts | Lang::JsTs => Language::TypeScript,
        Lang::Rust => Language::Rust,
        Lang::Python => Language::Python,
        Lang::Go => Language::Go,
        Lang::Java => Language::Java,
        Lang::Vue => Language::Vue,
    }
}

/// 统一输出逻辑：根据格式和目标（stdout 或文件）写出依赖图
fn write_output(
    graph: &DependencyGraph,
    format: &OutputFormat,
    out_file: Option<&Path>,
) -> Result<()> {
    // 创建 writer
    let stdout = io::stdout();
    let mut stdout_lock;
    let mut file_writer;

    let writer: &mut dyn Write = if let Some(path) = out_file {
        file_writer = BufWriter::new(File::create(path)?);
        println!("{} {}", "输出到:".cyan(), path.display());
        &mut file_writer
    } else {
        stdout_lock = BufWriter::new(stdout.lock());
        &mut stdout_lock
    };

    match format {
        OutputFormat::Json => JsonOutput::new(true).write(graph, writer)?,
        OutputFormat::Dot => DotOutput::new().write(graph, writer)?,
        OutputFormat::Mermaid => MermaidOutput::new().write(graph, writer)?,
        OutputFormat::Tree => TreeOutput::new().write(graph, writer)?,
    }

    Ok(())
}
