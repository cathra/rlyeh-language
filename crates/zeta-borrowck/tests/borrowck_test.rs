//! zeta-borrowck 集成测试：L0 静态所有权验证。
//!
//! 覆盖：use-after-move（transfer 后使用）、不可变绑定赋值、
//! 区域块值传递例外、作用域与 shadowing、防御性错误变体 Display。

use zeta_borrowck::{BorrowChecker, BorrowError};
use zeta_typecheck::typecheck_source;

fn check(src: &str) -> Result<(), Vec<BorrowError>> {
    let hir = typecheck_source(src).expect("typecheck should succeed");
    let mut checker = BorrowChecker::new();
    checker.check_program(&hir)
}

// ---------- 合法代码（不应误报） ----------

#[test]
fn test_simple_use_ok() {
    let src = r#"
fn main() -> u32 {
    let x = 5;
    x + 1
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_mutable_assign_ok() {
    let src = r#"
fn main() -> u32 {
    let mut x = 0;
    x += 1;
    x = x * 2;
    x
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_transfer_as_block_value_ok() {
    // transfer 后 x 作为区域块尾值 = 所有权转出，合法
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        x
    }
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_transfer_via_let_ok() {
    // transfer 作为 let 初始化，moved 获得所有权，随后使用 moved
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        let moved = transfer x out of 'a;
        moved
    }
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_shadowing_after_transfer_ok() {
    // shadowing 覆盖旧绑定，新 x 是可用的
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        let x = 2;
        x
    }
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_nested_region_transfer_ok() {
    // 内层 transfer 内层对象 + 外层 transfer 外层对象，均以块值传递
    let src = r#"
fn main() -> u32 {
    region 'outer {
        let a = 1 in 'outer;
        region 'inner {
            let b = 2 in 'inner;
            transfer b out of 'inner;
            b
        };
        transfer a out of 'outer;
        a
    }
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_param_read_ok() {
    let src = r#"
fn add(a: u32, b: u32) -> u32 {
    a + b
}

fn main() -> u32 {
    add(1, 2)
}
"#;
    assert!(check(src).is_ok());
}

// ---------- 非法代码（应报错） ----------

#[test]
fn test_use_after_transfer_rejected() {
    // P005 遗留用例：`transfer data` 后 `data.use()` 是 use-after-move
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        let y = x;
        y
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::UseAfterTransfer { .. })),
        "expected UseAfterTransfer, got {errs:?}"
    );
}

#[test]
fn test_assign_to_immutable_rejected() {
    let src = r#"
fn main() -> u32 {
    let x = 1;
    x = 2;
    x
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::AssignToImmutable { .. })),
        "expected AssignToImmutable, got {errs:?}"
    );
}

#[test]
fn test_compound_assign_to_immutable_rejected() {
    // `+=` 也需要可变绑定
    let src = r#"
fn main() -> u32 {
    let x = 1;
    x += 1;
    x
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::AssignToImmutable { .. })),
        "expected AssignToImmutable, got {errs:?}"
    );
}

#[test]
fn test_transfer_then_compute_rejected() {
    // transfer 后参与计算（读取值）→ use-after-move
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        x + 1
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::UseAfterTransfer { .. })),
        "expected UseAfterTransfer, got {errs:?}"
    );
}

#[test]
fn test_assign_after_transfer_rejected() {
    // 对已转移变量赋值 → use-after-move（优先于不可变赋值）
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        x = 5;
        x
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::UseAfterTransfer { .. })),
        "expected UseAfterTransfer, got {errs:?}"
    );
}

#[test]
fn test_transfer_twice_rejected() {
    // 重复 transfer 在 borrowck 视角同样是 use-after-move
    //（regionck 侧另行报 DoubleTransfer）
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        transfer x out of 'a;
        x
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::UseAfterTransfer { .. })),
        "expected UseAfterTransfer, got {errs:?}"
    );
}

#[test]
fn test_use_in_nested_block_after_transfer_rejected() {
    // 块表达式包裹的 x 不是直接块值传递 → 报错（保守规则）
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        { x }
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::UseAfterTransfer { .. })),
        "expected UseAfterTransfer, got {errs:?}"
    );
}

#[test]
fn test_use_after_transfer_in_while_rejected() {
    // 循环体内使用已转移变量
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer x out of 'a;
        let mut i = 0;
        while i < 3 {
            i += x;
        };
        i
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, BorrowError::UseAfterTransfer { .. })),
        "expected UseAfterTransfer, got {errs:?}"
    );
}

// ---------- 防御性变体 Display ----------

#[test]
fn test_defensive_error_variants_display() {
    let errs = [
        BorrowError::BorrowConflict {
            detail: "cannot borrow `data` as mutable more than once at a time".into(),
            line: 0,
            col: 0,
        },
        BorrowError::MoveWhileBorrowed {
            detail: "cannot transfer `x` while borrowed".into(),
            line: 0,
            col: 0,
        },
    ];
    for e in &errs {
        assert!(!e.to_string().is_empty(), "empty display for {e:?}");
    }
    assert_eq!(
        errs[0].to_string(),
        "borrow conflict: cannot borrow `data` as mutable more than once at a time"
    );
    assert_eq!(
        errs[1].to_string(),
        "cannot move out of a borrowed value: cannot transfer `x` while borrowed"
    );
}
