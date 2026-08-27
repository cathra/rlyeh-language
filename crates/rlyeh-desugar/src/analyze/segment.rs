//! 表达式检查子模块：segment。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

pub(super) fn flush(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    cur_stmts: &mut Vec<AstStmt>,
    cur_uses: &mut HashSet<String>,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Option<usize> {
    if cur_stmts.is_empty() {
        return None;
    }
    let idx = out.len();
    out.push(Segment {
        stmts: std::mem::take(cur_stmts),
        await_info: None,
        final_expr: None,
        next: Some(idx + 1),
        inline_jump: false,
    });
    ctx.seg_uses.push(std::mem::take(cur_uses));
    if first.is_none() {
        *first = Some(idx);
    }
    *last = Some(idx);
    Some(idx)
}

pub(super) fn flush_tail(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    cur_stmts: &mut Vec<AstStmt>,
    cur_uses: &mut HashSet<String>,
    final_expr: AstExpr,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> usize {
    let mut tail_uses = HashSet::new();
    let _ = scan_expr(&final_expr, &mut tail_uses);
    cur_uses.extend(tail_uses);
    let idx = out.len();
    out.push(Segment {
        stmts: std::mem::take(cur_stmts),
        await_info: None,
        final_expr: Some(final_expr),
        next: None,
        inline_jump: false,
    });
    ctx.seg_uses.push(std::mem::take(cur_uses));
    if first.is_none() {
        *first = Some(idx);
    }
    *last = Some(idx);
    idx
}

pub(super) fn flush_raw(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    cur_stmts: &mut Vec<AstStmt>,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> Option<usize> {
    if cur_stmts.is_empty() {
        return None;
    }
    let idx = out.len();
    out.push(Segment {
        stmts: std::mem::take(cur_stmts),
        await_info: None,
        final_expr: None,
        next: Some(idx + 1),
        inline_jump: false,
    });
    ctx.seg_uses.push(HashSet::new());
    if first.is_none() {
        *first = Some(idx);
    }
    *last = Some(idx);
    Some(idx)
}

pub(super) fn push_await(
    ctx: &mut Ctx,
    out: &mut Vec<Segment>,
    cur_uses: &mut HashSet<String>,
    ai: AwaitInfo,
    terminal: bool,
    first: &mut Option<usize>,
    last: &mut Option<usize>,
) -> usize {
    let idx = out.len();
    out.push(Segment {
        stmts: Vec::new(),
        await_info: Some(ai),
        final_expr: None,
        next: if terminal { None } else { Some(idx + 1) },
        inline_jump: false,
    });
    ctx.seg_uses.push(std::mem::take(cur_uses));
    if first.is_none() {
        *first = Some(idx);
    }
    *last = Some(idx);
    idx
}
