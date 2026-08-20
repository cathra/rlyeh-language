//! `Zeta.toml` 项目清单的解析、序列化与依赖增删。
//!
//! 参考格式：
//! ```toml
//! [package]
//! name = "myapp"
//! version = "0.1.0"
//! edition = "0.2"
//!
//! [dependencies]
//! foo = "^1.0"
//!
//! [dev-dependencies]
//! bar = "*"
//! ```

use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::{Result, ZepError};
use crate::version::VersionReq;

/// 包元信息。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PackageInfo {
    /// 包名（小写字母/数字/`_`/`-`，不以 `-` 开头）。
    pub name: String,
    /// 语义化版本。
    pub version: String,
    /// 语言版本（`0.1`/`0.2` 等），默认 `0.2`。
    #[serde(default = "default_edition")]
    pub edition: String,
    /// 简介。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// 许可证。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// 作者。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,
}

fn default_edition() -> String {
    "0.2".to_string()
}

/// 构建配置（发布/调试）。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Profile {
    /// 优化等级 0-3，默认 0。
    #[serde(default)]
    pub opt_level: u8,
    /// 是否保留调试信息，默认 true。
    #[serde(default = "default_true")]
    pub debug: bool,
}

fn default_true() -> bool {
    true
}

impl Default for Profile {
    fn default() -> Self {
        Self {
            opt_level: 0,
            debug: true,
        }
    }
}

/// 工作区成员（目录列表）。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Workspace {
    /// 成员目录，相对清单所在目录。
    #[serde(default)]
    pub members: Vec<String>,
}

/// 顶层清单。
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Manifest {
    /// 包元信息。
    pub package: PackageInfo,
    /// 常规依赖：包名 → 版本需求字符串。
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub dependencies: HashMap<String, String>,
    /// 开发依赖。
    #[serde(default, skip_serializing_if = "HashMap::is_empty", rename = "dev-dependencies")]
    pub dev_dependencies: HashMap<String, String>,
    /// 构建依赖。
    #[serde(default, skip_serializing_if = "HashMap::is_empty", rename = "build-dependencies")]
    pub build_dependencies: HashMap<String, String>,
    /// 构建配置。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<Profile>,
    /// 工作区。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace: Option<Workspace>,
}

/// 依赖种类。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepKind {
    /// 常规依赖。
    Normal,
    /// 开发依赖。
    Dev,
    /// 构建依赖。
    Build,
}

impl DepKind {
    /// 显示名称（用于消息）。
    pub fn label(self) -> &'static str {
        match self {
            Self::Normal => "dependencies",
            Self::Dev => "dev-dependencies",
            Self::Build => "build-dependencies",
        }
    }
}

impl Manifest {
    /// 从字符串解析。
    pub fn parse(s: &str) -> Result<Self> {
        let m: Manifest = toml::from_str(s)
            .map_err(|e| ZepError::Manifest(format!("TOML 解析失败: {e}")))?;
        m.validate()?;
        Ok(m)
    }

    /// 从文件加载。
    pub fn load(path: &Path) -> Result<Self> {
        let s = fs::read_to_string(path).map_err(|e| {
            ZepError::Manifest(format!("读取清单 {} 失败: {e}", path.display()))
        })?;
        Self::parse(&s)
    }

    /// 序列化为 TOML 字符串。
    pub fn to_string(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| ZepError::Manifest(format!("清单序列化失败: {e}")))
    }

    /// 写入文件。
    pub fn save(&self, path: &Path) -> Result<()> {
        let s = self.to_string()?;
        fs::write(path, s).map_err(|e| {
            ZepError::Manifest(format!("写入清单 {} 失败: {e}", path.display()))
        })
    }

    /// 校验清单合法性与依赖需求可解析性。
    pub fn validate(&self) -> Result<()> {
        validate_package_name(&self.package.name).map_err(|e| ZepError::Manifest(e.to_string()))?;
        crate::version::Version::parse(&self.package.version)
            .map_err(|e| ZepError::Manifest(format!("package.version 不合法: {e}")))?;
        for (table, kind) in [
            (&self.dependencies, DepKind::Normal),
            (&self.dev_dependencies, DepKind::Dev),
            (&self.build_dependencies, DepKind::Build),
        ] {
            for (name, req) in table {
                validate_package_name(name)
                    .map_err(|e| ZepError::Manifest(e.to_string()))?;
                if req.trim().is_empty() {
                    return Err(ZepError::Manifest(format!(
                        "{}.{} 的版本需求为空",
                        kind.label(),
                        name
                    )));
                }
                // 校验需求可解析
                VersionReq::parse(req).map_err(|e| {
                    ZepError::Manifest(format!("{}.{}: {e}", kind.label(), name))
                })?;
            }
        }
        if let Some(ws) = &self.workspace {
            for m in &ws.members {
                if m.trim().is_empty() {
                    return Err(ZepError::Manifest("workspace.members 含空路径".into()));
                }
            }
        }
        Ok(())
    }

    /// 添加依赖。已存在同类型同名依赖时更新其版本需求。
    pub fn add_dependency(&mut self, name: &str, req: &str, kind: DepKind) -> Result<()> {
        validate_package_name(name)
            .map_err(|e| ZepError::Dependency(e.to_string()))?;
        VersionReq::parse(req)
            .map_err(|e| ZepError::Dependency(format!("{name}: {e}")))?;
        let table = self.table_mut(kind);
        if let Some(existing) = table.get(name) {
            if existing != req {
                table.insert(name.to_string(), req.to_string());
            }
        } else {
            table.insert(name.to_string(), req.to_string());
        }
        Ok(())
    }

    /// 移除依赖，返回是否删除成功。
    pub fn remove_dependency(&mut self, name: &str, kind: DepKind) -> bool {
        self.table_mut(kind).remove(name).is_some()
    }

    /// 取某类依赖表。
    pub fn table(&self, kind: DepKind) -> &HashMap<String, String> {
        match kind {
            DepKind::Normal => &self.dependencies,
            DepKind::Dev => &self.dev_dependencies,
            DepKind::Build => &self.build_dependencies,
        }
    }

    fn table_mut(&mut self, kind: DepKind) -> &mut HashMap<String, String> {
        match kind {
            DepKind::Normal => &mut self.dependencies,
            DepKind::Dev => &mut self.dev_dependencies,
            DepKind::Build => &mut self.build_dependencies,
        }
    }
}

/// 校验包名合法性。
pub fn validate_package_name(name: &str) -> std::result::Result<(), String> {
    let mut chars = name.chars();
    match chars.next() {
        None => return Err("包名为空".into()),
        Some(c) if !(c.is_ascii_alphanumeric() || c == '_') => {
            return Err(format!("包名 {name:?} 必须以字母/数字/下划线开头"))
        }
        Some(_) => {}
    }
    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-') {
        return Err(format!("包名 {name:?} 只能包含字母、数字、_ 和 -"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
[package]
name = "myapp"
version = "0.1.0"

[dependencies]
foo = "^1.0"
bar = "0.4"

[dev-dependencies]
baz = "*"
"#;

    #[test]
    fn parse_manifest() {
        let m = Manifest::parse(SAMPLE).unwrap();
        assert_eq!(m.package.name, "myapp");
        assert_eq!(m.package.version, "0.1.0");
        assert_eq!(m.dependencies.len(), 2);
        assert_eq!(m.dependencies.get("foo").unwrap(), "^1.0");
        assert_eq!(m.dev_dependencies.get("baz").unwrap(), "*");
    }

    #[test]
    fn validate_rejects_bad_name() {
        let bad = r#"
[package]
name = "-bad"
version = "0.1.0"
"#;
        assert!(Manifest::parse(bad).is_err());
    }

    #[test]
    fn validate_rejects_bad_req() {
        let bad = r#"
[package]
name = "x"
version = "0.1.0"

[dependencies]
foo = "not-a-version"
"#;
        assert!(Manifest::parse(bad).is_err());
    }

    #[test]
    fn add_and_remove() {
        let mut m = Manifest::parse(SAMPLE).unwrap();
        m.add_dependency("newdep", "^2.0", DepKind::Normal).unwrap();
        assert_eq!(m.dependencies.get("newdep").unwrap(), "^2.0");
        // 更新已有
        m.add_dependency("foo", "^2.0", DepKind::Normal).unwrap();
        assert_eq!(m.dependencies.get("foo").unwrap(), "^2.0");
        // 移除
        assert!(m.remove_dependency("foo", DepKind::Normal));
        assert!(!m.remove_dependency("foo", DepKind::Normal));
        assert!(!m.remove_dependency("nope", DepKind::Dev));
    }

    #[test]
    fn add_rejects_bad() {
        let mut m = Manifest::parse(SAMPLE).unwrap();
        assert!(m.add_dependency("-bad", "^1", DepKind::Normal).is_err());
        assert!(m.add_dependency("ok", "bad!!", DepKind::Normal).is_err());
        assert_eq!(m.dependencies.len(), 2);
    }

    #[test]
    fn roundtrip_serde() {
        let m = Manifest::parse(SAMPLE).unwrap();
        let s = m.to_string().unwrap();
        let m2 = Manifest::parse(&s).unwrap();
        assert_eq!(m2.package.name, m.package.name);
        assert_eq!(m2.dependencies, m.dependencies);
    }
}
