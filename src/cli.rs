/// CLI 参数定义（使用 clap derive）
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

/// context 子命令的输出格式（只支持 json 和 markdown）
#[derive(ValueEnum, Debug, Clone, PartialEq, Eq)]
pub enum ContextOutputFormat {
    /// JSON 格式（结构化，适合程序解析）
    Json,
    /// Markdown 格式（紧凑，适合 AI 直接读取）
    Markdown,
}

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

    /// 分析修改某文件后的影响范围（适合 AI 调用）
    Impact(ImpactArgs),

    /// 列出项目中所有函数定义（JSON 格式，供 AI 探查使用）
    Functions(FunctionsArgs),

    /// 检测未被任何文件引用的死代码（文件级 + 函数级）
    Unused(UnusedArgs),

    /// 列出被最多文件依赖的高风险节点
    Hotspot(HotspotArgs),

    /// 查找两个文件之间的最短依赖路径
    Path(PathArgs),

    /// 生成目标文件/函数的完整上下文（影响范围 + 调用者 + 循环检测），一次调用供 AI 决策
    Context(ContextArgs),
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

    /// 输出统计摘要（节点数、入/出度、孤立节点等）
    #[arg(long)]
    pub summary: bool,
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

/// `impact` 子命令参数
#[derive(Parser, Debug)]
pub struct ImpactArgs {
    /// 被修改的目标文件路径
    pub target: PathBuf,

    /// 项目根目录（用于构建完整依赖图，默认为当前目录）
    #[arg(long, short = 'r')]
    pub root: Option<PathBuf>,

    /// 最大影响深度（0 = 不限制）
    #[arg(long, short = 'd', default_value = "0")]
    pub depth: usize,

    /// 指定语言（不指定则自动检测）
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 以纯文本格式输出（默认 JSON）
    #[arg(long)]
    pub text: bool,

    /// 指定函数名，执行函数级影响分析（需要 tree-sitter 支持的语言）
    /// 示例：--function verify_token
    #[arg(long, short = 'n')]
    pub function: Option<String>,
}

/// `functions` 子命令参数
#[derive(Parser, Debug)]
pub struct FunctionsArgs {
    /// 要分析的目录路径
    pub path: PathBuf,

    /// 指定语言过滤（不指定则全部支持的语言）
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 输出格式（json/dot/mermaid/tree）
    #[arg(long, short = 'o', value_enum, default_value = "json")]
    pub output: OutputFormat,

    /// 输出到文件（不指定则输出到 stdout）
    #[arg(long, short = 'f')]
    pub out_file: Option<PathBuf>,
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
    /// Go
    Go,
    /// Java
    Java,
    /// Vue（单文件组件）
    Vue,
}

/// `unused` 子命令参数
#[derive(Parser, Debug)]
pub struct UnusedArgs {
    /// 要分析的目录路径
    pub path: PathBuf,

    /// 指定语言
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 同时检测未被调用的函数（基于 tree-sitter）
    #[arg(long)]
    pub functions: bool,

    /// 包含常见入口文件（main.rs、index.ts 等，默认跳过）
    #[arg(long)]
    pub include_entry: bool,

    /// 输出格式
    #[arg(long, short = 'o', value_enum, default_value = "tree")]
    pub output: OutputFormat,
}

/// `hotspot` 子命令参数
#[derive(Parser, Debug)]
pub struct HotspotArgs {
    /// 要分析的目录路径
    pub path: PathBuf,

    /// 指定语言
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 显示前 N 个高风险节点（0 = 全部）
    #[arg(long, short = 'n', default_value = "10")]
    pub top: usize,

    /// 输出格式
    #[arg(long, short = 'o', value_enum, default_value = "tree")]
    pub output: OutputFormat,
}

/// `path` 子命令参数
#[derive(Parser, Debug)]
pub struct PathArgs {
    /// 要分析的目录路径
    pub path: PathBuf,

    /// 起始文件
    #[arg(long)]
    pub from: PathBuf,

    /// 目标文件
    #[arg(long)]
    pub to: PathBuf,

    /// 指定语言
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 输出格式
    #[arg(long, short = 'o', value_enum, default_value = "tree")]
    pub output: OutputFormat,
}

/// `context` 子命令参数
#[derive(Parser, Debug)]
pub struct ContextArgs {
    /// 目标文件的绝对路径
    pub target: PathBuf,

    /// 项目根目录（用于构建依赖图，默认当前目录）
    #[arg(long, short = 'r')]
    pub root: Option<PathBuf>,

    /// 同时分析指定函数的调用者（需语言支持 tree-sitter）
    #[arg(long, short = 'n')]
    pub function: Option<String>,

    /// 影响范围最大深度（0 = 不限）
    #[arg(long, short = 'd', default_value = "5")]
    pub depth: usize,

    /// 指定语言（不指定则自动检测）
    #[arg(long, short = 'l', value_enum, default_value = "auto")]
    pub lang: Lang,

    /// 输出格式（默认 markdown，token 更紧凑）
    #[arg(long, short = 'o', value_enum, default_value = "markdown")]
    pub output: ContextOutputFormat,
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
