# P008: 包管理器 Zep

> **模块路径**：`zep/`  
> **预估工期**：5-7 天  
> **前置依赖**：P007（编译器驱动）  
> **输出**：可用的包管理器，支持依赖解析、构建、发布

---

## 任务描述

实现 Zeta 的包管理器 **Zep**（Zeta Package Manager）：
1. **项目初始化与配置**
2. **依赖解析**（PubGrub 算法）
3. **沙箱构建**（安全下载和编译依赖）
4. **注册表与发布**

---

## 配置文件格式

### Zeta.toml

```toml
[package]
name = "my-project"
version = "0.1.0"
edition = "2021"
authors = ["Alice <alice@example.com>"]
description = "A Zeta project"
license = "MIT OR Apache-2.0"
repository = "https://github.com/example/my-project"

[dependencies]
serde = "1.0"
tokio = { version = "2.0", features = ["full"] }
regex = "1.5"

[dependencies.local-crate]
path = "../local-crate"

[dev-dependencies]
criterion = "0.5"

[build-dependencies]
cc = "1.0"

[profile.release]
opt_level = 3
lto = true
codegen_units = 1

[profile.dev]
opt_level = 0
debug = true

[workspace]
members = ["crates/*"]
```

---

## 代码框架

```rust
// zep/Cargo.toml
[package]
name = "zep"
version = "0.1.0"
edition = "2021"

[dependencies]
toml = "0"
serde = { version = "1", features = ["derive"] }
pubgrub = "0"
reqwest = { version = "0.11", features = ["json"] }
tar = "0"
flate2 = "1"
sha2 = "0.10"
dirs = "5"
anyhow = "1"
thiserror = "1"
clap = { version = "4", features = ["derive"] }
colored = "2"
```

```rust
// zep/src/main.rs

#![warn(missing_docs)]

use clap::{Parser, Subcommand};
use thiserror::Error;

#[derive(Parser)]
#[command(name = "zep")]
#[command(about = "Zeta Package Manager", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 创建新项目
    New {
        name: String,
        #[arg(long)]
        lib: bool,
    },
    /// 初始化现有项目
    Init {
        #[arg(long)]
        name: Option<String>,
    },
    /// 添加依赖
    Add {
        packages: Vec<String>,
        #[arg(long)]
        dev: bool,
    },
    /// 移除依赖
    Remove {
        packages: Vec<String>,
    },
    /// 构建项目
    Build {
        #[arg(long)]
        release: bool,
        #[arg(long)]
        jobs: Option<usize>,
    },
    /// 运行项目
    Run {
        #[arg(long)]
        release: bool,
        args: Vec<String>,
    },
    /// 运行测试
    Test {
        #[arg(long)]
        filter: Option<String>,
    },
    /// 更新依赖
    Update {
        #[arg(long)]
        dry_run: bool,
    },
    /// 发布到注册表
    Publish {
        #[arg(long)]
        registry: Option<String>,
    },
    /// 搜索包
    Search {
        query: String,
    },
    /// 清理构建产物
    Clean,
}

#[derive(Debug, Error)]
enum ZepError {
    #[error("failed to read {path}: {source}")]
    Io { path: String, source: std::io::Error },
    
    #[error("invalid Zeta.toml: {reason}")]
    InvalidManifest { reason: String },
    
    #[error("dependency resolution failed: {reason}")]
    ResolveFailed { reason: String },
    
    #[error("package '{name}' not found in registry")]
    PackageNotFound { name: String },
    
    #[error("checksum mismatch for {package}: expected {expected}, got {actual}")]
    ChecksumMismatch { package: String, expected: String, actual: String },
    
    #[error("sandbox violation: {reason}")]
    SandboxViolation { reason: String },
    
    #[error("build failed: {reason}")]
    BuildFailed { reason: String },
}

fn main() {
    let cli = Cli::parse();
    
    let result = match cli.command {
        Commands::New { name, lib } => cmd_new(name, lib),
        Commands::Init { name } => cmd_init(name),
        Commands::Add { packages, dev } => cmd_add(packages, dev),
        Commands::Remove { packages } => cmd_remove(packages),
        Commands::Build { release, jobs } => cmd_build(release, jobs),
        Commands::Run { release, args } => cmd_run(release, args),
        Commands::Test { filter } => cmd_test(filter),
        Commands::Update { dry_run } => cmd_update(dry_run),
        Commands::Publish { registry } => cmd_publish(registry),
        Commands::Search { query } => cmd_search(query),
        Commands::Clean => cmd_clean(),
    };
    
    if let Err(e) = result {
        eprintln!("error: {}", e);
        std::process::exit(1);
    }
}
```

---

## 必须实现的功能清单

### 1. 项目初始化

```rust
// zep/src/commands/new.rs

/// 创建新项目
pub fn cmd_new(name: String, is_lib: bool) -> Result<(), ZepError> {
    todo!("创建项目目录结构");
    todo!("生成 Zeta.toml");
    todo!("生成 .gitignore");
    todo!("生成 src/main.zeta 或 src/lib.zeta");
    todo!("初始化 git 仓库（如果可用）");
}

/// 项目模板
fn generate_main_template(name: &str) -> String {
    format!(r#"
// {name}
fn main() {{
    println!("Hello, Zeta!");
}}
"#, name = name)
}

fn generate_lib_template(name: &str) -> String {
    format!(r#"
// {name} library
pub fn hello() {{
    println!("Hello from {name}!");
}}
"#, name = name)
}
```

### 2. 依赖解析（PubGrub）

```rust
// zep/src/resolve/mod.rs

use pubgrub::{
    solver::{resolve, Dependencies, Offset, PubGrubError},
    version::NumVersion,
};
use std::collections::HashMap;

/// 包版本
pub type Version = NumVersion;

/// 依赖关系图
pub struct DependencyGraph {
    /// package -> version -> dependencies
    pub deps: HashMap<String, HashMap<Version, Dependencies<String, Version>>>,
}

impl DependencyGraph {
    pub fn new() -> Self {
        Self { deps: HashMap::new() }
    }
    
    /// 从注册表获取依赖信息
    pub fn fetch_from_registry(&mut self, package: &str, version: &Version) {
        todo!("查询注册表 API");
        todo!("解析依赖列表");
        todo!("递归获取传递依赖");
    }
}

/// 解析依赖
pub fn resolve_dependencies(
    root: &str,
    root_version: Version,
    graph: &DependencyGraph,
) -> Result<HashMap<String, Version>, ResolveError> {
    // 使用 PubGrub 算法
    let solution = resolve(
        root.to_string(),
        root_version,
        |package, version| {
            graph.deps
                .get(&package)
                .and_then(|versions| versions.get(&version))
                .cloned()
                .unwrap_or_else(Dependencies::unknown)
        },
        |package, version, deps| {
            // 偏移量计算
            Offset::zero()
        },
    )?;
    
    Ok(solution.into_iter().collect())
}
```

### 3. 沙箱构建

```rust
// zep/src/sandbox/mod.rs

use std::path::Path;
use std::process::{Command, Stdio};

/// 沙箱配置
pub struct SandboxConfig {
    /// 允许的网络访问（白名单域名）
    pub allowed_hosts: Vec<String>,
    /// 允许的文件系统访问
    pub allowed_paths: Vec<std::path::PathBuf>,
    /// 内存限制（MB）
    pub memory_limit: usize,
    /// CPU 时间限制（秒）
    pub cpu_limit: usize,
}

impl Default for SandboxConfig {
    fn default() -> Self {
        Self {
            allowed_hosts: vec![
                "crates.io".to_string(),
                "github.com".to_string(),
            ],
            allowed_paths: vec![
                std::env::temp_dir(),
            ],
            memory_limit: 1024,  // 1GB
            cpu_limit: 300,       // 5 分钟
        }
    }
}

/// 在沙箱中构建依赖
pub fn build_in_sandbox(
    package_dir: &Path,
    config: &SandboxConfig,
) -> Result<BuildArtifact, SandboxError> {
    #[cfg(target_os = "linux")]
    {
        build_with_bwrap(package_dir, config)
    }
    #[cfg(not(target_os = "linux"))]
    {
        // macOS/Windows 使用其他隔离机制
        build_with_restricted_env(package_dir, config)
    }
}

#[cfg(target_os = "linux")]
fn build_with_bwrap(
    package_dir: &Path,
    config: &SandboxConfig,
) -> Result<BuildArtifact, SandboxError> {
    let mut cmd = Command::new("bwrap");
    
    // 基本隔离
    cmd.arg("--unshare-net");  // 默认无网络
    cmd.arg("--ro-bind").arg("/usr").arg("/usr");
    cmd.arg("--ro-bind").arg("/lib").arg("/lib");
    cmd.arg("--ro-bind").arg("/bin").arg("/bin");
    
    // 允许的网络（通过 proxy）
    // ...
    
    // 临时根目录
    cmd.arg("--tmpfs").arg("/tmp");
    cmd.arg("--bind").arg(package_dir).arg(package_dir);
    
    // 内存限制
    cmd.arg("--size").arg(&format!("{}M", config.memory_limit));
    
    // 执行构建
    cmd.arg("zeta").arg("build").arg("--release");
    cmd.arg("--").arg(package_dir);
    
    let output = cmd.stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;
    
    if !output.status.success() {
        return Err(SandboxError::BuildFailed {
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    
    Ok(BuildArtifact {
        path: package_dir.join("target/release"),
        metadata: ArtifactMetadata::parse(&output.stdout)?,
    })
}
```

### 4. 注册表与发布

```rust
// zep/src/registry/mod.rs

use serde::{Deserialize, Serialize};
use reqwest::Client;

/// 注册表客户端
pub struct RegistryClient {
    base_url: String,
    client: Client,
    auth_token: Option<String>,
}

/// 包元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub license: String,
    pub repository: Option<String>,
    pub dependencies: Vec<DependencySpec>,
    pub checksum: String,  // SHA-256
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencySpec {
    pub name: String,
    pub version_req: String,  // e.g. "^1.0"
    pub features: Vec<String>,
}

/// 版本约束
#[derive(Debug, Clone)]
pub enum VersionReq {
    /// 兼容版本（^1.0 = >=1.0,<2.0）
    Compatible(String),
    /// 精确版本（=1.2.3）
    Exact(String),
    /// 大于等于（>=1.0）
    GreaterEq(String),
    /// 任意版本（*）
    Any,
    /// 范围（>=1.0, <2.0）
    Range { min: Option<String>, max: Option<String> },
}

impl VersionReq {
    /// 检查版本是否满足约束
    pub fn matches(&self, version: &Version) -> bool {
        match self {
            VersionReq::Any => true,
            VersionReq::Exact(v) => version == &parse_version(v),
            VersionReq::Compatible(v) => {
                let base = parse_version(v);
                version >= &base && version.major() == base.major()
            }
            VersionReq::GreaterEq(v) => version >= &parse_version(v),
            VersionReq::Range { min, max } => {
                let ok_min = min.as_ref().map_or(true, |v| version >= &parse_version(v));
                let ok_max = max.as_ref().map_or(true, |v| version < &parse_version(v));
                ok_min && ok_max
            }
        }
    }
}

impl RegistryClient {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
            auth_token: None,
        }
    }
    
    /// 搜索包
    pub async fn search(&self, query: &str) -> Result<Vec<PackageMeta>, RegistryError> {
        let url = format!("{}/search?q={}", self.base_url, query);
        let resp = self.client.get(&url).send().await?;
        let results: Vec<PackageMeta> = resp.json().await?;
        Ok(results)
    }
    
    /// 下载包
    pub async fn download(
        &self,
        name: &str,
        version: &Version,
    ) -> Result<Vec<u8>, RegistryError> {
        let url = format!("{}/packages/{}/{}", self.base_url, name, version);
        let resp = self.client.get(&url).send().await?;
        Ok(resp.bytes().await?.to_vec())
    }
    
    /// 获取包元数据
    pub async fn get_metadata(
        &self,
        name: &str,
        version: &Version,
    ) -> Result<PackageMeta, RegistryError> {
        let url = format!("{}/meta/{}/{}", self.base_url, name, version);
        let resp = self.client.get(&url).send().await?;
        let meta: PackageMeta = resp.json().await?;
        Ok(meta)
    }
    
    /// 发布包
    pub async fn publish(
        &self,
        package: &PackageMeta,
        tarball: Vec<u8>,
    ) -> Result<(), RegistryError> {
        let url = format!("{}/publish", self.base_url);
        
        let mut form = reqwest::multipart::Form::new();
        form = form.text("metadata", serde_json::to_string(package)?);
        form = form.part(
            "package",
            reqwest::multipart::Part::bytes(tarball)
                .file_name(format!("{}-{}.tar.gz", package.name, package.version)),
        );
        
        self.client
            .post(&url)
            .bearer_auth(self.auth_token.as_deref().unwrap_or(""))
            .multipart(form)
            .send()
            .await?
            .error_for_status()?;
        
        Ok(())
    }
    
    /// 登录
    pub async fn login(&mut self, token: String) {
        self.auth_token = Some(token);
    }
}
```

### 5. 构建集成

```rust
// zep/src/build/mod.rs

use std::path::Path;
use std::process::Command;

/// 构建配置
pub struct BuildConfig {
    pub release: bool,
    pub jobs: usize,
    pub target: Option<String>,
    pub features: Vec<String>,
}

/// 构建产物
pub struct BuildArtifact {
    pub path: std::path::PathBuf,
    pub binary: Option<std::path::PathBuf>,
    pub libraries: Vec<std::path::PathBuf>,
}

/// 执行构建
pub fn build(
    project_dir: &Path,
    config: &BuildConfig,
) -> Result<BuildArtifact, BuildError> {
    let mut cmd = Command::new("zeta");
    cmd.arg("build");
    
    if config.release {
        cmd.arg("--release");
    }
    
    cmd.arg("--jobs").arg(config.jobs.to_string());
    
    if let Some(target) = &config.target {
        cmd.arg("--target").arg(target);
    }
    
    for feature in &config.features {
        cmd.arg("--feature").arg(feature);
    }
    
    cmd.current_dir(project_dir);
    
    let output = cmd.output()?;
    
    if !output.status.success() {
        return Err(BuildError::Failed {
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    
    // 查找产物
    let target_dir = project_dir.join("target");
    let profile = if config.release { "release" } else { "debug" };
    let artifact_dir = target_dir.join(profile);
    
    Ok(BuildArtifact {
        path: artifact_dir.clone(),
        binary: find_binary(&artifact_dir),
        libraries: find_libraries(&artifact_dir),
    })
}

fn find_binary(dir: &Path) -> Option<std::path::PathBuf> {
    // 查找可执行文件
    todo!()
}

fn find_libraries(dir: &Path) -> Vec<std::path::PathBuf> {
    // 查找 .so / .dll / .dylib
    todo!()
}
```

---

## 测试用例

```rust
// zep/tests/zep_test.rs

use std::fs;
use tempfile::TempDir;

fn setup_test_dir() -> TempDir {
    let dir = TempDir::new().unwrap();
    // 创建测试项目
    dir
}

#[test]
fn test_init_project() {
    let dir = setup_test_dir();
    let output = std::process::Command::new("zep")
        .arg("init")
        .arg("--name=test-proj")
        .current_dir(dir.path())
        .output()
        .unwrap();
    
    assert!(output.status.success());
    assert!(dir.path().join("Zeta.toml").exists());
}

#[test]
fn test_add_dependency() {
    let dir = setup_test_dir();
    init_project(dir.path());
    
    let output = std::process::Command::new("zep")
        .arg("add").arg("serde@1.0")
        .current_dir(dir.path())
        .output()
        .unwrap();
    
    assert!(output.status.success());
    
    let manifest = fs::read_to_string(dir.path().join("Zeta.toml")).unwrap();
    assert!(manifest.contains("serde"));
    assert!(manifest.contains("1.0"));
}

#[test]
fn test_remove_dependency() {
    let dir = setup_test_dir();
    init_project(dir.path());
    add_dep(dir.path(), "serde@1.0");
    
    let output = std::process::Command::new("zep")
        .arg("remove").arg("serde")
        .current_dir(dir.path())
        .output()
        .unwrap();
    
    assert!(output.status.success());
    
    let manifest = fs::read_to_string(dir.path().join("Zeta.toml")).unwrap();
    assert!(!manifest.contains("serde"));
}

#[test]
fn test_dependency_resolution() {
    // 测试 PubGrub 解析
    let mut graph = DependencyGraph::new();
    
    // 添加测试数据
    graph.add("app".into(), ver(1, 0, 0), vec![
        ("lib-a".into(), VersionReq::Compatible("1.0".into())),
        ("lib-b".into(), VersionReq::Compatible("2.0".into())),
    ]);
    graph.add("lib-a".into(), ver(1, 0, 0), vec![
        ("lib-c".into(), VersionReq::Compatible("1.0".into())),
    ]);
    graph.add("lib-b".into(), ver(2, 0, 0), vec![
        ("lib-c".into(), VersionReq::Range {
            min: Some("2.0".into()),
            max: None,
        }),
    ]);
    // lib-c 1.0 和 2.0 冲突
    graph.add("lib-c".into(), ver(1, 5, 0), vec![]);
    graph.add("lib-c".into(), ver(2, 1, 0), vec![]);
    
    let result = resolve_dependencies("app", ver(1, 0, 0), &graph);
    
    // 应该检测到冲突
    assert!(result.is_err());
}

#[test]
fn test_version_req_matching() {
    let v = parse_version("1.2.3");
    
    assert!(VersionReq::Compatible("1.0".into()).matches(&v));
    assert!(VersionReq::GreaterEq("1.0".into()).matches(&v));
    assert!(!VersionReq::Exact("1.2.3".into()).matches(&parse_version("1.2.4")));
    assert!(VersionReq::Any.matches(&v));
}

#[test]
fn test_checksum_verification() {
    let tarball = create_test_tarball("test-package", "1.0.0");
    let expected = compute_sha256(&tarball);
    
    // 验证校验和
    assert!(verify_checksum(&tarball, &expected));
    
    // 篡改后验证失败
    let mut tampered = tarball.clone();
    tampered[0] ^= 0xFF;
    assert!(!verify_checksum(&tampered, &expected));
}

#[test]
fn test_sandbox_build() {
    let dir = setup_test_dir();
    create_malicious_build_script(dir.path());
    
    let config = SandboxConfig {
        allowed_hosts: vec![],
        allowed_paths: vec![dir.path().to_path_buf()],
        memory_limit: 128,
        cpu_limit: 10,
    };
    
    let result = build_in_sandbox(dir.path(), &config);
    
    // 恶意脚本应该被阻止
    assert!(result.is_err());
}

#[test]
fn test_workspace_build() {
    let dir = setup_test_dir();
    create_workspace(dir.path());
    
    let output = std::process::Command::new("zep")
        .arg("build")
        .current_dir(dir.path())
        .output()
        .unwrap();
    
    assert!(output.status.success());
    
    // 验证所有成员都被构建
    for member in ["lib-a", "lib-b", "app"] {
        let artifact = dir.path().join("target/debug").join(member);
        assert!(artifact.exists());
    }
}

#[test]
fn test_publish_flow() {
    let dir = setup_test_dir();
    init_project(dir.path());
    
    // 模拟发布（使用本地注册表）
    let output = std::process::Command::new("zep")
        .arg("publish")
        .arg("--registry=http://localhost:8080")
        .current_dir(dir.path())
        .output()
        .unwrap();
    
    // 在没有 registry 的情况下应该失败（但流程正确）
    assert!(!output.status.success());
}
```

---

## 性能基准

```rust
// zep/benches/resolve_bench.rs
use criterion::{black_box, Criterion};

fn bench_resolve_small(c: &mut Criterion) {
    // 10 个包，依赖深度 3
    let graph = create_test_graph(10, 3);
    c.bench_function("resolve_10_pkgs", |b| {
        b.iter(|| {
            resolve_dependencies(
                black_box("root"),
                ver(1, 0, 0),
                &graph,
            )
        })
    });
}

fn bench_resolve_large(c: &mut Criterion) {
    // 500 个包，依赖深度 10
    let graph = create_test_graph(500, 10);
    c.bench_function("resolve_500_pkgs", |b| {
        b.iter(|| {
            resolve_dependencies(
                black_box("root"),
                ver(1, 0, 0),
                &graph,
            )
        })
    });
}

// 目标：
// - 10 个包 < 1ms
// - 500 个包 < 100ms
```

---

## 验收标准

| 标准 | 要求 |
|------|------|
| 所有测试通过 | 100% |
| `zep new` | 创建完整项目结构 |
| `zep add/remove` | 正确修改 Zeta.toml |
| 依赖解析 | PubGrub 正确处理冲突 |
| 沙箱构建 | 阻止网络/文件系统越权 |
| 校验和 | SHA-256 验证通过 |
| 发布流程 | 完整 publish 流程可用 |
| 性能 | 500 包解析 < 100ms |

---

## 交付文件

```
zep/
├── Cargo.toml
├── src/
│   ├── main.rs           ← CLI 入口
│   ├── error.rs          ← ZepError
│   ├── manifest.rs       ← Zeta.toml 解析
│   ├── commands/
│   │   ├── new.rs
│   │   ├── init.rs
│   │   ├── add.rs
│   │   ├── remove.rs
│   │   ├── build.rs
│   │   ├── run.rs
│   │   ├── test.rs
│   │   ├── update.rs
│   │   ├── publish.rs
│   │   └── search.rs
│   ├── resolve/
│   │   ├── mod.rs        ← PubGrub 集成
│   │   └── version.rs     ← 版本约束
│   ├── sandbox/
│   │   └── mod.rs        ← 沙箱构建
│   ├── registry/
│   │   └── mod.rs        ← 注册表客户端
│   └── build/
│       └── mod.rs         ← 构建集成
└── tests/
    └── zep_test.rs
```

---

## 完成后下一步

进入 **P009_标准库核心模块.md**，实现标准库。
