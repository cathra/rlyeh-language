//! zeta-lir 集成测试：HIR → MIR（lower + 优化）→ LIR lowering。

use zeta_lir::lower::lower_program;
use zeta_lir::{LirOperand, LirProgram, LirStmt, LirTerminator, LirType};
use zeta_mir::lower::lower_program as lower_mir;
use zeta_mir::passes::optimize;
use zeta_typecheck::typecheck_source;

/// 完整降低：typecheck → MIR → 优化 → LIR。
fn lower(src: &str) -> LirProgram {
    let hir = typecheck_source(src).expect("typecheck");
    let mut mir = lower_mir(&hir);
    optimize(&mut mir);
    lower_program(&mir).expect("lir lowering")
}

/// 提取函数（按名）。
fn find_fn<'a>(p: &'a LirProgram, name: &str) -> &'a zeta_lir::LirFunction {
    p.functions
        .iter()
        .find(|f| f.name == name)
        .unwrap_or_else(|| panic!("函数 {name} 不存在"))
}

/// 收集块内全部语句。
fn all_stmts(f: &zeta_lir::LirFunction) -> Vec<&LirStmt> {
    f.blocks.iter().flat_map(|b| b.stmts.iter()).collect()
}

#[test]
fn lower_arith_to_three_address() {
    let p = lower("fn add(a: i64, b: i64) -> i64 { a + b }");
    let f = find_fn(&p, "add");
    // 参数类型推断
    assert_eq!(f.params.len(), 2);
    assert_eq!(f.params[0].1, LirType::I64);
    assert_eq!(f.return_type, LirType::I64);
    // 三地址二元运算
    let stmts = all_stmts(f);
    let bin = stmts.iter().find_map(|s| match s {
        LirStmt::Binary {
            op: _,
            ty,
            lhs,
            rhs,
            ..
        } => Some((*ty, lhs, rhs)),
        _ => None,
    });
    let (ty, lhs, rhs) = bin.expect("存在 Binary 指令");
    assert_eq!(ty, LirType::I64);
    assert!(matches!(lhs, LirOperand::Local(l) if l == "a"));
    assert!(matches!(rhs, LirOperand::Local(l) if l == "b"));
}

#[test]
fn lower_compare_cond_jump() {
    let p = lower(
        r#"
fn main() {
    let x = 5;
    if x > 10 {
        println("big");
    };
}
"#,
    );
    let f = find_fn(&p, "main");
    // 比较指令操作数类型为 i64
    let has_gt = all_stmts(f).iter().any(|s| {
        matches!(
            s,
            LirStmt::Binary {
                op: zeta_hir::HirBinaryOp::Gt,
                ty: LirType::I64,
                ..
            }
        )
    });
    assert!(has_gt, "应存在 i64 比较指令");
    // 条件跳转
    let has_cond = f
        .blocks
        .iter()
        .any(|b| matches!(&b.terminator, LirTerminator::CondJump { .. }));
    assert!(has_cond, "应存在 CondJump 终止符");
}

#[test]
fn lower_builtin_call_keeps_args() {
    let p = lower(
        r#"
fn main() {
    println("Hello");
    println(42);
    println(true);
}
"#,
    );
    let f = find_fn(&p, "main");
    let calls: Vec<&String> = all_stmts(f)
        .iter()
        .filter_map(|s| match s {
            LirStmt::Call { callee, args, .. } if callee == "println" => {
                Some(args.first().expect("args"))
            }
            _ => None,
        })
        .collect();
    assert_eq!(calls.len(), 3, "println 调用应保留参数");
}

#[test]
fn lower_region_annotations() {
    let p = lower(
        r#"
fn main() {
    region 'r {
        let x = 5 in 'r;
        transfer x out of 'r;
    };
}
"#,
    );
    let f = find_fn(&p, "main");
    let stmts = all_stmts(f);
    assert!(stmts
        .iter()
        .any(|s| matches!(s, LirStmt::RegionEnter { .. })));
    assert!(stmts
        .iter()
        .any(|s| matches!(s, LirStmt::AllocInRegion { .. })));
    assert!(stmts.iter().any(|s| matches!(s, LirStmt::Transfer { .. })));
    assert!(stmts
        .iter()
        .any(|s| matches!(s, LirStmt::RegionExit { .. })));
}

#[test]
fn lower_type_inference() {
    let p = lower(
        r#"
fn main() {
    let s = "hi";
    let b = 1 < 2;
    println(s);
    println(b);
}
"#,
    );
    let f = find_fn(&p, "main");
    let locals = &f.locals;
    let ty = |n: &str| locals.iter().find(|(x, _)| x == n).map(|(_, t)| *t);
    assert_eq!(ty("s"), Some(LirType::Str));
    assert_eq!(ty("b"), Some(LirType::Bool));
}

#[test]
fn lower_nested_expr_flat() {
    let p = lower("fn f(a: i64) -> i64 { (a + 1) * 2 }");
    let f = find_fn(&p, "f");
    // 嵌套表达式拆平为多条三地址指令
    let binned = all_stmts(f)
        .iter()
        .filter(|s| matches!(s, LirStmt::Binary { .. }))
        .count();
    assert!(binned >= 2, "嵌套运算应拆为 ≥2 条 Binary，实际 {binned}");
}
