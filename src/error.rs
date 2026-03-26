/// 统一错误类型
/// 使用 anyhow 作为底层，这里定义领域相关的具体错误

/// 全局 Result 类型别名
pub type Result<T> = anyhow::Result<T>;
