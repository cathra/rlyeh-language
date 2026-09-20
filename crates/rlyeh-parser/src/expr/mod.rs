//! 表达式解析（Pratt 解析器）。

use crate::error::ParseError;
use crate::parser::{Parser, MAX_MACRO_DEPTH};
use rlyeh_ast::{
    AssignOp, AstBlock, AstExpr, AstPattern, AstStmt, BinaryOp, CaptureMode, CompareOp, ExprKind,
    MatchArm, UnaryOp,
};
use rlyeh_lexer::{LocatedToken, Span, Token};
use rlyeh_macro::expand as expand_macro;

/// 运算符优先级（数值越大优先级越高）。
///
/// 层次：赋值(5) < `||`(10) < `&&`(20) < `|`(30) < `^`(35) < `&`(40)
/// < 范围(45) < 比较链(60) < 移位(70) < `+ -`(80)
/// < `* / %`(90) < `as`(100) < 前缀(110) < 后缀(120) < 路径(130)。
mod prec {
    /// 赋值（右结合）
    pub const ASSIGN: u8 = 5;
    /// `||`
    pub const OR: u8 = 10;
    /// `&&`
    pub const AND: u8 = 20;
    /// `|`
    pub const BIT_OR: u8 = 30;
    /// `^`
    pub const BIT_XOR: u8 = 35;
    /// `&`
    pub const BIT_AND: u8 = 40;
    /// `..<` / `...` / `<..`
    pub const RANGE: u8 = 45;
    /// 比较运算符
    pub const COMPARE: u8 = 60;
    /// `<<` `>>`
    pub const SHIFT: u8 = 70;
    /// `+ -`
    pub const ADD: u8 = 80;
    /// `* / %`
    pub const MUL: u8 = 90;
    /// `as`
    pub const CAST: u8 = 100;
}

/// `in`/`not in` 右侧目标
pub(crate) enum InTarget {
    /// 集合 `{a, b, c}`（元素可为单值或范围，范围元素语义为离散展开）
    Set(Vec<AstExpr>),
    /// 裸范围 `a..<b` / `a...b` / `a<..b`
    Range(AstExpr),
    /// 运行时容器（`[a, b, c]` / 标识符 / `vec!` / 字符串等）：
    /// 成员判断在 typecheck 阶段运行时遍历容器逐元素比较
    Container(AstExpr),
}

impl<'src> Parser<'src> {
    /// 解析完整表达式
    pub(crate) fn parse_expr(&mut self) -> Result<AstExpr, ParseError> {
        self.parse_expr_prec(0)
    }

    /// 以最小优先级解析表达式（Pratt 主循环）
    fn parse_expr_prec(&mut self, min_prec: u8) -> Result<AstExpr, ParseError> {
        let mut lhs = self.parse_prefix()?;
        loop {
            // 后缀运算符（优先级最高之一）
            if self.check(&Token::Dot) {
                lhs = self.parse_postfix_dot(lhs)?;
                continue;
            }
            if self.check(&Token::LParen) || self.check_turbofish() {
                let start = lhs.span;
                // turbofish 泛型类型实参：`foo::<T1, T2>(args)`（L2：`json.parse::<T>(s)`）。
                // 关闭处支持 `>>` 拆分为两层 `>`（嵌套泛型：`::<HashMap<i64, Vec<i64>>>(s)`）
                let type_args = if self.check_turbofish() {
                    self.bump(); // :
                    self.bump(); // :
                    self.bump(); // <
                    let mut tys = Vec::new();
                    loop {
                        if self.at_eof() {
                            return Err(self.unexpected("'>'"));
                        }
                        tys.push(self.parse_type()?);
                        if !self.eat(&Token::Comma) {
                            break;
                        }
                    }
                    // 关闭 turbofish：优先消费 parse_type 留下的 `>>` 拆分层（pending_gt），
                    // 否则消费一个 `>`；`Shr` 自身也可拆一层留到外层
                    if self.pending_gt > 0 {
                        self.pending_gt -= 1;
                    } else if !self.eat(&Token::Gt) {
                        if self.check(&Token::Shr) {
                            self.bump();
                            self.pending_gt += 1;
                        } else {
                            return Err(self.unexpected("'>'"));
                        }
                    }
                    tys
                } else {
                    Vec::new()
                };
                // P7b（2026-08-29）：关联路径段间 turbofish——`Type::<T>::method(args)`
                // （`Vec::<i64>::new()`）：turbofish 关闭后若继续 `::`，收集后续路径段
                // 拼到 callee 路径名（此前仅支持 `foo::<T>(args)` 自由函数 turbofish，
                // `Type::<T>::method` 报 `expected '(', found Colon`）。
                while self.eat_colon_colon() {
                    let seg = self.expect_ident()?;
                    let cspan = lhs.span;
                    let new_kind = match &*lhs.kind {
                        ExprKind::Ident(name) => ExprKind::Path(vec![name.clone(), seg]),
                        ExprKind::Path(segs) => {
                            let mut new_segs = segs.clone();
                            new_segs.push(seg);
                            ExprKind::Path(new_segs)
                        }
                        _ => return Err(self.unexpected("路径段")),
                    };
                    lhs = AstExpr::new(new_kind, cspan);
                }
                let (args, end) = self.parse_call_args()?;
                let span = self.merge_span(start, end);
                lhs = AstExpr::new(ExprKind::Call { callee: lhs, args, type_args }, span);
                continue;
            }
            if self.check(&Token::LBracket) {
                let start = lhs.span;
                self.bump();
                // P8：下界省略——`v[..]` / `v[..<3]`（`[` 后直接是范围运算符，无下界表达式）。
                // 中缀 Range 要求左侧有 lhs，故省略下界的切片在此特判构造。
                // `Token::Range`（旧语法 `..`）：切片上下文接受为「全量切片 `v[..]`」
                // （符合 Rust 直觉；其他上下文的裸 `..` 仍按废弃语法报错）。
                let index = match self.peek().map(|t| t.token.clone()) {
                    Some(Token::DotDotLt)
                    | Some(Token::DotDotDot)
                    | Some(Token::LtDotDot)
                    | Some(Token::Range) => {
                        let rstart = self.peek().expect("non-eof").span;
                        let (lower_inclusive, upper_inclusive) =
                            match self.peek().map(|t| t.token.clone()) {
                                Some(Token::DotDotLt) => (true, false),
                                Some(Token::DotDotDot) => (true, true),
                                Some(Token::LtDotDot) => (false, true),
                                _ => (true, false), // `..` → `..<` 语义
                            };
                        self.bump();
                        let upper = if self.check(&Token::RBracket) {
                            None
                        } else {
                            Some(self.parse_expr()?)
                        };
                        let end = upper.as_ref().map(|u| u.span).unwrap_or(rstart);
                        AstExpr::new(
                            ExprKind::Range {
                                lower: None,
                                upper,
                                lower_inclusive,
                                upper_inclusive,
                            },
                            self.merge_span(rstart, end),
                        )
                    }
                    _ => self.parse_expr()?,
                };
                let rb = self.expect(&Token::RBracket, "']'")?;
                let span = self.merge_span(start, rb.span);
                lhs = AstExpr::new(ExprKind::Index { expr: lhs, index }, span);
                continue;
            }
            // `?` 错误传播（K1）：后缀运算符，优先级同其他后缀
            if self.check(&Token::Question) {
                let start = lhs.span;
                self.bump();
                let span = self.span_until_current(start);
                lhs = AstExpr::new(ExprKind::Question(Box::new(lhs)), span);
                continue;
            }
            if self.check(&Token::In) {
                let start = lhs.span;
                self.bump();
                if self
                    .current()
                    .is_some_and(|t| matches!(t, Token::Lifetime(_)))
                {
                    let region = self.expect_lifetime()?;
                    let end = lhs.span;
                    lhs = AstExpr::new(
                        ExprKind::InRegion { expr: lhs, region },
                        self.merge_span(start, end),
                    );
                } else {
                    lhs = match self.parse_in_target()? {
                        InTarget::Set(set) => AstExpr::new(
                            ExprKind::InSet {
                                value: lhs,
                                set,
                                negated: false,
                            },
                            self.span_until_current(start),
                        ),
                        InTarget::Range(range) => AstExpr::new(
                            ExprKind::InRange {
                                value: lhs,
                                range,
                                negated: false,
                            },
                            self.span_until_current(start),
                        ),
                        InTarget::Container(c) => AstExpr::new(
                            ExprKind::InContainer {
                                value: lhs,
                                container: c,
                                negated: false,
                            },
                            self.span_until_current(start),
                        ),
                    };
                }
                continue;
            }
            if self.check(&Token::NotIn) {
                let start = lhs.span;
                self.bump();
                lhs = match self.parse_in_target()? {
                    InTarget::Set(set) => AstExpr::new(
                        ExprKind::InSet {
                            value: lhs,
                            set,
                            negated: true,
                        },
                        self.span_until_current(start),
                    ),
                    InTarget::Range(range) => AstExpr::new(
                        ExprKind::InRange {
                            value: lhs,
                            range,
                            negated: true,
                        },
                        self.span_until_current(start),
                    ),
                    InTarget::Container(c) => AstExpr::new(
                        ExprKind::InContainer {
                            value: lhs,
                            container: c,
                            negated: true,
                        },
                        self.span_until_current(start),
                    ),
                };
                continue;
            }

            // 中缀运算符
            let Some(tok) = self.current().cloned() else {
                break;
            };
            // 旧范围语法 `a..b` / `a..=b` 已废弃，给出明确提示
            if matches!(tok, Token::Range) {
                return Err(self.unexpected("new range syntax `..<` or `...`"));
            }
            let Some((p, right_assoc)) = infix_info(&tok) else {
                break;
            };
            if p < min_prec {
                break;
            }
            // 块类表达式（if/match/while/loop/for/region）以 `}` 结尾；若后续中缀
            // 运算符与 `}` 跨行，视为语句结束而非运算延续：
            // `if k == 0 { return st; }` 换行后的 `-1` 应为独立表达式而非 `(if...) - 1`
            let op_line = self.peek().map(|lt| lt.span.line).unwrap_or(0);
            if is_block_expr(&lhs) && self.last_line != op_line {
                break;
            }

            // 赋值（右结合）
            if right_assoc {
                let op = assign_op(&tok);
                self.bump();
                let start = lhs.span;
                let value = self.parse_expr_prec(prec::ASSIGN)?;
                let span = self.merge_span(start, value.span);
                lhs = AstExpr::new(
                    ExprKind::Assign {
                        target: lhs,
                        op,
                        value,
                    },
                    span,
                );
                continue;
            }

            // 比较链：收集连续比较运算符
            if p == prec::COMPARE {
                let start = lhs.span;
                let mut elements = vec![lhs];
                let mut operators = Vec::new();
                while let Some(op) = compare_op(self.current()) {
                    self.bump();
                    operators.push(op);
                    let rhs = self.parse_expr_prec(prec::COMPARE + 1)?;
                    elements.push(rhs);
                }
                let end = elements.last().map(|e| e.span).unwrap_or(start);
                lhs = AstExpr::new(
                    ExprKind::ComparisonChain {
                        elements,
                        operators,
                    },
                    self.merge_span(start, end),
                );
                continue;
            }

            // 范围表达式：`a..<b`（左闭右开）/ `a...b`（闭）/ `a<..b`（左开右闭）
            if p == prec::RANGE {
                let (lower_inclusive, upper_inclusive) = match tok {
                    Token::DotDotLt => (true, false),
                    Token::DotDotDot => (true, true),
                    Token::LtDotDot => (false, true),
                    _ => unreachable!("guarded by infix_info"),
                };
                self.bump();
                let start = lhs.span;
                // P8：上界省略——`v[0..]` / `v[0..<]`（`]` 前无上界表达式）
                let upper = if self.check(&Token::RBracket) {
                    None
                } else {
                    Some(self.parse_expr_prec(prec::RANGE + 1)?)
                };
                let end = upper.as_ref().map(|u| u.span).unwrap_or(start);
                let span = self.merge_span(start, end);
                lhs = AstExpr::new(
                    ExprKind::Range {
                        lower: Some(lhs),
                        upper,
                        lower_inclusive,
                        upper_inclusive,
                    },
                    span,
                );
                continue;
            }

            // as 类型转换
            if p == prec::CAST {
                self.bump();
                let start = lhs.span;
                let target_type = self.parse_type()?;
                let end = self.span_until_current(start);
                lhs = AstExpr::new(
                    ExprKind::Cast {
                        expr: lhs,
                        target_type,
                    },
                    end,
                );
                continue;
            }

            // 普通左结合二元运算
            let op = binary_op(&tok);
            self.bump();
            let start = lhs.span;
            let rhs = self.parse_expr_prec(p + 1)?;
            let span = self.merge_span(start, rhs.span);
            lhs = AstExpr::new(
                ExprKind::Binary {
                    op,
                    left: lhs,
                    right: rhs,
                },
                span,
            );
        }
        Ok(lhs)
    }

    /// 解析 `in`/`not in` 右侧目标：
    /// - `{a, b, c}` → 集合（`InSet`，成员判断，范围元素展开为离散成员）
    /// - `(a, b, c)` → 元组值（`InContainer`，运行时容器成员判断；与元组值
    ///   字面量 `(a,b,c)` 语义一致）
    /// - `[a, b, c]` / 标识符 / `vec!` / 字符串等 → 运行时容器（`InContainer`）
    /// - `a..<b` / `a...b` / `a<..b` → 裸范围（`InRange`，区间判断）
    ///
    /// 区域目标 `'r`（`InRegion`）由调用方在处理 `Lifetime` 时单独处理。
    fn parse_in_target(&mut self) -> Result<InTarget, ParseError> {
        if self.check(&Token::LBrace) {
            return Ok(InTarget::Set(self.parse_brace_set()?));
        }
        // `(a, b, c)` → 元组值（运行时容器成员判断）
        if self.check(&Token::LParen) {
            return Ok(InTarget::Container(self.parse_expr()?));
        }
        // 裸范围：`x in 0..<10`（否则当作运行时容器表达式）
        let expr = self.parse_expr_prec(prec::RANGE)?;
        if matches!(&*expr.kind, ExprKind::Range { .. }) {
            return Ok(InTarget::Range(expr));
        }
        Ok(InTarget::Container(expr))
    }

    /// 解析 `{elem, elem, ...}` 集合元素（集合字面量语法）
    fn parse_brace_set(&mut self) -> Result<Vec<AstExpr>, ParseError> {
        self.expect(&Token::LBrace, "'{'")?;
        let mut set = Vec::new();
        while !self.check(&Token::RBrace) {
            if self.at_eof() {
                return Err(self.unexpected("'}'"));
            }
            set.push(self.parse_expr()?);
            if !self.eat(&Token::Comma) {
                break;
            }
        }
        self.expect(&Token::RBrace, "'}'")?;
        Ok(set)
    }

    /// 解析调用实参 `(a, b, c)`，返回参数列表与结束位置

    /// 处理后缀 `.`（字段访问 / 方法调用 / `.await`）

    /// 解析前缀表达式（字面量 / 标识符 / 路径 / 一元 / 控制流 / 闭包）

    /// 数组字面量 `[a, b, c]`（允许尾逗号；空数组 `[]` 在 typecheck 阶段报错）

    /// 标识符开头的前缀：`move` 闭包 / 路径 / 普通标识符

    /// 解析宏调用 `name!(...)` / `name![...]` / `name!{...}`（I1）。
    ///
    /// - 内置格式化宏（`println!`/`print!`/`format!`/`dbg!`）：收集定界内容，
    ///   参数按表达式列表解析，产出 `ExprKind::MacroCall`（由 typecheck 层 I2
    ///   desugar 为字符串拼接 + 打印内建）；
    /// - 用户 `macro_rules!`：收集定界内容 token 流 → `rlyeh-macro` 展开 →
    ///   子 Parser 递归解析为表达式（展开发生在 parse 阶段、typecheck 之前）；
    /// - 未知宏：报错。

    /// 解析 I3 集合宏 `arr!` / `vec!` / `map!`（parse 期 desugar）。
    ///
    /// - `arr![a, b, c]` → `ExprKind::ArrayLit([a, b, c])`（元素须类型统一，typecheck 校验）
    /// - `vec![a, b, c]` → 块：`let mut __vec_N = Vec::with_capacity(3);`
    ///   `__vec_N.push(a); ...; __vec_N`（空 → `Vec::new()`）
    /// - `map![k1 => v1, ...]` → 块：`let mut __map_N = HashMap::with_capacity(n);`
    ///   `__map_N.insert(k1, v1); ...; __map_N`（空 → `HashMap::new()`）
    ///
    /// 元素 token 流经子 Parser 解析（继承宏注册表，支持嵌套宏调用与完整表达式）；
    /// `Vec::with_capacity`/`Vec::push`、`HashMap::insert` 均为 typecheck 已有路径
    /// （构造器特判 + std 方法查询），零新增 IR 节点。

    /// 判断当前位置是否为结构体字面量构造 `Ident { field: value, ... }`。
    ///
    /// 通过 lookahead 区分：
    /// - `Point { x: 3 }` → `{` 后是 `ident :` 且冒号后不是 `:`（非 `::`）
    /// - `match s { Status::Ok => 200 }` → `s {` 后是 `ident ::`，返回 false
    /// - `x in y {}` → `y {` 后不是 `ident :` 形式，返回 false
    ///
    /// 空结构体构造 `Point {}` 在 MVP 阶段不支持（与块/`in` 目标存在歧义）。

    /// U8：泛型结构体构造 lookahead——`Pair<i64> { ... }`。
    ///
    /// 当前 token 为 `<`，向前扫描到匹配的 `>`，若其后紧跟 `{` 则判定为
    /// 结构体构造的类型实参列表（不消费 token，仅前瞻）。避免与比较运算
    /// `a < b`（`<` 后非 `类型 > {` 模式）歧义。

    /// 消费 `::`（由两个相邻 `Colon` 组成）
    pub(crate) fn eat_colon_colon(&mut self) -> bool {
        if self.check(&Token::Colon) && self.peek_n(1).is_some_and(|t| t.token == Token::Colon) {
            self.bump();
            self.bump();
            true
        } else {
            false
        }
    }

    /// turbofish 检测：`::<`（PathSep + Lt），用于 `foo::<T>(args)` 泛型类型实参
    pub(crate) fn check_turbofish(&self) -> bool {
        self.check(&Token::Colon)
            && self.peek_n(1).is_some_and(|t| t.token == Token::Colon)
            && self.peek_n(2).is_some_and(|t| t.token == Token::Lt)
    }

    /// 括号表达式：空集合 / 集合字面量 / 分组
    ///
    /// 括号内逗号分隔的元素构成集合（`(1, 3, 5)`、`(0...10, 20, 30)`）；
    /// 单元素时若为范围（`(0...10)`）构成单元素集合，否则为分组（`(x + y)`）。

    /// if / else if / else 表达式

    /// match 表达式

    /// while 表达式

    /// loop 表达式

    /// for 表达式：`for pattern in iterator { body }`

    /// return 表达式

    /// break 表达式

    /// 语句终止符：`;` `}` `,`（match 臂尾）或 EOF。
    /// `,` 用于 `match { None => break, Some(v) => return v, }` 等臂体为
    /// 无值 break/return 的场景（`break,` / `return,`）。

    /// send 表达式：`send actor.method(args)`

    /// 闭包：`|params| body` / `|| body` / `move |params| body` / `move || body`

    /// 从 `start` 到当前 token 之前构造 span
    pub(crate) fn span_until_current(&self, start: Span) -> Span {
        match self.peek() {
            Some(lt) => Span {
                start: start.start,
                end: lt.span.start,
                line: start.line,
                col: start.col,
            },
            None => start,
        }
    }
}

/// 块类表达式（以 `}` 结尾，可独立成语句）：跨行后不应与后续运算符组成二元表达式
fn is_block_expr(e: &AstExpr) -> bool {
    matches!(
        *e.kind,
        ExprKind::If { .. }
            | ExprKind::Match { .. }
            | ExprKind::While { .. }
            | ExprKind::Loop { .. }
            | ExprKind::For { .. }
            | ExprKind::Region { .. }
    )
}

/// 中缀运算符信息：`(优先级, 是否右结合)`
fn infix_info(tok: &Token) -> Option<(u8, bool)> {
    let p = match tok {
        Token::Assign | Token::PlusEq | Token::MinusEq | Token::StarEq | Token::SlashEq => {
            prec::ASSIGN
        }
        Token::OrOr => prec::OR,
        Token::AndAnd => prec::AND,
        Token::BitOr => prec::BIT_OR,
        Token::BitXor => prec::BIT_XOR,
        Token::BitAnd => prec::BIT_AND,
        Token::DotDotLt | Token::DotDotDot | Token::LtDotDot => prec::RANGE,
        Token::Lt | Token::Le | Token::Gt | Token::Ge | Token::Eq | Token::Ne => prec::COMPARE,
        Token::Shl | Token::Shr => prec::SHIFT,
        Token::Plus | Token::Minus => prec::ADD,
        Token::Star | Token::Slash | Token::Percent => prec::MUL,
        Token::As => prec::CAST,
        _ => return None,
    };
    Some((p, p == prec::ASSIGN))
}

/// token → 比较运算符
fn compare_op(tok: Option<&Token>) -> Option<CompareOp> {
    match tok {
        Some(Token::Lt) => Some(CompareOp::Lt),
        Some(Token::Le) => Some(CompareOp::Le),
        Some(Token::Gt) => Some(CompareOp::Gt),
        Some(Token::Ge) => Some(CompareOp::Ge),
        Some(Token::Eq) => Some(CompareOp::Eq),
        Some(Token::Ne) => Some(CompareOp::Ne),
        _ => None,
    }
}

/// token → 赋值运算符
fn assign_op(tok: &Token) -> AssignOp {
    match tok {
        Token::Assign => AssignOp::Assign,
        Token::PlusEq => AssignOp::AddAssign,
        Token::MinusEq => AssignOp::SubAssign,
        Token::StarEq => AssignOp::MulAssign,
        Token::SlashEq => AssignOp::DivAssign,
        _ => unreachable!("guarded by infix_info"),
    }
}

/// token → 二元运算符
fn binary_op(tok: &Token) -> BinaryOp {
    match tok {
        Token::Plus => BinaryOp::Add,
        Token::Minus => BinaryOp::Sub,
        Token::Star => BinaryOp::Mul,
        Token::Slash => BinaryOp::Div,
        Token::Percent => BinaryOp::Mod,
        Token::BitAnd => BinaryOp::BitAnd,
        Token::BitOr => BinaryOp::BitOr,
        Token::BitXor => BinaryOp::BitXor,
        Token::Shl => BinaryOp::Shl,
        Token::Shr => BinaryOp::Shr,
        Token::AndAnd => BinaryOp::And,
        Token::OrOr => BinaryOp::Or,
        _ => unreachable!("guarded by infix_info"),
    }
}

/// 内置格式化宏（I2：由 typecheck 层 desugar 为字符串拼接 + 打印内建；
/// `eprintln!`/`eprint!` 输出到 stderr，desugar 目标为 `eprint`/`eprintln` 内建）。
pub(crate) fn is_builtin_macro(name: &str) -> bool {
    matches!(
        name,
        "println"
            | "print"
            | "format"
            | "dbg"
            | "eprintln"
            | "eprint"
            | "assert"
            | "assert_eq"
            | "assert_ne"
            | "panic"
            | "unreachable"
            | "todo"
    )
}

/// I3 集合宏名（`arr!`/`vec!`/`map!`，parse 期 desugar 为数组字面量或块表达式）
pub(crate) fn is_collection_macro(name: &str) -> bool {
    matches!(name, "arr" | "vec" | "map")
}

// 注意：parse_match_arm 定义在 stmt.rs，见 `impl Parser` 的 parse_match_arm。


mod primary;
mod control;
mod macro_;
