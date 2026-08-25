//! zeta-regionck 集成测试：区域嵌套、归属与 transfer 合法性。

use zeta_regionck::{RegionChecker, RegionError};
use zeta_typecheck::typecheck_source;

fn check(src: &str) -> Result<(), Vec<RegionError>> {
    let hir = typecheck_source(src).expect("typecheck should succeed");
    let mut checker = RegionChecker::new();
    checker.check_program(&hir)
}

#[test]
fn test_nested_regions_ok() {
    let src = r#"
fn main() -> u32 {
    region 'outer {
        let a = 1 in 'outer;
        region 'inner {
            let b = 2 in 'inner;
            b
        }
    }
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_anonymous_regions_ok() {
    let src = r#"
fn main() -> u32 {
    region {
        let x = 1;
        region {
            let y = 2;
            y
        }
    }
}
"#;
    assert!(check(src).is_ok());
}

#[test]
fn test_region_not_found() {
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'b;
        x
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, RegionError::RegionNotFound { .. })),
        "expected RegionNotFound, got {errs:?}"
    );
}

#[test]
fn test_valid_transfer_ok() {
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
fn test_transfer_wrong_region() {
    // x 分配在 'b，却 transfer out of 'a
    let src = r#"
fn main() -> u32 {
    region 'a {
        region 'b {
            let x = 1 in 'b;
            transfer x out of 'a;
            x
        }
    }
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, RegionError::InvalidTransfer { .. })),
        "expected InvalidTransfer, got {errs:?}"
    );
}

#[test]
fn test_transfer_twice() {
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
            .any(|e| matches!(e, RegionError::DoubleTransfer { .. })),
        "expected DoubleTransfer, got {errs:?}"
    );
}

#[test]
fn test_transfer_out_of_scope_region() {
    // 区域结束后再 transfer（区域已不可见）
    let src = r#"
fn main() -> u32 {
    region 'a {
        let x = 1 in 'a;
    };
    transfer x out of 'a;
    0
}
"#;
    let errs = check(src).unwrap_err();
    assert!(
        errs.iter()
            .any(|e| matches!(e, RegionError::RegionNotFound { .. })),
        "expected RegionNotFound, got {errs:?}"
    );
}
