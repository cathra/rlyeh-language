//! 顶层项解析：函数、结构体、枚举、trait、impl、模块、use、const。

use crate::error::ParseError;
use crate::parser::Parser;
use rlyeh_ast::{
    AstConstDecl, AstEnumDecl, AstEnumVariant, AstFnDecl, AstImplBlock, AstModDecl, AstParam,
    AstStructDecl, AstStructField, AstTraitDecl, AstType, AstTypeParam, AstUseDecl,
};
use rlyeh_lexer::Token;

impl<'src> Parser<'src> {
    /// 解析函数声明（含 pub / async / unsafe / extern 前置修饰符）
    pub(crate) fn parse_fn(&mut self) -> Result<AstFnDecl, ParseError> {
        let mut is_pub = false;
        let mut is_async = false;
        let mut is_extern = false;
        loop {
            if self.eat(&Token::Pub) {
                is_pub = true;
                continue;
            }
            if self.eat(&Token::Async) {
                is_async = true;
                continue;
            }
            if self.eat(&Token::Unsafe) {
                continue;
            }
            if self.eat(&Token::Extern) {
                is_extern = true;
                continue;
            }
            break;
        }
        let start = self.expect(&Token::Fn, "'fn'")?.span;
        let name = self.expect_ident()?;
        // `r#` 前缀为关键字转义 / 根命名空间显式引用标记；声明名归一化为无前缀名
        // （调用处保留 `r#` 前缀，typecheck resolve_callable 去前缀后绑定根命名空间）。
        let name = name.strip_prefix("r#").unwrap_or(&name).to_string();
        let generics = self.parse_generics()?;
        let params = self.parse_params()?;
        let return_type = if self.eat(&Token::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        let body = if self.check(&Token::LBrace) {
            Some(self.parse_block()?)
        } else {
            // trait 抽象方法：`fn foo(...);`
            self.expect(&Token::Semicolon, "';' or '{'")?;
            None
        };
        let span = self.span_until_current(start);
        Ok(AstFnDecl {
            name,
            generics,
            params,
            return_type,
            body,
            is_pub,
            is_async,
            is_extern,
            span,
        })
    }

    /// 泛型参数列表 `<T, U>`（跳过 bound 细节）
    fn parse_generics(&mut self) -> Result<Vec<AstTypeParam>, ParseError> {
        if !self.check(&Token::Lt) {
            return Ok(Vec::new());
        }
        self.bump();
        let mut params = Vec::new();
        loop {
            if self.at_eof() {
                return Err(self.unexpected("'>'"));
            }
            // 生命周期参数 `'a`（G4）：解析后丢弃（MVP 语法接受，borrowck 严格检查规划中）
            if matches!(self.current(), Some(Token::Lifetime(_))) {
                self.bump(); // `'a`
                // 可选 `: 'b` bound
                if self.eat(&Token::Colon) {
                    while !self.check(&Token::Comma) && !self.check(&Token::Gt) {
                        if self.at_eof() {
                            return Err(self.unexpected("'>'"));
                        }
                        self.bump();
                    }
                }
                if self.eat(&Token::Gt) {
                    break;
                }
                if !self.eat(&Token::Comma) {
                    return Err(self.unexpected("',' or '>'"));
                }
                continue;
            }
            let n = self.expect_ident()?;
            // 约束 `T: Bound1 [+ Bound2]`（U3；MVP 支持简单 trait 路径 ident）
            let mut bounds = Vec::new();
            if self.eat(&Token::Colon) {
                loop {
                    let b = self.expect_ident()?;
                    bounds.push(b);
                    if !self.eat(&Token::Plus) {
                        break;
                    }
                }
            }
            params.push(AstTypeParam { name: n, bounds });
            if self.eat(&Token::Gt) {
                break;
            }
            if !self.eat(&Token::Comma) {
                return Err(self.unexpected("',' or '>'"));
            }
        }
        Ok(params)
    }

    /// 可选 where 子句（U3）：`where K: Bound1 [+ Bound2], V: Bound3`，
    /// 约束按参数名合并到给定泛型参数列表（不存在的参数名忽略）。
    fn parse_where_clause(
        &mut self,
        generics: &mut Vec<AstTypeParam>,
    ) -> Result<(), ParseError> {
        if !self.eat(&Token::Where) {
            return Ok(());
        }
        loop {
            let name = self.expect_ident()?;
            self.expect(&Token::Colon, "':'")?;
            let mut bounds = Vec::new();
            loop {
                let b = self.expect_ident()?;
                bounds.push(b);
                if !self.eat(&Token::Plus) {
                    break;
                }
            }
            if let Some(p) = generics.iter_mut().find(|p| p.name == name) {
                p.bounds.extend(bounds);
            }
            if self.eat(&Token::Comma) {
                continue;
            }
            break;
        }
        Ok(())
    }

    /// 参数列表 `(name: Type, other: Type = default)`
    fn parse_params(&mut self) -> Result<Vec<AstParam>, ParseError> {
        self.expect(&Token::LParen, "'('")?;
        let mut params = Vec::new();
        while !self.check(&Token::RParen) {
            if self.at_eof() {
                return Err(self.unexpected("')'"));
            }
            let start = self.peek().expect("non-eof").span;
            // 裸 `self` 值接收者（`self` 为标识符，`Self` 才是关键字）
            if matches!(&self.peek().map(|t| t.token.clone()), Some(Token::Ident(n)) if n == "self")
            {
                let name = self.expect_ident()?; // "self"
                params.push(AstParam {
                    name,
                    type_: AstType::Path("Self".to_string(), Vec::new()),
                    default: None,
                    is_mut: false,
                    span: self.span_until_current(start),
                });
                if !self.eat(&Token::Comma) {
                    break;
                }
                continue;
            }
            // `&self` / `&mut self` 引用接收者（`self` 为标识符，`Self` 才是关键字）
            if self.check(&Token::BitAnd)
                && self.peek_n(1).is_some_and(|t| {
                    t.token == Token::Mut || matches!(&t.token, Token::Ident(n) if n == "self")
                })
            {
                self.bump(); // `&`
                let is_mut = self.eat(&Token::Mut);
                let self_name = self.expect_ident()?;
                debug_assert_eq!(self_name, "self");
                params.push(AstParam {
                    name: self_name,
                    type_: AstType::Ref(
                        Box::new(AstType::Path("Self".to_string(), Vec::new())),
                        is_mut,
                    ),
                    default: None,
                    is_mut: false,
                    span: self.span_until_current(start),
                });
                if !self.eat(&Token::Comma) {
                    break;
                }
                continue;
            }
            let is_mut = self.eat(&Token::Mut);
            let name = self.expect_ident()?;
            self.expect(&Token::Colon, "':'")?;
            let type_ = self.parse_type()?;
            let default = if self.eat(&Token::Assign) {
                Some(self.parse_expr()?)
            } else {
                None
            };
            params.push(AstParam {
                name,
                type_,
                default,
                is_mut,
                span: self.span_until_current(start),
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RParen, "')'")?;
        Ok(params)
    }

    /// 结构体声明
    pub(crate) fn parse_struct(&mut self) -> Result<AstStructDecl, ParseError> {
        let start = self.expect(&Token::Struct, "'struct'")?.span;
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        self.expect(&Token::LBrace, "'{'")?;
        let mut fields = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            let fstart = self.peek().expect("non-eof").span;
            let is_pub = self.eat(&Token::Pub);
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon, "':'")?;
            let type_ = self.parse_type()?;
            fields.push(AstStructField {
                name: fname,
                type_,
                is_pub,
                span: self.span_until_current(fstart),
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstStructDecl {
            name,
            generics,
            fields,
            derive: Vec::new(),
            span: self.merge_span(start, end),
        })
    }

    /// 枚举声明
    pub(crate) fn parse_enum(&mut self) -> Result<AstEnumDecl, ParseError> {
        let start = self.expect(&Token::Enum, "'enum'")?.span;
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        self.expect(&Token::LBrace, "'{'")?;
        let mut variants = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            let vstart = self.peek().expect("non-eof").span;
            let vname = self.expect_ident()?;
            let mut tuple_fields = Vec::new();
            let mut struct_fields = Vec::new();
            if self.check(&Token::LParen) {
                self.bump();
                while !self.check(&Token::RParen) {
                    if self.at_eof() {
                        return Err(self.unexpected("')'"));
                    }
                    tuple_fields.push(self.parse_type()?);
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RParen, "')'")?;
            } else if self.check(&Token::LBrace) {
                self.bump();
                while !self.check(&Token::RBrace) {
                    if self.at_eof() {
                        return Err(self.unexpected("'}'"));
                    }
                    let fname = self.expect_ident()?;
                    self.expect(&Token::Colon, "':'")?;
                    let type_ = self.parse_type()?;
                    struct_fields.push(AstStructField {
                        name: fname,
                        type_,
                        is_pub: false,
                        span: self.span_until_current(vstart),
                    });
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RBrace, "'}'")?;
            }
            variants.push(AstEnumVariant {
                name: vname,
                tuple_fields,
                struct_fields,
                span: self.span_until_current(vstart),
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstEnumDecl {
            name,
            generics,
            variants,
            span: self.merge_span(start, end),
        })
    }

    /// Trait 声明（抽象方法无函数体）
    pub(crate) fn parse_trait(&mut self) -> Result<AstTraitDecl, ParseError> {
        let start = self.expect(&Token::Trait, "'trait'")?.span;
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        self.expect(&Token::LBrace, "'{'")?;
        let mut types = Vec::new();
        let mut methods = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            if self.check(&Token::Type) {
                // 关联类型声明 `type Item;`（U2）
                self.bump();
                let tname = self.expect_ident()?;
                self.expect(&Token::Semicolon, "';'")?;
                types.push(tname);
                continue;
            }
            methods.push(self.parse_fn()?);
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstTraitDecl {
            name,
            generics,
            types,
            methods,
            span: self.merge_span(start, end),
        })
    }

    /// impl 块：`impl [Trait for] Type { ... }`
    pub(crate) fn parse_impl(&mut self) -> Result<AstImplBlock, ParseError> {
        let start = self.expect(&Token::Impl, "'impl'")?.span;
        let mut generics = self.parse_generics()?;
        let first = self.expect_ident()?;
        let (trait_name, type_name) = if self.eat(&Token::For) {
            (Some(first), self.expect_ident()?)
        } else {
            (None, first)
        };
        // 消费被实现类型的泛型参数列表（如 `impl<T> Option<T>` 的 `<T>`）。
        // MVP：仅校验参数为标识符列表并丢弃；self 类型由 typecheck 依据 impl
        // generics 重建（`Named(type_name, generics)`），故无需保留此处实参。
        if self.check(&Token::Lt) {
            self.bump();
            while !self.check(&Token::Gt) {
                if self.at_eof() {
                    return Err(self.unexpected("'>'"));
                }
                self.expect_ident()?;
                if !self.eat(&Token::Comma) {
                    break;
                }
            }
            self.expect(&Token::Gt, "'>'")?;
        }
        // where 子句（U3）：`impl<K, V> Trait for Type where K: Hash + Eq { ... }`
        self.parse_where_clause(&mut generics)?;
        self.expect(&Token::LBrace, "'{'")?;
        let mut types = Vec::new();
        let mut methods = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            if self.check(&Token::Type) {
                // 关联类型定义 `type Item = Concrete;`（U2）
                self.bump();
                let tname = self.expect_ident()?;
                self.expect(&Token::Assign, "'='")?;
                let ty = self.parse_type()?;
                self.expect(&Token::Semicolon, "';'")?;
                types.push((tname, ty));
                continue;
            }
            methods.push(self.parse_fn()?);
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstImplBlock {
            trait_name,
            type_name,
            generics,
            types,
            methods,
            span: self.merge_span(start, end),
        })
    }

    /// 模块声明：`mod name { ... }` 或 `mod name;`
    pub(crate) fn parse_mod(&mut self) -> Result<AstModDecl, ParseError> {
        let start = self.expect(&Token::Mod, "'mod'")?.span;
        let name = self.expect_ident()?;
        let (items, external) = if self.check(&Token::LBrace) {
            self.bump();
            let mut items = Vec::new();
            while !self.check(&Token::RBrace) {
                if self.at_eof() {
                    return Err(self.unexpected("'}'"));
                }
                items.push(self.parse_item()?);
            }
            self.expect(&Token::RBrace, "'}'")?;
            (items, false)
        } else {
            self.expect(&Token::Semicolon, "';' or '{'")?;
            (Vec::new(), true)
        };
        let end = self.span_until_current(start);
        Ok(AstModDecl {
            name,
            items,
            external,
            span: self.merge_span(start, end),
        })
    }

    /// use 导入：`use path::to::item [as alias];`（末段允许 `*` glob）
    pub(crate) fn parse_use(&mut self) -> Result<AstUseDecl, ParseError> {
        let start = self.expect(&Token::Use, "'use'")?.span;
        let mut path = Vec::new();
        loop {
            // 路径末段允许 `*`（glob 导入，由 typecheck 报不支持）
            let seg = if self.check(&Token::Star) {
                self.bump();
                "*".to_string()
            } else {
                self.expect_ident()?
            };
            path.push(seg);
            if self.eat_colon_colon() {
                continue;
            }
            break;
        }
        let alias = if self.eat(&Token::As) {
            Some(self.expect_ident()?)
        } else {
            None
        };
        self.expect(&Token::Semicolon, "';'")?;
        let end = self.span_until_current(start);
        Ok(AstUseDecl {
            path,
            alias,
            span: self.merge_span(start, end),
        })
    }

    /// const / static 声明
    pub(crate) fn parse_const(&mut self) -> Result<AstConstDecl, ParseError> {
        let (is_static, start) = if self.check(&Token::Static) {
            (true, self.bump().expect("checked token exists").span)
        } else {
            (false, self.expect(&Token::Const, "'const'")?.span)
        };
        let name = self.expect_ident()?;
        let type_ = if self.eat(&Token::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(&Token::Assign, "'='")?;
        let value = self.parse_expr()?;
        self.expect(&Token::Semicolon, "';'")?;
        let span = self.merge_span(start, value.span);
        Ok(AstConstDecl {
            name,
            type_,
            value,
            is_static,
            span,
        })
    }
}
