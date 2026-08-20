//! # 模块加载
//!
//! 将入口文件及其全部外部子模块（`mod foo;`）展开为单文件等价源码。
//!
//! 文件解析规则（对齐 Rust）：
//! - 入口 `main.zeta` 中 `mod math;` → `<项目目录>/math.zeta` 或 `<项目目录>/math/mod.zeta`
//! - `a.zeta` 中 `mod b;` → `<项目目录>/a/b.zeta` 或 `<项目目录>/a/b/mod.zeta`
//! - `a/b/mod.zeta` 中 `mod c;` → `<项目目录>/a/b/c.zeta`
//!
//! 展开方式为文本级变换：解析每个文件，利用 AST span 精确定位外部
//! `mod name;` 声明，替换为 `mod name { <子模块源码> }`（递归展开）。
//! 展开结果交给既有的单文件流水线（typecheck 已支持嵌套 `mod` 与扁平
//! 符号名 `mod::item`），增量缓存哈希天然覆盖全部模块文件。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use zeta_ast::AstItem;

use crate::error::DriverError;

/// 加载入口文件及其全部外部子模块，返回组合后的单文件等价源码。
pub(crate) fn load_combined_source(entry: &Path) -> Result<String, DriverError> {
    let mut visited = HashSet::new();
    let project_dir = entry
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    load_file(entry, Path::new(""), &project_dir, &mut visited)
}

/// 读取并解析单个模块文件，将其外部子模块声明展开为内联模块。
///
/// `mod_root` 是该文件在项目内的逻辑模块路径（入口为空，`a.zeta` 为 `a`）。
fn load_file(
    path: &Path,
    mod_root: &Path,
    project_dir: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<String, DriverError> {
    let canon = path.canonicalize().map_err(DriverError::Io)?;
    if !visited.insert(canon.clone()) {
        return Err(DriverError::Module(format!(
            "检测到模块循环引用: `{}`（{})",
            path.display(),
            canon.display()
        )));
    }

    let source = std::fs::read_to_string(path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            DriverError::Module(format!("找不到模块文件: {}", path.display()))
        } else {
            DriverError::Io(e)
        }
    })?;

    let program = zeta_parser::parse(&source).map_err(|e| {
        DriverError::Typecheck(format!("语法错误 ({}): {e}", path.display()))
    })?;

    let mut out = String::with_capacity(source.len() + 64);
    let mut cursor = 0usize;

    for item in &program.items {
        let AstItem::ModDecl(m) = item else { continue };
        if !m.external {
            continue;
        }
        // 保留 `mod foo;` 之前的源码
        out.push_str(&source[cursor..m.span.start]);
        // 展开为内联模块 `mod foo { <子模块源码> }`
        out.push_str(&format!("mod {} {{\n", m.name));
        out.push_str(&load_submodule(mod_root, &m.name, project_dir, visited)?);
        out.push_str("\n}\n");
        cursor = m.span.end;
    }

    // 追加最后一个外部模块声明之后的源码
    out.push_str(&source[cursor..]);
    Ok(out)
}

/// 定位并加载子模块：优先 `<project>/<mod_root>/<name>.zeta`，
/// 回退 `<project>/<mod_root>/<name>/mod.zeta`。
fn load_submodule(
    mod_root: &Path,
    name: &str,
    project_dir: &Path,
    visited: &mut HashSet<PathBuf>,
) -> Result<String, DriverError> {
    let sub_root = mod_root.join(name);
    let file_candidate = project_dir.join(&sub_root).with_extension("zeta");
    let dir_candidate = project_dir.join(&sub_root).join("mod.zeta");
    if file_candidate.is_file() {
        load_file(&file_candidate, &sub_root, project_dir, visited)
    } else if dir_candidate.is_file() {
        load_file(&dir_candidate, &sub_root, project_dir, visited)
    } else {
        Err(DriverError::Module(format!(
            "模块 `{name}` 未找到：尝试过 {} 与 {}",
            file_candidate.display(),
            dir_candidate.display()
        )))
    }
}
