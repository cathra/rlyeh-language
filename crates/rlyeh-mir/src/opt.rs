//! 优化级别（对齐 C/C++ 的 -O0/-O1/-O2/-O3/-Os）。
//!
//! - O0：关闭所有优化（MIR 不跑 pass，clang 用 -O0）；
//! - O1：常量折叠 + 死代码消除；
//! - O2：在 O1 基础上做基础函数内联；
//! - O3：当前发布级流水线（O2 的 MIR pass + clang -O3）；
//! - Os：大小优先（MIR 仅常量折叠 + DCE，clang -Os）。

/// 优化级别（对齐 C/C++ 的 -O0/-O1/-O2/-O3/-Os）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum OptLevel {
    /// 关闭优化（clang -O0，不跑 MIR pass）。
    O0,
    /// 基础优化（常量折叠 + DCE，clang -O1）。
    O1,
    /// 常规优化（O1 + 基础内联，clang -O2）。
    O2,
    /// 发布级优化（O2 的 MIR pass + clang -O3）。
    #[default]
    O3,
    /// 大小优先（常量折叠 + DCE，clang -Os）。
    Os,
}

impl OptLevel {
    /// 解析 -O0 / -O1 / -O2 / -O3 / -Os（也接受 O2 形式）。
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim_start_matches('-') {
            "0" | "O0" => Some(Self::O0),
            "1" | "O1" => Some(Self::O1),
            "2" | "O2" => Some(Self::O2),
            "3" | "O3" => Some(Self::O3),
            "s" | "Os" => Some(Self::Os),
            _ => None,
        }
    }

    /// 传给 clang 的优化开关。
    pub fn clang_flag(self) -> &'static str {
        match self {
            Self::O0 => "-O0",
            Self::O1 => "-O1",
            Self::O2 => "-O2",
            Self::O3 => "-O3",
            Self::Os => "-Os",
        }
    }

    /// 稳定短名（用于缓存键 / 诊断）。
    pub fn name(self) -> &'static str {
        match self {
            Self::O0 => "O0",
            Self::O1 => "O1",
            Self::O2 => "O2",
            Self::O3 => "O3",
            Self::Os => "Os",
        }
    }

    /// 是否启用 MIR 常量折叠 / DCE。
    pub fn mir_basic(self) -> bool {
        !matches!(self, Self::O0)
    }

    /// 是否启用 MIR 函数内联。
    pub fn mir_inline(self) -> bool {
        matches!(self, Self::O2 | Self::O3)
    }
}
