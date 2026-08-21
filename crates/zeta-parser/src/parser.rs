//! 解析器核心：Token 流管理、辅助方法、顶层项分发。

use crate::error::ParseError;
use std::marker::PhantomData;
use zeta_ast::{AstItem, AstProgram};

use zeta_lexer::{Lexer, LocatedToken, Span, Token};

/// Zeta 语法分析器。
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
            _source: PhantomData,
        })
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
        match self.current() {
            Some(Token::Fn) => Ok(AstItem::FnDecl(Box::new(self.parse_fn()?))),
            Some(Token::Struct) => Ok(AstItem::StructDecl(Box::new(self.parse_struct()?))),
            Some(Token::Enum) => Ok(AstItem::EnumDecl(Box::new(self.parse_enum()?))),
            Some(Token::Trait) => Ok(AstItem::TraitDecl(Box::new(self.parse_trait()?))),
            Some(Token::Impl) => Ok(AstItem::ImplBlock(Box::new(self.parse_impl()?))),
            Some(Token::Mod) => Ok(AstItem::ModDecl(Box::new(self.parse_mod()?))),
            Some(Token::Use) => Ok(AstItem::UseDecl(Box::new(self.parse_use()?))),
            Some(Token::Const) | Some(Token::Static) => {
                Ok(AstItem::ConstDecl(Box::new(self.parse_const()?)))
            }
            Some(Token::Actor) => Ok(AstItem::ActorDecl(Box::new(self.parse_actor()?))),
            // `pub` 后按实际关键字分派：mod / const / static / struct / enum /
            // trait / impl / use / actor 消费 pub 后进入各自解析器；
            // `fn`（及 async / unsafe / extern 前缀）**不消费 pub**，交给
            // `parse_fn` 自行处理以正确记录 `is_pub`。
            Some(Token::Pub) => {
                let next = self.peek_n(1).map(|lt| lt.token.clone());
                match next {
                    Some(Token::Mod) => {
                        self.bump(); // 消费 pub
                        Ok(AstItem::ModDecl(Box::new(self.parse_mod()?)))
                    }
                    Some(Token::Const) | Some(Token::Static) => {
                        self.bump(); // 消费 pub
                        Ok(AstItem::ConstDecl(Box::new(self.parse_const()?)))
                    }
                    Some(Token::Struct) => {
                        self.bump();
                        Ok(AstItem::StructDecl(Box::new(self.parse_struct()?)))
                    }
                    Some(Token::Enum) => {
                        self.bump();
                        Ok(AstItem::EnumDecl(Box::new(self.parse_enum()?)))
                    }
                    Some(Token::Trait) => {
                        self.bump();
                        Ok(AstItem::TraitDecl(Box::new(self.parse_trait()?)))
                    }
                    Some(Token::Impl) => {
                        self.bump();
                        Ok(AstItem::ImplBlock(Box::new(self.parse_impl()?)))
                    }
                    Some(Token::Use) => {
                        self.bump();
                        Ok(AstItem::UseDecl(Box::new(self.parse_use()?)))
                    }
                    Some(Token::Actor) => {
                        self.bump();
                        Ok(AstItem::ActorDecl(Box::new(self.parse_actor()?)))
                    }
                    _ => Ok(AstItem::FnDecl(Box::new(self.parse_fn()?))),
                }
            }
            Some(Token::Async) | Some(Token::Unsafe) | Some(Token::Extern) => {
                Ok(AstItem::FnDecl(Box::new(self.parse_fn()?)))
            }
            _ => {
                let stmt = self.parse_stmt()?;
                Ok(AstItem::Statement(Box::new(stmt)))
            }
        }
    }

    /// 当前 token 是否可能开启一个顶层项
    pub(crate) fn is_item_start(&self) -> bool {
        matches!(
            self.current(),
            Some(
                Token::Fn
                    | Token::Struct
                    | Token::Enum
                    | Token::Trait
                    | Token::Impl
                    | Token::Mod
                    | Token::Use
                    | Token::Const
                    | Token::Static
                    | Token::Actor
                    | Token::Pub
                    | Token::Async
                    | Token::Unsafe
            )
        )
    }
}
