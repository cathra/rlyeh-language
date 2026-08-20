//! 全部 CLI 子命令的实现。

use std::fs;
use std::path::{Path, PathBuf};

use crate::build::{BuildConfig, build_project, run_project, test_project};
use crate::error::{Result, ZepError};
use crate::manifest::{DepKind, Manifest, validate_package_name};
use crate::registry::{
    PackageIndex, PackageVersion, client_from_url, default_registry, pack_directory, sha256_hex,
};
use crate::resolve::{DependencyGraph, LockFile};
use crate::sandbox::run_sandboxed;
use crate::version::{Version, VersionReq};

/// 命令执行上下文。
#[derive(Debug, Clone)]
pub struct Ctx {
    /// 详细输出。
    pub verbose: bool,
    /// 当前工作目录。
    pub cwd: PathBuf,
    /// 注册表地址（覆盖默认）。
    pub registry: Option<String>,
}

impl Ctx {
    /// 新建上下文。
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            registry: None,
        }
    }

    /// 注册表客户端（懒构造）。
    fn registry_client(&self) -> Result<Box<dyn crate::registry::RegistryClient>> {
        client_from_url(self.registry.as_deref().unwrap_or(&default_registry()))
    }
}

/// `zep new <name>`：创建新项目。
pub fn cmd_new(ctx: &Ctx, name: &str, lib: bool) -> Result<()> {
    validate_package_name(name).map_err(ZepError::Dependency)?;
    let dir = ctx.cwd.join(name);
    if dir.exists() {
        return Err(ZepError::Other(format!(
            "目录 {} 已存在",
            dir.display()
        )));
    }
    fs::create_dir_all(dir.join("src"))?;
    write_project_files(&dir, name, lib)?;
    println!("已创建项目 {name} 于 {}", dir.display());
    Ok(())
}

/// `zep init`：初始化当前目录。
pub fn cmd_init(ctx: &Ctx, lib: bool) -> Result<()> {
    let manifest_path = ctx.cwd.join("Zeta.toml");
    if manifest_path.exists() {
        return Err(ZepError::Other("当前目录已有 Zeta.toml".into()));
    }
    let name = ctx
        .cwd
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("app")
        .to_string();
    let name = if name.is_empty() { "app".to_string() } else { name };
    validate_package_name(&name).map_err(ZepError::Dependency)?;
    fs::create_dir_all(ctx.cwd.join("src"))?;
    write_project_files(&ctx.cwd, &name, lib)?;
    println!("已在 {} 初始化项目 {name}", ctx.cwd.display());
    Ok(())
}

fn write_project_files(dir: &Path, name: &str, lib: bool) -> Result<()> {
    let mut manifest = Manifest::default();
    manifest.package = crate::manifest::PackageInfo {
        name: name.to_string(),
        version: "0.1.0".to_string(),
        edition: "0.2".to_string(),
        description: None,
        license: None,
        authors: vec![],
    };
    manifest
        .save(&dir.join("Zeta.toml"))
        .map_err(|e| ZepError::Manifest(format!("写入 Zeta.toml 失败: {e}")))?;
    let entry = if lib { "lib.zeta" } else { "main.zeta" };
    let body = if lib {
        "// {name} 库入口\n"
    } else {
        "// {name} 应用入口\nfn main() {\n    println(\"Hello from {name}!\");\n}\n"
    };
    fs::write(
        dir.join("src").join(entry),
        body.replace("{name}", name),
    )?;
    fs::write(
        dir.join(".gitignore"),
        "target/\nzeta-out/\n",
    )?;
    // git init（失败可忽略，例如无 git 环境）
    let _ = run_sandboxed("git", &["init".to_string(), "-q".to_string()], &[], std::time::Duration::from_secs(10), false)
        .map(|_| ())
        .map_err(|_| ());
    Ok(())
}

/// 解析 `name@req` 形式（无 `@` 时需求为 `*`）。
pub fn parse_package_spec(spec: &str) -> (String, String) {
    match spec.split_once('@') {
        Some((name, req)) if !req.is_empty() => (name.to_string(), req.to_string()),
        _ => (spec.to_string(), "*".to_string()),
    }
}

/// `zep add <name[@req]>`：添加依赖并重解析锁定。
pub fn cmd_add(ctx: &Ctx, spec: &str, dev: bool) -> Result<()> {
    let (name, req) = parse_package_spec(spec);
    let manifest_path = ctx.cwd.join("Zeta.toml");
    let mut manifest = Manifest::load(&manifest_path)?;
    let original = manifest.to_string()?;
    let kind = if dev { DepKind::Dev } else { DepKind::Normal };
    manifest.add_dependency(&name, &req, kind)?;
    // 先解析：失败则回滚清单
    match resolve_and_lock(ctx, &manifest, false) {
        Ok(lock_path) => {
            manifest
                .save(&manifest_path)
                .map_err(|e| ZepError::Manifest(format!("保存清单失败: {e}")))?;
            println!(
                "已添加 {} {}（写入 {}）",
                name,
                req,
                lock_path.display()
            );
            Ok(())
        }
        Err(e) => {
            let _ = fs::write(&manifest_path, &original);
            Err(e)
        }
    }
}

/// `zep remove <name>`：移除依赖并重解析锁定。
pub fn cmd_remove(ctx: &Ctx, name: &str, dev: bool) -> Result<()> {
    let manifest_path = ctx.cwd.join("Zeta.toml");
    let mut manifest = Manifest::load(&manifest_path)?;
    let kind = if dev { DepKind::Dev } else { DepKind::Normal };
    if !manifest.remove_dependency(name, kind) {
        return Err(ZepError::Dependency(format!(
            "{}.{} 不存在",
            kind.label(),
            name
        )));
    }
    let original = manifest.to_string()?;
    match resolve_and_lock(ctx, &manifest, false) {
        Ok(_) => {
            manifest.save(&manifest_path).map_err(|e| {
                ZepError::Manifest(format!("保存清单失败: {e}"))
            })?;
            println!("已移除 {name}");
            Ok(())
        }
        Err(e) => {
            let _ = fs::write(&manifest_path, &original);
            Err(e)
        }
    }
}

/// `zep build`：编译项目。
pub fn cmd_build(ctx: &Ctx, release: bool) -> Result<()> {
    let manifest_path = ctx.cwd.join("Zeta.toml");
    let manifest = Manifest::load(&manifest_path)?;
    let config = BuildConfig {
        release,
        verbose: ctx.verbose,
        ..Default::default()
    };
    let out = build_project(&ctx.cwd, &manifest, &config)?;
    println!("编译完成: {}", out.display());
    Ok(())
}

/// `zep run [args...]`：编译并运行。
pub fn cmd_run(ctx: &Ctx, args: &[String]) -> Result<()> {
    let config = BuildConfig {
        verbose: ctx.verbose,
        ..Default::default()
    };
    let output = run_project(&ctx.cwd, &config, args)?;
    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
    }
    if !output.success() {
        return Err(ZepError::Build(format!(
            "程序退出码 {}",
            output.status
        )));
    }
    Ok(())
}

/// `zep test`：构建并运行测试。
pub fn cmd_test(ctx: &Ctx) -> Result<()> {
    let manifest_path = ctx.cwd.join("Zeta.toml");
    let manifest = Manifest::load(&manifest_path)?;
    let config = BuildConfig {
        verbose: ctx.verbose,
        ..Default::default()
    };
    test_project(&ctx.cwd, &manifest, &config)?;
    println!("测试通过");
    Ok(())
}

/// `zep update [name]`：重解析依赖并更新锁文件。
pub fn cmd_update(ctx: &Ctx, _name: Option<String>) -> Result<()> {
    let manifest_path = ctx.cwd.join("Zeta.toml");
    let manifest = Manifest::load(&manifest_path)?;
    let lock_path = resolve_and_lock(ctx, &manifest, true)?;
    println!("依赖已更新（写入 {}）", lock_path.display());
    Ok(())
}

/// `zep publish`：打包并发布到注册表。
pub fn cmd_publish(ctx: &Ctx, registry_url: Option<String>) -> Result<()> {
    let manifest_path = ctx.cwd.join("Zeta.toml");
    let manifest = Manifest::load(&manifest_path)?;
    let version = Version::parse(&manifest.package.version)?;
    let name = manifest.package.name.clone();

    let mut reg_ctx = ctx.clone();
    reg_ctx.registry = registry_url.or(ctx.registry.clone());
    let registry = reg_ctx.registry_client()?;

    // 打包（排除 target/、.git、zeta-out）
    let tarball = pack_directory(
        &ctx.cwd,
        &["target".to_string(), ".git".to_string(), "zeta-out".to_string()],
    )?;

    // 构造/更新索引
    let mut index = registry
        .get_index(&name)?
        .unwrap_or_else(|| PackageIndex::new(&name));
    let mut deps = std::collections::HashMap::new();
    for (dep, req) in &manifest.dependencies {
        deps.insert(dep.clone(), req.clone());
    }
    index.upsert(PackageVersion {
        version: version.to_string(),
        dependencies: deps,
        checksum: Some(sha256_hex(&tarball)),
        size: Some(tarball.len() as u64),
    });
    registry.publish(&index, &version.to_string(), &tarball)?;
    println!(
        "已发布 {name} {}（{} 字节）",
        version,
        tarball.len()
    );
    Ok(())
}

/// `zep search <query>`：搜索注册表。
pub fn cmd_search(ctx: &Ctx, query: &str, registry_url: Option<String>) -> Result<()> {
    let mut reg_ctx = ctx.clone();
    reg_ctx.registry = registry_url.or(ctx.registry.clone());
    let registry = reg_ctx.registry_client()?;
    let hits = registry.search(query)?;
    if hits.is_empty() {
        println!("未找到匹配 \"{query}\" 的包");
        return Ok(());
    }
    for name in hits {
        println!("{name}");
    }
    Ok(())
}

/// `zep clean`：删除 target 目录。
pub fn cmd_clean(ctx: &Ctx) -> Result<()> {
    let target = ctx.cwd.join("target");
    if target.exists() {
        fs::remove_dir_all(&target)?;
        println!("已清理 {}", target.display());
    } else {
        println!("无 target 目录可清理");
    }
    Ok(())
}

/// 收集根依赖（常规 + 构建依赖参与解析；MVP 不含 dev）。
fn root_deps(manifest: &Manifest) -> std::collections::HashMap<String, VersionReq> {
    let mut deps = std::collections::HashMap::new();
    for (k, v) in manifest
        .dependencies
        .iter()
        .chain(manifest.build_dependencies.iter())
    {
        // 已由清单校验保证可解析
        if let Ok(req) = VersionReq::parse(v) {
            deps.insert(k.clone(), req);
        }
    }
    deps
}

/// 解析依赖并写 `Zeta.lock`。`required` 为 false 时若锁文件已存在且无根依赖
/// 变更则跳过（MVP 始终重新解析，保持简单）。
fn resolve_and_lock(ctx: &Ctx, manifest: &Manifest, _required: bool) -> Result<PathBuf> {
    let registry = ctx.registry_client()?;
    let root_version = Version::parse(&manifest.package.version)?;
    let deps = root_deps(manifest);
    let graph = DependencyGraph::from_registry(&manifest.package.name, &deps, registry.as_ref())?;
    let selected = graph.resolve(&manifest.package.name, &root_version, &deps)?;
    let lock = LockFile::from_resolution(
        &manifest.package.name,
        &root_version,
        &graph,
        &selected,
        &deps,
    );
    let lock_path = ctx.cwd.join("Zeta.lock");
    fs::write(&lock_path, lock.to_toml()?)
        .map_err(|e| ZepError::Resolve(format!("写锁文件失败: {e}")))?;
    Ok(lock_path)
}
