//! 优化 passes 测试：常量折叠、死代码消除、基础内联。

use zeta_mir::lower::lower_program;
use zeta_mir::passes::{constant_fold, dead_code_elimination, inline_small_functions, optimize};
use zeta_mir::{MirProgram, MirStmt, MirTerminator, MirValue};
use zeta_typecheck::typecheck_source;

fn lower(src: &str) -> MirProgram {
    let hir = typecheck_source(src).expect("typecheck 应成功");
    lower_program(&hir)
}

#[test]
fn test_const_fold_arithmetic() {
    let mut m = lower("fn main() -> u32 { 1 + 2 * 3 }");
    constant_fold(&mut m);
    let f = &m.functions[0];
    // `_t0 = 7`（常量折叠后）
    assert_eq!(
        f.blocks[0].stmts[0],
        MirStmt::Assign {
            target: "_t0".into(),
            value: MirValue::Int(7),
        }
    );
}

#[test]
fn test_const_fold_div_by_zero_preserved() {
    let mut m = lower("fn main() -> u32 { 1 / 0 }");
    constant_fold(&mut m);
    let f = &m.functions[0];
    // 除零不折叠（保留运算）
    assert!(matches!(
        f.blocks[0].stmts[0],
        MirStmt::Assign {
            value: MirValue::Binary { .. },
            ..
        }
    ));
}

#[test]
fn test_const_fold_cond_and_dce() {
    let mut m = lower("fn main() -> u32 { if 1 < 2 { 10 } else { 20 } }");
    constant_fold(&mut m);
    dead_code_elimination(&mut m);
    let f = &m.functions[0];
    // 条件折叠为 Jump，else 分支不可达被删除：entry / then / merge = 3 块
    assert_eq!(f.blocks.len(), 3);
    assert_eq!(f.blocks[0].terminator, Some(MirTerminator::Jump(1)));
    assert!(matches!(
        f.blocks[1].terminator,
        Some(MirTerminator::Jump(2))
    ));
    assert!(matches!(
        f.blocks[2].terminator,
        Some(MirTerminator::Return(_))
    ));
}

#[test]
fn test_dce_keeps_live_assignments() {
    let mut m = lower("fn main() -> u32 { let a = 1; let b = a + 1; b }");
    dead_code_elimination(&mut m);
    let f = &m.functions[0];
    // a、b 均被后续使用，应保留
    let targets: Vec<&str> = f.blocks[0]
        .stmts
        .iter()
        .filter_map(|s| match s {
            MirStmt::Assign { target, .. } => Some(target.as_str()),
            _ => None,
        })
        .collect();
    assert!(targets.contains(&"a"));
    assert!(targets.contains(&"b"));
    assert_eq!(
        f.blocks[0].terminator,
        Some(MirTerminator::Return(Some("b".into())))
    );
}

#[test]
fn test_inline_small_function() {
    let mut m = lower(
        r#"
fn add1(x: u32) -> u32 { x + 1 }
fn main() -> u32 {
    add1(41)
}
"#,
    );
    inline_small_functions(&mut m);
    let f = &m.functions[1]; // main
                             // Call 被替换为内联体
    let has_call = f.blocks[0]
        .stmts
        .iter()
        .any(|s| matches!(s, MirStmt::Call { .. }));
    assert!(!has_call, "add1(41) 应被内联，不再有 Call");
    // 内联体：`_i0 = _t1 + 1`，返回值绑定到调用点目标 `_t0`
    assert!(f.blocks[0].stmts.iter().any(|s| matches!(
        s,
        MirStmt::Assign {
            value: MirValue::Binary {
                op: zeta_hir::HirBinaryOp::Add,
                ..
            },
            ..
        }
    )));
}

#[test]
fn test_inline_skips_recursive() {
    let mut m = lower(
        r#"
fn fact(n: u32) -> u32 {
    if n == 0 { 1 } else { n * fact(n - 1) }
}
fn main() -> u32 {
    fact(5)
}
"#,
    );
    inline_small_functions(&mut m);
    // fact 不是单块函数（if 生成多块）→ 不可内联，调用保留
    let f = &m.functions[1];
    assert!(f.blocks[0]
        .stmts
        .iter()
        .any(|s| matches!(s, MirStmt::Call { callee, .. } if callee == "fact")));
}

#[test]
fn test_optimize_pipeline_while() {
    let mut m = lower(
        r#"
fn main() -> u32 {
    let mut i = 0;
    while i < 10 {
        i += 1;
    };
    i
}
"#,
    );
    optimize(&mut m);
    let f = &m.functions[0];
    // 流水线不破坏循环 CFG（entry/head/body/after 仍 4 块）
    assert_eq!(f.blocks.len(), 4);
    assert!(matches!(
        f.blocks[1].terminator,
        Some(MirTerminator::CondJump { .. })
    ));
}
