//! 编译驱动错误。

use std::fmt;

/// 编译驱动错误（聚合流水线各阶段错误）。
#[derive(Debug)]
pub enum DriverError {
    /// I/O 错误（读文件 / 临时文件）
    Io(std::io::Error),
    /// 类型检查阶段
    Typecheck(String),
    /// 借用检查阶段
    Borrow(String),
    /// 区域检查阶段
    Region(String),
    /// LIR lowering 阶段
    Lir(String),
    /// 代码生成阶段
    Codegen(String),
    /// clang 汇编 / 链接失败
    Clang(String),
    /// 运行产物失败
    Run(String),
    /// 增量缓存操作失败
    Cache(String),
    /// CLI 参数错误
    Usage(String),
    /// 模块加载失败（`mod foo;` 外部模块缺失 / 循环引用 / 解析失败）
    Module(String),
}

impl fmt::Display for DriverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DriverError::Io(e) => write!(f, "I/O 错误: {e}"),
            DriverError::Typecheck(m) => write!(f, "[typecheck] {m}"),
            DriverError::Borrow(m) => write!(f, "[borrowck] {m}"),
            DriverError::Region(m) => write!(f, "[regionck] {m}"),
            DriverError::Lir(m) => write!(f, "[lir] {m}"),
            DriverError::Codegen(m) => write!(f, "[codegen] {m}"),
            DriverError::Clang(m) => write!(f, "[clang] {m}"),
            DriverError::Run(m) => write!(f, "[run] {m}"),
            DriverError::Cache(m) => write!(f, "[cache] {m}"),
            DriverError::Usage(m) => write!(f, "{m}"),
            DriverError::Module(m) => write!(f, "[module] {m}"),
        }
    }
}

impl std::error::Error for DriverError {}
