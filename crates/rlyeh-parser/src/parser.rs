//! 解析器核心：Token 流管理、辅助方法、顶层项分发。

use crate::error::ParseError;
use std::collections::HashMap;
use std::marker::PhantomData;
use rlyeh_ast::{AstItem, AstMacroDecl, AstProgram};
use rlyeh_macro::{parse_matcher, parse_transcriber, MacroRule};

use rlyeh_lexer::{Lexer, LocatedToken, Span, Token};

/// 宏展开递归深度上限（防无限递归展开）
pub(crate) const MAX_MACRO_DEPTH: usize = 64;

/// Rlyeh 语法分析器。
///
/// 内部持有 Token 流与读取位置，各语法模块（表达式、语句、项、
/// 模式、类型、区域、Actor）通过 `impl Parser` 扩展其方法。
#[derive(Debug)]
pub struct Parser<'src> {
    /// Token 流（不含末尾 `Eof`）
    pub(crate) tokens: Vec<LocatedToken>,
    /// 当前读取位置
    pub(crate) pos: usize,
    /// 待补位的 `>` 数量（处理 `>>` 关闭嵌套泛型）
    pub(crate) pending_gt: usize,
    /// 最后被消费 token 的起始行号（用于判断块类表达式 `}` 与后续中缀运算符是否跨行）
    pub(crate) last_line: usize,
    /// 声明式宏注册表（`macro_rules!`，宏名 → 规则列表）
    pub(crate) macros: HashMap<String, Vec<MacroRule>>,
    /// 宏展开递归深度（防无限展开）
    pub(crate) macro_depth: usize,
    /// 集合宏（`vec!`/`map!`/`arr!`）desugar 临时变量序号
    pub(crate) collection_temp_seq: usize,
    /// 生命周期占位：保留泛型参数以兼容宏体切片等未来扩展
    _source: PhantomData<&'src str>,
}

impl<'src> Parser<'src> {
    /// 从源码构造分析器（内部先执行词法分析）
    pub fn new(source: &'src str) -> Result<Self, ParseError> {
        let tokens = Lexer::new(source).tokenize()?;
        Ok(Self {
            tokens,
            pos: 0,
            pending_gt: 0,
            last_line: 0,
            macros: HashMap::new(),
            macro_depth: 0,
            collection_temp_seq: 0,
            _source: PhantomData,
        })
    }

    /// 从展开后的 token 序列构造子分析器（宏展开产物递归解析用）。
    ///
    /// 继承宏注册表（嵌套宏调用可在展开产物中继续展开）与展开深度计数。
    pub(crate) fn from_tokens(
        tokens: Vec<LocatedToken>,
        macros: HashMap<String, Vec<MacroRule>>,
        macro_depth: usize,
    ) -> Self {
        Self {
            tokens,
            pos: 0,
            pending_gt: 0,
            last_line: 0,
            macros,
            macro_depth,
            collection_temp_seq: 0,
            _source: PhantomData,
        }
    }

    /// 解析完整程序
    pub fn parse_program(&mut self) -> Result<AstProgram, ParseError> {
        let mut items = Vec::new();
        while !self.at_eof() {
            items.push(self.parse_item()?);
        }
        Ok(AstProgram { items })
    }

    /// 当前是否已到文件末尾
    pub(crate) fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// 当前 token（不消费）
    pub(crate) fn peek(&self) -> Option<&LocatedToken> {
        self.tokens.get(self.pos)
    }

    /// 向前看 n 个 token（n=1 为下一个）
    pub(crate) fn peek_n(&self, n: usize) -> Option<&LocatedToken> {
        self.tokens.get(self.pos + n)
    }

    /// 当前 Token 种类（不消费）
    pub(crate) fn current(&self) -> Option<&Token> {
        self.peek().map(|lt| &lt.token)
    }

    /// 消费并返回当前 token
    pub(crate) fn bump(&mut self) -> Option<LocatedToken> {
        let tok = self.tokens.get(self.pos).cloned();
        if let Some(t) = &tok {
            self.pos += 1;
            self.last_line = t.span.line;
        }
        tok
    }

    /// 检查当前 token 是否为指定类型
    pub(crate) fn check(&self, tok: &Token) -> bool {
        self.current().is_some_and(|t| t == tok)
    }

    /// 若当前 token 匹配则消费并返回 `true`
    pub(crate) fn eat(&mut self, tok: &Token) -> bool {
        if self.check(tok) {
            self.bump();
            true
        } else {
            false
        }
    }

    /// 期望当前 token 为指定类型并消费，否则报错
    pub(crate) fn expect(
        &mut self,
        tok: &Token,
        expected: &str,
    ) -> Result<LocatedToken, ParseError> {
        if self.check(tok) {
            Ok(self.bump().expect("checked token exists"))
        } else {
            Err(self.unexpected(expected))
        }
    }

    /// 期望当前为标识符并返回其名称
    pub(crate) fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.current().cloned() {
            Some(Token::Ident(name)) => {
                self.bump();
                Ok(name)
            }
            _ => Err(self.unexpected("identifier")),
        }
    }

    /// 期望当前为生命周期/区域标签（如 `'r`）并返回区域名（不含引号）
    pub(crate) fn expect_lifetime(&mut self) -> Result<String, ParseError> {
        match self.current().cloned() {
            Some(Token::Lifetime(label)) => {
                self.bump();
                Ok(label.trim_start_matches('\'').to_string())
            }
            _ => Err(self.unexpected("region label (e.g. 'r)")),
        }
    }

    /// 构造"期望 X，实际遇到 Y"的错误
    pub(crate) fn unexpected(&self, expected: &str) -> ParseError {
        match self.peek() {
            Some(lt) => ParseError::UnexpectedToken {
                expected: expected.to_string(),
                found: format!("{:?}", lt.token),
                line: lt.span.line,
                col: lt.span.col,
            },
            None => ParseError::UnexpectedEof,
        }
    }

    /// 合并两个 span（取起点与终点）
    pub(crate) fn merge_span(&self, start: Span, end: Span) -> Span {
        Span {
            start: start.start,
            end: end.end,
            line: start.line,
            col: start.col,
        }
    }

    /// 解析一个顶层项（或语句）
    pub(crate) fn parse_item(&mut self) -> Result<AstItem, ParseError> {
        // `#[derive(Serialize, Deserialize)]` attribute（阶段 Q1b）：MVP 仅支持
        // struct 声明前的 derive 标记；其它项宽松忽略（typecheck 不感知 derive）。
        let (derive, repr_c, memory) = self.parse_attributes()?;
        match self.current() {
            Some(Token::Fn) => Ok(AstItem::FnDecl(Box::new(self.parse_fn()?))),
            Some(Token::Struct) => {
                let mut s = self.parse_struct()?;
                s.derive = derive;
                s.repr_c = repr_c;
                Ok(AstItem::StructDecl(Box::new(s)))
            }
            Some(Token::Enum) => Ok(AstItem::EnumDecl(Box::new(self.parse_enum()?))),
            Some(Token::Protocol) => Ok(AstItem::ProtocolDecl(Box::new(self.parse_protocol()?))),
            Some(Token::Impl) => Ok(AstItem::ImplBlock(Box::new(self.parse_impl()?))),
            Some(Token::Mod) => Ok(AstItem::ModDecl(Box::new(self.parse_mod(memory.clone(), false)?))),
            Some(Token::Use) => Ok(AstItem::UseDecl(Box::new(self.parse_use(false)?))),
            Some(Token::Const) | Some(Token::Static) => {
                Ok(AstItem::ConstDecl(Box::new(self.parse_const()?)))
            }
            Some(Token::Actor) => Ok(AstItem::ActorDecl(Box::new(self.parse_actor()?))),
            // 顶层类型别名声明 `type Name = Type;`
            Some(Token::Type) => Ok(AstItem::TypeAlias(Box::new(self.parse_type_alias()?))),
            // `pub` 后按实际关键字分派：mod / const / static / struct / enum /
            // protocol / impl / use / actor 消费 pub 后进入各自解析器；
            // `fn`（及 async / unsafe / extern 前缀）**不消费 pub**，交给
            // `parse_fn` 自行处理以正确记录 `is_pub`。
            Some(Token::Pub) => {
                let next = self.peek_n(1).map(|lt| lt.token.clone());
                match next {
                    Some(Token::Mod) => {
                        self.bump(); // 消费 pub
                        Ok(AstItem::ModDecl(Box::new(self.parse_mod(memory.clone(), true)?)))
                    }
                    Some(Token::Const) | Some(Token::Static) => {
                        self.bump(); // 消费 pub
                        Ok(AstItem::ConstDecl(Box::new(self.parse_const()?)))
                    }
                    Some(Token::Struct) => {
                        self.bump();
                        let mut s = self.parse_struct()?;
                        s.derive = derive;
                        s.is_pub = true;
                        Ok(AstItem::StructDecl(Box::new(s)))
                    }
                    Some(Token::Enum) => {
                        self.bump();
                        let mut e = self.parse_enum()?;
                        e.is_pub = true;
                        Ok(AstItem::EnumDecl(Box::new(e)))
                    }
                    Some(Token::Protocol) => {
                        self.bump();
                        let mut t = self.parse_protocol()?;
                        t.is_pub = true;
                        Ok(AstItem::ProtocolDecl(Box::new(t)))
                    }
                    Some(Token::Impl) => {
                        self.bump();
                        Ok(AstItem::ImplBlock(Box::new(self.parse_impl()?)))
                    }
                    Some(Token::Use) => {
                        self.bump();
                        Ok(AstItem::UseDecl(Box::new(self.parse_use(true)?)))
                    }
                    Some(Token::Actor) => {
                        self.bump();
                        let mut a = self.parse_actor()?;
                        a.is_pub = true;
                        Ok(AstItem::ActorDecl(Box::new(a)))
                    }
                    Some(Token::Type) => {
                        self.bump();
                        let mut ta = self.parse_type_alias()?;
                        ta.is_pub = true;
                        Ok(AstItem::TypeAlias(Box::new(ta)))
                    }
                    _ => Ok(AstItem::FnDecl(Box::new(self.parse_fn()?))),
                }
            }
            Some(Token::Async) | Some(Token::Unsafe) | Some(Token::Extern) => {
                Ok(AstItem::FnDecl(Box::new(self.parse_fn()?)))
            }
            // `macro_rules! name { ... }` 声明式宏（I1）：定义注册到
            // `self.macros`，产出占位 AstItem（typecheck 忽略，展开发生在 parse 阶段）
            // （`macro_rules` 为单个标识符，lexer 不拆分为 `macro` + `rules`）
            Some(Token::Ident(name)) if name == "macro_rules" => {
                Ok(AstItem::MacroDecl(Box::new(self.parse_macro_rules()?)))
            }
            _ => {
                let stmt = self.parse_stmt()?;
                Ok(AstItem::Statement(Box::new(stmt)))
            }
        }
    }

    /// 解析 `#[derive(Serialize, Deserialize)]` 与 `#[repr(C)]` attribute（阶段 Q1b / SH-P0-1 E2）。
    ///
    /// MVP 仅支持 struct 声明前的 `derive` 标记（`#[derive(..)]`，可多个、可空
    /// `#[derive]`）与 `#[repr(C)]`；其它 attribute 名报错。返回
    /// `(derive protocol 名列表, 是否 repr(C))`。
    fn parse_attributes(&mut self) -> Result<(Vec<String>, bool, Option<String>), ParseError> {
        let mut derives = Vec::new();
        let mut repr_c = false;
        let mut memory = None;
        while self.eat(&Token::Pound) {
            self.expect(&Token::LBracket, "'['")?;
            let attr_name = self.expect_ident()?;
            match attr_name.as_str() {
                "derive" => {
                    if self.eat(&Token::LParen) {
                        if !self.check(&Token::RParen) {
                            loop {
                                derives.push(self.expect_ident()?);
                                if !self.eat(&Token::Comma) {
                                    break;
                                }
                            }
                        }
                        self.expect(&Token::RParen, "')'")?;
                    }
                }
                "repr" => {
                    self.expect(&Token::LParen, "'('")?;
                    let repr_arg = self.expect_ident()?;
                    if repr_arg != "C" {
                        return Err(self.unexpected("'#[repr(C)]'"));
                    }
                    repr_c = true;
                    self.expect(&Token::RParen, "')'")?;
                }
                "memory" => {
                    self.expect(&Token::LParen, "'('")?;
                    let mem_arg = self.expect_ident()?;
                    if mem_arg != "gc" {
                        return Err(self.unexpected("'#[memory(gc)]'"));
                    }
                    memory = Some("gc".to_string());
                    self.expect(&Token::RParen, "')'")?;
                }
                _ => {
                    return Err(self.unexpected(
                        "'#[derive(..)]' / '#[repr(C)]' / '#[memory(gc)]'",
                    ))
                }
            }
            self.expect(&Token::RBracket, "']'")?;
        }
        Ok((derives, repr_c, memory))
    }

    /// 解析 `macro_rules! name { (matcher) => { transcriber }; ... }`（MVP）。
    ///
    /// matcher / transcriber 为定界组（内容 token 剥离外层定界符后交给
    /// `rlyeh-macro` 解析）；transcriber 也可为裸 token 序列（到顶层 `;`）。
    fn parse_macro_rules(&mut self) -> Result<AstMacroDecl, ParseError> {
        let start = self
            .peek()
            .map(|lt| lt.span)
            .unwrap_or(Span { start: 0, end: 0, line: 1, col: 1 });
        self.bump(); // macro_rules
        self.expect(&Token::NotNot, "'!'")?;
        let name = self.expect_ident()?;
        self.expect(&Token::LBrace, "'{'")?;
        let mut rules = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}' (macro_rules 未闭合)"));
            }
            let matcher_span = self
                .peek()
                .map(|lt| lt.span)
                .unwrap_or(Span { start: 0, end: 0, line: 1, col: 1 });
            // matcher `( tokens )`：`collect_group_content` 会自行消费开定界符，
            // 这里不能预先 expect(LParen)，否则会双重消费吞掉首个 `$` token
            let matcher_tokens = self.collect_group_content(&Token::RParen)?;
            let matcher = parse_matcher(&matcher_tokens).map_err(|e| ParseError::Macro {
                msg: format!("{}：{}", name, e.0),
                line: matcher_span.line,
                col: matcher_span.col,
            })?;
            self.expect(&Token::FatArrow, "'=>'")?;
            // transcriber：定界组整体 或 裸 token 序列（到顶层 `;`）
            let trans_tokens = if self.check(&Token::LBrace)
                || self.check(&Token::LBracket)
                || self.check(&Token::LParen)
            {
                let open = self.current().cloned().expect("checked");
                let close = match open {
                    Token::LBrace => Token::RBrace,
                    Token::LBracket => Token::RBracket,
                    _ => Token::RParen,
                };
                self.collect_group_content(&close)?
            } else {
                // 裸 token 序列：收集到顶层 `;`
                let mut out = Vec::new();
                while !self.at_eof() && !self.check(&Token::Semicolon) {
                    out.push(self.bump().expect("checked").token);
                }
                out
            };
            let transcriber =
                parse_transcriber(&trans_tokens).map_err(|e| ParseError::Macro {
                    msg: format!("{}：{}", name, e.0),
                    line: matcher_span.line,
                    col: matcher_span.col,
                })?;
            rules.push(MacroRule {
                matcher,
                transcriber,
            });
            self.eat(&Token::Semicolon);
        }
        self.expect(&Token::RBrace, "'}'")?;
        self.macros.insert(name.clone(), rules);
        let end = self.peek().map(|lt| lt.span).unwrap_or(start);
        Ok(AstMacroDecl {
            name,
            params: Vec::new(),
            body: String::new(),
            span: self.merge_span(start, end),
        })
    }

    /// 收集当前开定界符组的内容 token（深度计数），并消费到配对的 `close`。
    ///
    /// 当前 token 须为 `close` 对应的开定界符；返回内容 token（不含定界符本身）。
    pub(crate) fn collect_group_content(
        &mut self,
        close: &Token,
    ) -> Result<Vec<Token>, ParseError> {
        let span = self
            .peek()
            .map(|lt| lt.span)
            .unwrap_or(Span { start: 0, end: 0, line: 1, col: 1 });
        self.bump(); // 开定界符
        let mut depth = 1usize;
        let mut out = Vec::new();
        while !self.at_eof() {
            let t = self.current().cloned().expect("checked");
            match t {
                // 所有开定界符加深
                Token::LParen | Token::LBracket | Token::LBrace => {
                    depth += 1;
                    out.push(t);
                    self.bump();
                }
                // 闭合符：目标 close 归零时结束（其余闭合符仅减深）
                Token::RParen | Token::RBracket | Token::RBrace => {
                    depth -= 1;
                    if &t == close && depth == 0 {
                        self.bump(); // 消费 close
                        return Ok(out);
                    }
                    out.push(t);
                    self.bump();
                }
                _ => {
                    out.push(t);
                    self.bump();
                }
            }
        }
        Err(ParseError::Macro {
            msg: "定界组未闭合".into(),
            line: span.line,
            col: span.col,
        })
    }

    /// 当前 token 是否可能开启一个顶层项
    pub(crate) fn is_item_start(&self) -> bool {
        let is_start = matches!(
            self.current(),
            Some(
                Token::Fn
                    | Token::Struct
                    | Token::Enum
                    | Token::Protocol
                    | Token::Impl
                    | Token::Mod
                    | Token::Use
                    | Token::Const
                    | Token::Static
                    | Token::Actor
                    | Token::Pub
                    | Token::Async
                    | Token::Unsafe
                    | Token::Pound // `#[derive(...)]` attribute 后接 item（Q1b）
            )
        );
        if !is_start {
            return false;
        }
        // `unsafe { ... }` 是表达式语句（裸指针作用域），仅 `unsafe fn/impl/...` 才是项；
        // 故 `unsafe` 紧跟 `{` 时不作为项起始，交由表达式解析。
        if matches!(self.current(), Some(Token::Unsafe))
            && self.peek_n(1).map_or(false, |lt| matches!(lt.token, Token::LBrace))
        {
            return false;
        }
        true
    }
}
