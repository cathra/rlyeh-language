//! 顶层项解析：函数、结构体、枚举、protocol、impl、模块、use、const。

use crate::error::ParseError;
use crate::parser::Parser;
use rlyeh_ast::{
    AstConstDecl, AstEnumDecl, AstEnumVariant, AstFnDecl, AstImplBlock, AstModDecl, AstParam,
    AstStructDecl, AstStructField, AstProtocolDecl, AstType, AstTypeParam, AstUseDecl, AstUseMember,
};
use rlyeh_lexer::Token;
use rlyeh_ast::AstTypeAlias;

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
        let mut generics = self.parse_generics()?;
        let params = self.parse_params()?;
        let return_type = if self.eat(&Token::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };
        // A4（SH-P1-1，2026-09-02）：函数 / 方法级 `where` 子句——
        // `fn f<T>(x: T) -> i64 where T: Speak { .. }`。此前仅 impl 块支持
        // `where`（`parse_impl`），函数声明遇到 `where` 会直接报语法错误。
        // 约束按参数名合并进 `generics`（不存在名忽略），复用既有 bound 校验路径。
        self.parse_where_clause(&mut generics)?;
        let body = if self.check(&Token::LBrace) {
            Some(self.parse_block()?)
        } else {
            // protocol 抽象方法：`fn foo(...);`
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
            // 约束 `T: Bound1 [+ Bound2]`（U3；支持模块路径 protocol 名，如 `T: m::Protocol`）
            let mut bounds = Vec::new();
            if self.eat(&Token::Colon) {
                loop {
                    let b = self.parse_qualified_name()?;
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

    /// P6b（2026-08-29）：where bound 类型 → 约束名字符串。
    /// 路径类型取路径名（泛型实参不参与约束校验，P6c 待专项）；其余递归取内层名。
    fn ast_type_bound_name(ty: &rlyeh_ast::AstType) -> String {
        use rlyeh_ast::AstType;
        match ty {
            AstType::Path(n, _) => n.clone(),
            AstType::Dyn(n) => n.clone(),
            AstType::Ref(inner, _, _) | AstType::RawPtr(inner, _) | AstType::Array(inner, _) => {
                Self::ast_type_bound_name(inner)
            }
            AstType::Tuple(ts) => ts
                .first()
                .map(Self::ast_type_bound_name)
                .unwrap_or_else(|| "Tuple".to_string()),
            AstType::Fn(_, ret) => Self::ast_type_bound_name(ret),
            // U1：联合作为 bound 时取首个成员名（MVP：联合 bound 不参与约束校验）
            AstType::Union(ts) => ts
                .first()
                .map(Self::ast_type_bound_name)
                .unwrap_or_else(|| "Union".to_string()),
            AstType::Infer => "_".to_string(),
        }
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
                // P6b（2026-08-29）：bound 走完整类型解析——支持 `::` 路径
                // （`T: io::some::Protocol`）与带泛型实参的 protocol（`U: From<T>`）。
                // MVP：bound 记录 protocol 路径名（泛型实参不参与约束校验，P6c 待专项）。
                let ty = self.parse_type()?;
                let b = match &ty {
                    AstType::Path(n, _) => n.clone(),
                    AstType::Dyn(n) => n.clone(),
                    other => Self::ast_type_bound_name(other),
                };
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
                        is_mut, None),
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

    /// 解析可能含模块路径限定的名称（`A::B::C`）。词法上 `::` 为连续两个 `Colon`。
    /// 使跨模块协议引用（如 `impl T: m::P`、`struct C: m::Protocol`、`fn f<T: m::Protocol>()`）
    /// 可正确解析（此前仅接受单标识符，模块路径语法无法表达，见 import-improvement-plan
    /// 已知限制 #2）。
    fn parse_qualified_name(&mut self) -> Result<String, ParseError> {
        let mut name = self.expect_ident()?;
        // 连续两个 `Colon` 即路径分隔符 `::`
        while self.check(&Token::Colon)
            && self.peek_n(1).is_some_and(|t| t.token == Token::Colon)
        {
            self.bump(); // 第一个 Colon
            self.bump(); // 第二个 Colon
            name.push_str("::");
            name.push_str(&self.expect_ident()?);
        }
        Ok(name)
    }

    /// 声明点一致性列表（PC-1）：`QualifiedName GenArgs? (',' QualifiedName GenArgs?)*`
    /// （前导 `:` 由调用方消费）。`QualifiedName` 支持模块路径（如 `m::P`）。
    fn parse_conformance_list(&mut self) -> Result<Vec<(String, Vec<AstType>)>, ParseError> {
        let mut list = Vec::new();
        loop {
            let name = self.parse_qualified_name()?;
            let mut args = Vec::new();
            if self.check(&Token::Lt) {
                self.bump();
                while !self.check(&Token::Gt) {
                    if self.at_eof() {
                        return Err(self.unexpected("'>'"));
                    }
                    args.push(self.parse_type()?);
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::Gt, "'>'")?;
            }
            list.push((name, args));
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        Ok(list)
    }

    /// 结构体声明
    pub(crate) fn parse_struct(&mut self) -> Result<AstStructDecl, ParseError> {
        let start = self.expect(&Token::Struct, "'struct'")?.span;
        let name = self.expect_ident()?;
        // B-4：可选 region 参数后缀 `struct Foo 'a { ... }`（region 参数化语法）。
        // 当前仅捕获存储；生命周期仍按既有 drop 语义处理，region 感知校验留待严格借用检查专项。
        let region_param = if matches!(self.current(), Some(Token::Lifetime(_))) {
            Some(self.expect_lifetime()?)
        } else {
            None
        };
        let generics = self.parse_generics()?;
        // PC-1：声明点一致性 `struct C: P, Q { .. }`。
        let conformances = if self.eat(&Token::Colon) {
            self.parse_conformance_list()?
        } else {
            Vec::new()
        };
        self.expect(&Token::LBrace, "'{'")?;
        let mut fields = Vec::new();
        let mut methods = Vec::new();
        let mut assoc_types = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            // PC-1：类型体内联方法 / 关联类型（Swift 风格）
            if self.check(&Token::Fn) {
                methods.push(self.parse_fn()?);
                continue;
            }
            if self.check(&Token::Type) {
                self.bump();
                let tname = self.expect_ident()?;
                self.expect(&Token::Assign, "'='")?;
                let ty = self.parse_type()?;
                self.expect(&Token::Semicolon, "';'")?;
                assoc_types.push((tname, ty));
                continue;
            }
            let fstart = self.peek().expect("non-eof").span;
            // EH-5：字段级属性（`#[from]` / `#[source]` / `#[error("..")]`），可空。
            let fattrs = self.parse_member_attrs()?;
            let is_pub = self.eat(&Token::Pub);
            let fname = self.expect_ident()?;
            self.expect(&Token::Colon, "':'")?;
            let type_ = self.parse_type()?;
            fields.push(AstStructField {
                name: fname,
                type_,
                is_pub,
                attrs: fattrs,
                span: self.span_until_current(fstart),
            });
            // 字段间以 `,` 分隔；字段后允许直接接 `fn` / `type` / `}`（混排）
            if !self.eat(&Token::Comma)
                && !self.check(&Token::RBrace)
                && !self.check(&Token::Fn)
                && !self.check(&Token::Type)
            {
                return Err(self.unexpected("',' / '}' / 'fn' / 'type'"));
            }
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstStructDecl {
            name,
            is_pub: false,
            region_param,
            generics,
            conformances,
            fields,
            methods,
            assoc_types,
            derive: Vec::new(),
            // EH-5：项级属性由 `parse_item` 回填（同 `derive` / `repr_c`）。
            attrs: Vec::new(),
            repr_c: false,
            span: self.merge_span(start, end),
        })
    }

    /// 类型别名声明（`type Name<T> = Type;`，`is_pub` 由调用方（pub 前缀分支）设置）
    pub(crate) fn parse_type_alias(&mut self) -> Result<AstTypeAlias, ParseError> {
        let start = self.expect(&Token::Type, "'type'")?.span;
        let name = self.expect_ident()?;
        let generics = self.parse_generics()?;
        self.expect(&Token::Assign, "'='")?;
        let target = self.parse_type()?;
        let end = self.expect(&Token::Semicolon, "';'")?.span;
        Ok(AstTypeAlias {
            name,
            is_pub: false,
            generics,
            target,
            span: self.merge_span(start, end),
        })
    }

    /// 枚举声明
    pub(crate) fn parse_enum(&mut self) -> Result<AstEnumDecl, ParseError> {
        let start = self.expect(&Token::Enum, "'enum'")?.span;
        let name = self.expect_ident()?;
        // B-4：可选 region 参数后缀 `enum E 'a { ... }`。
        let region_param = if matches!(self.current(), Some(Token::Lifetime(_))) {
            Some(self.expect_lifetime()?)
        } else {
            None
        };
        let generics = self.parse_generics()?;
        // PC-1：声明点一致性 `enum E: P { .. }`。
        let conformances = if self.eat(&Token::Colon) {
            self.parse_conformance_list()?
        } else {
            Vec::new()
        };
        self.expect(&Token::LBrace, "'{'")?;
        let mut variants = Vec::new();
        let mut methods = Vec::new();
        let mut assoc_types = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            // PC-1：类型体内联方法 / 关联类型
            if self.check(&Token::Fn) {
                methods.push(self.parse_fn()?);
                continue;
            }
            if self.check(&Token::Type) {
                self.bump();
                let tname = self.expect_ident()?;
                self.expect(&Token::Assign, "'='")?;
                let ty = self.parse_type()?;
                self.expect(&Token::Semicolon, "';'")?;
                assoc_types.push((tname, ty));
                continue;
            }
            let vstart = self.peek().expect("non-eof").span;
            // EH-5：变体级属性（`#[error("..")]` / `#[from]`），可空。
            let vattrs = self.parse_member_attrs()?;
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
                        attrs: Vec::new(),
                        span: self.span_until_current(vstart),
                    });
                    if !self.eat(&Token::Comma) {
                        break;
                    }
                }
                self.expect(&Token::RBrace, "'}'")?;
            }
            // U3：显式判别式 `Variant = 42`（受限标量枚举）。未标注时按声明序号，
            // 判别值由收集阶段写入 `VariantDef::tag`（构造 / match 均复用该值）。
            let discriminant = if self.eat(&Token::Assign) {
                match self.current().cloned() {
                    Some(Token::IntLiteral(v)) => {
                        self.bump();
                        Some(v as i64)
                    }
                    _ => return Err(self.unexpected("判别式整数字面量")),
                }
            } else {
                None
            };
            variants.push(AstEnumVariant {
                name: vname,
                tuple_fields,
                struct_fields,
                attrs: vattrs,
                discriminant,
                span: self.span_until_current(vstart),
            });
            if !self.eat(&Token::Comma)
                && !self.check(&Token::RBrace)
                && !self.check(&Token::Fn)
                && !self.check(&Token::Type)
            {
                return Err(self.unexpected("',' / '}' / 'fn' / 'type'"));
            }
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstEnumDecl {
            name,
            is_pub: false,
            region_param,
            generics,
            conformances,
            variants,
            // EH-5：项级 derive / attrs 由 `parse_item` 的 `parse_attributes` 结果回填
            //（与 struct 的 `derive` 同构）。
            derive: Vec::new(),
            attrs: Vec::new(),
            methods,
            assoc_types,
            span: self.merge_span(start, end),
        })
    }

    /// Protocol 声明（抽象方法无函数体）
    pub(crate) fn parse_protocol(&mut self) -> Result<AstProtocolDecl, ParseError> {
        // 协议声明关键字为 `protocol`（`protocol` 已从语法中彻底移除）。
        let start = self.expect(&Token::Protocol, "'protocol'")?.span;
        let name = self.expect_ident()?;
        // B-4：可选 region 参数后缀 `protocol T 'a { ... }`。
        let region_param = if matches!(self.current(), Some(Token::Lifetime(_))) {
            Some(self.expect_lifetime()?)
        } else {
            None
        };
        let generics = self.parse_generics()?;
        // PC-4：父协议（superprotocol）列表 `protocol A: B, C { .. }`。
        let superprotocols = if self.eat(&Token::Colon) {
            self.parse_conformance_list()?
        } else {
            Vec::new()
        };
        self.expect(&Token::LBrace, "'{'")?;
        let mut types = Vec::new();
        let mut methods = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            if self.check(&Token::Type) {
                // 关联类型声明 `type Item;` 或 `type Item = Concrete;`（U2 / V3-A1）
                self.bump();
                let tname = self.expect_ident()?;
                // V3-A1：支持可选默认具体化 `type Item = Concrete;`——默认类型
                // 由 typecheck 消费（V3-A3），此处仅消费 `= <type>;` 语法、记录名字。
                if self.check(&Token::Assign) {
                    self.bump();
                    self.parse_type()?; // 丢弃默认具体化（AST 仅记录名字，见 V3-A3）
                }
                self.expect(&Token::Semicolon, "';'")?;
                types.push(tname);
                continue;
            }
            methods.push(self.parse_fn()?);
        }
        let end = self.expect(&Token::RBrace, "'}'")?.span;
        Ok(AstProtocolDecl {
            name,
            is_pub: false,
            region_param,
            generics,
            superprotocols,
            types,
            methods,
            span: self.merge_span(start, end),
        })
    }

    /// impl 块（RFC `docs/rfc/protocol-syntax.md` §3.4）：
    ///
    /// **唯一语序**：`impl [<G>] Type [<...>] (: ProtocolList)? WhereClause? { .. }`
    /// ——`impl T: P`（协议一致性，可多协议）/ `impl T`（固有实现）。
    /// 旧 Rust 语序 `impl Protocol for Type` 已移除（PC-12）。
    pub(crate) fn parse_impl(&mut self) -> Result<AstImplBlock, ParseError> {
        let start = self.expect(&Token::Impl, "'impl'")?.span;
        let mut generics = self.parse_generics()?;
        // `first` 即被实现类型；随后消费其泛型实参（仅校验并丢弃——
        // self 类型由 typecheck 依据 impl `generics` 重建）。
        let first = self.expect_ident()?;
        let self_type_args = self.parse_type_generic_args()?;
        // P6c（2026-08-29）：协议泛型实参收集（如 `impl T: From<IoErrorKind>` 的
        // `IoErrorKind`），此前消费后丢弃导致协议关联方法泛型无法绑定。
        let mut protocol_type_args: Vec<AstType> = Vec::new();
        let mut extra_protocols: Vec<(String, Vec<AstType>)> = Vec::new();
        let (protocol_name, type_name) = if self.eat(&Token::Colon) {
            // PC-9：`impl T: A, B`——协议一致性列表（可多协议，协议可带泛型实参如
            // `impl T: From<i64>`）。首个协议写入 `protocol_name`，其余记录到
            // `extra_protocols`，由 desugar 按协议成员名裁决拆分为多个 impl 块。
            let mut list = self.parse_conformance_list()?;
            let (proto, args) = list.remove(0);
            protocol_type_args = args;
            extra_protocols = list;
            (Some(proto), first)
        } else {
            (None, first)
        };
        // where 子句（U3）：`impl<T> Type: Protocol where T: Hash + Eq { ... }`
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
            protocol_name,
            type_name,
            generics,
            self_type_args,
            protocol_type_args,
            extra_protocols,
            types,
            methods,
            span: self.merge_span(start, end),
        })
    }

    /// 解析类型名后的泛型实参列表（`<T, U>`；无则返回空）。
    ///
    /// 2026-09-21：由「消费并丢弃」改为**保留**——typecheck 需要真实实参以构建
    /// 嵌套泛型 self 类型（`impl<T> Option<Option<T>>` 的 `Option<Option<T>>`）；
    /// 此前仅用 impl 泛型参数重建，内层实参丢失导致方法解析错配。
    fn parse_type_generic_args(&mut self) -> Result<Vec<AstType>, ParseError> {
        let mut args = Vec::new();
        if !self.check(&Token::Lt) {
            return Ok(args);
        }
        self.bump();
        while !self.check(&Token::Gt) && self.pending_gt == 0 {
            if self.at_eof() {
                return Err(self.unexpected("'>'"));
            }
            args.push(self.parse_type()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        // 关闭本层：`parse_type` 经 `close_generics` 把 `>>` 拆层时，本层的 `>`
        // 已作为 `pending_gt` 留存（如 `impl<T> Opt<Opt<T>>`）——须消费它，
        // 而非再 `expect(Gt)`（否则会报 `expected '>', found '{'`）。
        if self.pending_gt > 0 {
            self.pending_gt -= 1;
        } else {
            self.expect(&Token::Gt, "'>'")?;
        }
        Ok(args)
    }

    /// 模块声明：`mod name { ... }` 或 `mod name;`
    pub(crate) fn parse_mod(&mut self, memory: Option<String>, is_pub: bool) -> Result<AstModDecl, ParseError> {
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
            is_pub,
            memory,
            span: self.merge_span(start, end),
        })
    }

    /// use 导入：`use path::to::item [as alias];` / 组导入 `use a::{b, c as d};` /
    /// glob 导入 `use a::*;`（末段 `*` 由 typecheck 解析为模块全部可见符号）。
    ///
    /// `is_pub` 由 `parse_item` 的分派层在消费 `pub` 后传入（本函数不再自行消费
    /// `pub`，避免与 `pub fn` 等不消费 `pub` 的路径不一致）。
    pub(crate) fn parse_use(&mut self, is_pub: bool) -> Result<AstUseDecl, ParseError> {
        let start = self.expect(&Token::Use, "'use'")?.span;
        let mut path = Vec::new();
        loop {
            // 路径末段允许 `*`（glob 导入）
            let seg = if self.check(&Token::Star) {
                self.bump();
                "*".to_string()
            } else {
                self.expect_ident()?
            };
            path.push(seg);
            if !self.eat_colon_colon() {
                break;
            }
            // `a::{...}`：遇到 `{` 即停止前缀解析，转入组导入
            if self.check(&Token::LBrace) {
                break;
            }
        }
        // 组导入：`base::{m1, m2 as a2, m3::{x, y}, ...}`
        let group = if self.check(&Token::LBrace) {
            Some(self.parse_use_group()?)
        } else {
            None
        };
        // 简单导入的 `as` 别名（组导入的别名在成员上各自指定）
        let alias = if group.is_none() {
            if self.eat(&Token::As) {
                Some(self.expect_ident()?)
            } else {
                None
            }
        } else {
            None
        };
        self.expect(&Token::Semicolon, "';'")?;
        let end = self.span_until_current(start);
        Ok(AstUseDecl {
            path,
            alias,
            group,
            is_pub,
            span: self.merge_span(start, end),
        })
    }

    /// 解析组导入的 `{ ... }` 成员列表，支持嵌套子组 `name::{ ... }`。
    ///
    /// 每个成员为 `ident`（可选 `as alias`）或 `ident::{ 嵌套成员 }`；嵌套子组
    /// 不可 `as` 重命名（与 Rust 一致）。递归处理任意深度嵌套（如 `a::{b::{c::{x, y}}}}`）。
    fn parse_use_group(&mut self) -> Result<Vec<AstUseMember>, ParseError> {
        self.expect(&Token::LBrace, "'{'")?;
        let mut members = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            let mname = self.expect_ident()?;
            // 嵌套子组 `name::{ ... }`：吞掉 `::{` 后递归解析
            let nested = if self.eat_colon_colon() && self.check(&Token::LBrace) {
                Some(self.parse_use_group()?)
            } else {
                None
            };
            // 嵌套组不可 `as` 重命名；仅简单成员支持别名
            let malias = if nested.is_none() && self.eat(&Token::As) {
                Some(self.expect_ident()?)
            } else {
                None
            };
            members.push(AstUseMember {
                name: mname,
                alias: malias,
                nested,
            });
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RBrace, "'}'")?;
        Ok(members)
    }

    /// const / static 声明
    pub(crate) fn parse_const(&mut self) -> Result<AstConstDecl, ParseError> {
        let (is_static, start) = if self.check(&Token::Static) {
            (true, self.bump().expect("checked token exists").span)
        } else {
            (false, self.expect(&Token::Const, "'const'")?.span)
        };
        // `static mut`：可变的全局变量（需位于 `unsafe` 块内访问）。
        let is_mut = if is_static && self.eat(&Token::Mut) {
            true
        } else {
            false
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
            is_mut,
            span,
        })
    }
}
