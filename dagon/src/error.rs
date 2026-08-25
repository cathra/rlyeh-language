//! Dagon 统一错误类型。

/// Dagon 全链路错误。
#[derive(Debug, thiserror::Error)]
pub enum DagonError {
    /// 清单（Rlyeh.toml）相关错误。
    #[error("清单错误: {0}")]
    Manifest(String),

    /// 依赖声明不合法。
    #[error("依赖声明不合法: {0}")]
    Dependency(String),

    /// 版本字符串解析失败。
    #[error("版本解析失败: {0}")]
    Version(String),

    /// 版本需求字符串解析失败。
    #[error("版本需求解析失败: {0}")]
    VersionReq(String),

    /// 依赖解析（PubGrub）无解或出错。
    #[error("依赖解析失败: {0}")]
    Resolve(String),

    /// 注册表访问失败。
    #[error("注册表错误: {0}")]
    Registry(String),

    /// 包内容损坏（校验和/格式）。
    #[error("包校验失败: {0}")]
    Package(String),

    /// 构建子进程失败。
    #[error("构建失败: {0}")]
    Build(String),

    /// 外部命令找不到。
    #[error("找不到命令: {0}")]
    CommandNotFound(String),

    /// 子进程执行超时。
    #[error("命令执行超时: {0}")]
    Timeout(String),

    /// 沙箱拒绝。
    #[error("沙箱拒绝: {0}")]
    Sandbox(String),

    /// 一般 IO 错误。
    #[error("IO 错误: {0}")]
    Io(#[from] std::io::Error),

    /// 一般错误。
    #[error("{0}")]
    Other(String),
}

/// Dagon 便捷结果别名。
pub type Result<T> = std::result::Result<T, DagonError>;
