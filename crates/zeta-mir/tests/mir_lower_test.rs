//! HIR → MIR lowering 集成测试：
//! 从源码经 typecheck 得到 HIR，再验证 CFG 结构与指令序列。

use zeta_mir::lower::lower_program;
use zeta_mir::{MirProgram, MirStmt, MirTerminator, MirValue};
use zeta_typecheck::typecheck_source;

fn lower(src: &str) -> MirProgram {
    let hir = typecheck_source(src).expect("typecheck 应成功");
    lower_program(&hir)
}

fn first_fn(p: &MirProgram) -> &zeta_mir::MirFunction {
    &p.functions[0]
}

#[test]
fn test_lower_arithmetic() {
    let m = lower("fn main() -> u32 { 1 + 2 * 3 }");
    let f = first_fn(&m);
    assert_eq!(f.blocks.len(), 1);
    assert_eq!(f.blocks[0].stmts.len(), 1);
    // `_t0 = 1 + (2 * 3)`（乘法树内联）
    match &f.blocks[0].stmts[0] {
        MirStmt::Assign { target, value } => {
            assert_eq!(target, "_t0");
            assert!(matches!(
                value,
                MirValue::Binary {
                    op: zeta_hir::HirBinaryOp::Add,
                    ..
                }
            ));
        }
        other => panic!("expected Assign, got {other:?}"),
    }
    assert_eq!(
        f.blocks[0].terminator,
        Some(MirTerminator::Return(Some("_t0".into())))
    );
}

#[test]
fn test_lower_if_cfg() {
    let m = lower("fn main() -> u32 { if 1 < 2 { 10 } else { 20 } }");
    let f = first_fn(&m);
    // entry(cond) / then / else / merge
    assert_eq!(f.blocks.len(), 4);

    // entry：条件求值 + CondJump（then=1，有 else 时 otherwise=3）
    assert!(matches!(
        f.blocks[0].terminator,
        Some(MirTerminator::CondJump { ref then, ref otherwise, .. }) if *then == 1 && *otherwise == 3
    ));
    // then 分支：写入结果变量 + Jump(merge=2)
    assert_eq!(f.blocks[1].terminator, Some(MirTerminator::Jump(2)));
    // else 分支：Jump(merge=2)
    assert_eq!(f.blocks[3].terminator, Some(MirTerminator::Jump(2)));
    // merge：返回结果变量
    assert!(matches!(
        f.blocks[2].terminator,
        Some(MirTerminator::Return(Some(_)))
    ));
}

#[test]
fn test_lower_if_no_else() {
    let m = lower("fn main() -> u32 { if 1 < 2 { 10 }; 0 }");
    let f = first_fn(&m);
    // entry(cond) / then / merge（无 else 分支块）
    assert_eq!(f.blocks.len(), 3);
    assert!(matches!(
        f.blocks[0].terminator,
        Some(MirTerminator::CondJump { ref then, ref otherwise, .. }) if *then == 1 && *otherwise == 2 // 直接跳 merge
    ));
    assert_eq!(f.blocks[1].terminator, Some(MirTerminator::Jump(2)));
    assert!(matches!(
        f.blocks[2].terminator,
        Some(MirTerminator::Return(_))
    ));
}

#[test]
fn test_lower_while_cfg() {
    let m = lower(
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
    let f = first_fn(&m);
    // entry / head / body / after
    assert_eq!(f.blocks.len(), 4);

    // entry：初始化 i，跳到 head
    assert_eq!(f.blocks[0].terminator, Some(MirTerminator::Jump(1)));
    // head：条件求值 + CondJump（真→body，假→after）
    assert!(matches!(
        f.blocks[1].terminator,
        Some(MirTerminator::CondJump { ref then, ref otherwise, .. }) if *then == 2 && *otherwise == 3
    ));
    // body：`i += 1` 展开为 `i = i + 1`，块尾回跳 head
    assert_eq!(f.blocks[2].terminator, Some(MirTerminator::Jump(1)));
    assert!(f.blocks[2].stmts.iter().any(|s| matches!(
        s,
        MirStmt::Assign {
            value: MirValue::Binary {
                op: zeta_hir::HirBinaryOp::Add,
                ..
            },
            ..
        }
    )));
    // after：返回 i
    assert!(matches!(
        f.blocks[3].terminator,
        Some(MirTerminator::Return(Some(ref v))) if v == "i"
    ));
}

#[test]
fn test_lower_loop_break() {
    let m = lower(
        r#"
fn main() -> u32 {
    loop {
        break;
    };
    0
}
"#,
    );
    let f = first_fn(&m);
    // entry / body / after
    assert_eq!(f.blocks.len(), 3);
    assert_eq!(f.blocks[0].terminator, Some(MirTerminator::Jump(1)));
    // body：break → Jump(after)
    assert_eq!(f.blocks[1].terminator, Some(MirTerminator::Jump(2)));
    assert!(matches!(
        f.blocks[2].terminator,
        Some(MirTerminator::Return(_))
    ));
}

#[test]
fn test_lower_break_continue_targets() {
    let m = lower(
        r#"
fn main() -> u32 {
    while true {
        continue;
        break;
    };
    0
}
"#,
    );
    let f = first_fn(&m);
    // entry / head / body / after
    assert_eq!(f.blocks.len(), 4);
    // body 内：continue → Jump(head=1)；break 不可达（lower 阶段跳过）
    assert_eq!(f.blocks[2].terminator, Some(MirTerminator::Jump(1)));
}

#[test]
fn test_lower_region_transfer() {
    let m = lower(
        r#"
fn main() -> u32 {
    region 'r {
        let x = 5 in 'r;
        transfer x out of 'r;
        x
    }
}
"#,
    );
    let f = first_fn(&m);
    let stmts = &f.blocks[0].stmts;
    let kinds: Vec<&str> = stmts
        .iter()
        .map(|s| match s {
            MirStmt::RegionEnter { .. } => "region_enter",
            MirStmt::RegionExit => "region_exit",
            MirStmt::AllocInRegion { .. } => "alloc",
            MirStmt::Transfer { .. } => "transfer",
            MirStmt::Assign { .. } => "assign",
            MirStmt::Call { .. } => "call",
            MirStmt::Alloc { .. } => "alloc_obj",
            MirStmt::FieldGet { .. } => "field_get",
            MirStmt::FieldSet { .. } => "field_set",
            MirStmt::IndexGet { .. } => "index_get",
            MirStmt::IndexSet { .. } => "index_set",
        })
        .collect();
    // 顺序：RegionEnter → `_t0 = 5`（init 求值）→ AllocInRegion(_t0)
    // → `x = _t0`（Let 绑定）→ Transfer(x) → RegionExit
    assert_eq!(
        kinds,
        vec![
            "region_enter",
            "assign",
            "alloc",
            "assign",
            "transfer",
            "region_exit"
        ]
    );
    // transfer 指令携带区域名
    assert!(stmts.iter().any(|s| matches!(
        s,
        MirStmt::Transfer { region, .. } if region == "r"
    )));
}

#[test]
fn test_lower_range_check_expansion() {
    let m = lower("fn main() -> bool { let x = 5; x in 0..<10 }");
    let f = first_fn(&m);
    // 区间判断展开为 And(Ge(x, 0), Lt(x, 10)) 的运算树
    let has_ge = f.blocks[0].stmts.iter().any(|s| {
        matches!(
            s,
            MirStmt::Assign {
                value: MirValue::Binary {
                    op: zeta_hir::HirBinaryOp::And,
                    ..
                },
                ..
            }
        )
    });
    assert!(has_ge, "range check 应展开为 And 运算树");
}

#[test]
fn test_lower_set_lookup_expansion() {
    let m = lower("fn main() -> bool { let x = 3; x in (1, 3, 5) }");
    let f = first_fn(&m);
    let has_or = f.blocks[0].stmts.iter().any(|s| {
        matches!(
            s,
            MirStmt::Assign {
                value: MirValue::Binary {
                    op: zeta_hir::HirBinaryOp::Or,
                    ..
                },
                ..
            }
        )
    });
    assert!(has_or, "集合成员判断应展开为 Or 链");
}

#[test]
fn test_lower_return_early_stops() {
    let m = lower(
        r#"
fn main() -> u32 {
    let x = 1;
    return 5;
    let y = 2;
    x
}
"#,
    );
    let f = first_fn(&m);
    // return 后的语句不降低
    assert!(matches!(
        f.blocks[0].terminator,
        Some(MirTerminator::Return(Some(_)))
    ));
    let has_y = f.blocks[0]
        .stmts
        .iter()
        .any(|s| matches!(s, MirStmt::Assign { target, .. } if target == "y"));
    assert!(!has_y, "return 后的 let y 不应被降低");
}

#[test]
fn test_lower_call_and_args() {
    let m = lower(
        r#"
fn add(a: u32, b: u32) -> u32 { a + b }
fn main() -> u32 {
    add(1, 2)
}
"#,
    );
    // add 被降低为独立函数（add 在前、main 在后）
    assert_eq!(m.functions.len(), 2);
    assert_eq!(m.functions[0].name, "add");
    let main = &m.functions[1];
    assert_eq!(main.name, "main");
    assert!(main.blocks[0].stmts.iter().any(|s| matches!(
        s,
        MirStmt::Call { callee, args, .. } if callee == "add" && args.len() == 2
    )));
}
