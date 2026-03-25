/// Radar - 项目依赖关系分析工具
/// 入口文件：解析 CLI 参数，分发到对应子命令处理逻辑
mod analyzer;
mod cli;
mod error;
mod graph;
mod output;

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use clap::Parser;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};

use crate::analyzer::{create_analyzer, detect_language};
use crate::cli::{Cli, Commands, Lang, OutputFormat};
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

    analyzer.analyze_dir(&path, &mut graph)?;

    if let Some(pb) = pb {
        pb.finish_with_message(format!(
            "分析完成：{} 个节点，{} 条边",
            graph.node_count(),
            graph.edge_count()
        ));
    }

    // 如果指定了 focus 文件，过滤图
    if let Some(focus) = &args.focus {
        println!("{} {}", "聚焦文件:".cyan(), focus.display());
        // TODO: 实现聚焦过滤逻辑
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

    analyzer.analyze_dir(&path, &mut graph)?;

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

    analyzer.analyze_dir(&path, &mut graph)?;

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
        OutputFormat::Tree => TreeOutput::new(out_file.is_none()).write(graph, writer)?,
    }

    Ok(())
}
