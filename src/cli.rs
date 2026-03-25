/// CLI 参数定义（使用 clap derive）
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// Radar —— 项目依赖关系分析工具
#[derive(Parser, Debug)]
#[command(
    name = "radar",
    version,
    about = "分析项目依赖关系，生成依赖地图",
    long_about = "支持 JS/TS、Rust、Python 等多种语言，\n输出格式支持 JSON、DOT（Graphviz）、Mermaid、Tree"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// 子命令
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// 分析指定目录的依赖关系
    Analyze(AnalyzeArgs),

    /// 生成完整依赖图（默认输出 DOT 格式）
    Graph(GraphArgs),

    /// 检测循环依赖
    Cycles(CyclesArgs),
}

/// `analyze` 子命令参数
#[derive(Parser, Debug)]
pub struct AnalyzeArgs {
    /// 要分析的目录路径
    pub path: PathBuf,

    /// 指定分析语言（不指定则自动检测）
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 依赖分析的最大深度（0 表示不限制）
    #[arg(long, short = 'd', default_value = "0")]
    pub depth: usize,

    /// 输出格式
    #[arg(long, short = 'o', value_enum, default_value = "tree")]
    pub output: OutputFormat,

    /// 输出到文件（不指定则输出到 stdout）
    #[arg(long, short = 'f')]
    pub out_file: Option<PathBuf>,

    /// 包含的文件 glob 模式（可多次指定）
    #[arg(long, short = 'i')]
    pub include: Vec<String>,

    /// 排除的文件 glob 模式（可多次指定）
    #[arg(long, short = 'e')]
    pub exclude: Vec<String>,

    /// 聚焦某个文件，只显示该文件的直接依赖
    #[arg(long)]
    pub focus: Option<PathBuf>,

    /// 是否显示进度条
    #[arg(long, default_value = "true")]
    pub progress: bool,
}

/// `graph` 子命令参数
#[derive(Parser, Debug)]
pub struct GraphArgs {
    /// 要分析的目录路径
    pub path: PathBuf,

    /// 输出格式
    #[arg(long, short = 'o', value_enum, default_value = "dot")]
    pub output: OutputFormat,

    /// 输出到文件
    #[arg(long, short = 'f')]
    pub out_file: Option<PathBuf>,

    /// 指定语言
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,
}

/// `cycles` 子命令参数
#[derive(Parser, Debug)]
pub struct CyclesArgs {
    /// 要分析的目录路径
    pub path: PathBuf,

    /// 指定语言
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 以 JSON 格式输出循环依赖列表
    #[arg(long)]
    pub json: bool,
}

/// 支持的语言类型
#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum Lang {
    /// 自动检测
    Auto,
    /// JavaScript
    Js,
    /// TypeScript
    Ts,
    /// JavaScript + TypeScript（混合项目）
    JsTs,
    /// Rust
    Rust,
    /// Python
    Python,
}

/// 输出格式
#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum OutputFormat {
    /// JSON 格式
    Json,
    /// Graphviz DOT 格式
    Dot,
    /// Mermaid 图表格式
    Mermaid,
    /// 终端树形展示
    Tree,
}
