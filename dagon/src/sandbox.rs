//! 构建沙箱：受限环境变量 + 资源限制 + 超时终止。
//!
//! MVP 采用"受限环境"隔离：
//! - 环境变量白名单（清除代理、密钥等敏感变量）
//! - PATH 仅保留系统目录
//! - 子进程 CPU 时间/文件大小上限（Unix）
//! - 执行超时强制终止（防挂起）

use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::channel;
use std::thread;
use std::time::{Duration, Instant};

use crate::error::{Result, DagonError};

/// 受限环境构建器。
pub struct RestrictedEnv;

impl RestrictedEnv {
    /// 允许保留的环境变量白名单。
    const ALLOWED: &'static [&'static str] = &[
        "PATH", "HOME", "TMPDIR", "TEMP", "TMP", "TERM", "USER", "LANG", "LC_ALL", "SHELL",
        "PWD",
    ];

    /// PATH 白名单（仅系统可执行目录）。
    const PATH_WHITELIST: &'static [&'static str] = &[
        "/usr/bin",
        "/bin",
        "/usr/sbin",
        "/sbin",
        "/usr/local/bin",
        "/opt/homebrew/bin",
    ];

    /// 生成净化后的环境变量映射：PATH 仅白名单目录，其余仅白名单键。
    pub fn env_map() -> HashMap<String, String> {
        let mut out = HashMap::new();
        if let Ok(path) = std::env::var("PATH") {
            let clean: Vec<&str> = path
                .split(':')
                .filter(|p| Self::PATH_WHITELIST.contains(p))
                .collect();
            if !clean.is_empty() {
                out.insert("PATH".to_string(), clean.join(":"));
            }
        }
        for key in Self::ALLOWED {
            if *key == "PATH" {
                continue;
            }
            if let Ok(v) = std::env::var(key) {
                out.insert(key.to_string(), v);
            }
        }
        out
    }

    /// 净化后的 PATH 值。
    pub fn clean_path() -> String {
        Self::env_map().get("PATH").cloned().unwrap_or_default()
    }
}

/// 子进程输出。
#[derive(Debug)]
pub struct SandboxOutput {
    /// 退出状态。
    pub status: std::process::ExitStatus,
    /// 标准输出（UTF-8 宽松解码）。
    pub stdout: String,
    /// 标准错误。
    pub stderr: String,
}

impl SandboxOutput {
    /// 是否成功退出（exit code 0）。
    pub fn success(&self) -> bool {
        self.status.success()
    }
}

/// 在 Unix 子进程中应用资源限制（须在 `pre_exec` 中调用）。
#[cfg(unix)]
fn apply_rlimits() {
    // CPU 时间上限：120 秒
    let cpu = libc::rlimit {
        rlim_cur: 120,
        rlim_max: 120,
    };
    // 单文件大小上限：1 GiB
    let fsize = libc::rlimit {
        rlim_cur: 1 << 30,
        rlim_max: 1 << 30,
    };
    unsafe {
        libc::setrlimit(libc::RLIMIT_CPU, &cpu);
        libc::setrlimit(libc::RLIMIT_FSIZE, &fsize);
    }
}

/// 执行命令：可选受限环境、必带超时。
///
/// 超时后强制终止子进程并返回 [`DagonError::Timeout`]（防挂起硬性规则）。
pub fn run_sandboxed(
    program: &str,
    args: &[String],
    env_extra: &[(String, String)],
    timeout: Duration,
    restricted: bool,
) -> Result<SandboxOutput> {
    let started = Instant::now();
    let mut cmd = Command::new(program);
    cmd.args(args);
    if restricted {
        cmd.env_clear();
        for (k, v) in RestrictedEnv::env_map() {
            cmd.env(k, v);
        }
    }
    for (k, v) in env_extra {
        cmd.env(k, v);
    }
    #[cfg(unix)]
    if restricted {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                apply_rlimits();
                Ok(())
            });
        }
    }
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child: Child = cmd.spawn().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            DagonError::CommandNotFound(program.to_string())
        } else {
            DagonError::Build(format!("启动 {program} 失败: {e}"))
        }
    })?;

    // 用读线程收集 stdout/stderr（管道可能填满，必须及时消费）
    let (stdout_tx, stdout_rx) = channel::<Vec<u8>>();
    if let Some(mut pipe) = child.stdout.take() {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = stdout_tx.send(buf);
        });
    } else {
        drop(stdout_tx);
    }
    let (stderr_tx, stderr_rx) = channel::<Vec<u8>>();
    if let Some(mut pipe) = child.stderr.take() {
        thread::spawn(move || {
            let mut buf = Vec::new();
            let _ = pipe.read_to_end(&mut buf);
            let _ = stderr_tx.send(buf);
        });
    } else {
        drop(stderr_tx);
    }

    // 轮询退出状态，超时强制终止
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                if started.elapsed() > timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(DagonError::Timeout(format!(
                        "{program} 执行超过 {:?}（实际运行 {:?}）",
                        timeout,
                        started.elapsed()
                    )));
                }
                thread::sleep(Duration::from_millis(5));
            }
            Err(e) => return Err(DagonError::Build(format!("等待 {program} 失败: {e}"))),
        }
    };

    // 回收管道输出（进程退出后读线程必然 EOF）
    let stdout = stdout_rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    let stderr = stderr_rx.recv_timeout(Duration::from_secs(2)).unwrap_or_default();
    Ok(SandboxOutput {
        status,
        stdout: String::from_utf8_lossy(&stdout).to_string(),
        stderr: String::from_utf8_lossy(&stderr).to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_map_filters_sensitive() {
        std::env::set_var("DAGON_SECRET_TEST_KEY", "topsecret");
        std::env::set_var("HTTP_PROXY", "http://proxy:8080");
        let map = RestrictedEnv::env_map();
        assert!(!map.contains_key("DAGON_SECRET_TEST_KEY"));
        assert!(!map.contains_key("HTTP_PROXY"));
        assert!(!map.contains_key("AWS_ACCESS_KEY_ID"));
        // PATH 只含白名单
        if let Some(path) = map.get("PATH") {
            for dir in path.split(':') {
                assert!(
                    RestrictedEnv::PATH_WHITELIST.contains(&dir),
                    "PATH 含非白名单目录 {dir}"
                );
            }
        }
    }

    #[test]
    fn timeout_kills_after_deadline() {
        let args = vec!["5".to_string()];
        let err = run_sandboxed("sleep", &args, &[], Duration::from_millis(300), false).unwrap_err();
        assert!(matches!(err, DagonError::Timeout(_)), "期望超时，实际 {err:?}");
    }

    #[test]
    fn successful_run_collects_output() {
        let args = vec!["hello".to_string()];
        let out = run_sandboxed("echo", &args, &[], Duration::from_secs(5), false).unwrap();
        assert!(out.success());
        assert_eq!(out.stdout.trim(), "hello");
    }

    #[test]
    fn not_found_reports_command() {
        let err =
            run_sandboxed("dagon-nonexistent-cmd-xyz", &[], &[], Duration::from_secs(2), false)
                .unwrap_err();
        assert!(matches!(err, DagonError::CommandNotFound(_)));
    }

    #[test]
    fn non_zero_exit_returns_status() {
        let out = run_sandboxed(
            "sh",
            &["-c".to_string(), "exit 3".to_string()],
            &[],
            Duration::from_secs(5),
            false,
        )
        .unwrap();
        assert!(!out.success());
    }
}
