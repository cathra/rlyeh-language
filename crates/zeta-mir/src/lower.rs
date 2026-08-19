//! HIR → MIR 降低（lowering）。
//!
//! 将类型检查后的结构化 HIR 降低为 CFG 形式：
//! - 表达式求值到局部变量（`_tN` 临时变量），副作用指令按序发射；
//! - `if` / `while` / `loop` 生成对应的控制流块；
//! - `break` / `continue` 跳转到循环上下文的出口 / 头部；
//! - `SetLookup` / `RangeCheck` 展开为比较链（`==`/`!=` 与 `&&`/`||`）；
//! - 区域操作（`RegionEnter` / `RegionExit` / `AllocInRegion` / `Transfer`）显式化。

use crate::{
    BasicBlock, Local, MirFunction, MirProgram, MirStmt, MirTerminator, MirValue,
};
use zeta_hir::{
    HirBinaryOp, HirBlock, HirExpr, HirItemKind, HirProgram, HirStmt, HirUnaryOp,
};

/// 循环上下文：`break` / `continue` 的跳转目标。
struct LoopCtx {
    /// `break` 跳转的出口块
    break_target: usize,
    /// `continue` 跳转的头部块
    continue_target: usize,
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
    fn lower_function(&mut self, name: &str, params: &[zeta_hir::HirParam], body: &HirBlock) -> MirFunction {
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
        match stmt {
            HirStmt::Let { name, init, .. } => {
                if let Some(v) = self.lower_value(init) {
                    self.emit(MirStmt::Assign {
                        target: name.clone(),
                        value: v,
                    });
                }
            }
            HirStmt::Expr(e) | HirStmt::Semi(e) => {
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
        match expr {
            HirExpr::IntLiteral(v) => Some(MirValue::Int(*v)),
            HirExpr::FloatLiteral(v) => Some(MirValue::Float(*v)),
            HirExpr::StringLiteral(s) => Some(MirValue::String(s.clone())),
            HirExpr::CharLiteral(c) => Some(MirValue::Char(*c)),
            HirExpr::BoolLiteral(b) => Some(MirValue::Bool(*b)),
            HirExpr::Unit => Some(MirValue::Unit),
            HirExpr::Variable(v) => Some(MirValue::Place(v.clone())),
            HirExpr::Assign { target, op, value } => {
                // 先求值右侧（副作用顺序），再读 target 构造复合赋值
                let v = self.lower_expr(value)?;
                let value = match op {
                    zeta_hir::HirAssignOp::Assign => MirValue::Place(v),
                    zeta_hir::HirAssignOp::AddAssign => MirValue::Binary {
                        op: HirBinaryOp::Add,
                        lhs: Box::new(MirValue::Place(target.clone())),
                        rhs: Box::new(MirValue::Place(v)),
                    },
                    zeta_hir::HirAssignOp::SubAssign => MirValue::Binary {
                        op: HirBinaryOp::Sub,
                        lhs: Box::new(MirValue::Place(target.clone())),
                        rhs: Box::new(MirValue::Place(v)),
                    },
                    zeta_hir::HirAssignOp::MulAssign => MirValue::Binary {
                        op: HirBinaryOp::Mul,
                        lhs: Box::new(MirValue::Place(target.clone())),
                        rhs: Box::new(MirValue::Place(v)),
                    },
                    zeta_hir::HirAssignOp::DivAssign => MirValue::Binary {
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
            HirExpr::Binary(op, l, r) => {
                let lv = self.lower_value(l)?;
                let rv = self.lower_value(r)?;
                Some(MirValue::Binary {
                    op: *op,
                    lhs: Box::new(lv),
                    rhs: Box::new(rv),
                })
            }
            HirExpr::Unary(op, e) => {
                let v = self.lower_value(e)?;
                Some(MirValue::Unary {
                    op: *op,
                    operand: Box::new(v),
                })
            }
            HirExpr::Call { callee, args } => {
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
            HirExpr::If {
                cond,
                then_block,
                else_block,
            } => self.lower_if(cond, then_block, else_block.as_deref()),
            HirExpr::While { cond, body } => self.lower_while(cond, body),
            HirExpr::Loop { body } => self.lower_loop(body),
            HirExpr::Return(e) => {
                let val = match e {
                    Some(inner) => self.lower_expr(inner),
                    None => None,
                };
                self.terminate(MirTerminator::Return(val));
                None
            }
            HirExpr::Break(e) => {
                if let Some(inner) = e {
                    let _ = self.lower_expr(inner); // break 携带的值 MVP 阶段丢弃
                }
                let ctx = self
                    .loop_stack
                    .last()
                    .expect("break 出现在循环之外（typecheck 应已拒绝）");
                self.terminate(MirTerminator::Jump(ctx.break_target));
                None
            }
            HirExpr::Continue => {
                let ctx = self
                    .loop_stack
                    .last()
                    .expect("continue 出现在循环之外（typecheck 应已拒绝）");
                self.terminate(MirTerminator::Jump(ctx.continue_target));
                None
            }
            HirExpr::Block(b) => {
                let val = self.lower_block(b)?;
                Some(MirValue::Place(val))
            }
            HirExpr::Region {
                name,
                options,
                body,
            } => {
                self.emit(MirStmt::RegionEnter {
                    name: name.clone(),
                    options: *options,
                });
                let val = self.lower_block(body)?;
                self.emit(MirStmt::RegionExit);
                Some(MirValue::Place(val))
            }
            HirExpr::InRegion { expr, region } => {
                let val = self.lower_expr(expr)?;
                self.emit(MirStmt::AllocInRegion {
                    target: val.clone(),
                    region: region.clone(),
                });
                Some(MirValue::Place(val))
            }
            HirExpr::Transfer { expr, region } => {
                let val = self.lower_expr(expr)?;
                self.emit(MirStmt::Transfer {
                    place: val.clone(),
                    region: region.clone(),
                });
                Some(MirValue::Place(val))
            }
            HirExpr::SetLookup {
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
            HirExpr::RangeCheck {
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
        let then_id = self.new_block();
        let merge_id = self.new_block();
        let (else_id, has_else) = match else_block {
            Some(_) => (self.new_block(), true),
            None => (merge_id, false),
        };
        self.terminate(MirTerminator::CondJump {
            cond: cond_place,
            then: then_id,
            otherwise: else_id,
        });

        let result = self.fresh_temp();
        // then 分支
        self.cur = then_id;
        let then_val = self.lower_block(then_block);
        if let Some(v) = then_val {
            self.emit(MirStmt::Assign {
                target: result.clone(),
                value: MirValue::Place(v),
            });
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
            }
            if !self.cur_closed() {
                self.terminate(MirTerminator::Jump(merge_id));
            }
        }
        self.cur = merge_id;
        Some(MirValue::Place(result))
    }

    /// 降低 while 循环：head（条件）/ body / after + 回跳。
    fn lower_while(&mut self, cond: &HirExpr, body: &HirBlock) -> Option<MirValue> {
        let head_id = self.new_block();
        let body_id = self.new_block();
        let after_id = self.new_block();
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
        self.loop_stack.push(LoopCtx {
            break_target: after_id,
            continue_target: head_id,
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
        let body_id = self.new_block();
        let after_id = self.new_block();
        self.terminate(MirTerminator::Jump(body_id));

        self.cur = body_id;
        self.loop_stack.push(LoopCtx {
            break_target: after_id,
            continue_target: body_id,
        });
        let _ = self.lower_block(body);
        self.loop_stack.pop();
        if !self.cur_closed() {
            self.terminate(MirTerminator::Jump(body_id));
        }

        self.cur = after_id;
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
