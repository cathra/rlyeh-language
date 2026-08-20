//! 依赖解析：基于 PubGrub 的版本求解与锁定。
//!
//! 流程：
//! 1. 从注册表拉取根包及其传递依赖的版本索引，构建 [`DependencyGraph`]；
//! 2. 用 PubGrub 求解满足所有约束的版本组合；
//! 3. 结果写入 `Zeta.lock`。

use std::cmp::Reverse;
use std::collections::HashMap;

use pubgrub::{
    Dependencies, DependencyConstraints, DependencyProvider, PackageResolutionStatistics, Ranges,
    SemanticVersion, resolve,
};

use crate::error::{Result, ZepError};
use crate::registry::RegistryClient;
use crate::version::{self, Version, VersionReq};

/// 图中一个包的已索引版本。
#[derive(Debug, Clone)]
pub struct IndexedVersion {
    /// 版本号。
    pub version: Version,
    /// 该版本的依赖（已解析为 [`VersionReq`]）。
    pub dependencies: HashMap<String, VersionReq>,
}

/// 依赖图：包名 → 可用版本列表。
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    packages: HashMap<String, Vec<IndexedVersion>>,
}

impl DependencyGraph {
    /// 新建空图。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从注册表构建图：BFS 拉取根包及其传递依赖的索引。
    ///
    /// `root_deps` 为根包的直接依赖（来自清单）。
    pub fn from_registry(
        root: &str,
        root_deps: &HashMap<String, VersionReq>,
        registry: &dyn RegistryClient,
    ) -> Result<Self> {
        let mut graph = Self::new();
        let mut queue: Vec<String> = root_deps.keys().cloned().collect();
        let mut fetched: HashMap<String, HashMap<String, VersionReq>> = HashMap::new();
        fetched.insert(
            root.to_string(),
            root_deps
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
        );

        while let Some(name) = queue.pop() {
            if graph.packages.contains_key(&name) {
                continue;
            }
            let idx = registry.get_index(&name)?.ok_or_else(|| {
                ZepError::Resolve(format!(
                    "依赖 {name} 不存在于注册表（{root} 需要它）"
                ))
            })?;
            let mut versions = Vec::with_capacity(idx.versions.len());
            for pv in &idx.versions {
                let v = Version::parse(&pv.version)
                    .map_err(|e| ZepError::Resolve(format!("包 {name} 版本不合法: {e}")))?;
                let mut deps = HashMap::new();
                for (dep, req) in &pv.dependencies {
                    let parsed = VersionReq::parse(req).map_err(|e| {
                        ZepError::Resolve(format!("包 {name} 的依赖 {dep} 需求非法: {e}"))
                    })?;
                    deps.insert(dep.clone(), parsed);
                    if !graph.packages.contains_key(dep) && !fetched.contains_key(dep) {
                        fetched.insert(dep.clone(), HashMap::new());
                        queue.push(dep.clone());
                    }
                }
                versions.push(IndexedVersion {
                    version: v,
                    dependencies: deps,
                });
            }
            if versions.is_empty() {
                return Err(ZepError::Resolve(format!("包 {name} 在注册表中没有任何版本")));
            }
            graph.packages.insert(name, versions);
        }
        Ok(graph)
    }

    /// 求解依赖，返回 `包名 → 选定版本`。
    pub fn resolve(
        &self,
        root: &str,
        root_version: &Version,
        root_deps: &HashMap<String, VersionReq>,
    ) -> Result<HashMap<String, Version>> {
        let provider = ZepProvider {
            graph: self,
            root: root.to_string(),
            root_version: root_version.to_semantic(),
            root_deps: root_deps.clone(),
        };
        match resolve(&provider, root.to_string(), root_version.to_semantic()) {
            Ok(selected) => {
                let mut map = HashMap::new();
                for (name, v) in selected {
                    map.insert(name, version::from_semantic(&v));
                }
                Ok(map)
            }
            Err(e) => Err(ZepError::Resolve(format!("无可用版本组合: {e}"))),
        }
    }

    /// 包的数量（用于诊断）。
    pub fn package_count(&self) -> usize {
        self.packages.len()
    }
}

/// PubGrub 依赖提供者：从内存 [`DependencyGraph`] 读取。
struct ZepProvider<'a> {
    graph: &'a DependencyGraph,
    root: String,
    /// 根包自身的版本：pubgrub 会把它当作待决策包询问版本。
    root_version: SemanticVersion,
    root_deps: HashMap<String, VersionReq>,
}

impl ZepProvider<'_> {
    /// 返回包在需求范围内的全部候选版本。
    fn candidates(&self, package: &str, range: &Ranges<SemanticVersion>) -> Vec<SemanticVersion> {
        self.graph
            .packages
            .get(package)
            .map(|versions| {
                versions
                    .iter()
                    .map(|iv| iv.version.to_semantic())
                    .filter(|v| range.contains(v))
                    .collect()
            })
            .unwrap_or_default()
    }
}

impl DependencyProvider for ZepProvider<'_> {
    type P = String;
    type V = SemanticVersion;
    type VS = Ranges<SemanticVersion>;
    type Priority = Reverse<usize>;
    type M = String;
    type Err = ZepError;

    fn prioritize(
        &self,
        package: &Self::P,
        range: &Self::VS,
        _conflicts: &PackageResolutionStatistics,
    ) -> Self::Priority {
        // 候选越少优先级越高：先解析约束最紧的包
        Reverse(self.candidates(package, range).len())
    }

    fn choose_version(&self, package: &Self::P, range: &Self::VS) -> Result<Option<Self::V>> {
        // 根包不在 graph 中：pubgrub 会以单例范围询问根包版本，直接返回固定版本
        if package == &self.root {
            return Ok(range
                .contains(&self.root_version)
                .then_some(self.root_version));
        }
        Ok(self.candidates(package, range).into_iter().max())
    }

    fn get_dependencies(
        &self,
        package: &Self::P,
        version: &Self::V,
    ) -> Result<Dependencies<Self::P, Self::VS, Self::M>> {
        if package == &self.root {
            let constraints: DependencyConstraints<_, _> = self
                .root_deps
                .iter()
                .map(|(dep, req)| (dep.clone(), req.to_ranges()))
                .collect();
            return Ok(Dependencies::Available(constraints));
        }
        let Some(versions) = self.graph.packages.get(package) else {
            return Ok(Dependencies::Unavailable(format!("包 {package} 不存在")));
        };
        let target = version::from_semantic(version);
        let Some(iv) = versions.iter().find(|iv| iv.version == target) else {
            return Ok(Dependencies::Unavailable(format!(
                "包 {package} 没有版本 {version}"
            )));
        };
        let constraints: DependencyConstraints<_, _> = iv
            .dependencies
            .iter()
            .map(|(dep, req)| (dep.clone(), req.to_ranges()))
            .collect();
        Ok(Dependencies::Available(constraints))
    }
}

/// 锁定文件 `Zeta.lock` 中的单个包条目。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LockedPackage {
    /// 包名。
    pub name: String,
    /// 锁定版本。
    pub version: String,
    /// 直接依赖声明（`name req` 形式），便于审计。
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dependencies: Vec<String>,
}

/// 锁定文件 `Zeta.lock`。
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct LockFile {
    /// 锁文件格式版本。
    pub version: u32,
    /// 根包名。
    pub root: String,
    /// 锁定条目（含根包自身）。
    pub packages: Vec<LockedPackage>,
}

impl LockFile {
    /// 锁文件格式版本。
    pub const FORMAT_VERSION: u32 = 1;

    /// 由解析结果生成锁文件。
    pub fn from_resolution(
        root: &str,
        root_version: &Version,
        graph: &DependencyGraph,
        selected: &HashMap<String, Version>,
        root_deps: &HashMap<String, VersionReq>,
    ) -> Self {
        let mut packages = Vec::with_capacity(selected.len() + 1);
        // 根包
        packages.push(LockedPackage {
            name: root.to_string(),
            version: root_version.to_string(),
            dependencies: root_deps
                .iter()
                .map(|(d, r)| format!("{d} {r}"))
                .collect(),
        });
        // 依赖包
        let mut rest: Vec<(&String, &Version)> = selected
            .iter()
            .filter(|(name, _)| *name != root)
            .collect();
        rest.sort_by(|a, b| a.0.cmp(b.0));
        for (name, version) in rest {
            let deps = graph
                .packages
                .get(name)
                .and_then(|versions| versions.iter().find(|iv| iv.version == *version))
                .map(|iv| {
                    let mut d: Vec<String> = iv
                        .dependencies
                        .iter()
                        .map(|(dep, req)| format!("{dep} {req}"))
                        .collect();
                    d.sort();
                    d
                })
                .unwrap_or_default();
            packages.push(LockedPackage {
                name: name.clone(),
                version: version.to_string(),
                dependencies: deps,
            });
        }
        Self {
            version: Self::FORMAT_VERSION,
            root: root.to_string(),
            packages,
        }
    }

    /// 序列化为 TOML。
    pub fn to_toml(&self) -> Result<String> {
        toml::to_string_pretty(self)
            .map_err(|e| ZepError::Resolve(format!("锁文件序列化失败: {e}")))
    }

    /// 解析 TOML。
    pub fn from_toml(s: &str) -> Result<Self> {
        let lf: LockFile = toml::from_str(s)
            .map_err(|e| ZepError::Resolve(format!("锁文件解析失败: {e}")))?;
        Ok(lf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{LocalRegistry, PackageIndex, PackageVersion};

    fn index(name: &str, versions: &[(&str, &[(&str, &str)])]) -> PackageIndex {
        let mut idx = PackageIndex::new(name);
        for (ver, deps) in versions {
            let dependencies: HashMap<String, String> =
                deps.iter().map(|(d, r)| (d.to_string(), r.to_string())).collect();
            idx.upsert(PackageVersion {
                version: ver.to_string(),
                dependencies,
                checksum: None,
                size: None,
            });
        }
        idx
    }

    fn seed_registry(root: &std::path::Path) {
        let reg = LocalRegistry::new(root);
        // publish 的语义是「合并单个版本到磁盘索引」，因此每个版本单独发布
        reg.publish(&index("foo", &[("1.0.0", &[])]), "1.0.0", b"foo1").unwrap();
        reg.publish(&index("foo", &[("1.5.0", &[])]), "1.5.0", b"foo15")
            .unwrap();
        reg.publish(&index("foo", &[("2.0.0", &[])]), "2.0.0", b"foo2").unwrap();
        reg.publish(&index("bar", &[("0.4.1", &[("baz", "^0.1")])]), "0.4.1", b"bar")
            .unwrap();
        reg.publish(&index("baz", &[("0.1.0", &[])]), "0.1.0", b"baz").unwrap();
        reg.publish(&index("qux", &[("1.0.0", &[])]), "1.0.0", b"qux1").unwrap();
        reg.publish(&index("qux", &[("1.1.0", &[])]), "1.1.0", b"qux11")
            .unwrap();
    }

    #[test]
    fn resolve_selects_highest_compatible() {
        let root = std::env::temp_dir().join(format!("zep-resolve-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        seed_registry(&root);

        let reg = LocalRegistry::new(&root);
        let mut root_deps = HashMap::new();
        root_deps.insert("foo".to_string(), VersionReq::parse("^1.0").unwrap());
        root_deps.insert("bar".to_string(), VersionReq::parse("^0.4").unwrap());

        let graph = DependencyGraph::from_registry("app", &root_deps, &reg).unwrap();
        assert_eq!(graph.package_count(), 3);
        let selected = graph.resolve("app", &Version::new(0, 1, 0), &root_deps).unwrap();
        // foo 选 ^1.0 内最高 1.5.0（2.0.0 不满足）
        assert_eq!(selected.get("foo").unwrap(), &Version::new(1, 5, 0));
        // bar 选 0.4.1，并传递拉取 baz 0.1.0
        assert_eq!(selected.get("bar").unwrap(), &Version::new(0, 4, 1));
        assert_eq!(selected.get("baz").unwrap(), &Version::new(0, 1, 0));
    }

    #[test]
    fn resolve_no_solution_on_conflict() {
        let root = std::env::temp_dir().join(format!("zep-conflict-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let reg = LocalRegistry::new(&root);
        reg.publish(&index("a", &[("1.0.0", &[]), ("2.0.0", &[])]), "2.0.0", b"a")
            .unwrap();
        reg.publish(
            &index("b", &[("1.0.0", &[("a", "=1.0.0")])]),
            "1.0.0",
            b"b",
        )
        .unwrap();

        let mut root_deps = HashMap::new();
        root_deps.insert("a".to_string(), VersionReq::parse("=2.0.0").unwrap());
        root_deps.insert("b".to_string(), VersionReq::parse("^1.0").unwrap());
        let graph = DependencyGraph::from_registry("app", &root_deps, &reg).unwrap();
        // a=2.0.0 与 b 要求的 a=1.0.0 冲突
        assert!(graph.resolve("app", &Version::new(0, 1, 0), &root_deps).is_err());
    }

    #[test]
    fn lockfile_roundtrip() {
        let mut root_deps = HashMap::new();
        root_deps.insert("foo".to_string(), VersionReq::parse("^1.0").unwrap());
        let mut selected = HashMap::new();
        selected.insert("foo".to_string(), Version::new(1, 5, 0));
        let mut graph = DependencyGraph::new();
        graph.packages.insert(
            "foo".to_string(),
            vec![IndexedVersion {
                version: Version::new(1, 5, 0),
                dependencies: HashMap::new(),
            }],
        );
        let lf = LockFile::from_resolution("app", &Version::new(0, 1, 0), &graph, &selected, &root_deps);
        let toml = lf.to_toml().unwrap();
        let lf2 = LockFile::from_toml(&toml).unwrap();
        assert_eq!(lf2.root, "app");
        assert_eq!(lf2.packages.len(), 2);
        assert_eq!(lf2.packages[1].name, "foo");
        assert_eq!(lf2.packages[1].version, "1.5.0");
    }
}
