//! 类型解析。

use crate::error::ParseError;
use crate::parser::Parser;
use zeta_ast::AstType;
use zeta_lexer::Token;

impl<'src> Parser<'src> {
    /// 解析类型
    ///
    /// 支持：路径类型（含泛型参数）、引用 `&T` / `&mut T`、元组、
    /// 数组 `[T; N]`、函数类型 `fn(A) -> B`、推断类型 `_`。
    pub(crate) fn parse_type(&mut self) -> Result<AstType, ParseError> {
        match self.current().cloned() {
            Some(Token::BitAnd) => {
                self.bump();
                // 生命周期标注 `&'a T`（G4）：MVP 解析后丢弃（严格借用检查规划中）
                if matches!(self.current(), Some(Token::Lifetime(_))) {
                    self.bump();
                }
                let is_mut = self.eat(&Token::Mut);
                let inner = self.parse_type()?;
                Ok(AstType::Ref(Box::new(inner), is_mut))
            }
            // 裸指针 `*const T` / `*mut T`
            Some(Token::Star) => {
                self.bump();
                let is_mut = self.eat(&Token::Mut);
                if !is_mut {
                    self.expect(&Token::Const, "'const'")?;
                }
                let inner = self.parse_type()?;
                Ok(AstType::RawPtr(Box::new(inner), is_mut))
            }
            Some(Token::LParen) => self.parse_tuple_type(),
            Some(Token::LBracket) => self.parse_array_type(),
            Some(Token::Fn) => self.parse_fn_type(),
            // trait 对象 `dyn Trait`（H4）：`dyn` 后跟 trait 路径名
            Some(Token::Dyn) => {
                self.bump();
                let name = self.expect_ident()?;
                Ok(AstType::Dyn(name))
            }
            Some(Token::SelfKw) => {
                self.bump();
                Ok(AstType::Path("Self".to_string(), Vec::new()))
            }
            Some(Token::Ident(name)) => {
                self.bump();
                if name == "_" {
                    return Ok(AstType::Infer);
                }
                self.parse_path_type(name)
            }
            _ => Err(self.unexpected("type")),
        }
    }

    /// 路径类型（含 `::` 段与泛型参数，如 `geo::Point`、`Result<Response, Error>`）
    fn parse_path_type(&mut self, name: String) -> Result<AstType, ParseError> {
        // 收集 `a::b::c` 完整路径
        let mut full = name;
        while self.eat_colon_colon() {
            full.push_str("::");
            full.push_str(&self.expect_ident()?);
        }
        let name = full;
        if self.check(&Token::Lt) {
            self.bump();
            let mut args = Vec::new();
            loop {
                args.push(self.parse_type()?);
                if self.close_generics()? {
                    break;
                }
                // 返回 false 时逗号已被消费，继续解析下一个参数
            }
            Ok(AstType::Path(name, args))
        } else {
            Ok(AstType::Path(name, Vec::new()))
        }
    }

    /// 泛型参数列表关闭/续读：返回 `true` 表示已关闭当前层。
    ///
    /// - 遇 `,` 时消费并返回 `false`（继续读下一个参数）
    /// - 支持 `>>` 拆分为两层 `>`（`Vec<Vec<u32>>`）
    fn close_generics(&mut self) -> Result<bool, ParseError> {
        if self.eat(&Token::Gt) {
            return Ok(true);
        }
        if self.check(&Token::Shr) {
            // 一个 `>` 关闭当前层，另一个留到外层
            self.bump();
            self.pending_gt += 1;
            return Ok(true);
        }
        if self.pending_gt > 0 {
            self.pending_gt -= 1;
            return Ok(true);
        }
        if self.eat(&Token::Comma) {
            return Ok(false);
        }
        Err(self.unexpected("'>' or ','"))
    }

    /// 元组类型 `(A, B)`
    fn parse_tuple_type(&mut self) -> Result<AstType, ParseError> {
        self.expect(&Token::LParen, "'('")?;
        let mut elems = Vec::new();
        while !self.check(&Token::RParen) {
            if self.at_eof() {
                return Err(self.unexpected("')'"));
            }
            elems.push(self.parse_type()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen, "')'")?;
        Ok(AstType::Tuple(elems))
    }

    /// 数组类型 `[T; N]`
    fn parse_array_type(&mut self) -> Result<AstType, ParseError> {
        self.expect(&Token::LBracket, "'['")?;
        let inner = self.parse_type()?;
        let size = if self.eat(&Token::Semicolon) {
            Some(Box::new(self.parse_expr()?))
        } else {
            None
        };
        self.expect(&Token::RBracket, "']'")?;
        Ok(AstType::Array(Box::new(inner), size))
    }

    /// 函数类型 `fn(A, B) -> C`
    fn parse_fn_type(&mut self) -> Result<AstType, ParseError> {
        self.expect(&Token::Fn, "'fn'")?;
        self.expect(&Token::LParen, "'('")?;
        let mut args = Vec::new();
        while !self.check(&Token::RParen) {
            if self.at_eof() {
                return Err(self.unexpected("')'"));
            }
            args.push(self.parse_type()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen, "')'")?;
        let ret = if self.eat(&Token::Arrow) {
            Box::new(self.parse_type()?)
        } else {
            Box::new(AstType::Path("()".to_string(), Vec::new()))
        };
        Ok(AstType::Fn(args, ret))
    }
}
