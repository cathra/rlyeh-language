//! Zeta 声明式宏展开器（`macro_rules!`，MVP）。
//!
//! 负责：matcher（`(...)`）token 解析 → 输入 token 流匹配 → 元变量绑定 →
//! transcriber（`{...}`）token 展开。展开产物为纯 token 序列，
//! 由 parser 递归解析为 AST（展开发生在 parse 阶段、typecheck 之前）。
//!
//! MVP 限制（详见 `docs/grammar.md` §2.14）：
//! - 元变量种类支持 `$x:expr` / `$x:ident` / `$x:ty` / `$x:tt`；
//!   `$x:expr` 捕获完整表达式 token 序列（token 级优先级爬升：前缀一元
//!   `-`/`!`/`not`/`&`/`*`、二元中缀、后缀调用/索引/成员/`?`/`as` 转换），
//!   如 `a > b`、`-1`、`f(x) + 1` 均可捕获。
//! - 重复支持 `$($inner),sep op`（op 为 `*` `+` `?`）；transcriber 中的 `$x`
//!   与 `$($inner),*` 展开按捕获迭代。
//! - 宏须在使用前定义（单遍展开，无前向引用）；无 hygiene（全局名称匹配）。

use std::collections::HashMap;

use zeta_lexer::Token;

/// 元变量种类（`$x:kind`）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MetaKind {
    /// `expr`：表达式
    Expr,
    /// `ident`：标识符
    Ident,
    /// `ty`：类型
    Ty,
    /// `tt`：单 token（或定界组整体）
    Tt,
}

/// 重复操作符（`*` `+` `?`）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatOp {
    /// `*`：0 或多次
    ZeroOrMore,
    /// `+`：1 或多次
    OneOrMore,
    /// `?`：0 或 1 次
    Optional,
}

/// matcher token（规则左侧 `(...)` 内）
#[derive(Debug, Clone, PartialEq)]
pub enum MatcherToken {
    /// 字面 token（必须精确匹配）
    Lit(Token),
    /// 元变量 `$name:kind`
    Meta { name: String, kind: MetaKind },
    /// 定界组（须成对匹配）
    Delim {
        open: Token,
        inner: Vec<MatcherToken>,
        close: Token,
    },
    /// 重复 `$($inner),sep op`
    Repeat {
        inner: Vec<MatcherToken>,
        sep: Option<Token>,
        op: RepeatOp,
    },
}

/// transcriber token（规则右侧 `{...}` 内）
#[derive(Debug, Clone, PartialEq)]
pub enum TransToken {
    /// 字面 token
    Lit(Token),
    /// 元变量引用 `$name`
    Meta { name: String },
    /// 定界组
    Delim {
        open: Token,
        inner: Vec<TransToken>,
        close: Token,
    },
    /// 重复 `$($inner),sep op`
    Repeat {
        inner: Vec<TransToken>,
        sep: Option<Token>,
        op: RepeatOp,
    },
}

/// 一条宏规则（matcher => transcriber）
#[derive(Debug, Clone)]
pub struct MacroRule {
    /// 已解析的 matcher
    pub matcher: Vec<MatcherToken>,
    /// 已解析的 transcriber
    pub transcriber: Vec<TransToken>,
}

/// 宏展开错误
#[derive(Debug, Clone)]
pub struct MacroError(pub String);

/// 元变量绑定：name → 每次捕获的 token 序列（顺序 = 捕获/迭代顺序）
pub type Bindings = HashMap<String, Vec<Vec<Token>>>;

// ===== 定界符辅助 =====

fn is_open(t: &Token) -> bool {
    matches!(t, Token::LParen | Token::LBracket | Token::LBrace)
}

fn is_close(t: &Token) -> bool {
    matches!(t, Token::RParen | Token::RBracket | Token::RBrace)
}

fn close_for(t: &Token) -> Token {
    match t {
        Token::LParen => Token::RParen,
        Token::LBracket => Token::RBracket,
        Token::LBrace => Token::RBrace,
        _ => Token::Eof,
    }
}

/// 从 `tokens[pos]` 起（须为开定界符）找到配对闭合，返回 `(inner, close 后下标)`。
fn find_group(tokens: &[Token], pos: usize) -> Option<(Vec<Token>, usize)> {
    let open = tokens.get(pos)?;
    let close = close_for(open);
    if matches!(close, Token::Eof) {
        return None;
    }
    let mut depth = 1usize;
    let mut i = pos + 1;
    while i < tokens.len() {
        let t = &tokens[i];
        if is_open(t) {
            depth += 1;
        } else if is_close(t) {
            depth -= 1;
            if depth == 0 {
                return Some((tokens[pos + 1..i].to_vec(), i + 1));
            }
        }
        i += 1;
    }
    None
}

// ===== matcher 解析 =====

/// 解析 matcher token 序列（`(...)` 的内容，顶层读完为止）。
pub fn parse_matcher(tokens: &[Token]) -> Result<Vec<MatcherToken>, MacroError> {
    let mut pos = 0;
    let (out, end) = parse_matcher_inner(tokens, &mut pos, None)?;
    if end != tokens.len() {
        return Err(MacroError("matcher 尾部有多余 token".into()));
    }
    Ok(out)
}

/// 内部解析；`stop` 为 Some 时遇到该 token 停止（重复 `)` 分隔）。
fn parse_matcher_inner(
    tokens: &[Token],
    pos: &mut usize,
    stop: Option<&Token>,
) -> Result<(Vec<MatcherToken>, usize), MacroError> {
    let mut out = Vec::new();
    while *pos < tokens.len() {
        let t = &tokens[*pos];
        if let Some(s) = stop {
            if t == s {
                return Ok((out, *pos));
            }
        }
        match t {
            Token::Dollar => {
                // `$name:kind` 或 `$(` 重复
                let next = tokens.get(*pos + 1).ok_or_else(|| {
                    MacroError("`$` 后缺少元变量名或 `(`".into())
                })?;
                match next {
                    Token::LParen => {
                        // 重复：`$(` inner `)` [sep] op
                        let inner_tokens = tokens[*pos + 2..].to_vec();
                        let mut inner_pos = 0usize;
                        let (inner, inner_end) = parse_matcher_inner(
                            &inner_tokens,
                            &mut inner_pos,
                            Some(&Token::RParen),
                        )?;
                        if inner_tokens.get(inner_end) != Some(&Token::RParen) {
                            return Err(MacroError("重复 `$(` 缺少 `)`".into()));
                        }
                        // 剩余 token（`]` `)` `*` `+` `?` 等）
                        let rest = &inner_tokens[inner_end + 1..];
                        let (sep, op, consumed) = parse_sep_op(rest)?;
                        let total = (*pos) + 2 + inner_end + 1 + consumed;
                        out.push(MatcherToken::Repeat {
                            inner,
                            sep,
                            op,
                        });
                        *pos = total;
                    }
                    Token::Ident(name) => {
                        // `$name:kind`
                        if tokens.get(*pos + 2) != Some(&Token::Colon) {
                            return Err(MacroError(format!(
                                "元变量 `{name}` 缺少 `:kind`"
                            )));
                        }
                        let kind_name = match tokens.get(*pos + 3) {
                            Some(Token::Ident(k)) => k.clone(),
                            _ => {
                                return Err(MacroError(format!(
                                    "元变量 `{name}` 缺少种类（expr/ident/ty/tt）"
                                )))
                            }
                        };
                        let kind = match kind_name.as_str() {
                            "expr" => MetaKind::Expr,
                            "ident" => MetaKind::Ident,
                            "ty" => MetaKind::Ty,
                            "tt" => MetaKind::Tt,
                            _ => {
                                return Err(MacroError(format!(
                                    "未知元变量种类 `{kind_name}`（支持 expr/ident/ty/tt）"
                                )))
                            }
                        };
                        out.push(MatcherToken::Meta {
                            name: name.clone(),
                            kind,
                        });
                        *pos += 4;
                    }
                    _ => {
                        return Err(MacroError("`$` 后须为元变量名或 `(`".into()));
                    }
                }
            }
            Token::LParen | Token::LBracket | Token::LBrace => {
                let (inner_tokens, end) = find_group(tokens, *pos).ok_or_else(|| {
                    MacroError("定界组未闭合（matcher）".into())
                })?;
                let mut inner_pos = 0usize;
                let (inner, _) = parse_matcher_inner(&inner_tokens, &mut inner_pos, None)?;
                out.push(MatcherToken::Delim {
                    open: t.clone(),
                    inner,
                    close: close_for(t),
                });
                *pos = end;
            }
            _ => {
                out.push(MatcherToken::Lit(t.clone()));
                *pos += 1;
            }
        }
    }
    Ok((out, *pos))
}

/// 解析重复后缀 `[sep] op`，返回 `(sep, op, 消费数)`。
fn parse_sep_op(tokens: &[Token]) -> Result<(Option<Token>, RepeatOp, usize), MacroError> {
    if tokens.is_empty() {
        return Err(MacroError("重复 `$(` 缺少操作符（`*`/`+`/`?`）".into()));
    }
    let first = &tokens[0];
    let op = match first {
        Token::Star => RepeatOp::ZeroOrMore,
        Token::Plus => RepeatOp::OneOrMore,
        Token::Question => RepeatOp::Optional,
        _ => {
            // 分隔符 + 操作符
            let op = match tokens.get(1) {
                Some(Token::Star) => RepeatOp::ZeroOrMore,
                Some(Token::Plus) => RepeatOp::OneOrMore,
                Some(Token::Question) => RepeatOp::Optional,
                _ => {
                    return Err(MacroError(
                        "重复 `$(` 后须为分隔符 + 操作符（`*`/`+`/`?`）".into(),
                    ))
                }
            };
            return Ok((Some(first.clone()), op, 2));
        }
    };
    Ok((None, op, 1))
}

// ===== transcriber 解析 =====

/// 解析 transcriber token 序列（`{...}` 的内容）。
pub fn parse_transcriber(tokens: &[Token]) -> Result<Vec<TransToken>, MacroError> {
    let mut pos = 0;
    let (out, end) = parse_transcriber_inner(tokens, &mut pos, None)?;
    if end != tokens.len() {
        return Err(MacroError("transcriber 尾部有多余 token".into()));
    }
    Ok(out)
}

fn parse_transcriber_inner(
    tokens: &[Token],
    pos: &mut usize,
    stop: Option<&Token>,
) -> Result<(Vec<TransToken>, usize), MacroError> {
    let mut out = Vec::new();
    while *pos < tokens.len() {
        let t = &tokens[*pos];
        if let Some(s) = stop {
            if t == s {
                return Ok((out, *pos));
            }
        }
        match t {
            Token::Dollar => {
                let next = tokens.get(*pos + 1).ok_or_else(|| {
                    MacroError("`$` 后缺少元变量名或 `(`（transcriber）".into())
                })?;
                match next {
                    Token::Ident(name) => {
                        out.push(TransToken::Meta { name: name.clone() });
                        *pos += 2;
                    }
                    Token::LParen => {
                        // 重复
                        let inner_tokens = tokens[*pos + 2..].to_vec();
                        let mut inner_pos = 0usize;
                        let (inner, inner_end) =
                            parse_transcriber_inner(&inner_tokens, &mut inner_pos, Some(&Token::RParen))?;
                        if inner_tokens.get(inner_end) != Some(&Token::RParen) {
                            return Err(MacroError("重复 `$(` 缺少 `)`（transcriber）".into()));
                        }
                        let rest = &inner_tokens[inner_end + 1..];
                        let (sep, op, consumed) = parse_sep_op(rest)?;
                        out.push(TransToken::Repeat { inner, sep, op });
                        *pos += 2 + inner_end + 1 + consumed;
                    }
                    _ => {
                        return Err(MacroError(
                            "`$` 后须为元变量名或 `(`（transcriber）".into(),
                        ))
                    }
                }
            }
            Token::LParen | Token::LBracket | Token::LBrace => {
                let (inner_tokens, end) = find_group(tokens, *pos)
                    .ok_or_else(|| MacroError("定界组未闭合（transcriber）".into()))?;
                let mut inner_pos = 0usize;
                let (inner, _) = parse_transcriber_inner(&inner_tokens, &mut inner_pos, None)?;
                out.push(TransToken::Delim {
                    open: t.clone(),
                    inner,
                    close: close_for(t),
                });
                *pos = end;
            }
            _ => {
                out.push(TransToken::Lit(t.clone()));
                *pos += 1;
            }
        }
    }
    Ok((out, *pos))
}

// ===== 匹配 =====

/// 元变量捕获：向 bindings[name] 追加一次捕获。
fn push_binding(bindings: &mut Bindings, name: &str, tokens: Vec<Token>) {
    bindings.entry(name.to_string()).or_default().push(tokens);
}

/// 判断 token 是否为前缀一元运算符（`-x` / `!x` / `not x` / `&x` / `*p`）。
fn is_prefix_op(t: &Token) -> bool {
    matches!(
        t,
        Token::Minus
            | Token::Plus
            | Token::NotNot
            | Token::Not
            | Token::BitAnd
            | Token::Star
    )
}

/// 判断 token 是否为二元中缀运算符。
fn is_binary_op(t: &Token) -> bool {
    matches!(
        t,
        Token::OrOr
            | Token::AndAnd
            | Token::BitOr
            | Token::BitXor
            | Token::BitAnd
            | Token::Eq
            | Token::Ne
            | Token::Lt
            | Token::Le
            | Token::Gt
            | Token::Ge
            | Token::Shl
            | Token::Shr
            | Token::Plus
            | Token::Minus
            | Token::Star
            | Token::Slash
            | Token::Percent
            | Token::In
            | Token::NotIn
            | Token::Range
            | Token::DotDotLt
            | Token::DotDotDot
            | Token::LtDotDot
    )
}

/// 消费一个类型 token 序列（`i64` / `String` / `Vec<i64>` / `mod::Type`），返回消费数。
fn consume_ty(input: &[Token], pos: usize) -> Option<usize> {
    match input.get(pos)? {
        Token::Ident(_) => {
            let mut p = pos + 1;
            // 路径段 `a::b::Type`
            while input.get(p) == Some(&Token::Colon) && input.get(p + 1) == Some(&Token::Colon) {
                p += 2;
                if !matches!(input.get(p), Some(Token::Ident(_))) {
                    return None;
                }
                p += 1;
            }
            // 泛型参数 `Vec<i64>` / `HashMap<i64, String>`（尖括号深度配对）
            if input.get(p) == Some(&Token::Lt) {
                let mut depth = 1usize;
                let mut q = p + 1;
                while q < input.len() && depth > 0 {
                    match input.get(q) {
                        Some(Token::Lt) => depth += 1,
                        Some(Token::Gt) => depth -= 1,
                        None => return None,
                        _ => {}
                    }
                    q += 1;
                }
                p = q;
            }
            Some(p)
        }
        _ => None,
    }
}

/// 消费一个操作数（前缀一元 * 原子 * 后缀），返回消费数。
fn consume_operand(input: &[Token], mut pos: usize) -> Option<usize> {
    // 前缀一元
    while input.get(pos).is_some_and(is_prefix_op) {
        pos += 1;
    }
    // 原子操作数：定界组整体 / 标识符 / 字面量
    match input.get(pos)? {
        t if is_open(t) => pos = find_group(input, pos)?.1,
        Token::Ident(_)
        | Token::True
        | Token::False
        | Token::IntLiteral(_)
        | Token::FloatLiteral(_)
        | Token::StringLiteral(_)
        | Token::CharLiteral(_)
        | Token::BoolLiteral(_)
        | Token::TimeLiteral { .. } => pos += 1,
        _ => return None,
    }
    // 后缀：调用 `(...)` / 索引 `[...]` / 成员 `.x` / `?` / `as` 类型转换
    loop {
        match input.get(pos) {
            None => break,
            Some(Token::LParen) | Some(Token::LBracket) => pos = find_group(input, pos)?.1,
            Some(Token::Dot) => {
                pos += 1;
                if !matches!(input.get(pos), Some(Token::Ident(_))) {
                    return None;
                }
                pos += 1;
            }
            Some(Token::Question) => pos += 1,
            // 宏调用操作数 `f!(...)`：`!` 后须跟开定界符组
            Some(Token::NotNot) if matches!(input.get(pos + 1), Some(t) if is_open(t)) => {
                pos += 1;
                pos = find_group(input, pos)?.1;
            }
            Some(Token::As) => {
                pos += 1;
                pos = consume_ty(input, pos)?;
            }
            Some(_) => break,
        }
    }
    Some(pos)
}

/// 消费一个完整表达式（token 级优先级爬升），返回消费数。
///
/// 支持：前缀一元（`-` `+` `!` `not` `&` `*`）、二元中缀（`||` `&&` 位运算
/// 比较 加减乘除模 移位 `in` 范围）、后缀（调用 / 索引 / 成员 / `?` / `as`）。
/// 边界：遇 `,` `)` `]` `}` `;` `=` `=>` `:` 等分隔 token 即停止。
fn consume_expr(input: &[Token]) -> Option<usize> {
    let mut pos = consume_operand(input, 0)?;
    loop {
        match input.get(pos) {
            None => break,
            Some(t) if is_binary_op(t) => {
                pos += 1;
                pos = consume_operand(input, pos)?;
            }
            Some(_) => break,
        }
    }
    Some(pos)
}

/// 匹配 matcher 与输入 token 序列；成功返回消费数。
fn match_matcher(
    matcher: &[MatcherToken],
    input: &[Token],
    bindings: &mut Bindings,
) -> Option<usize> {
    let mut off = 0usize;
    for m in matcher {
        let rest = &input[off..];
        match m {
            MatcherToken::Lit(t) => {
                if rest.first() == Some(t) {
                    off += 1;
                } else {
                    return None;
                }
            }
            MatcherToken::Meta { name, kind } => match kind {
                MetaKind::Ident => match rest.first() {
                    Some(Token::Ident(_)) => {
                        push_binding(bindings, name, vec![rest[0].clone()]);
                        off += 1;
                    }
                    _ => return None,
                },
                MetaKind::Ty => match rest.first() {
                    Some(t) if !is_open(t) && !is_close(t) => {
                        push_binding(bindings, name, vec![rest[0].clone()]);
                        off += 1;
                    }
                    _ => return None,
                },
                MetaKind::Tt => {
                    if rest.is_empty() {
                        return None;
                    }
                    let n = if is_open(&rest[0]) {
                        find_group(rest, 0)?.1
                    } else {
                        1
                    };
                    push_binding(bindings, name, rest[..n].to_vec());
                    off += n;
                }
                MetaKind::Expr => {
                    let n = consume_expr(rest)?;
                    push_binding(bindings, name, rest[..n].to_vec());
                    off += n;
                }
            },
            MatcherToken::Delim { open, inner, close } => {
                if rest.first() != Some(open) {
                    return None;
                }
                let (inner_tokens, end) = find_group(rest, 0)?;
                let mut sub = Bindings::new();
                let consumed = match_matcher(inner, &inner_tokens, &mut sub)?;
                if consumed != inner_tokens.len() {
                    return None;
                }
                merge_bindings(bindings, &sub);
                off += end;
                let _ = close;
            }
            MatcherToken::Repeat { inner, sep, op } => {
                let mut iters = 0usize;
                loop {
                    let saved = bindings.clone();
                    let mut sub = Bindings::new();
                    if let Some(consumed) = match_matcher(inner, &input[off..], &mut sub) {
                        merge_bindings(bindings, &sub);
                        iters += 1;
                        off += consumed;
                        // 分隔符
                        if let Some(sep_t) = sep {
                            if input.get(off) == Some(sep_t) {
                                off += 1;
                            }
                        }
                        // 检查是继续迭代还是已经耗尽
                        // 内层匹配成功即继续尝试（回溯由调用方处理）
                        let _ = saved;
                    } else {
                        break;
                    }
                }
                match op {
                    RepeatOp::OneOrMore if iters == 0 => return None,
                    _ => {}
                }
            }
        }
    }
    Some(off)
}

/// 把子绑定合并进父绑定。
fn merge_bindings(parent: &mut Bindings, child: &Bindings) {
    for (k, v) in child {
        parent.entry(k.clone()).or_default().extend(v.clone());
    }
}

// ===== 展开 =====

/// 展开 transcriber；返回 token 序列。
pub fn expand_trans(
    trans: &[TransToken],
    bindings: &Bindings,
) -> Result<Vec<Token>, MacroError> {
    let mut out = Vec::new();
    expand_inner(trans, bindings, &mut out)?;
    Ok(out)
}

fn expand_inner(
    trans: &[TransToken],
    bindings: &Bindings,
    out: &mut Vec<Token>,
) -> Result<(), MacroError> {
    for t in trans {
        match t {
            TransToken::Lit(tok) => out.push(tok.clone()),
            TransToken::Meta { name } => {
                let captures = bindings
                    .get(name)
                    .ok_or_else(|| MacroError(format!("元变量 `${name}` 未绑定")))?;
                let cap = captures.last().ok_or_else(|| {
                    MacroError(format!("元变量 `${name}` 无捕获"))
                })?;
                out.extend(cap.iter().cloned());
            }
            TransToken::Delim { open, inner, close } => {
                out.push(open.clone());
                expand_inner(inner, bindings, out)?;
                out.push(close.clone());
            }
            TransToken::Repeat { inner, sep, op } => {
                // 迭代次数 = inner 中各元变量捕获数最大值
                let mut iters = 0usize;
                let mut has_meta = false;
                collect_meta_names(inner, &mut |name| {
                    has_meta = true;
                    if let Some(caps) = bindings.get(name) {
                        iters = iters.max(caps.len());
                    }
                });
                if !has_meta {
                    iters = if matches!(op, RepeatOp::OneOrMore) { 1 } else { 0 };
                }
                if iters == 0 && matches!(op, RepeatOp::OneOrMore) {
                    return Err(MacroError("重复展开 `$(` 无捕获但要求至少一次".into()));
                }
                for i in 0..iters {
                    if i > 0 {
                        if let Some(sep_t) = sep {
                            out.push(sep_t.clone());
                        }
                    }
                    let sub = project_bindings(inner, bindings, i);
                    expand_inner(inner, &sub, out)?;
                }
            }
        }
    }
    Ok(())
}

/// 收集 transcriber 片段中出现的元变量名。
fn collect_meta_names<F: FnMut(&str)>(tokens: &[TransToken], f: &mut F) {
    for t in tokens {
        match t {
            TransToken::Meta { name } => f(name),
            TransToken::Delim { inner, .. } | TransToken::Repeat { inner, .. } => {
                collect_meta_names(inner, f)
            }
            TransToken::Lit(_) => {}
        }
    }
}

/// 为第 `i` 次重复迭代投影绑定：重复片段内元变量取第 i 次捕获，其余继承。
fn project_bindings(inner: &[TransToken], bindings: &Bindings, i: usize) -> Bindings {
    let mut sub = bindings.clone();
    for name in inner_meta_names(inner) {
        if let Some(caps) = sub.get(&name) {
            if let Some(cap) = caps.get(i) {
                sub.insert(name.clone(), vec![cap.clone()]);
            }
        }
    }
    sub
}

fn inner_meta_names(tokens: &[TransToken]) -> Vec<String> {
    let mut names = Vec::new();
    collect_meta_names(tokens, &mut |n| names.push(n.to_string()));
    names
}

// ===== 顶层展开 =====

/// 展开宏调用：按规则顺序匹配，返回首个成功规则的展开 token 序列。
pub fn expand(
    macros: &HashMap<String, Vec<MacroRule>>,
    name: &str,
    input: &[Token],
) -> Result<Vec<Token>, MacroError> {
    let rules = macros
        .get(name)
        .ok_or_else(|| MacroError(format!("未定义的宏 `{name}`")))?;
    for rule in rules {
        let mut bindings = Bindings::new();
        if let Some(consumed) = match_matcher(&rule.matcher, input, &mut bindings) {
            if consumed == input.len() {
                return expand_trans(&rule.transcriber, &bindings);
            }
        }
    }
    Err(MacroError(format!("宏 `{name}` 的规则均不匹配")))
}

#[cfg(test)]
mod tests;
