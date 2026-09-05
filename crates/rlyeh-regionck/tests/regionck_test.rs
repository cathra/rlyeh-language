//! rlyeh-regionck 集成测试：区域嵌套、归属与 transfer 合法性。

use rlyeh_regionck::{RegionChecker, RegionError};
use rlyeh_typecheck::typecheck_source;

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
    // 区域结束后再 transfer（区域名已不可见；x 须在块外定义，
    // 因 region 块内变量块外不可见——typecheck 块级作用域）
    let src = r#"
fn main() -> u32 {
    let x = 1;
    region 'a {
        let y = 2 in 'a;
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

/// 重复 transfer：structured 输出应含错误码 `[RC005]` 与相关位置标注
/// `= note: 首次 transfer 位于此`（SH-P2-6 L2 遗留缺口补齐）。
#[test]
fn test_double_transfer_related_span() {
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
    let e = errs
        .iter()
        .find(|e| matches!(e, RegionError::DoubleTransfer { .. }))
        .expect("expected DoubleTransfer");
    let out = e.render_structured(0);
    assert!(out.contains("[RC005]"), "code missing: {out}");
    assert!(
        out.contains("= note: 首次 transfer 位于此"),
        "related note missing: {out}"
    );
}

/// 对象归属其它区域：structured 输出应回指该区域声明处（`= note: ... 分配于区域`）。
#[test]
fn test_invalid_transfer_related_span() {
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
    let e = errs
        .iter()
        .find(|e| matches!(e, RegionError::InvalidTransfer { .. }))
        .expect("expected InvalidTransfer");
    let out = e.render_structured(0);
    assert!(out.contains("[RC002]"), "code missing: {out}");
    assert!(
        out.contains("= note:") && out.contains("分配于区域"),
        "related note missing: {out}"
    );
}

/// 从内层区域 transfer 外层区域对象：structured 输出应回指外层区域声明处
/// （`= note: ... 声明于此`）。
#[test]
fn test_outer_region_transfer_related_span() {
    let src = r#"
fn main() -> u32 {
    region 'outer {
        let x = 1 in 'outer;
        region 'inner {
            transfer x out of 'outer;
            x
        }
    }
}
"#;
    let errs = check(src).unwrap_err();
    let e = errs
        .iter()
        .find(|e| matches!(e, RegionError::OuterRegionTransfer { .. }))
        .expect("expected OuterRegionTransfer");
    let out = e.render_structured(0);
    assert!(out.contains("[RC007]"), "code missing: {out}");
    assert!(
        out.contains("= note:") && out.contains("声明于此"),
        "related note missing: {out}"
    );
}
