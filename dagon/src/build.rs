//! 构建集成：调用 `rlyeh` 编译器完成 build/run/test。
//!
//! `rlyeh` 二进制路径可通过 `RLYEH_BIN` 环境变量或 [`BuildConfig`] 指定；
//! 默认使用 PATH 中的 `rlyeh`。

use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::error::{Result, DagonError};
use crate::manifest::Manifest;
use crate::sandbox::{SandboxOutput, run_sandboxed};

/// 构建配置。
#[derive(Debug, Clone)]
pub struct BuildConfig {
    /// `rlyeh` 可执行文件路径（默认取 `RLYEH_BIN` 环境变量，否则 `rlyeh`）。
    pub rlyeh_bin: String,
    /// 是否 release 构建（MVP 透传，驱动暂未区分）。
    pub release: bool,
    /// 编译器缓存目录（默认 `target/rlyeh-cache`）。
    pub cache_dir: Option<PathBuf>,
    /// 子进程超时。
    pub timeout: Duration,
    /// 是否启用受限环境沙箱。
    pub sandbox: bool,
    /// 是否透传 `--verbose` 到 rlyeh。
    pub verbose: bool,
}

impl Default for BuildConfig {
    fn default() -> Self {
        let rlyeh_bin = std::env::var("RLYEH_BIN").unwrap_or_else(|_| "rlyeh".to_string());
        Self {
            rlyeh_bin,
            release: false,
            cache_dir: None,
            timeout: Duration::from_secs(300),
            sandbox: true,
            verbose: false,
        }
    }
}

/// 项目的源码入口（仅支持 bin 目标：`src/main.rl`）。
pub fn entry_point(project_dir: &Path) -> Result<PathBuf> {
    let main = project_dir.join("src").join("main.rl");
    if !main.exists() {
        return Err(DagonError::Build(format!(
            "缺少入口文件 {}（MVP 仅支持 src/main.rl）",
            main.display()
        )));
    }
    Ok(main)
}

/// 编译项目，返回产物路径。
pub fn build_project(
    project_dir: &Path,
    manifest: &Manifest,
    config: &BuildConfig,
) -> Result<PathBuf> {
    let entry = entry_point(project_dir)?;
    let target = project_dir.join("target");
    let out_dir = target.join("rlyeh");
    std::fs::create_dir_all(&out_dir)
        .map_err(|e| DagonError::Build(format!("创建 {} 失败: {e}", out_dir.display())))?;
    let out = out_dir.join(&manifest.package.name);

    let mut args = vec![
        "build".to_string(),
        entry.display().to_string(),
        "-o".to_string(),
        out.display().to_string(),
    ];
    let cache_dir = config
        .cache_dir
        .clone()
        .unwrap_or_else(|| target.join("rlyeh-cache"));
    args.push("--cache-dir".to_string());
    args.push(cache_dir.display().to_string());
    if config.verbose {
        args.push("--verbose".to_string());
    }

    let output = run_sandboxed(
        &config.rlyeh_bin,
        &args,
        &[],
        config.timeout,
        config.sandbox,
    )?;
    if !output.success() {
        return Err(DagonError::Build(format!(
            "编译失败（{} {}）：\n{}",
            config.rlyeh_bin,
            args.join(" "),
            output.stderr.trim()
        )));
    }
    if !out.exists() {
        return Err(DagonError::Build(format!(
            "编译成功但未找到产物 {}",
            out.display()
        )));
    }
    Ok(out)
}

/// 编译并运行项目，透传程序参数。
pub fn run_project(
    project_dir: &Path,
    config: &BuildConfig,
    program_args: &[String],
) -> Result<SandboxOutput> {
    let entry = entry_point(project_dir)?;
    let target = project_dir.join("target");
    let mut args = vec![
        "run".to_string(),
        entry.display().to_string(),
        "--cache-dir".to_string(),
        target
            .join("rlyeh-cache")
            .display()
            .to_string(),
    ];
    if config.verbose {
        args.push("--verbose".to_string());
    }
    args.extend_from_slice(program_args);

    let output = run_sandboxed(
        &config.rlyeh_bin,
        &args,
        &[],
        config.timeout,
        config.sandbox,
    )?;
    Ok(output)
}

/// 运行测试：先构建入口做冒烟测试；若存在 `tests/` 目录则逐个编译运行。
pub fn test_project(project_dir: &Path, manifest: &Manifest, config: &BuildConfig) -> Result<()> {
    build_project(project_dir, manifest, config)?;

    let tests_dir = project_dir.join("tests");
    if !tests_dir.is_dir() {
        return Ok(());
    }
    let mut failed = Vec::new();
    let mut entries: Vec<_> = std::fs::read_dir(&tests_dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|x| x == "rlyeh"))
        .collect();
    entries.sort();
    for test_file in entries {
        let output = run_sandboxed(
            &config.rlyeh_bin,
            &[
                "run".to_string(),
                test_file.display().to_string(),
            ],
            &[],
            config.timeout,
            config.sandbox,
        )?;
        if !output.success() {
            failed.push((test_file, output.stderr));
        }
    }
    if !failed.is_empty() {
        let mut msg = String::from("测试失败：\n");
        for (f, err) in &failed {
            msg.push_str(&format!("- {}: {}\n", f.display(), err.trim()));
        }
        return Err(DagonError::Build(msg));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fake_rlyeh_bin(dir: &Path) -> PathBuf {
        // 生成一个 fake rlyeh 脚本：参数记录到脚本同目录 log.txt 并模拟产物。
        // 日志路径取自 $0，避免并行测试共享环境变量的竞争。
        let bin = dir.join("rlyeh");
        let script = r##"#!/bin/sh
log="$(dirname "$0")/log.txt"
echo "$@" >> "$log"
if [ "$1" = "build" ]; then
  # 输出路径是 -o 之后的参数
  out=""
  prev=""
  for a in "$@"; do
    if [ "$prev" = "-o" ]; then out="$a"; fi
    prev="$a"
  done
  if [ -n "$out" ]; then mkdir -p "$(dirname "$out")"; printf '#!/bin/sh\n' > "$out"; chmod +x "$out"; fi
fi
exit 0
"##;
        let mut f = std::fs::File::create(&bin).unwrap();
        f.write_all(script.as_bytes()).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&bin, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        bin
    }

    fn tmp_project(name: &str, with_tests: bool) -> (std::path::PathBuf, std::path::PathBuf) {
        let dir = std::env::temp_dir().join(format!("dagon-build-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("src")).unwrap();
        std::fs::write(dir.join("Rlyeh.toml"), "[package]\nname = \"app\"\nversion = \"0.1.0\"\n").unwrap();
        std::fs::write(dir.join("src/main.rl"), "fn main() { println(\"hi\"); }\n").unwrap();
        if with_tests {
            std::fs::create_dir_all(dir.join("tests")).unwrap();
            std::fs::write(dir.join("tests/t1.rl"), "fn main() {}\n").unwrap();
        }
        let bin_dir = std::env::temp_dir().join(format!("dagon-bin-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&bin_dir);
        std::fs::create_dir_all(&bin_dir).unwrap();
        (dir, bin_dir)
    }

    #[test]
    fn build_invokes_rlyeh_with_expected_args() {
        let (project, bin_dir) = tmp_project("build", false);
        let log = bin_dir.join("log.txt");
        let rlyeh = fake_rlyeh_bin(&bin_dir);
        let manifest = Manifest::load(&project.join("Rlyeh.toml")).unwrap();
        let config = BuildConfig {
            rlyeh_bin: rlyeh.display().to_string(),
            sandbox: false,
            timeout: Duration::from_secs(10),
            ..Default::default()
        };

        let out = build_project(&project, &manifest, &config).unwrap();
        assert_eq!(out, project.join("target/rlyeh/app"));

        // fake rlyeh 记录了调用参数
        let log_text = std::fs::read_to_string(&log).unwrap();
        assert!(log_text.contains("build"), "参数应为 build: {log_text}");
        assert!(log_text.contains("src/main.rl"));
        assert!(log_text.contains("-o"));
    }

    #[test]
    fn run_passes_program_args() {
        let (project, bin_dir) = tmp_project("run", false);
        let log = bin_dir.join("log.txt");
        let rlyeh = fake_rlyeh_bin(&bin_dir);
        let config = BuildConfig {
            rlyeh_bin: rlyeh.display().to_string(),
            sandbox: false,
            timeout: Duration::from_secs(10),
            ..Default::default()
        };

        run_project(&project, &config, &["--flag".to_string(), "value".to_string()]).unwrap();
        let log_text = std::fs::read_to_string(&log).unwrap();
        assert!(log_text.contains("run"));
        assert!(log_text.contains("--flag"));
        assert!(log_text.contains("value"));
    }

    #[test]
    fn test_project_builds_and_runs_tests() {
        let (project, bin_dir) = tmp_project("test", true);
        let rlyeh = fake_rlyeh_bin(&bin_dir);
        let manifest = Manifest::load(&project.join("Rlyeh.toml")).unwrap();
        let config = BuildConfig {
            rlyeh_bin: rlyeh.display().to_string(),
            sandbox: false,
            timeout: Duration::from_secs(10),
            ..Default::default()
        };

        test_project(&project, &manifest, &config).unwrap();
    }

    #[test]
    fn missing_entry_reports_error() {
        let dir = std::env::temp_dir().join(format!("dagon-noentry-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(entry_point(&dir).is_err());
    }
}
