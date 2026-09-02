//! 表达式检查子模块：decl。
//! （由 mod.rs 二次拆分而来，保持语义等价）

use super::*;

#[test]
fn test_struct_decl() {
    let program = parse_ok("struct Point { x: f64, pub y: f64 }");
    let AstItem::StructDecl(s) = &program.items[0] else {
        panic!();
    };
    assert_eq!(s.name, "Point");
    assert_eq!(s.fields.len(), 2);
    assert!(s.fields[1].is_pub);
}

#[test]
fn test_enum_decl() {
    let program = parse_ok("enum Result2<T> { Ok(T), Err { msg: String } }");
    let AstItem::EnumDecl(e) = &program.items[0] else {
        panic!();
    };
    assert_eq!(e.name, "Result2");
    assert_eq!(e.generics.len(), 1);
    assert_eq!(e.generics[0].name, "T");
    assert!(e.generics[0].bounds.is_empty());
    assert_eq!(e.variants.len(), 2);
    assert_eq!(e.variants[0].name, "Ok");
    assert_eq!(e.variants[0].tuple_fields.len(), 1);
    assert_eq!(e.variants[1].name, "Err");
    assert_eq!(e.variants[1].struct_fields.len(), 1);
}

#[test]
fn test_trait_and_impl() {
    let program = parse_ok(
        "trait Shape { fn area(&self) -> f64; } impl Shape for Point { fn area(&self) -> f64 { 0.0 } }",
    );
    let AstItem::TraitDecl(t) = &program.items[0] else {
        panic!();
    };
    assert_eq!(t.name, "Shape");
    assert!(t.methods[0].body.is_none());
    let AstItem::ImplBlock(i) = &program.items[1] else {
        panic!();
    };
    assert_eq!(i.trait_name.as_deref(), Some("Shape"));
    assert_eq!(i.type_name, "Point");
    assert!(i.methods[0].body.is_some());
}

#[test]
fn test_trait_assoc_type_decl() {
    // V3-A1：trait 内关联类型声明 `type Item;`（无默认）与 `type Item = i64;`（默认具体化）
    let program = parse_ok(
        "trait IterA { type Item; fn next(&mut self) -> Option<Self::Item>; } \
         trait IterB { type Item = i64; fn next(&mut self) -> Option<Self::Item>; }",
    );
    let AstItem::TraitDecl(ta) = &program.items[0] else {
        panic!();
    };
    assert_eq!(ta.types, vec!["Item".to_string()]);
    assert!(ta.methods[0].body.is_none());
    let AstItem::TraitDecl(tb) = &program.items[1] else {
        panic!();
    };
    // `type Item = i64;` 记录名字 `Item`（默认具体化由 typecheck 消费）
    assert_eq!(tb.types, vec!["Item".to_string()]);
}

#[test]
fn test_use_and_mod() {
    let program = parse_ok("import foo::bar as baz; module m { fn inner() {} }");
    let AstItem::UseDecl(u) = &program.items[0] else {
        panic!();
    };
    assert_eq!(u.path, &["foo".to_string(), "bar".to_string()]);
    assert_eq!(u.alias.as_deref(), Some("baz"));
    let AstItem::ModDecl(m) = &program.items[1] else {
        panic!();
    };
    assert_eq!(m.name, "m");
    assert_eq!(m.items.len(), 1);
}

#[test]
fn test_use_group_and_pub() {
    // 0.2.0-B-1/B-2：组导入与 `pub use` 重导出的 AST 形状。
    // `pub import a::{b, c as d};` → path=["a"], group=[("b",None),("c",Some("d"))], is_pub=true
    let program = parse_ok("pub import a::{b, c as d};");
    let AstItem::UseDecl(u) = &program.items[0] else {
        panic!();
    };
    assert!(u.is_pub);
    assert_eq!(u.path, &["a".to_string()]);
    assert_eq!(
        u.group,
        Some(vec![
            ("b".to_string(), None),
            ("c".to_string(), Some("d".to_string()))
        ])
    );
    assert_eq!(u.alias, None);

    // 简单导入仍保持 path 全路径 + 可选 alias，is_pub=false
    let program2 = parse_ok("import a::b as c;");
    let AstItem::UseDecl(u2) = &program2.items[0] else {
        panic!();
    };
    assert!(!u2.is_pub);
    assert_eq!(u2.path, &["a".to_string(), "b".to_string()]);
    assert_eq!(u2.alias.as_deref(), Some("c"));
    assert_eq!(u2.group, None);
}
