//! P005 Transfer 语义集成测试：
//! 嵌套方向检查（OuterRegionTransfer）、无法判定归属的 transfer（PartialTransfer）、
//! 防御性错误变体（CannotTransferReference / UnsizedTransfer）。

use rlyeh_regionck::{RegionChecker, RegionError};
use rlyeh_typecheck::typecheck_source;

fn check(src: &str) -> Result<(), Vec<RegionError>> {
    let hir = typecheck_source(src).expect("typecheck should succeed");
    let mut checker = RegionChecker::new();
    checker.check_program(&hir)
}

#[test]
fn test_transfer_inner_object_inside_inner_region_ok() {
    // 内层 transfer 内层对象：合法
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
fn test_transfer_outer_from_inner_rejected() {
    // 从内层区域转移外层区域对象：P005 嵌套方向检查必须报错
    let src = r#"
fn main() -> u32 {
    region 'outer {
        let a = 1 in 'outer;
        region 'inner {
            transfer a out of 'outer;
            a
        }
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, RegionError::OuterRegionTransfer { .. })),
        "expected OuterRegionTransfer, got {errs:?}"
    );
}

#[test]
fn test_transfer_outer_after_inner_region_ok() {
    // 内层区域结束后，在外层直接作用域 transfer 外层对象：合法
    let src = r#"
fn main() -> u32 {
    region 'outer {
        let a = 1 in 'outer;
        region 'inner {
            let b = 2 in 'inner;
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
fn test_transfer_non_variable_rejected() {
    // transfer 复合表达式（无法静态判定归属区域）：PartialTransfer
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
        transfer (x + 1) out of 'a;
        x
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, RegionError::PartialTransfer { .. })),
        "expected PartialTransfer, got {errs:?}"
    );
}

#[test]
fn test_transfer_call_result_rejected() {
    // transfer 调用结果（临时值不分配在区域内）：PartialTransfer
    let src = r#"
fn helper() -> u32 { 0 }

fn main() -> u32 {
    region 'a {
        transfer helper() out of 'a;
        0
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, RegionError::PartialTransfer { .. })),
        "expected PartialTransfer, got {errs:?}"
    );
}

#[test]
fn test_defensive_error_variants_display() {
    // 防御性变体（CannotTransferReference / UnsizedTransfer）的构造与 Display
    let errs = [
        RegionError::CannotTransferReference {
            detail: "transferring `&x` is forbidden".into(),
            line: 0,
            col: 0,
        },
        RegionError::UnsizedTransfer {
            detail: "dyn Trait has no statically known size".into(),
            line: 0,
            col: 0,
        },
    ];
    for e in &errs {
        assert!(!e.to_string().is_empty(), "empty display for {e:?}");
    }
    assert_eq!(
        errs[0].to_string(),
        "cannot transfer a reference: transferring `&x` is forbidden"
    );
    assert_eq!(
        errs[1].to_string(),
        "cannot transfer an unsized value: dyn Trait has no statically known size"
    );
}
