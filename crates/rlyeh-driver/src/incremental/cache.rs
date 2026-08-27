//! 增量编译缓存：`.rlyeh_cache/` 目录下的产物持久化。
//!
//! 布局：
//! ```text
//! .rlyeh_cache/
//! ├── index.json                 ← 缓存索引（文件名 → 元数据）
//! └── artifacts/<hash>.ll        ← LLVM IR 产物（以源码哈希命名）
//! ```
//!
//! 损坏恢复：`index.json` 缺失或解析失败时按空缓存处理（全量重建），
//! 不中断编译。过期产物由 [`IncrementalCache::clean_stale`] 清理。

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::DriverError;

/// 缓存格式版本：**IR 生成变化（codegen 输出 / 汇编管线）必须递增**，
/// 否则用户升级工具链后旧缓存中的 IR 与新版编译器语义不一致
/// （如 2026-08-24 发布级优化：calloc 清零 + clang -O2/-O3 改动、push_str
/// 字面量快速路径特判、WASM calloc 位宽适配后，旧缓存 IR 语义不一致）。
pub const CACHE_VERSION: u32 = 5;

/// 缓存索引（`index.json` 内容）。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct CacheIndex {
    /// 缓存格式版本
    pub version: u32,
    /// 文件名 → 多版本缓存条目
    pub files: BTreeMap<String, FileVersions>,
}

/// 单个文件的历史版本集：以源码哈希为键，保留多个历史产物。
///
/// 与 ccache 相同的语义：源码内容相同（无论何时编译过）即命中。
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct FileVersions {
    /// 源码哈希 → 缓存条目
    pub versions: BTreeMap<String, CachedFile>,
}

/// 单个源码版本的缓存条目。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CachedFile {
    /// 接口哈希（函数签名，供未来多模块传播）
    pub interface_hash: String,
    /// LLVM IR 产物相对路径
    pub llvm: String,
    /// 最近一次编译时间（RFC 3339）
    pub compiled_at: String,
}

/// 编译统计（供 CLI 报告缓存命中率）。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CacheStats {
    /// 缓存命中次数（跳过流水线直接复用产物）
    pub hits: u64,
    /// 缓存未命中次数（执行完整流水线）
    pub misses: u64,
    /// 缓存索引损坏被恢复的次数
    pub recovered: u64,
}

impl CacheStats {
    /// 当前会话总编译次数。
    pub fn total(&self) -> u64 {
        self.hits + self.misses
    }

    /// 命中率（0.0 - 1.0；无编译时返回 0）。
    pub fn hit_rate(&self) -> f64 {
        let total = self.total();
        if total == 0 {
            0.0
        } else {
            self.hits as f64 / total as f64
        }
    }
}

/// 增量编译缓存（管理 `.rlyeh_cache` 目录）。
#[derive(Debug, Clone)]
pub struct IncrementalCache {
    /// 缓存根目录
    root: PathBuf,
    /// 缓存索引（内存态）
    index: CacheIndex,
    /// 索引加载时是否发生损坏恢复
    recovered: bool,
}

impl IncrementalCache {
    /// 打开（或创建）`<dir>/.rlyeh_cache` 缓存。
    ///
    /// 索引损坏时静默降级为空缓存并记录 `recovered`。
    pub fn open(dir: &Path) -> Result<Self, DriverError> {
        let root = dir.join(".rlyeh_cache");
        let mut recovered = false;
        let index = match Self::load_index(&root) {
            Ok(Some(idx)) if idx.version == CACHE_VERSION => idx,
            Ok(Some(_)) => {
                // 版本不匹配：整目录作废
                recovered = true;
                let _ = std::fs::remove_dir_all(&root);
                CacheIndex::default()
            }
            Ok(None) => CacheIndex::default(),
            Err(_) => {
                // 解析失败：视为损坏，重建索引
                recovered = true;
                CacheIndex::default()
            }
        };
        Ok(Self {
            root,
            index,
            recovered,
        })
    }

    /// 索引是否在加载时被恢复（损坏 / 版本不符）。
    pub fn was_recovered(&self) -> bool {
        self.recovered
    }

    /// 查询文件缓存：命中返回缓存条目与 LLVM IR 内容。
    ///
    /// 命中条件：该源码哈希的产物已缓存且产物文件存在。
    pub fn lookup_llvm(
        &self,
        file: &str,
        source_hash: &str,
    ) -> Result<Option<String>, DriverError> {
        let entry = match self.index.files.get(file) {
            Some(v) => v.versions.get(source_hash),
            None => None,
        };
        let entry = match entry {
            Some(e) => e,
            None => return Ok(None),
        };
        let path = self.root.join(&entry.llvm);
        if !path.is_file() {
            return Ok(None);
        }
        let llvm = std::fs::read_to_string(&path).map_err(DriverError::Io)?;
        Ok(Some(llvm))
    }

    /// 记录一次全量编译：写入 LLVM IR 产物并更新索引（保留历史版本）。
    pub fn store_llvm(
        &mut self,
        file: &str,
        source_hash: &str,
        interface_hash: &str,
        llvm: &str,
    ) -> Result<(), DriverError> {
        let artifacts = self.root.join("artifacts");
        std::fs::create_dir_all(&artifacts).map_err(DriverError::Io)?;
        let artifact_path = artifacts.join(format!("{source_hash}.ll"));
        std::fs::write(&artifact_path, llvm).map_err(DriverError::Io)?;
        let versions = self.index.files.entry(file.to_string()).or_default();
        versions.versions.insert(
            source_hash.to_string(),
            CachedFile {
                interface_hash: interface_hash.to_string(),
                llvm: format!("artifacts/{source_hash}.ll"),
                compiled_at: now_rfc3339(),
            },
        );
        self.save_index()
    }

    /// 持久化索引（原子写：先写临时文件再 rename）。
    pub fn save_index(&self) -> Result<(), DriverError> {
        std::fs::create_dir_all(&self.root).map_err(DriverError::Io)?;
        let index = CacheIndex {
            version: CACHE_VERSION,
            files: self.index.files.clone(),
        };
        let json = serde_json::to_string_pretty(&index)
            .map_err(|e| DriverError::Cache(format!("索引序列化失败: {e}")))?;
        let tmp = self.root.join("index.json.tmp");
        let final_path = self.root.join("index.json");
        std::fs::write(&tmp, json).map_err(DriverError::Io)?;
        std::fs::rename(&tmp, &final_path).map_err(DriverError::Io)
    }

    /// 清理未被当前索引引用的产物文件。
    ///
    /// 返回清理的文件数（测试可断言）。
    pub fn clean_stale(&self) -> Result<usize, DriverError> {
        let artifacts = self.root.join("artifacts");
        if !artifacts.is_dir() {
            return Ok(0);
        }
        let referenced: std::collections::HashSet<PathBuf> = self
            .index
            .files
            .values()
            .flat_map(|v| v.versions.values())
            .map(|e| PathBuf::from(&e.llvm))
            .collect();
        let mut removed = 0;
        for entry in std::fs::read_dir(&artifacts).map_err(DriverError::Io)? {
            let entry = entry.map_err(DriverError::Io)?;
            let rel = PathBuf::from("artifacts").join(entry.file_name());
            if entry.path().is_file() && !referenced.contains(&rel) {
                std::fs::remove_file(entry.path()).map_err(DriverError::Io)?;
                removed += 1;
            }
        }
        Ok(removed)
    }

    /// 缓存根目录路径。
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// 加载 `index.json`：`Ok(Some)` 正常 / `Ok(None)` 不存在 / `Err` 损坏。
    fn load_index(root: &Path) -> Result<Option<CacheIndex>, DriverError> {
        let path = root.join("index.json");
        if !path.is_file() {
            return Ok(None);
        }
        let text = std::fs::read_to_string(&path).map_err(DriverError::Io)?;
        serde_json::from_str(&text)
            .map(Some)
            .map_err(|e| DriverError::Cache(format!("缓存索引损坏: {e}")))
    }
}

/// 当前 UTC 时间的 RFC 3339 表示（秒精度）。
fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    // 简化为 Unix 时间戳秒，避免 chrono 依赖；字段仅用于诊断展示
    format!("@{secs}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("rlyeh-cache-test-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn store_and_lookup_roundtrip() {
        let dir = temp_dir("roundtrip");
        let mut cache = IncrementalCache::open(&dir).unwrap();
        assert!(cache.lookup_llvm("a.rl", "h1").unwrap().is_none());

        cache
            .store_llvm("a.rl", "h1", "iface1", "define i32 @main()")
            .unwrap();
        let llvm = cache.lookup_llvm("a.rl", "h1").unwrap().expect("命中");
        assert_eq!(llvm, "define i32 @main()");

        // 源码哈希变化 → 未命中
        assert!(cache.lookup_llvm("a.rl", "h2").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn index_persists_across_open() {
        let dir = temp_dir("persist");
        {
            let mut cache = IncrementalCache::open(&dir).unwrap();
            cache.store_llvm("a.rl", "h1", "i1", "ir-a").unwrap();
        }
        {
            let cache = IncrementalCache::open(&dir).unwrap();
            assert_eq!(
                cache.lookup_llvm("a.rl", "h1").unwrap().as_deref(),
                Some("ir-a")
            );
        }
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn corrupted_index_recovers_to_empty() {
        let dir = temp_dir("corrupt");
        let cache_dir = dir.join(".rlyeh_cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        std::fs::write(cache_dir.join("index.json"), "{ 这不是 JSON !!!").unwrap();

        let cache = IncrementalCache::open(&dir).unwrap();
        assert!(cache.was_recovered());
        assert!(cache.lookup_llvm("a.rl", "h1").unwrap().is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn version_mismatch_invalidates() {
        let dir = temp_dir("version");
        let cache_dir = dir.join(".rlyeh_cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let bad = CacheIndex {
            version: CACHE_VERSION + 99,
            files: BTreeMap::new(),
        };
        std::fs::write(
            cache_dir.join("index.json"),
            serde_json::to_string(&bad).unwrap(),
        )
        .unwrap();

        let cache = IncrementalCache::open(&dir).unwrap();
        assert!(cache.was_recovered());
        assert!(!cache.root().join("index.json").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn clean_stale_removes_unreferenced_only() {
        let dir = temp_dir("stale");
        let mut cache = IncrementalCache::open(&dir).unwrap();
        cache.store_llvm("a.rl", "keep", "i1", "ir-keep").unwrap();

        // 模拟历史残留产物
        let stale = cache.root().join("artifacts/stale-hash.ll");
        std::fs::write(&stale, "ir-stale").unwrap();

        let removed = cache.clean_stale().unwrap();
        assert_eq!(removed, 1);
        assert!(!stale.exists());
        assert!(cache.root().join("artifacts/keep.ll").exists());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stats_accumulate() {
        let mut stats = CacheStats::default();
        assert_eq!(stats.total(), 0);
        stats.hits += 2;
        stats.misses += 1;
        assert_eq!(stats.total(), 3);
        assert!((stats.hit_rate() - 2.0 / 3.0).abs() < 1e-9);
    }
}
