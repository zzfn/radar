/// 统一错误类型
/// 使用 anyhow 作为底层，这里定义领域相关的具体错误
use std::path::PathBuf;

/// Radar 自定义错误枚举
#[derive(Debug, thiserror::Error)]
pub enum RadarError {
    #[error("路径不存在: {0}")]
    PathNotFound(PathBuf),

    #[error("不支持的语言: {0}")]
    UnsupportedLanguage(String),

    #[error("文件读取失败: {path}, 原因: {source}")]
    FileReadError {
        path: PathBuf,
        source: std::io::Error,
    },

    #[error("输出格式不支持: {0}")]
    UnsupportedOutputFormat(String),

    #[error("依赖图构建失败: {0}")]
    GraphBuildError(String),

    #[error("正则表达式错误: {0}")]
    RegexError(#[from] regex::Error),

    #[error("JSON 序列化错误: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO 错误: {0}")]
    IoError(#[from] std::io::Error),
}

/// 全局 Result 类型别名
pub type Result<T> = anyhow::Result<T>;
