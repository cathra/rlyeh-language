//! 类型检查警告（建议性诊断，非致命）。
//!
//! 与 [`crate::TypeError`] 不同，警告不阻断编译，仅提示可改进点
//! （如冗余的 `*` 解引用，见借用简化 RFC 的 B-1）。

use rlyeh_lexer::Span;

/// 类型检查警告。
#[derive(Debug, Clone, PartialEq)]
pub struct Warning {
    /// 警告种类（携带附加上下文）。
    pub kind: WarningKind,
    /// 触发警告的源码位置。
    pub span: Span,
}

/// 警告种类。
#[derive(Debug, Clone, PartialEq)]
pub enum WarningKind {
    /// 冗余显式解引用：`(*r).field` / `(*r).method()` / `(*r)[i]`。
    ///
    /// 引用会自动解引用，无需手写 `*`；裸指针（`*p`）与自定义 `Deref` trait
    /// 解引用仍需 `*`——前者操作数为 `Type::RawPtr`，后者本身不是引用。
    RedundantDeref {
        /// 改进建议（已并入 [`Warning::message`]）。
        suggestion: String,
    },
    /// 并发安全基线（SH-P3-1 M3）：跨线程捕获的类型不满足 `Send + Sync`。
    ///
    /// MVP 为**告警式**（非硬阻塞）：跨线程共享该类型可能不安全，提示用户确认。
    /// 引用类型由 `Thread::start` 的 `'static` 检查先行拦截，此处仅覆盖拥有所有权的类型。
    NotSendSync {
        /// 不满足 `Send + Sync` 的类型文本。
        ty: String,
    },
}

impl Warning {
    /// 渲染警告正文（不含位置前缀）。
    fn message(&self) -> String {
        match &self.kind {
            WarningKind::RedundantDeref { suggestion } => {
                format!("redundant explicit dereference: {suggestion}")
            }
            WarningKind::NotSendSync { ty } => {
                format!("captured type `{ty}` may not be `Send + Sync`; sharing across threads may be unsafe")
            }
        }
    }

    /// 修复建议（结构化诊断 `= help:` 行）。
    fn help(&self) -> Option<&'static str> {
        match &self.kind {
            WarningKind::RedundantDeref { .. } => Some(
                "引用会自动解引用，直接写 `x.field` / `x.method()` / `x[i]`，无需 `*x`",
            ),
            WarningKind::NotSendSync { .. } => Some(
                "确保捕获的数据拥有所有权且可跨线程共享（如用 `Arc<Mutex<T>>` 替代 `Rc` / 裸指针）",
            ),
        }
    }

    /// 稳定警告码（跨编译器版本稳定，供工具 / CI 锚定）。
    fn code(&self) -> &'static str {
        match self.kind {
            WarningKind::RedundantDeref { .. } => "W001",
            WarningKind::NotSendSync { .. } => "W002",
        }
    }

    /// 渲染诊断文本（用户文件坐标下 `行:列: warning[CODE]: 消息` + `= help:`）。
    ///
    /// 坐标还原逻辑与 [`crate::TypeError::to_string_with_offset`] 一致：落在新代码上的
    /// 警告（`span.start >= prelude_len`）行号减去预置行数，对齐用户 `.rl` 文件。
    pub fn to_string_with_offset(&self, prelude_len: usize, prelude_lines: usize) -> String {
        let span = self.span;
        let loc = if span.start < prelude_len {
            format!("{}:{}", span.line, span.col)
        } else {
            format!("{}:{}", span.line.saturating_sub(prelude_lines), span.col)
        };
        let mut out = format!("{loc}: warning[{}]: {}", self.code(), self.message());
        if let Some(h) = self.help() {
            out.push_str(&format!("\n  = help: {h}"));
        }
        out
    }
}
