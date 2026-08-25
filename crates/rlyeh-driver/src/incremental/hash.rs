//! 增量编译哈希：源码哈希 + 模块接口（函数签名）哈希。
//!
//! 双层判断语义：
//! - **源码哈希**：决定模块自身是否需要重编译（任何内容变化都触发）。
//! - **接口哈希**：仅反映公开函数签名（参数类型 + 返回类型）。
//!   函数体 / 注释 / 空白变化不改变接口哈希，供未来多模块
//!   依赖传播使用（依赖者只在被依赖模块的接口变化时重编译）。

use sha2::{Digest, Sha256};

use crate::error::DriverError;

/// 模块接口版本号：接口提取逻辑变化时应递增，使旧缓存失效。
pub const INTERFACE_VERSION: u32 = 1;

/// 模块接口：公开函数签名列表（按名称排序，保证表示稳定）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleInterface {
    /// 函数签名（已按名称排序）
    pub functions: Vec<FunctionSignature>,
}

/// 函数签名（类型以规范化字符串表示，与 `rlyeh_typecheck::Type` 的
/// `Display` 保持一致）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionSignature {
    /// 函数名
    pub name: String,
    /// 参数类型
    pub params: Vec<String>,
    /// 返回类型
    pub return_type: String,
}

/// 计算源码的 SHA-256 十六进制哈希。
pub fn compute_source_hash(source: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hex(&hasher.finalize())
}

/// 计算接口哈希：对规范化文本（每行 `name(param1,param2):ret`）做 SHA-256。
///
/// 仅依赖接口内容本身，与函数体实现无关。
pub fn compute_interface_hash(interface: &ModuleInterface) -> String {
    let mut canonical = String::new();
    canonical.push_str(&format!("rlyeh-interface-v{INTERFACE_VERSION}\n"));
    for f in &interface.functions {
        canonical.push_str(&f.name);
        canonical.push('(');
        canonical.push_str(&f.params.join(","));
        canonical.push_str("):");
        canonical.push_str(&f.return_type);
        canonical.push('\n');
    }
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    hex(&hasher.finalize())
}

/// 从源码解析并提取模块接口（仅 parse + 签名解析，不检查函数体）。
///
/// 显著轻于完整流水线，增量命中时无需走到类型检查。
pub fn extract_interface(source: &str) -> Result<ModuleInterface, DriverError> {
    let program =
        rlyeh_parser::parse(source).map_err(|e| DriverError::Typecheck(format!("语法错误: {e}")))?;
    let sigs = rlyeh_typecheck::collect_fn_signatures(&program)
        .map_err(|e| DriverError::Typecheck(e.to_string()))?;
    Ok(ModuleInterface {
        functions: sigs
            .into_iter()
            .map(|(name, sig)| FunctionSignature {
                name,
                params: sig.params.iter().map(|t| t.to_string()).collect(),
                return_type: sig.return_type.to_string(),
            })
            .collect(),
    })
}

/// SHA-256 摘要转十六进制字符串。
fn hex(digest: &[u8]) -> String {
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_hash_is_stable() {
        let src = "fn main() { println(\"hi\"); }";
        assert_eq!(compute_source_hash(src), compute_source_hash(src));
    }

    #[test]
    fn source_hash_differs_on_change() {
        let a = compute_source_hash("fn main() {}");
        let b = compute_source_hash("fn main() { println(\"x\"); }");
        assert_ne!(a, b);
    }

    #[test]
    fn interface_hash_ignores_body_changes() {
        let a = extract_interface("fn add(a: i64, b: i64) -> i64 { a + b }").unwrap();
        // 函数体变化但签名不变 → 接口哈希不变
        let b = extract_interface("fn add(a: i64, b: i64) -> i64 { a - b }").unwrap();
        assert_eq!(compute_interface_hash(&a), compute_interface_hash(&b));
    }

    #[test]
    fn interface_hash_changes_on_signature_change() {
        let a = extract_interface("fn add(a: i64, b: i64) -> i64 { a + b }").unwrap();
        let b = extract_interface("fn add(a: i64, b: f64) -> i64 { a }").unwrap();
        assert_ne!(compute_interface_hash(&a), compute_interface_hash(&b));
    }

    #[test]
    fn interface_hash_is_sorted_stable() {
        let a = extract_interface("fn foo() {} fn bar(a: i64) -> i64 { a }").unwrap();
        let b = extract_interface("fn bar(a: i64) -> i64 { a } fn foo() {}").unwrap();
        // 声明顺序不同但接口相同 → 哈希相同
        assert_eq!(compute_interface_hash(&a), compute_interface_hash(&b));
    }

    #[test]
    fn extract_interface_reports_parse_error() {
        assert!(extract_interface("fn ( {").is_err());
    }
}
