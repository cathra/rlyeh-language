//! 包注册表：本地目录注册表 + 极简 HTTP 注册表客户端。
//!
//! 注册表布局：
//! ```text
//! <root>/
//!   index/<name>.json              # 包的版本索引（含依赖与校验和）
//!   pkgs/<name>-<version>.tar.gz   # 包内容（tar.gz）
//! ```
//!
//! `HttpRegistry` 使用标准库实现最小 HTTP/1.1 客户端（仅 `http://`，
//! HTTPS 需要 TLS，留待后续）。测试通过内存 mock 服务器验证。

use std::collections::HashMap;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::error::{Result, DagonError};

/// 默认注册表：用户目录下的本地注册表。
pub fn default_registry() -> String {
    if let Ok(home) = std::env::var("HOME") {
        format!("file://{home}/.rl/registry")
    } else {
        "file://~/.rl/registry".to_string()
    }
}

/// 包在注册表中的单个版本元数据。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersion {
    /// 版本号。
    pub version: String,
    /// 该版本的依赖：包名 → 版本需求。
    #[serde(default)]
    pub dependencies: HashMap<String, String>,
    /// 包内容 sha256（十六进制）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    /// 包内容字节数。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

/// 包索引（所有已发布版本）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageIndex {
    /// 包名。
    pub name: String,
    /// 版本列表。
    #[serde(default)]
    pub versions: Vec<PackageVersion>,
}

impl PackageIndex {
    /// 新建空索引。
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            versions: Vec::new(),
        }
    }

    /// 查找指定版本。
    pub fn find(&self, version: &str) -> Option<&PackageVersion> {
        self.versions.iter().find(|v| v.version == version)
    }

    /// 合并一个新版本（已存在则整体替换）。
    pub fn upsert(&mut self, pv: PackageVersion) {
        if let Some(existing) = self.versions.iter_mut().find(|v| v.version == pv.version) {
            *existing = pv;
        } else {
            self.versions.push(pv);
        }
    }
}

/// 注册表客户端抽象。
pub trait RegistryClient {
    /// 获取包索引（不存在返回 `None`）。
    fn get_index(&self, name: &str) -> Result<Option<PackageIndex>>;
    /// 下载包内容并校验 sha256。
    fn download(&self, name: &str, version: &str) -> Result<Vec<u8>>;
    /// 发布一个版本的包内容（更新索引）。
    fn publish(&self, index: &PackageIndex, version: &str, tarball: &[u8]) -> Result<()>;
    /// 按名称关键词搜索，返回匹配的包名。
    fn search(&self, query: &str) -> Result<Vec<String>>;
}

/// 根据 URL 构造注册表客户端。
///
/// - `file://` 前缀或任意普通路径 → [`LocalRegistry`]
/// - `http://` → [`HttpRegistry`]
/// - `https://` → 报错（暂不支持 TLS）
pub fn client_from_url(url: &str) -> Result<Box<dyn RegistryClient>> {
    if url.starts_with("https://") {
        return Err(DagonError::Registry(
            "HTTPS 注册表需要 TLS，暂不支持；请使用 http:// 或本地路径".into(),
        ));
    }
    if url.starts_with("http://") {
        return Ok(Box::new(HttpRegistry::new(url)));
    }
    if let Some(path) = url.strip_prefix("file://") {
        return Ok(Box::new(LocalRegistry::new(expand_home(path))));
    }
    Ok(Box::new(LocalRegistry::new(expand_home(url))))
}

/// 展开 `~` 为用户主目录。
fn expand_home(p: &str) -> PathBuf {
    if p == "~" {
        return std::env::var("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("~"));
    }
    if let Some(rest) = p.strip_prefix("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(home).join(rest);
        }
    }
    PathBuf::from(p)
}

/// 本地目录注册表。
#[derive(Debug, Clone)]
pub struct LocalRegistry {
    root: PathBuf,
}

impl LocalRegistry {
    /// 新建本地注册表（根目录无需存在，publish 时自动创建）。
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn index_path(&self, name: &str) -> PathBuf {
        self.root.join("index").join(format!("{name}.json"))
    }

    fn tarball_path(&self, name: &str, version: &str) -> PathBuf {
        self.root
            .join("pkgs")
            .join(format!("{name}-{version}.tar.gz"))
    }

    /// 根目录。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 列出注册表内全部包名。
    pub fn list_packages(&self) -> Result<Vec<String>> {
        let dir = self.root.join("index");
        if !dir.exists() {
            return Ok(Vec::new());
        }
        let mut names = Vec::new();
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if entry.path().extension().is_some_and(|e| e == "json") {
                if let Some(stem) = entry.path().file_stem().and_then(|s| s.to_str()) {
                    names.push(stem.to_string());
                }
            }
        }
        Ok(names)
    }
}

impl RegistryClient for LocalRegistry {
    fn get_index(&self, name: &str) -> Result<Option<PackageIndex>> {
        let path = self.index_path(name);
        if !path.exists() {
            return Ok(None);
        }
        let s = fs::read_to_string(&path)
            .map_err(|e| DagonError::Registry(format!("读取索引 {} 失败: {e}", path.display())))?;
        let idx: PackageIndex = serde_json::from_str(&s)
            .map_err(|e| DagonError::Registry(format!("索引 {} 解析失败: {e}", path.display())))?;
        Ok(Some(idx))
    }

    fn download(&self, name: &str, version: &str) -> Result<Vec<u8>> {
        let idx = self.get_index(name)?.ok_or_else(|| {
            DagonError::Registry(format!("包 {name} 不存在于注册表"))
        })?;
        let meta = idx.find(version).ok_or_else(|| {
            DagonError::Registry(format!("包 {name} 没有版本 {version}"))
        })?;
        let path = self.tarball_path(name, version);
        let bytes = fs::read(&path)
            .map_err(|e| DagonError::Registry(format!("读取 {} 失败: {e}", path.display())))?;
        verify_checksum(&bytes, meta.checksum.as_deref(), name, version)?;
        Ok(bytes)
    }

    fn publish(&self, index: &PackageIndex, version: &str, tarball: &[u8]) -> Result<()> {
        let meta = index.find(version).ok_or_else(|| {
            DagonError::Registry(format!("索引中缺少版本 {version}"))
        })?;
        // 校验和一致性
        let checksum = sha256_hex(tarball);
        if let Some(recorded) = &meta.checksum {
            if recorded != &checksum {
                return Err(DagonError::Registry(format!(
                    "包 {}-{} 校验和不一致",
                    index.name, version
                )));
            }
        }
        // 写内容
        let tarball_dir = self.root.join("pkgs");
        fs::create_dir_all(&tarball_dir)?;
        let path = self.tarball_path(&index.name, version);
        fs::write(&path, tarball)
            .map_err(|e| DagonError::Registry(format!("写入 {} 失败: {e}", path.display())))?;
        // 更新索引
        let index_dir = self.root.join("index");
        fs::create_dir_all(&index_dir)?;
        let index_path = self.index_path(&index.name);
        let mut merged = self
            .get_index(&index.name)?
            .unwrap_or_else(|| PackageIndex::new(&index.name));
        if let Some(existing) = merged.versions.iter_mut().find(|v| v.version == version) {
            *existing = meta.clone();
        } else {
            merged.versions.push(meta.clone());
        }
        let json = serde_json::to_string_pretty(&merged)
            .map_err(|e| DagonError::Registry(format!("索引序列化失败: {e}")))?;
        fs::write(&index_path, json)
            .map_err(|e| DagonError::Registry(format!("写入索引 {} 失败: {e}", index_path.display())))?;
        Ok(())
    }

    fn search(&self, query: &str) -> Result<Vec<String>> {
        let q = query.to_lowercase();
        let mut hits = Vec::new();
        for name in self.list_packages()? {
            if name.to_lowercase().contains(&q) {
                hits.push(name);
            }
        }
        hits.sort();
        Ok(hits)
    }
}

/// 极简 HTTP 注册表客户端（标准库实现，仅 `http://`）。
#[derive(Debug, Clone)]
pub struct HttpRegistry {
    base_url: String,
}

impl HttpRegistry {
    /// 新建客户端，`base_url` 如 `http://localhost:8080/registry`。
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
        }
    }

    fn request(&self, method: &str, path: &str, body: Option<&[u8]>) -> Result<(u16, Vec<u8>)> {
        let rest = self
            .base_url
            .strip_prefix("http://")
            .ok_or_else(|| DagonError::Registry("仅支持 http:// 协议".into()))?;
        let (host, port) = match rest.split_once(':') {
            Some((h, p)) => {
                let port: u16 = p
                    .parse()
                    .map_err(|_| DagonError::Registry(format!("非法端口: {p:?}")))?;
                (h.to_string(), port)
            }
            None => (rest.to_string(), 80),
        };
        let payload = body.unwrap_or(&[]);
        let req_head = format!(
            "{method} {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        let mut stream = TcpStream::connect((host.as_str(), port))
            .map_err(|e| DagonError::Registry(format!("连接 {host}:{port} 失败: {e}")))?;
        stream
            .write_all(req_head.as_bytes())
            .map_err(|e| DagonError::Registry(format!("发送请求失败: {e}")))?;
        if !payload.is_empty() {
            stream
                .write_all(payload)
                .map_err(|e| DagonError::Registry(format!("发送请求体失败: {e}")))?;
        }
        stream
            .flush()
            .map_err(|e| DagonError::Registry(format!("冲刷请求失败: {e}")))?;
        let mut buf = Vec::new();
        stream
            .read_to_end(&mut buf)
            .map_err(|e| DagonError::Registry(format!("读取响应失败: {e}")))?;
        let text = String::from_utf8_lossy(&buf);
        let Some((status_line, rest)) = text.split_once("\r\n") else {
            return Err(DagonError::Registry("响应格式非法（缺状态行）".into()));
        };
        let status: u16 = status_line
            .split_whitespace()
            .nth(1)
            .and_then(|s| s.parse().ok())
            .ok_or_else(|| DagonError::Registry(format!("响应状态行非法: {status_line:?}")))?;
        let (_, body_text) = rest
            .split_once("\r\n\r\n")
            .unwrap_or((rest, ""));
        Ok((status, body_text.as_bytes().to_vec()))
    }
}

impl RegistryClient for HttpRegistry {
    fn get_index(&self, name: &str) -> Result<Option<PackageIndex>> {
        let (status, body) = self.request("GET", &format!("/index/{name}.json"), None)?;
        match status {
            200 => {
                let idx: PackageIndex = serde_json::from_slice(&body)
                    .map_err(|e| DagonError::Registry(format!("索引解析失败: {e}")))?;
                Ok(Some(idx))
            }
            404 => Ok(None),
            s => Err(DagonError::Registry(format!("GET /index/{name}.json 返回 {s}"))),
        }
    }

    fn download(&self, name: &str, version: &str) -> Result<Vec<u8>> {
        let (status, body) = self.request(
            "GET",
            &format!("/pkgs/{name}-{version}.tar.gz"),
            None,
        )?;
        if status != 200 {
            return Err(DagonError::Registry(format!(
                "下载 {name}-{version} 失败（HTTP {status}）"
            )));
        }
        let idx = self.get_index(name)?.ok_or_else(|| {
            DagonError::Registry(format!("包 {name} 不存在于注册表"))
        })?;
        let meta = idx.find(version).ok_or_else(|| {
            DagonError::Registry(format!("包 {name} 没有版本 {version}"))
        })?;
        verify_checksum(&body, meta.checksum.as_deref(), name, version)?;
        Ok(body)
    }

    fn publish(&self, index: &PackageIndex, version: &str, tarball: &[u8]) -> Result<()> {
        // 先上传内容，再更新索引（索引中需已含该版本元数据）
        let (status, _) = self.request(
            "PUT",
            &format!("/pkgs/{}-{version}.tar.gz", index.name),
            Some(tarball),
        )?;
        if !(200..300).contains(&status) {
            return Err(DagonError::Registry(format!(
                "上传 {}-{version} 失败（HTTP {status}）",
                index.name
            )));
        }
        let json = serde_json::to_vec(index)
            .map_err(|e| DagonError::Registry(format!("索引序列化失败: {e}")))?;
        let (status, _) = self.request(
            "PUT",
            &format!("/index/{}.json", index.name),
            Some(&json),
        )?;
        if !(200..300).contains(&status) {
            return Err(DagonError::Registry(format!(
                "更新索引 {} 失败（HTTP {status}）",
                index.name
            )));
        }
        Ok(())
    }

    fn search(&self, query: &str) -> Result<Vec<String>> {
        let (status, body) = self.request(
            "GET",
            &format!("/search?q={query}"),
            None,
        )?;
        if status != 200 {
            return Err(DagonError::Registry(format!("搜索失败（HTTP {status}）")));
        }
        let hits: Vec<String> = serde_json::from_slice(&body)
            .map_err(|e| DagonError::Registry(format!("搜索结果解析失败: {e}")))?;
        Ok(hits)
    }
}

/// 计算 sha256 十六进制。
pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex_encode(&hasher.finalize())
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// 校验 sha256（`expected` 为 `None` 时跳过）。
fn verify_checksum(
    bytes: &[u8],
    expected: Option<&str>,
    name: &str,
    version: &str,
) -> Result<()> {
    if let Some(expected) = expected {
        let actual = sha256_hex(bytes);
        if actual != expected {
            return Err(DagonError::Package(format!(
                "{name}-{version} 校验和不匹配（期望 {expected}，实际 {actual}）"
            )));
        }
    }
    Ok(())
}

/// 收集目录下的全部文件（排除 target/、.git/ 与自定义排除项）。
pub fn collect_files(
    root: &Path,
    excludes: &[String],
) -> Result<Vec<(PathBuf, String)>> {
    fn walk(dir: &Path, rel: &Path, excludes: &[String], out: &mut Vec<(PathBuf, String)>) -> Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let rel_path = rel.join(&name);
            if excludes.iter().any(|e| e == &name || rel_path.starts_with(e)) {
                continue;
            }
            if path.is_dir() {
                walk(&path, &rel_path, excludes, out)?;
            } else {
                out.push((path, rel_path.to_string_lossy().to_string()));
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    walk(root, Path::new(""), excludes, &mut out)?;
    out.sort_by(|a, b| a.1.cmp(&b.1));
    Ok(out)
}

/// 将目录打包为 tar.gz（归档内路径为相对 `root` 的路径）。
pub fn pack_directory(root: &Path, excludes: &[String]) -> Result<Vec<u8>> {
    let files = collect_files(root, excludes)?;
    let mut buf = Vec::new();
    {
        let enc = GzEncoder::new(&mut buf, Compression::default());
        let mut builder = tar::Builder::new(enc);
        for (abs, rel) in &files {
            builder
                .append_path_with_name(abs, rel)
                .map_err(|e| DagonError::Package(format!("打包 {rel} 失败: {e}")))?;
        }
        let enc = builder
            .into_inner()
            .map_err(|e| DagonError::Package(format!("完成打包失败: {e}")))?;
        enc.finish()
            .map_err(|e| DagonError::Package(format!("压缩失败: {e}")))?;
    }
    Ok(buf)
}

/// 解包 tar.gz 到目标目录（防路径穿越）。
pub fn unpack_tarball(bytes: &[u8], dest: &Path) -> Result<()> {
    let decoder = GzDecoder::new(bytes);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive
        .entries()
        .map_err(|e| DagonError::Package(format!("读取压缩包失败: {e}")))?;
    for entry in entries {
        let mut entry =
            entry.map_err(|e| DagonError::Package(format!("读取压缩项失败: {e}")))?;
        let rel = entry
            .path()
            .map_err(|e| DagonError::Package(format!("读取路径失败: {e}")))?
            .to_path_buf();
        // 防路径穿越：拒绝绝对路径与 .. 组件
        if rel.is_absolute() || rel.components().any(|c| c == std::path::Component::ParentDir) {
            return Err(DagonError::Package(format!(
                "压缩包含非法路径: {}",
                rel.display()
            )));
        }
        let target = dest.join(&rel);
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)?;
        }
        entry
            .unpack(&target)
            .map_err(|e| DagonError::Package(format!("解包 {} 失败: {e}", rel.display())))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    fn tmpdir(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "dagon-registry-test-{}-{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn pack_unpack_roundtrip() {
        let dir = tmpdir("pack");
        fs::write(dir.join("Rlyeh.toml"), "[package]\nname=\"x\"\n").unwrap();
        fs::create_dir_all(dir.join("src")).unwrap();
        fs::write(dir.join("src/main.rl"), "fn main() {}\n").unwrap();
        fs::create_dir_all(dir.join("target")).unwrap();
        fs::write(dir.join("target/artifact"), "junk").unwrap();

        let tarball = pack_directory(&dir, &["target".to_string()]).unwrap();
        let dest = tmpdir("unpack");
        unpack_tarball(&tarball, &dest).unwrap();
        assert!(dest.join("Rlyeh.toml").exists());
        assert!(dest.join("src/main.rl").exists());
        assert!(!dest.join("target/artifact").exists());
    }

    #[test]
    fn unpack_rejects_traversal() {
        // tar crate 的 Builder 在 append_data 时就会拒绝 `..` 路径，
        // 因此手工构造一个含 ../evil.txt 条目的原始 tar 头（绕过 Builder 校验）。
        fn raw_entry(name: &str, content: &[u8]) -> Vec<u8> {
            let mut out = Vec::new();
            let mut h = [0u8; 512];
            let nb = name.as_bytes();
            h[..nb.len()].copy_from_slice(nb);
            h[100..108].copy_from_slice(b"0000644\0");
            let sz = format!("{:011o}\0", content.len());
            h[124..136].copy_from_slice(sz.as_bytes());
            h[136..148].copy_from_slice(b"00000000000\0");
            h[148..156].copy_from_slice(b"        ");
            h[156] = b'0';
            h[257..263].copy_from_slice(b"ustar\0");
            h[263..265].copy_from_slice(b"00");
            let sum: u32 = h.iter().map(|&b| b as u32).sum();
            h[148..156].copy_from_slice(format!("{sum:06o}\0 ").as_bytes());
            out.extend_from_slice(&h);
            out.extend_from_slice(content);
            let rem = 512 - (content.len() % 512);
            if rem != 512 {
                out.extend(vec![0u8; rem]);
            }
            out.extend_from_slice(&[0u8; 1024]);
            out
        }

        let mut buf = Vec::new();
        {
            let mut enc = GzEncoder::new(&mut buf, Compression::default());
            enc.write_all(&raw_entry("../evil.txt", b"evil")).unwrap();
            enc.finish().unwrap();
        }
        let dest = tmpdir("traversal");
        assert!(unpack_tarball(&buf, &dest).is_err());
    }

    #[test]
    fn local_registry_publish_download_search() {
        let root = tmpdir("local");
        let reg = LocalRegistry::new(&root);

        let mut idx = PackageIndex::new("foo");
        let mut deps = HashMap::new();
        deps.insert("bar".to_string(), "^0.4".to_string());
        idx.upsert(PackageVersion {
            version: "1.2.0".to_string(),
            dependencies: deps.clone(),
            checksum: None,
            size: None,
        });
        let tarball = b"fake-tarball-content";
        let mut idx2 = idx.clone();
        // 计算并写入校验和
        let pv = PackageVersion {
            version: "1.2.0".to_string(),
            dependencies: deps,
            checksum: Some(sha256_hex(tarball)),
            size: Some(tarball.len() as u64),
        };
        idx2.upsert(pv);
        reg.publish(&idx2, "1.2.0", tarball).unwrap();

        let fetched = reg.get_index("foo").unwrap().unwrap();
        assert_eq!(fetched.name, "foo");
        assert_eq!(fetched.find("1.2.0").unwrap().dependencies.get("bar").unwrap(), "^0.4");

        let dl = reg.download("foo", "1.2.0").unwrap();
        assert_eq!(dl, tarball);

        assert_eq!(reg.search("f").unwrap(), vec!["foo"]);
        assert!(reg.search("zzz").unwrap().is_empty());
        assert!(reg.get_index("nope").unwrap().is_none());
    }

    #[test]
    fn download_detects_corruption() {
        let root = tmpdir("corrupt");
        let reg = LocalRegistry::new(&root);
        let mut idx = PackageIndex::new("foo");
        idx.upsert(PackageVersion {
            version: "1.0.0".to_string(),
            dependencies: HashMap::new(),
            checksum: Some(sha256_hex(b"data")),
            size: Some(4),
        });
        reg.publish(&idx, "1.0.0", b"data").unwrap();
        // 篡改内容
        fs::write(reg.tarball_path("foo", "1.0.0"), b"DATA").unwrap();
        assert!(reg.download("foo", "1.0.0").is_err());
    }

    #[test]
    fn http_registry_roundtrip() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let mut store: HashMap<String, Vec<u8>> = HashMap::new();
            // publish(2) + get_index(1) + download(2) + search(1) = 6 次请求
            for _ in 0..6 {
                let (mut stream, _) = listener.accept().unwrap();
                // 只读请求头到空行，再按 Content-Length 精确读 body。
                // 若用 read_to_end 等客户端 EOF，会与客户端（read_to_end 等响应）互相死锁。
                let mut head = Vec::new();
                let mut byte = [0u8; 1];
                while !head.ends_with(b"\r\n\r\n") {
                    stream.read_exact(&mut byte).unwrap();
                    head.push(byte[0]);
                }
                let text = String::from_utf8_lossy(&head);
                let first = text.lines().next().unwrap().to_string();
                let mut parts = first.split_whitespace();
                let method = parts.next().unwrap().to_string();
                let path = parts.next().unwrap().to_string();
                let content_length = text
                    .lines()
                    .find_map(|l| {
                        l.to_ascii_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().to_string())
                    })
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(0);
                let mut body = vec![0u8; content_length];
                stream.read_exact(&mut body).unwrap();
                let (status, resp_body): (u16, Vec<u8>) = match (method.as_str(), path.as_str()) {
                    ("PUT", p) => {
                        store.insert(p.to_string(), body.clone());
                        (201, Vec::new())
                    }
                    ("GET", p) => {
                        if let Some(data) = store.get(p) {
                            (200, data.clone())
                        } else if p == "/search?q=foo" {
                            (200, br#"["foo","foobar"]"#.to_vec())
                        } else {
                            (404, Vec::new())
                        }
                    }
                    _ => (400, Vec::new()),
                };
                let resp = format!(
                    "HTTP/1.1 {status} X\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    resp_body.len()
                );
                stream.write_all(resp.as_bytes()).unwrap();
                stream.write_all(&resp_body).unwrap();
            }
        });

        let base = format!("http://{addr}");
        let reg = HttpRegistry::new(&base);
        let mut idx = PackageIndex::new("foo");
        let mut deps = HashMap::new();
        deps.insert("bar".to_string(), "^0.4".to_string());
        idx.upsert(PackageVersion {
            version: "1.0.0".to_string(),
            dependencies: deps,
            checksum: Some(sha256_hex(b"hello")),
            size: Some(5),
        });
        reg.publish(&idx, "1.0.0", b"hello").unwrap();
        let fetched = reg.get_index("foo").unwrap().unwrap();
        assert_eq!(fetched.find("1.0.0").unwrap().version, "1.0.0");
        assert_eq!(reg.download("foo", "1.0.0").unwrap(), b"hello");
        assert_eq!(reg.search("foo").unwrap(), vec!["foo", "foobar"]);
        server.join().unwrap();
    }

    #[test]
    fn client_from_url_kinds() {
        let _ = client_from_url("/tmp/reg").unwrap();
        let _ = client_from_url("file:///tmp/reg").unwrap();
        let _ = client_from_url("http://127.0.0.1:8080").unwrap();
        assert!(client_from_url("https://example.com").is_err());
    }

    #[test]
    fn sha256_known_vector() {
        // sha256("abc")
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
