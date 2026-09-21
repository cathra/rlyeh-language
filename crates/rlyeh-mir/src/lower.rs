//! HIR → MIR 降低（lowering）。
//!
//! 将类型检查后的结构化 HIR 降低为 CFG 形式：
//! - 表达式求值到局部变量（`_tN` 临时变量），副作用指令按序发射；
//! - `if` / `while` / `loop` 生成对应的控制流块；
//! - `break` / `continue` 跳转到循环上下文的出口 / 头部；
//! - `SetLookup` / `RangeCheck` 展开为比较链（`==`/`!=` 与 `&&`/`||`）；
//! - 区域操作（`RegionEnter` / `RegionExit` / `AllocInRegion` / `Transfer`）显式化。

use crate::{BasicBlock, Local, MirFunction, MirProgram, MirStmt, MirTerminator, MirValue};
use rlyeh_hir::{
    FieldScalar, HirBinaryOp, HirBlock, HirExpr, HirExprKind, HirItemKind, HirProgram, HirStmt,
    HirStmtKind, HirUnaryOp,
};

/// 循环上下文：`break` / `continue` 的跳转目标。
struct LoopCtx {
    /// `break` 跳转的出口块
    break_target: usize,
    /// 循环体内是否出现过 `break`（决定 after 块可达性与块值）
    had_break: bool,
    /// `continue` 跳转的头部块
    continue_target: usize,
    /// `break <value>` 的承载槽（EH-6 M1 前置）。
    ///
    /// `loop` 循环在降低循环体**之前**分配该临时变量，携带值的 `break` 先把
    /// 值写入该槽再跳出口块，出口块以该槽作为循环表达式的值——支持
    /// `let x = loop { break 5 };`（类型为 `i64`）。`while` 循环的 `break`
    /// 在语言语义中无值（循环类型恒为 `()`），故为 `None`。
    break_value: Option<Local>,
    /// 是否出现过**携带值**的 `break`（决定出口块返回该槽还是单元值；
    /// 仅 `break;` 的循环保持既有 `()` 值路径）。
    break_has_value: bool,
}

/// HIR → MIR 降低器（每函数复用，降低前重置状态）。
#[derive(Default)]
pub struct MirLowerer {
    /// 当前函数的基本块列表
    blocks: Vec<BasicBlock>,
    /// 当前填充块
    cur: usize,
    /// 临时变量计数器
    temp_counter: usize,
    /// 循环上下文栈
    loop_stack: Vec<LoopCtx>,
    /// 匿名区域序号（L3：匿名区域赋唯一内部名 `__anon_region_N`，
    /// 供 `RegionExit` 与代码生成的区域句柄槽配对）
    anon_region_seq: usize,
}

/// 将 HIR 程序降低为 MIR 程序。
pub fn lower_program(program: &HirProgram) -> MirProgram {
    let mut lowerer = MirLowerer::default();
    let mut functions = Vec::new();
    for item in &program.items {
        match &item.kind {
            HirItemKind::Fn(f) => {
                if let Some(body) = &f.body {
                    functions.push(lowerer.lower_function(&item.name, &f.params, body));
                } else if f.is_extern {
                    // extern 声明：保留空 CFG + 签名（LIR 解析 extern_sig 生成 declare）
                    functions.push(MirFunction {
                        name: item.name.clone(),
                        params: f.params.iter().map(|p| p.name.clone()).collect(),
                        blocks: Vec::new(),
                        is_extern: true,
                        extern_sig: f.extern_sig.clone(),
                    });
                }
            }
            HirItemKind::Const(_) => {
                // MVP：const 值由后续常量传播 pass 展开，降低阶段仅保留函数
            }
        }
    }
    MirProgram { functions }
}

impl MirLowerer {
    /// 降低一个函数为 CFG。
    fn lower_function(
        &mut self,
        name: &str,
        params: &[rlyeh_hir::HirParam],
        body: &HirBlock,
    ) -> MirFunction {
        self.blocks = vec![BasicBlock {
            stmts: Vec::new(),
            terminator: None,
        }];
        self.cur = 0;
        self.temp_counter = 0;
        self.loop_stack.clear();

        let val = self.lower_block(body);
        if !self.cur_closed() {
            self.terminate(MirTerminator::Return(val));
        }
        MirFunction {
            name: name.to_string(),
            params: params.iter().map(|p| p.name.clone()).collect(),
            blocks: std::mem::take(&mut self.blocks),
            is_extern: false,
            extern_sig: None,
        }
    }

    /// 降低一个代码块，返回块值所在的局部变量；
    /// 块内出现终止表达式（return/break/continue）时返回 `None`。
    fn lower_block(&mut self, block: &HirBlock) -> Option<Local> {
        for stmt in &block.stmts {
            if self.cur_closed() {
                return None; // 不可达代码直接跳过
            }
            self.lower_stmt(stmt);
        }
        if self.cur_closed() {
            return None;
        }
        match &block.final_expr {
            Some(e) => self.lower_expr(e),
            None => {
                let t = self.fresh_temp();
                self.emit(MirStmt::Assign {
                    target: t.clone(),
                    value: MirValue::Unit,
                });
                Some(t)
            }
        }
    }

    /// 降低一条语句。
    fn lower_stmt(&mut self, stmt: &HirStmt) {
        match &stmt.kind {
            HirStmtKind::Let { name, init, .. } => {
                if let Some(v) = self.lower_value(init) {
                    self.emit(MirStmt::Assign {
                        target: name.clone(),
                        value: v,
                    });
                }
            }
            HirStmtKind::Expr(e) | HirStmtKind::Semi(e) => {
                // 副作用执行，结果丢弃
                let _ = self.lower_value(e);
            }
        }
    }

    /// 将表达式求值到局部变量并返回其名字；
    /// 表达式终止控制流（return/break/continue）时返回 `None`。
    fn lower_expr(&mut self, expr: &HirExpr) -> Option<Local> {
        match self.lower_value(expr)? {
            MirValue::Place(p) => Some(p),
            v => {
                let tmp = self.fresh_temp();
                self.emit(MirStmt::Assign {
                    target: tmp.clone(),
                    value: v,
                });
                Some(tmp)
            }
        }
    }

    /// 将表达式降低为右值（可内联的运算树 / 常量 / 已求值的 place）。
    /// 可能发射副作用指令（调用、控制流）；终止控制流时返回 `None`。
    fn lower_value(&mut self, expr: &HirExpr) -> Option<MirValue> {
        match &expr.kind {
            HirExprKind::IntLiteral(v) => Some(MirValue::Int(*v)),
            HirExprKind::FloatLiteral(v) => Some(MirValue::Float(*v)),
            HirExprKind::StringLiteral(s) => Some(MirValue::String(s.clone())),
            HirExprKind::CharLiteral(c) => Some(MirValue::Char(*c)),
            HirExprKind::BoolLiteral(b) => Some(MirValue::Bool(*b)),
            HirExprKind::Unit => Some(MirValue::Unit),
            HirExprKind::FnPtr(name) => Some(MirValue::FnRef(name.clone())),
            HirExprKind::Variable(v) => Some(MirValue::Place(v.clone())),
            HirExprKind::Assign { target, op, value } => {
                // 先求值右侧（副作用顺序），再读 target 构造复合赋值
                let v = self.lower_expr(value)?;
                let value = match op {
                    rlyeh_hir::HirAssignOp::Assign => MirValue::Place(v),
                    rlyeh_hir::HirAssignOp::AddAssign => MirValue::Binary {
                        op: HirBinaryOp::Add,
                        lhs: Box::new(MirValue::Place(target.clone())),
                        rhs: Box::new(MirValue::Place(v)),
                    },
                    rlyeh_hir::HirAssignOp::SubAssign => MirValue::Binary {
                        op: HirBinaryOp::Sub,
                        lhs: Box::new(MirValue::Place(target.clone())),
                        rhs: Box::new(MirValue::Place(v)),
                    },
                    rlyeh_hir::HirAssignOp::MulAssign => MirValue::Binary {
                        op: HirBinaryOp::Mul,
                        lhs: Box::new(MirValue::Place(target.clone())),
                        rhs: Box::new(MirValue::Place(v)),
                    },
                    rlyeh_hir::HirAssignOp::DivAssign => MirValue::Binary {
                        op: HirBinaryOp::Div,
                        lhs: Box::new(MirValue::Place(target.clone())),
                        rhs: Box::new(MirValue::Place(v)),
                    },
                };
                self.emit(MirStmt::Assign {
                    target: target.clone(),
                    value,
                });
                Some(MirValue::Unit)
            }
            HirExprKind::Binary(op, l, r) => {
                let lv = self.lower_value(l)?;
                let rv = self.lower_value(r)?;
                Some(MirValue::Binary {
                    op: *op,
                    lhs: Box::new(lv),
                    rhs: Box::new(rv),
                })
            }
            HirExprKind::Unary(op, e) => {
                let v = self.lower_value(e)?;
                Some(MirValue::Unary {
                    op: *op,
                    operand: Box::new(v),
                })
            }
            HirExprKind::Call { callee, args } => {
                let mut arg_places = Vec::with_capacity(args.len());
                for a in args {
                    arg_places.push(self.lower_expr(a)?);
                }
                let tmp = self.fresh_temp();
                self.emit(MirStmt::Call {
                    target: Some(tmp.clone()),
                    callee: callee.clone(),
                    args: arg_places,
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::CallIndirect {
                callee,
                args,
                param_names,
                ret_name,
            } => {
                let callee_place = self.lower_expr(callee)?;
                let mut arg_places = Vec::with_capacity(args.len());
                for a in args {
                    arg_places.push(self.lower_expr(a)?);
                }
                let tmp = self.fresh_temp();
                self.emit(MirStmt::CallIndirect {
                    target: Some(tmp.clone()),
                    callee: callee_place,
                    args: arg_places,
                    param_names: param_names.clone(),
                    ret_name: ret_name.clone(),
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::If {
                cond,
                then_block,
                else_block,
            } => self.lower_if(cond, then_block, else_block.as_deref()),
            HirExprKind::While { cond, body } => self.lower_while(cond, body),
            HirExprKind::Loop { body } => self.lower_loop(body),
            HirExprKind::Return(e) => {
                let val = match e {
                    Some(inner) => self.lower_expr(inner),
                    None => None,
                };
                self.terminate(MirTerminator::Return(val));
                None
            }
            HirExprKind::Break(e) => {
                // EH-6 M1 前置（2026-09-21）：`break <value>` 的值不再丢弃——
                // 先求值，再写入循环上下文的 `break_value` 槽，出口块以该槽为
                // 循环表达式的值（`while` 无槽，值仍丢弃，语义为 `()`）。
                let val = match e {
                    Some(inner) => self.lower_expr(inner),
                    None => None,
                };
                let (break_target, slot) = {
                    let ctx = self
                        .loop_stack
                        .last_mut()
                        .expect("break 出现在循环之外（typecheck 应已拒绝）");
                    ctx.had_break = true;
                    if val.is_some() {
                        ctx.break_has_value = true;
                    }
                    (ctx.break_target, ctx.break_value.clone())
                };
                if let (Some(slot), Some(v)) = (slot, val) {
                    self.emit(MirStmt::Assign {
                        target: slot,
                        value: MirValue::Place(v),
                    });
                }
                self.terminate(MirTerminator::Jump(break_target));
                None
            }
            HirExprKind::Continue => {
                let ctx = self
                    .loop_stack
                    .last()
                    .expect("continue 出现在循环之外（typecheck 应已拒绝）");
                self.terminate(MirTerminator::Jump(ctx.continue_target));
                None
            }
            HirExprKind::Block(b) | HirExprKind::UnsafeBlock(b) => {
                let val = self.lower_block(b)?;
                Some(MirValue::Place(val))
            }
            HirExprKind::Region {
                name,
                options,
                body,
            } => {
                // 匿名区域赋唯一内部名，保证 RegionExit 与句柄槽正确配对
                let key = match name {
                    Some(n) => n.clone(),
                    None => {
                        let n = format!("__anon_region_{}", self.anon_region_seq);
                        self.anon_region_seq += 1;
                        n
                    }
                };
                self.emit(MirStmt::RegionEnter {
                    name: Some(key.clone()),
                    options: *options,
                });
                let val = self.lower_block(body)?;
                self.emit(MirStmt::RegionExit { name: Some(key) });
                Some(MirValue::Place(val))
            }
            HirExprKind::InRegion { expr, region, size } => {
                let val = self.lower_expr(expr)?;
                self.emit(MirStmt::AllocInRegion {
                    target: val.clone(),
                    region: region.clone(),
                    size: *size,
                });
                Some(MirValue::Place(val))
            }
            HirExprKind::Transfer { expr, region } => {
                let val = self.lower_expr(expr)?;
                self.emit(MirStmt::Transfer {
                    place: val.clone(),
                    region: region.clone(),
                });
                Some(MirValue::Place(val))
            }
            HirExprKind::SetLookup {
                value,
                members,
                negated,
            } => {
                // 展开为 `x == m1 || x == m2 || ...`，取反时包 `!`
                let v = self.lower_expr(value)?;
                let mut acc: Option<MirValue> = None;
                for m in members {
                    let mv = self.lower_value(m)?;
                    let cmp = MirValue::Binary {
                        op: HirBinaryOp::Eq,
                        lhs: Box::new(MirValue::Place(v.clone())),
                        rhs: Box::new(mv),
                    };
                    acc = Some(match acc {
                        None => cmp,
                        Some(prev) => MirValue::Binary {
                            op: HirBinaryOp::Or,
                            lhs: Box::new(prev),
                            rhs: Box::new(cmp),
                        },
                    });
                }
                let mut r = acc.unwrap_or(MirValue::Bool(false));
                if *negated {
                    r = MirValue::Unary {
                        op: HirUnaryOp::Not,
                        operand: Box::new(r),
                    };
                }
                Some(r)
            }
            HirExprKind::RangeCheck {
                value,
                lower,
                upper,
                lower_inclusive,
                upper_inclusive,
                negated,
            } => {
                // 展开为 `x >= l && x <= u`（含边界），取反时包 `!`
                let v = self.lower_expr(value)?;
                let mut acc: Option<MirValue> = None;
                if let Some(l) = lower {
                    let lv = self.lower_value(l)?;
                    let op = if *lower_inclusive {
                        HirBinaryOp::Ge
                    } else {
                        HirBinaryOp::Gt
                    };
                    acc = Some(MirValue::Binary {
                        op,
                        lhs: Box::new(MirValue::Place(v.clone())),
                        rhs: Box::new(lv),
                    });
                }
                if let Some(u) = upper {
                    let uv = self.lower_value(u)?;
                    let op = if *upper_inclusive {
                        HirBinaryOp::Le
                    } else {
                        HirBinaryOp::Lt
                    };
                    let cmp = MirValue::Binary {
                        op,
                        lhs: Box::new(MirValue::Place(v.clone())),
                        rhs: Box::new(uv),
                    };
                    acc = Some(match acc {
                        None => cmp,
                        Some(prev) => MirValue::Binary {
                            op: HirBinaryOp::And,
                            lhs: Box::new(prev),
                            rhs: Box::new(cmp),
                        },
                    });
                }
                let mut r = acc.unwrap_or(MirValue::Bool(true));
                if *negated {
                    r = MirValue::Unary {
                        op: HirUnaryOp::Not,
                        operand: Box::new(r),
                    };
                }
                Some(r)
            }
            HirExprKind::Alloc {
                slots,
                by_value,
                is_strfat,
            } => {
                let tmp = self.fresh_temp();
                self.emit(MirStmt::Alloc {
                    target: tmp.clone(),
                    slots: *slots,
                    by_value: *by_value,
                    is_strfat: *is_strfat,
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::FieldGet { base, index, ty } => {
                let b = self.lower_expr(base)?;
                let tmp = self.fresh_temp();
                self.emit(MirStmt::FieldGet {
                    target: tmp.clone(),
                    base: b,
                    index: *index,
                    ty: *ty,
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::FieldSet { base, index, value, ty } => {
                let b = self.lower_expr(base)?;
                let v = self.lower_expr(value)?;
                self.emit(MirStmt::FieldSet {
                    base: b,
                    index: *index,
                    value: v,
                    ty: *ty,
                });
                Some(MirValue::Unit)
            }
            HirExprKind::Index { base, index, elem, is_str, len } => {
                let b = self.lower_expr(base)?;
                let i = self.lower_expr(index)?;
                let ln = match len {
                    Some(e) => Some(self.lower_expr(e)?),
                    None => None,
                };
                let tmp = self.fresh_temp();
                self.emit(MirStmt::IndexGet {
                    target: tmp.clone(),
                    base: b,
                    index: i,
                    ty: *elem,
                    is_str: *is_str,
                    len: ln,
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::IndexSet { base, index, value, elem, is_str, len } => {
                let b = self.lower_expr(base)?;
                let i = self.lower_expr(index)?;
                let v = self.lower_expr(value)?;
                let ln = match len {
                    Some(e) => Some(self.lower_expr(e)?),
                    None => None,
                };
                self.emit(MirStmt::IndexSet {
                    base: b,
                    index: i,
                    value: v,
                    ty: *elem,
                    is_str: *is_str,
                    len: ln,
                });
                Some(MirValue::Unit)
            }
            HirExprKind::Ref {
                expr,
                is_mut: _,
                pointee,
            } => {
                // `&*p`（U5：解引用再取引用）：引用按指针传递，`&*p` 的值即
                // `p` 的指针值，直接透传内部指针——避免 DerefRead 拷贝临时再
                // 取址的语义错误（拷贝后地址 ≠ 原地址，`&mut` 写回不生效）。
                if let HirExprKind::Deref { expr: inner, .. } = &(expr.as_ref()).kind {
                    return self.lower_expr(inner).map(MirValue::Place);
                }
                // `&obj.field`（V1）：取字段槽真实地址（GEP）——写回经
                // DerefWrite(base=target) 直达原字段，`&mut` 写回生效
                if let HirExprKind::FieldGet { base, index, ty } = &(expr.as_ref()).kind {
                    let b = self.lower_expr(base)?;
                    let tmp = self.fresh_temp();
                    self.emit(MirStmt::AddrOfField {
                        target: tmp.clone(),
                        base: b,
                        index: *index,
                        ty: *ty,
                    });
                    return Some(MirValue::Place(tmp));
                }
                // `&arr[i]` / `&s[i]`（V1）：base 地址化 + 指针偏移（GEP）——
                // 真实元素地址（非拷贝临时地址），`&mut` 写回原元素
                if let HirExprKind::Index { base, index, elem, is_str, .. } = &(expr.as_ref()).kind {
                    let b = self.addr_of(base)?;
                    let i = self.lower_expr(index)?;
                    let tmp = self.fresh_temp();
                    self.emit(MirStmt::PtrAdd {
                        target: tmp.clone(),
                        base: b,
                        offset: i,
                        elem: *elem,
                        is_str: *is_str,
                    });
                    return Some(MirValue::Place(tmp));
                }
                // `&x` / `&mut x`：取引用（lower_expr 递归降任意目标表达式
                // 到临时槽再取址——非变量目标亦支持，U5）
                let o = self.lower_expr(expr)?;
                let tmp = self.fresh_temp();
                self.emit(MirStmt::AddrOf {
                    target: tmp.clone(),
                    operand: o,
                    pointee: *pointee,
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::Deref { expr, ty } => {
                // `*p` 读取：解引用（标量 load / 聚合指针拷贝）
                let b = self.lower_expr(expr)?;
                let tmp = self.fresh_temp();
                self.emit(MirStmt::DerefRead {
                    target: tmp.clone(),
                    base: b,
                    ty: *ty,
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::DerefSet { base, value, ty } => {
                // `*p = v`：解引用写入
                let b = self.lower_expr(base)?;
                let v = self.lower_expr(value)?;
                self.emit(MirStmt::DerefWrite {
                    base: b,
                    value: v,
                    ty: *ty,
                });
                Some(MirValue::Unit)
            }
            HirExprKind::PtrAdd { base, offset, elem } => {
                // `ptr + n`（V1）：裸指针算术——指针推进（迭代器瘦指针）。
                // `elem: Str`（V2 字符串字节偏移）→ 1 字节步长（is_str），
                // 供 `as_str_range` 子区间视图做 data 指针字节偏移。
                let b = self.lower_expr(base)?;
                let o = self.lower_expr(offset)?;
                let tmp = self.fresh_temp();
                self.emit(MirStmt::PtrAdd {
                    target: tmp.clone(),
                    base: b,
                    offset: o,
                    elem: *elem,
                    is_str: matches!(elem, FieldScalar::Str),
                });
                Some(MirValue::Place(tmp))
            }
            HirExprKind::Cast { expr, to } => {
                // `expr as target`（U6 Cast IR）：数值→数值类型转换
                let v = self.lower_expr(expr)?;
                let tmp = self.fresh_temp();
                self.emit(MirStmt::Cast {
                    target: tmp.clone(),
                    value: v,
                    to: to.clone(),
                });
                Some(MirValue::Place(tmp))
            }
        }
    }

    /// `addr_of(expr)`：对表达式地址化（V1）——返回指向 `expr` 的指针槽。
    /// 用于 `&arr[i]` 等取址场景的 base 求值：
    /// - 变量 → `AddrOf`（变量槽地址）
    /// - 解引用 `&*p` → 透传指针值（U5 折叠语义）
    /// - 字段链 `obj.field` → 对象求值 + `AddrOfField`（GEP 到字段槽）
    /// - 其他表达式 → 求值到临时再取址（拷贝语义，读可用）
    fn addr_of(&mut self, expr: &HirExpr) -> Option<Local> {
        match &expr.kind {
            // `&*p`：解引用再取址 → 透传指针值
            HirExprKind::Deref { expr: inner, .. } => self.lower_expr(inner),
            // `&x`：变量槽地址（数组/聚合为 8 字节槽区，Ptr 标量）
            HirExprKind::Variable(v) => {
                let tmp = self.fresh_temp();
                self.emit(MirStmt::AddrOf {
                    target: tmp.clone(),
                    operand: v.clone(),
                    pointee: FieldScalar::Ptr,
                });
                Some(tmp)
            }
            // `&obj.field`：base 求值为对象值（字段链剥层）
            HirExprKind::FieldGet { base, index, ty } => {
                if *ty == FieldScalar::Ptr {
                    // 聚合字段（数组/对象）：字段槽存对象指针（聚合拷贝指针
                    // 语义）——「地址」即字段值本身（FieldGet 拷贝指针），
                    // 如 `&v.data[0]` 中 `v.data`（[T; 0] 堆缓冲字段）
                    self.lower_expr(expr)
                } else {
                    // 标量字段：GEP 到字段槽（真实槽地址，写回生效）
                    let b = self.lower_expr(base)?;
                    let tmp = self.fresh_temp();
                    self.emit(MirStmt::AddrOfField {
                        target: tmp.clone(),
                        base: b,
                        index: *index,
                        ty: *ty,
                    });
                    Some(tmp)
                }
            }
            // 其他：求值到临时再取址（拷贝语义）
            _ => {
                let o = self.lower_expr(expr)?;
                let tmp = self.fresh_temp();
                self.emit(MirStmt::AddrOf {
                    target: tmp.clone(),
                    operand: o,
                    pointee: FieldScalar::Ptr,
                });
                Some(tmp)
            }
        }
    }

    /// 降低 if 表达式：`CondJump` 分叉 then/else，合并块读取结果变量。
    fn lower_if(
        &mut self,
        cond: &HirExpr,
        then_block: &HirBlock,
        else_block: Option<&HirBlock>,
    ) -> Option<MirValue> {
        let cond_place = self.lower_expr(cond)?;
        // cond 求值可能发射控制流（如条件为含 if 的块表达式）：CondJump
        // 必须从 cond 求值结束后的当前块发出，而非强制回到入口块——
        // 否则嵌套场景下入口块已被内层 if 闭合（「基本块已有终止符」）。
        // cond 无分支时收尾块 == 入口块，行为不变。
        let emit_id = self.cur;
        let then_id = self.new_block();
        let merge_id = self.new_block();
        let (else_id, has_else) = match else_block {
            Some(_) => (self.new_block(), true),
            None => (merge_id, false),
        };
        // new_block 已切换当前块，切回 cond 收尾块再发射 CondJump
        self.cur = emit_id;
        self.terminate(MirTerminator::CondJump {
            cond: cond_place,
            then: then_id,
            otherwise: else_id,
        });

        let result = self.fresh_temp();
        let mut assigned = false;
        // then 分支
        self.cur = then_id;
        let then_val = self.lower_block(then_block);
        if let Some(v) = then_val {
            self.emit(MirStmt::Assign {
                target: result.clone(),
                value: MirValue::Place(v),
            });
            assigned = true;
        }
        if !self.cur_closed() {
            self.terminate(MirTerminator::Jump(merge_id));
        }
        // else 分支
        if has_else {
            self.cur = else_id;
            let else_val = self.lower_block(else_block.expect("has_else 保证 Some"));
            if let Some(v) = else_val {
                self.emit(MirStmt::Assign {
                    target: result.clone(),
                    value: MirValue::Place(v),
                });
                assigned = true;
            }
            if !self.cur_closed() {
                self.terminate(MirTerminator::Jump(merge_id));
            }
        }
        self.cur = merge_id;
        // 两个分支都无值（均为 Never，如 `loop {}` 充当 panic）：
        // if 表达式不产生块值
        if assigned {
            Some(MirValue::Place(result))
        } else {
            None
        }
    }

    /// 降低 while 循环：head（条件）/ body / after + 回跳。
    fn lower_while(&mut self, cond: &HirExpr, body: &HirBlock) -> Option<MirValue> {
        let entry_id = self.cur;
        let head_id = self.new_block();
        let body_id = self.new_block();
        let after_id = self.new_block();
        // new_block 已切换当前块，切回 entry 再发射 Jump(head)
        self.cur = entry_id;
        self.terminate(MirTerminator::Jump(head_id));

        // head：条件求值 + CondJump
        self.cur = head_id;
        let cond_place = self.lower_expr(cond)?;
        self.terminate(MirTerminator::CondJump {
            cond: cond_place,
            then: body_id,
            otherwise: after_id,
        });

        // body：块尾回跳 head
        self.cur = body_id;
        // EH-6 M1 前置：`while` 的 `break` 无值（循环类型恒为 `()`），故无承载槽。
        self.loop_stack.push(LoopCtx {
            break_target: after_id,
            continue_target: head_id,
            had_break: false,
            break_value: None,
            break_has_value: false,
        });
        let _ = self.lower_block(body);
        self.loop_stack.pop();
        if !self.cur_closed() {
            self.terminate(MirTerminator::Jump(head_id));
        }

        // after：while 表达式值为 Unit
        self.cur = after_id;
        let unit = self.fresh_temp();
        self.emit(MirStmt::Assign {
            target: unit.clone(),
            value: MirValue::Unit,
        });
        Some(MirValue::Place(unit))
    }

    /// 降低 loop 循环：body 自回跳 + after 出口。
    fn lower_loop(&mut self, body: &HirBlock) -> Option<MirValue> {
        let entry_id = self.cur;
        let body_id = self.new_block();
        let after_id = self.new_block();
        // new_block 已切换当前块，切回 entry 再发射 Jump(body)
        self.cur = entry_id;
        self.terminate(MirTerminator::Jump(body_id));

        self.cur = body_id;
        // EH-6 M1 前置：先分配 `break <value>` 承载槽，再降低循环体——
        // 循环体内任意位置（含嵌套 if / match 分支）的带值 `break` 都写入该槽。
        let break_slot = self.fresh_temp();
        self.loop_stack.push(LoopCtx {
            break_target: after_id,
            continue_target: body_id,
            had_break: false,
            break_value: Some(break_slot.clone()),
            break_has_value: false,
        });
        let _ = self.lower_block(body);
        let ctx = self.loop_stack.pop().expect("lower_loop 的 LoopCtx 应存在");
        let (had_break, break_has_value) = (ctx.had_break, ctx.break_has_value);
        if !self.cur_closed() {
            self.terminate(MirTerminator::Jump(body_id));
        }
        if !had_break {
            // 无 break 的无限循环：类型 Never，after 块不可达，
            // 不产生块值（避免 `loop {}` 充当 panic 时污染 if/phi 合并类型）
            return None;
        }

        self.cur = after_id;
        if break_has_value {
            // 带值 break：出口块的值即承载槽（各 break 路径已在跳转前写入）。
            return Some(MirValue::Place(break_slot));
        }
        let unit = self.fresh_temp();
        self.emit(MirStmt::Assign {
            target: unit.clone(),
            value: MirValue::Unit,
        });
        Some(MirValue::Place(unit))
    }

    // ---- 基础设施 ----

    /// 当前块是否已设置终止符（块已闭合）。
    fn cur_closed(&self) -> bool {
        self.blocks[self.cur].terminator.is_some()
    }

    /// 发射一条指令到当前块。
    fn emit(&mut self, stmt: MirStmt) {
        debug_assert!(
            !self.cur_closed(),
            "不能向已终止的基本块发射指令（不可达代码应被跳过）"
        );
        self.blocks[self.cur].stmts.push(stmt);
    }

    /// 为当前块设置终止符。
    fn terminate(&mut self, term: MirTerminator) {
        debug_assert!(
            self.blocks[self.cur].terminator.is_none(),
            "基本块已有终止符"
        );
        self.blocks[self.cur].terminator = Some(term);
    }

    /// 创建新基本块并切换到它。
    fn new_block(&mut self) -> usize {
        self.blocks.push(BasicBlock {
            stmts: Vec::new(),
            terminator: None,
        });
        let id = self.blocks.len() - 1;
        self.cur = id;
        id
    }

    /// 生成一个新的临时变量名。
    fn fresh_temp(&mut self) -> Local {
        let name = format!("_t{}", self.temp_counter);
        self.temp_counter += 1;
        name
    }
}
