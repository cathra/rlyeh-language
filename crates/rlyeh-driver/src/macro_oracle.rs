//! M-M3c：`macro_rules` 展开的 Rust oracle（供 Rlyeh 版 `self-host/macro.rl` 对拍）。
//!
//! 输入为**组内容**（已剥掉外层定界符）的三段源码，与 `rlyeh-macro` 的调用约定一致：
//! - `matcher_src`：matcher `(...)` 的内容（如 `$x:ident , $y:tt`）；
//! - `transcriber_src`：transcriber `{...}` 的内容（如 `$y $x`）；
//! - `invocation_src`：宏调用定界组内的内容（如 `foo , ( 1 + 2 )`）。
//!
//! 输出与 `rlyeh-lexer` 的规范化 token 文本逐行对齐：每个展开后的 token 一行，
//! 格式同 `token_to_canonical`（见 `docs/tasks/self-hosting/self-host-lexer.md`），
//! 无尾随换行（空展开输出为空串）。

use std::collections::HashMap;

use rlyeh_lexer::{Lexer, Token};

use crate::error::DriverError;

/// 单规则宏 `m!` 展开：`matcher => transcriber`，输入为宏调用括号内的 token。
///
/// 返回展开后的规范化 token 文本（每行一个 token）。
pub fn emit_macro_expand(
    matcher_src: &str,
    transcriber_src: &str,
    invocation_src: &str,
) -> Result<String, DriverError> {
    emit_macro_expand_rules(&[matcher_src], &[transcriber_src], invocation_src)
}

/// 多规则宏 `m!` 展开：`matcher_srcs[i] => transcriber_srcs[i]`，按规则顺序取首个
/// 匹配成功者（与 `rlyeh_macro::expand` 语义一致）。
///
/// 返回展开后的规范化 token 文本（每行一个 token）。
pub fn emit_macro_expand_rules(
    matcher_srcs: &[&str],
    transcriber_srcs: &[&str],
    invocation_src: &str,
) -> Result<String, DriverError> {
    if matcher_srcs.len() != transcriber_srcs.len() {
        return Err(DriverError::Usage(
            "emit_macro_expand_rules: matcher/transcriber 规则数不一致".to_string(),
        ));
    }
    let mut rules: Vec<rlyeh_macro::MacroRule> = Vec::with_capacity(matcher_srcs.len());
    for (matcher_src, transcriber_src) in matcher_srcs.iter().zip(transcriber_srcs.iter()) {
        let matcher_tokens = lex_tokens(matcher_src)?;
        let matcher = rlyeh_macro::parse_matcher(&matcher_tokens)
            .map_err(|e| DriverError::Typecheck(format!("macro matcher: {}", e.0)))?;
        let trans_tokens = lex_tokens(transcriber_src)?;
        let transcriber = rlyeh_macro::parse_transcriber(&trans_tokens)
            .map_err(|e| DriverError::Typecheck(format!("macro transcriber: {}", e.0)))?;
        rules.push(rlyeh_macro::MacroRule {
            matcher,
            transcriber,
        });
    }
    let mut macros: HashMap<String, Vec<rlyeh_macro::MacroRule>> = HashMap::new();
    macros.insert("m".to_string(), rules);
    let input = lex_tokens(invocation_src)?;
    let expanded = rlyeh_macro::expand(&macros, "m", &input)
        .map_err(|e| DriverError::Typecheck(format!("macro expand: {}", e.0)))?;
    let lines: Vec<String> = expanded.iter().map(super::token_to_canonical).collect();
    Ok(lines.join("\n"))
}

/// 用 `rlyeh-lexer` 对一段源码做词法分析（与驱动流水线同源）。
fn lex_tokens(source: &str) -> Result<Vec<Token>, DriverError> {
    let mut lexer = Lexer::new(source);
    let located = lexer
        .tokenize()
        .map_err(|e| DriverError::Typecheck(format!("lex error: {e}")))?;
    Ok(located.into_iter().map(|t| t.token).collect())
}
