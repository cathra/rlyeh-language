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
fn test_protocol_and_impl() {
    let program = parse_ok(
        "protocol Shape { fn area(&self) -> f64; } impl Shape for Point { fn area(&self) -> f64 { 0.0 } }",
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
fn test_protocol_assoc_type_decl() {
    // V3-A1：协议内关联类型声明 `type Item;`（无默认）与 `type Item = i64;`（默认具体化）
    let program = parse_ok(
        "protocol IterA { type Item; fn next(&mut self) -> Option<Self::Item>; } \
         protocol IterB { type Item = i64; fn next(&mut self) -> Option<Self::Item>; }",
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
fn test_memory_gc_module_attr() {
    // B-6：模块属性 `#[memory(gc)]` 应被解析并写入 `AstModDecl.memory`。
    let program = parse_ok("#[memory(gc)] module graph { struct Node { next: &Node } }");
    let AstItem::ModDecl(m) = &program.items[0] else {
        panic!("expected mod decl");
    };
    assert_eq!(m.memory.as_deref(), Some("gc"));
    assert_eq!(m.name, "graph");
}

#[test]
fn test_region_param_suffix() {
    // B-4：`struct/enum/trait Foo 'a { ... }` region 参数化后缀语法应被解析并写入 `region_param`。
    let program = parse_ok(
        "struct Wrapper 'a { inner: &'a i64 } \
         enum E 'b { V(&'b i64), W } \
         trait T 'c { fn get(&self) -> &'c i64; }",
    );
    let AstItem::StructDecl(s) = &program.items[0] else {
        panic!("expected struct");
    };
    assert_eq!(s.region_param.as_deref(), Some("a"));
    assert!(s.generics.is_empty());

    let AstItem::EnumDecl(e) = &program.items[1] else {
        panic!("expected enum");
    };
    assert_eq!(e.region_param.as_deref(), Some("b"));
    assert_eq!(e.variants.len(), 2);

    let AstItem::TraitDecl(t) = &program.items[2] else {
        panic!("expected trait");
    };
    assert_eq!(t.region_param.as_deref(), Some("c"));
}

#[test]
fn test_region_param_absent_and_generic_form() {
    // 无 `'a` 后缀 / 既有 `<'a>` 写法：region_param 均为 None
    // （`<'a>` 仍按既有语义解析后丢弃，不进入 generics）。
    let program = parse_ok("struct Plain { x: i64 } struct Generic<'a> { r: &'a i64 }");
    let AstItem::StructDecl(p) = &program.items[0] else {
        panic!();
    };
    assert_eq!(p.region_param, None);
    let AstItem::StructDecl(g) = &program.items[1] else {
        panic!();
    };
    assert_eq!(g.region_param, None);
    assert!(g.generics.is_empty());
}

#[test]
fn test_protocol_and_impl_keywords() {
    // `protocol` 声明协议；`impl T: P` 一致性、`impl T` 固有实现。
    let program = parse_ok(
        "protocol Draw { fn draw(&self); } \
         struct Sq { s: i64 } \
         impl Sq: Draw { fn draw(&self) {} } \
         impl Sq { fn new(s: i64) -> Sq { Sq { s: s } } }",
    );
    let AstItem::TraitDecl(t) = &program.items[0] else {
        panic!("expected protocol decl");
    };
    assert_eq!(t.name, "Draw");
    assert_eq!(t.methods.len(), 1);

    let AstItem::ImplBlock(conf) = &program.items[2] else {
        panic!("expected conformance impl");
    };
    assert_eq!(conf.trait_name.as_deref(), Some("Draw"));
    assert_eq!(conf.type_name, "Sq");

    let AstItem::ImplBlock(inherent) = &program.items[3] else {
        panic!("expected inherent impl");
    };
    assert_eq!(inherent.trait_name, None);
    assert_eq!(inherent.type_name, "Sq");
}

#[test]
fn test_trait_extension_are_plain_identifiers() {
    // `trait` / `extension` 已从语法中彻底移除，成为普通标识符（可作变量名）。
    let program = parse_ok(
        "fn f() -> i64 { let trait = 1; let extension = 2; trait + extension }",
    );
    assert_eq!(program.items.len(), 1);
}

#[test]
fn test_struct_conformance_and_inline_members() {
    // PC-1：声明点一致性 `struct C: P` + 类型体内联方法。
    let program = parse_ok("struct Sq: Area { s: i64, fn area(&self) -> i64 { 0 } }");
    let AstItem::StructDecl(s) = &program.items[0] else {
        panic!("expected struct");
    };
    assert_eq!(s.conformances.len(), 1);
    assert_eq!(s.conformances[0].0, "Area");
    assert_eq!(s.fields.len(), 1);
    assert_eq!(s.methods.len(), 1);
}

#[test]
fn test_enum_conformance_and_inline_members() {
    // PC-1：枚举声明点一致性 + 内联方法（变体与方法混排）。
    let program = parse_ok("enum Kind: Greeter { A, B, fn hello(&self) -> i64 { 100 } }");
    let AstItem::EnumDecl(e) = &program.items[0] else {
        panic!("expected enum");
    };
    assert_eq!(e.conformances[0].0, "Greeter");
    assert_eq!(e.variants.len(), 2);
    assert_eq!(e.methods.len(), 1);
}

#[test]
fn test_protocol_inheritance() {
    // PC-4：`protocol A: B, C { .. }` 父协议列表。
    let program = parse_ok("protocol Loud: Named, Aged { fn shout(&self); }");
    let AstItem::TraitDecl(t) = &program.items[0] else {
        panic!("expected protocol");
    };
    assert_eq!(t.name, "Loud");
    assert_eq!(t.supertraits.len(), 2);
    assert_eq!(t.supertraits[0].0, "Named");
    assert_eq!(t.supertraits[1].0, "Aged");
}

#[test]
fn test_multi_conformance_list() {
    // PC-3：多协议声明点一致性 `struct C: A, B`。
    let program = parse_ok("struct Sq: Area, Named { s: i64 }");
    let AstItem::StructDecl(s) = &program.items[0] else {
        panic!("expected struct");
    };
    assert_eq!(s.conformances.len(), 2);
    assert_eq!(s.conformances[0].0, "Area");
    assert_eq!(s.conformances[1].0, "Named");
}

#[test]
fn test_struct_inline_inherent_method() {
    // PC-1：无协议的内联方法（desugar 归一为固有 impl）。
    let program = parse_ok("struct Pt { x: i64, fn norm2(&self) -> i64 { 0 } }");
    let AstItem::StructDecl(s) = &program.items[0] else {
        panic!("expected struct");
    };
    assert!(s.conformances.is_empty());
    assert_eq!(s.methods.len(), 1);
}

#[test]
fn test_impl_new_syntax_conformance() {
    // PC-8：`impl T: P` 归一为 trait impl；`impl T` 为固有；旧语序 `impl P for T` 仍兼容。
    let program = parse_ok(
        "impl Sq: Area { fn area(&self) -> i64 { 0 } } \
         impl Sq { fn new() -> Sq { Sq { s: 0 } } } \
         impl Area for Sq { fn area(&self) -> i64 { 0 } }",
    );
    let AstItem::ImplBlock(c) = &program.items[0] else {
        panic!("expected impl");
    };
    assert_eq!(c.trait_name.as_deref(), Some("Area"));
    assert_eq!(c.type_name, "Sq");

    let AstItem::ImplBlock(inherent) = &program.items[1] else {
        panic!("expected inherent impl");
    };
    assert_eq!(inherent.trait_name, None);
    assert_eq!(inherent.type_name, "Sq");

    let AstItem::ImplBlock(legacy) = &program.items[2] else {
        panic!("expected legacy impl");
    };
    assert_eq!(legacy.trait_name.as_deref(), Some("Area"));
    assert_eq!(legacy.type_name, "Sq");
}

#[test]
fn test_impl_new_syntax_generic() {
    // PC-8：`impl<T> Pair<T>: Wrap<T>`（泛型 + 协议泛型实参）与固有 `impl<T> Pair<T>`。
    let program = parse_ok(
        "impl<T> Pair<T>: Wrap<T> { fn wrap(&self) -> i64 { 0 } } \
         impl<T> Pair<T> { fn empty() -> i64 { 0 } }",
    );
    let AstItem::ImplBlock(i) = &program.items[0] else {
        panic!("expected impl");
    };
    assert_eq!(i.trait_name.as_deref(), Some("Wrap"));
    assert_eq!(i.type_name, "Pair");
    assert_eq!(i.generics.len(), 1);
    assert_eq!(i.trait_type_args.len(), 1);

    let AstItem::ImplBlock(inherent) = &program.items[1] else {
        panic!("expected inherent impl");
    };
    assert_eq!(inherent.trait_name, None);
    assert_eq!(inherent.type_name, "Pair");
}

#[test]
fn test_impl_multi_conformance_list() {
    // PC-9：`impl Sq: Area, Named`——首个协议入 `trait_name`，其余入 `extra_traits`，
    // 由 desugar 按协议成员名裁决拆分为多个 impl 块。
    let program = parse_ok("impl Sq: Area, Named { fn area(&self) -> i64 { 0 } }");
    let AstItem::ImplBlock(i) = &program.items[0] else {
        panic!("expected impl");
    };
    assert_eq!(i.trait_name.as_deref(), Some("Area"));
    assert_eq!(i.type_name, "Sq");
    assert_eq!(i.extra_traits.len(), 1);
    assert_eq!(i.extra_traits[0].0, "Named");
    assert!(i.extra_traits[0].1.is_empty());
}

#[test]
fn test_impl_multi_conformance_generic() {
    // PC-9：泛型 + 多协议 + 协议泛型实参。
    let program = parse_ok("impl<T> Pair<T>: Wrap<T>, Show { fn wrap(&self) -> i64 { 0 } }");
    let AstItem::ImplBlock(i) = &program.items[0] else {
        panic!("expected impl");
    };
    assert_eq!(i.trait_name.as_deref(), Some("Wrap"));
    assert_eq!(i.type_name, "Pair");
    assert_eq!(i.generics.len(), 1);
    assert_eq!(i.trait_type_args.len(), 1);
    assert_eq!(i.extra_traits.len(), 1);
    assert_eq!(i.extra_traits[0].0, "Show");
}

#[test]
fn test_use_group_and_pub() {
    // 0.2.0-B-1/B-2：组导入与 `pub use` 重导出的 AST 形状。
    // `pub import a::{b, c as d};` → path=["a"], group=[{b,None},{c,Some(d)}], is_pub=true
    let program = parse_ok("pub import a::{b, c as d};");
    let AstItem::UseDecl(u) = &program.items[0] else {
        panic!();
    };
    assert!(u.is_pub);
    assert_eq!(u.path, &["a".to_string()]);
    assert_eq!(
        u.group,
        Some(vec![
            AstUseMember {
                name: "b".into(),
                alias: None,
                nested: None
            },
            AstUseMember {
                name: "c".into(),
                alias: Some("d".into()),
                nested: None
            },
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

#[test]
fn test_use_nested_group() {
    // 0.2.0-B 嵌套组导入：`import a::{b::{x, y}, c}`。
    // 叶子名 x/y/c 入作用域；`b` 仅作子组前缀，不在组内注册。
    let program = parse_ok("import a::{b::{x, y}, c};");
    let AstItem::UseDecl(u) = &program.items[0] else {
        panic!();
    };
    assert_eq!(u.path, &["a".to_string()]);
    assert_eq!(
        u.group,
        Some(vec![
            AstUseMember {
                name: "b".into(),
                alias: None,
                nested: Some(vec![
                    AstUseMember {
                        name: "x".into(),
                        alias: None,
                        nested: None
                    },
                    AstUseMember {
                        name: "y".into(),
                        alias: None,
                        nested: None
                    },
                ])
            },
            AstUseMember {
                name: "c".into(),
                alias: None,
                nested: None
            },
        ])
    );
}
