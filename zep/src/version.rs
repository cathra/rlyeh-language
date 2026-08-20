//! 语义化版本（semver）与版本需求（VersionReq）。
//!
//! 与 PubGrub 的 `SemanticVersion` / `Ranges` 互转，供依赖解析使用。

use std::fmt;

use pubgrub::{Ranges, SemanticVersion};

use crate::error::{Result, ZepError};

/// 语义化版本 `major.minor.patch`。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// 主版本号。
    pub major: u64,
    /// 次版本号。
    pub minor: u64,
    /// 修订号。
    pub patch: u64,
}

impl Version {
    /// 构造新版本。
    pub fn new(major: u64, minor: u64, patch: u64) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }

    /// 从 `major.minor.patch`（patch 可省略）解析。
    pub fn parse(s: &str) -> Result<Self> {
        let parts: Vec<&str> = s.trim().split('.').collect();
        if parts.is_empty() || parts.len() > 3 {
            return Err(ZepError::Version(format!("非法版本号: {s:?}")));
        }
        let mut nums = [0u64; 3];
        for (i, p) in parts.iter().enumerate() {
            if p.is_empty() || !p.chars().all(|c| c.is_ascii_digit()) {
                return Err(ZepError::Version(format!("非法版本号: {s:?}")));
            }
            nums[i] = p
                .parse()
                .map_err(|_| ZepError::Version(format!("版本号溢出: {s:?}")))?;
        }
        Ok(Self::new(nums[0], nums[1], nums[2]))
    }

    /// 转换为 PubGrub 语义版本。
    pub fn to_semantic(&self) -> SemanticVersion {
        SemanticVersion::new(self.major as u32, self.minor as u32, self.patch as u32)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// 版本需求：声明依赖时使用的约束。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum VersionReq {
    /// 任意版本 `*`。
    Any,
    /// Caret 兼容：`^1.2.3`，也作为裸版本号的默认语义（cargo 风格）。
    Compatible(Version),
    /// 精确匹配：`=1.2.3`。
    Exact(Version),
    /// `>=1.2.3`
    Gte(Version),
    /// `>1.2.3`
    Gt(Version),
    /// `<=1.2.3`
    Lte(Version),
    /// `<1.2.3`
    Lt(Version),
}

impl VersionReq {
    /// 解析需求字符串。
    ///
    /// 支持：`*`、`^1.2.3`、`=1.2.3`、`>=1.2.3`、`>1.2.3`、`<=1.2.3`、
    /// `<1.2.3`、裸 `1.2.3`（等价 `^1.2.3`）。
    pub fn parse(s: &str) -> Result<Self> {
        let s = s.trim();
        if s.is_empty() || s == "*" {
            return Ok(Self::Any);
        }
        for (prefix, ctor) in [
            ("^", VersionReq::Compatible as fn(Version) -> VersionReq),
            ("=", VersionReq::Exact),
            (">=", VersionReq::Gte),
            (">", VersionReq::Gt),
            ("<=", VersionReq::Lte),
            ("<", VersionReq::Lt),
        ] {
            if let Some(rest) = s.strip_prefix(prefix) {
                let v = Version::parse(rest)
                    .map_err(|_| ZepError::VersionReq(format!("非法版本需求: {s:?}")))?;
                return Ok(ctor(v));
            }
        }
        // 裸版本号 → caret 兼容
        let v = Version::parse(s)
            .map_err(|_| ZepError::VersionReq(format!("非法版本需求: {s:?}")))?;
        Ok(Self::Compatible(v))
    }

    /// 判断某版本是否满足需求。
    pub fn matches(&self, v: &Version) -> bool {
        match self {
            Self::Any => true,
            Self::Exact(e) => v == e,
            Self::Gte(e) => v >= e,
            Self::Gt(e) => v > e,
            Self::Lte(e) => v <= e,
            Self::Lt(e) => v < e,
            Self::Compatible(base) => match compatible_range(base) {
                Some(hi) => *v >= *base && *v < hi,
                None => *v == *base,
            },
        }
    }

    /// 转换为 PubGrub `Ranges<SemanticVersion>`。
    pub fn to_ranges(&self) -> Ranges<SemanticVersion> {
        match self {
            Self::Any => Ranges::full(),
            Self::Exact(v) => Ranges::singleton(v.to_semantic()),
            Self::Gte(v) => Ranges::higher_than(v.to_semantic()),
            Self::Gt(v) => Ranges::strictly_higher_than(v.to_semantic()),
            Self::Lte(v) => Ranges::lower_than(v.to_semantic()),
            Self::Lt(v) => Ranges::strictly_lower_than(v.to_semantic()),
            Self::Compatible(v) => {
                let sv = v.to_semantic();
                match compatible_range(v) {
                    Some(hi) => Ranges::between(sv, hi.to_semantic()),
                    None => Ranges::singleton(sv),
                }
            }
        }
    }
}

impl fmt::Display for VersionReq {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Any => write!(f, "*"),
            Self::Compatible(v) => write!(f, "^{v}"),
            Self::Exact(v) => write!(f, "={v}"),
            Self::Gte(v) => write!(f, ">={v}"),
            Self::Gt(v) => write!(f, ">{v}"),
            Self::Lte(v) => write!(f, "<={v}"),
            Self::Lt(v) => write!(f, "<{v}"),
        }
    }
}

/// Caret 兼容的上界：
/// - `1.x.y` → `<(major+1).0.0`
/// - `0.x.y (x>0)` → `<0.(minor+1).0`
/// - `0.0.y` → `<0.0.(patch+1)`
/// - `0.0.0` → 精确（无上界）
fn compatible_range(v: &Version) -> Option<Version> {
    if v.major > 0 {
        Some(Version::new(v.major + 1, 0, 0))
    } else if v.minor > 0 {
        Some(Version::new(0, v.minor + 1, 0))
    } else if v.patch > 0 {
        Some(Version::new(0, 0, v.patch + 1))
    } else {
        None
    }
}

/// 从 PubGrub 语义版本还原为 [`Version`]。
pub fn from_semantic(v: &SemanticVersion) -> Version {
    let (major, minor, patch): (u32, u32, u32) = (*v).into();
    Version::new(major as u64, minor as u64, patch as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_version_basic() {
        assert_eq!(Version::parse("1.2.3").unwrap(), Version::new(1, 2, 3));
        assert_eq!(Version::parse("1.2").unwrap(), Version::new(1, 2, 0));
        assert_eq!(Version::parse("1").unwrap(), Version::new(1, 0, 0));
        assert!(Version::parse("1.2.3.4").is_err());
        assert!(Version::parse("a.b").is_err());
        assert!(Version::parse("").is_err());
    }

    #[test]
    fn parse_version_req() {
        assert_eq!(VersionReq::parse("*").unwrap(), VersionReq::Any);
        assert_eq!(
            VersionReq::parse("^1.2.3").unwrap(),
            VersionReq::Compatible(Version::new(1, 2, 3))
        );
        assert_eq!(
            VersionReq::parse("=1.2.3").unwrap(),
            VersionReq::Exact(Version::new(1, 2, 3))
        );
        assert_eq!(
            VersionReq::parse(">=0.4").unwrap(),
            VersionReq::Gte(Version::new(0, 4, 0))
        );
        assert_eq!(
            VersionReq::parse("1.5").unwrap(),
            VersionReq::Compatible(Version::new(1, 5, 0))
        );
        assert!(VersionReq::parse("^^1.0").is_err());
    }

    #[test]
    fn req_matches() {
        let req = VersionReq::parse("^1.2.0").unwrap();
        assert!(req.matches(&Version::new(1, 2, 0)));
        assert!(req.matches(&Version::new(1, 9, 9)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
        assert!(!req.matches(&Version::new(1, 1, 9)));

        let req = VersionReq::parse("^0.2.3").unwrap();
        assert!(req.matches(&Version::new(0, 2, 9)));
        assert!(!req.matches(&Version::new(0, 3, 0)));

        let req = VersionReq::parse("^0.0.3").unwrap();
        assert!(req.matches(&Version::new(0, 0, 3)));
        assert!(!req.matches(&Version::new(0, 0, 4)));

        // 逗号组合条件暂不支持，直接解析应报错
        assert!(VersionReq::parse(">=0.4, <0.5").is_err());
        let req = VersionReq::Gte(Version::new(0, 4, 0));
        assert!(req.matches(&Version::new(0, 4, 5)));
        assert!(req.matches(&Version::new(0, 5, 0)));
        assert!(!req.matches(&Version::new(0, 3, 9)));

        let req = VersionReq::parse("<2").unwrap();
        assert!(req.matches(&Version::new(1, 99, 99)));
        assert!(!req.matches(&Version::new(2, 0, 0)));
    }

    #[test]
    fn req_to_ranges() {
        let ranges = VersionReq::parse("^1.2.0").unwrap().to_ranges();
        assert!(ranges.contains(&SemanticVersion::new(1, 2, 0)));
        assert!(ranges.contains(&SemanticVersion::new(1, 9, 9)));
        assert!(!ranges.contains(&SemanticVersion::new(2, 0, 0)));

        let exact = VersionReq::parse("=1.2.3").unwrap().to_ranges();
        assert!(exact.contains(&SemanticVersion::new(1, 2, 3)));
        assert!(!exact.contains(&SemanticVersion::new(1, 2, 4)));

        let any = VersionReq::parse("*").unwrap().to_ranges();
        assert!(any.contains(&SemanticVersion::new(99, 0, 1)));

        let ge = VersionReq::parse(">=2.0").unwrap().to_ranges();
        assert!(ge.contains(&SemanticVersion::new(2, 0, 0)));
        assert!(!ge.contains(&SemanticVersion::new(1, 9, 9)));
    }
}
