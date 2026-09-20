//! # rlyeh-typecheck
//!
//! Rlyeh 语言类型检查器（MVP）。
//!
//! 将 rlyeh-ast 抽象语法树检查并展开为 rlyeh-hir 高级中间表示，
//! 核心语义：
//!
//! - **比较链**（`0 < x < 10`）：方向检查（正向 / 反向 / 混合报错），
//!   展开为低层比较运算。反向链 `0 > x > 10` 表示区间外。
//! - **`in` 集合成员判断**（`x in (0..<10)`）：集合内范围元素展开为
//!   离散成员（`x == 0 || x == 1 || ... || x == 9`），要求上下界为
//!   编译期整数常量；成员较多时保留为集合查找。
//! - **`in` 裸范围区间判断**（`x in 0..<10`）：展开为区间比较
//!   （`x >= 0 && x < 10`），端点运行时求值。
//!
//! 入口见 [`typecheck`]（AST → HIR）与 [`typecheck_source`]（源码 → HIR）。

#![warn(missing_docs)]
#![warn(unsafe_code)]

mod check_expr;
mod check_item;
mod check_stmt;
mod comparison;
mod context;
mod error;
mod in_expr;
mod types;
mod warning;

pub use error::TypeError;
pub use types::{FnSignature, Mutability, StructDef, Type};
pub use context::VisibilityMode;
pub use warning::{Warning, WarningKind};

use rlyeh_hir::HirProgram;
use rlyeh_lexer::Span;

/// 合成源码位置：用于编译器生成项（单态化实例等），其无对应源文件位置。
/// 这类项的 `span.start` 为 0，< `prelude_len`，故在「用户代码过滤」时被排除。
pub(crate) const DUMMY_SPAN: Span = Span {
    start: 0,
    end: 0,
    line: 0,
    col: 0,
};

pub use crate::check_item::{collect_fn_signatures, typecheck, typecheck_with_region_hints};

/// 便捷函数：解析源码并类型检查，返回 HIR。
pub fn typecheck_source(source: &str) -> Result<HirProgram, TypeError> {
    typecheck_source_with_region_hints(source, &Default::default(), 0, VisibilityMode::Off)
}

/// 便捷函数：解析源码并类型检查，注入 L3 PGO 回灌提示（区域名 → 推荐初始容量）。
pub fn typecheck_source_with_region_hints(
    source: &str,
    region_hints: &std::collections::HashMap<String, usize>,
    prelude_len: usize,
    visibility: VisibilityMode,
) -> Result<HirProgram, TypeError> {
    let mut program = rlyeh_parser::parse(source).map_err(|e| TypeError::Unsupported {
        what: format!("语法错误: {e}"),
        span: e.span(),
    })?;
    // S1c：async/await 状态机 desugar（parse 后、typecheck 前，AST → AST）
    rlyeh_desugar::desugar_program(&mut program).map_err(|e| TypeError::Unsupported {
        what: e.to_string(),
        span: e.span(),
    })?;
    typecheck_source_with_warnings(source, region_hints, prelude_len, visibility)
        .map(|(hir, _warnings)| hir)
}

/// 便捷函数：解析源码并类型检查，返回 HIR 与收集到的建议性警告（非致命）。
///
/// driver 编译路径使用本入口，以便把警告（如冗余 `*` 解引用）带正确行偏移打印给用户。
pub fn typecheck_source_with_warnings(
    source: &str,
    region_hints: &std::collections::HashMap<String, usize>,
    prelude_len: usize,
    visibility: VisibilityMode,
) -> Result<(HirProgram, Vec<Warning>), TypeError> {
    let mut program = rlyeh_parser::parse(source).map_err(|e| TypeError::Unsupported {
        what: format!("语法错误: {e}"),
        span: e.span(),
    })?;
    // S1c：async/await 状态机 desugar（parse 后、typecheck 前，AST → AST）
    rlyeh_desugar::desugar_program(&mut program).map_err(|e| TypeError::Unsupported {
        what: e.to_string(),
        span: e.span(),
    })?;
    let (hir, warnings) = typecheck_with_region_hints(&program, region_hints, prelude_len, visibility)?;
    Ok((hir, warnings))
}


#[cfg(test)]
mod tests {
    use super::*;

    fn warns(src: &str) -> Vec<Warning> {
        let (_, warnings) = typecheck_source_with_warnings(src, &Default::default(), 0, VisibilityMode::Off)
            .expect("类型检查应成功");
        warnings
    }

    #[test]
    fn redundant_deref_on_reference_warns() {
        let src = r#"
        struct Point { x: i64, y: i64 }
        fn get_x(p: &Point) -> i64 {
            (*p).x
        }
        fn call_method(p: &Point) -> i64 {
            (*p).x
        }
        fn index_arr(a: &[i64]) -> i64 {
            (*a)[0]
        }
        "#;
        let warnings = warns(src);
        assert!(
            warnings
                .iter()
                .any(|w| matches!(w.kind, WarningKind::RedundantDeref { .. })),
            "对引用的冗余 `*` 解引用应触发 W001 警告，实际：{warnings:?}"
        );
    }

    #[test]
    fn auto_deref_no_warning() {
        let src = r#"
        struct Point { x: i64, y: i64 }
        fn get_x(p: &Point) -> i64 {
            p.x
        }
        "#;
        let warnings = warns(src);
        assert!(
            warnings.is_empty(),
            "自动解引用写法不应触发警告，实际：{warnings:?}"
        );
    }

    #[test]
    fn raw_pointer_deref_no_warning() {
        // 裸指针解引用仍需 `*`，不应告警
        let src = r#"
        unsafe fn read(p: *const i64) -> i64 {
            *p
        }
        "#;
        let warnings = warns(src);
        assert!(
            warnings.is_empty(),
            "裸指针 `*` 解引用不应触发警告，实际：{warnings:?}"
        );
    }

    /// B-2（P0'）：源码是否能通过类型检查（coerce 成功路径应返回 Ok）。
    fn ok(src: &str) -> bool {
        typecheck_source_with_warnings(src, &Default::default(), 0, VisibilityMode::Off).is_ok()
    }

    #[test]
    fn auto_deref_coerce_let_binding() {
        // `let v: i64 = r;` 其中 `r: &i64` 应自动解引用取值（Copy 类型 &T→T）
        let src = r#"
        fn f() -> i64 {
            let x: i64 = 10;
            let r: &i64 = &x;
            let v: i64 = r;
            v
        }
        "#;
        assert!(ok(src), "带标注 let 绑定处 &i64→i64 应自动解引用强制");
    }

    #[test]
    fn auto_deref_coerce_assignment() {
        // `v = r;` 其中 `v: i64`、`r: &i64` 应自动解引用取值
        let src = r#"
        fn f() -> i64 {
            let x: i64 = 10;
            let r: &i64 = &x;
            let mut v: i64 = 0;
            v = r;
            v
        }
        "#;
        assert!(ok(src), "赋值处 &i64→i64 应自动解引用强制");
    }

    #[test]
    fn auto_deref_coerce_call_arg() {
        // `g(r)` 其中 `r: &i64`、形参 `a: i64` 应自动解引用取值
        let src = r#"
        fn g(a: i64) -> i64 { a }
        fn f() -> i64 {
            let x: i64 = 10;
            let r: &i64 = &x;
            g(r)
        }
        "#;
        assert!(ok(src), "调用实参处 &i64→i64 应自动解引用强制");
    }

    #[test]
    fn auto_deref_coerce_rejects_non_copy() {
        // 非 Copy 类型 `&Big → Big` 不应自动解引用，应报类型错误
        let src = r#"
        struct Big { a: i64, b: i64 }
        fn take(b: Big) -> i64 { b.a }
        fn bad(r: &Big) -> i64 {
            take(r)
        }
        "#;
        assert!(
            !ok(src),
            "非 Copy 类型 &Big→Big 不应自动解引用，应保持原类型错误"
        );
    }

    #[test]
    fn gc_module_maps_ref_param_to_gc() {
        // B-6（P3）：gc 模块内 `&i64` 形参映射为 `Gc<i64>`，可用 `Gc::new` 实参调用
        let src = r#"
        #[memory(gc)]
        module g {
            pub fn f(x: &i64) -> i64 { *x }
        }
        fn main() -> i64 {
            g::f(Gc::new(10))
        }
        "#;
        assert!(ok(src), "gc 模块内 &i64 形参应映射为 Gc<i64>，可用 Gc::new 实参调用");
    }

    #[test]
    fn gc_module_rejects_plain_value_for_ref_param() {
        // B-6（P3）：gc 模块内 `&i64` 形参已映射为 `Gc<i64>`，传裸 `i64` 应报类型错误
        let src = r#"
        #[memory(gc)]
        module g {
            pub fn f(x: &i64) -> i64 { *x }
        }
        fn main() -> i64 {
            g::f(10)
        }
        "#;
        assert!(
            !ok(src),
            "gc 模块内 &i64 形参已映射为 Gc<i64>，传裸 i64 应报类型错误"
        );
    }
}
