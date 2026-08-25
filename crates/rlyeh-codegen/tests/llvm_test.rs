//! rlyeh-codegen 测试：构造 LIR 程序验证 LLVM IR 文本生成。

use rlyeh_codegen::generate_llvm;
use rlyeh_lir::{HirBinaryOp, HirUnaryOp};
use rlyeh_lir::{LirBlock, LirFunction, LirOperand, LirProgram, LirStmt, LirTerminator, LirType};

fn simple_main(
    stmts: Vec<LirStmt>,
    terminator: LirTerminator,
    locals: Vec<(&str, LirType)>,
) -> LirProgram {
    LirProgram {
        functions: vec![LirFunction {
            name: "main".to_string(),
            params: vec![],
            return_type: LirType::Unit,
            locals: locals
                .into_iter()
                .map(|(n, t)| (n.to_string(), t))
                .collect(),
            blocks: vec![LirBlock { stmts, terminator }],
            is_extern: false,
            extern_ret32: false,
        }],
    }
}

#[test]
fn gen_hello_world_string() {
    let p = simple_main(
        vec![
            LirStmt::Assign {
                target: "_t0".to_string(),
                value: LirOperand::String("Hello, Rlyeh!".to_string()),
            },
            LirStmt::Call {
                target: Some("_t1".to_string()),
                callee: "println".to_string(),
                args: vec!["_t0".to_string()],
            },
        ],
        LirTerminator::Return(None),
        vec![("_t0", LirType::Str), ("_t1", LirType::Unit)],
    );
    let ll = generate_llvm(&p).expect("生成 LLVM IR");
    assert!(ll.contains("declare i32 @printf(i8*, ...)"));
    assert!(ll.contains("define i32 @main()"));
    assert!(ll.contains("c\"Hello, Rlyeh!\\00\""));
    assert!(ll.contains("c\"%s\\0A\\00\""));
    assert!(ll.contains("getelementptr"));
    assert!(ll.contains("ret i32 0"));
}

#[test]
fn gen_user_function_and_call() {
    let p = LirProgram {
        functions: vec![
            LirFunction {
                name: "add".to_string(),
                params: vec![
                    ("a".to_string(), LirType::I64),
                    ("b".to_string(), LirType::I64),
                ],
                return_type: LirType::I64,
                locals: vec![
                    ("_t0".to_string(), LirType::I64),
                    ("a".to_string(), LirType::I64),
                    ("b".to_string(), LirType::I64),
                ],
                blocks: vec![LirBlock {
                    stmts: vec![LirStmt::Binary {
                        target: "_t0".to_string(),
                        op: HirBinaryOp::Add,
                        ty: LirType::I64,
                        lhs: LirOperand::Local("a".to_string()),
                        rhs: LirOperand::Local("b".to_string()),
                    }],
                    terminator: LirTerminator::Return(Some("_t0".to_string())),
                }],
                is_extern: false,
                extern_ret32: false,
            },
            LirFunction {
                name: "main".to_string(),
                params: vec![],
                return_type: LirType::Unit,
                locals: vec![("_t1".to_string(), LirType::I64)],
                blocks: vec![LirBlock {
                    stmts: vec![LirStmt::Call {
                        target: Some("_t1".to_string()),
                        callee: "add".to_string(),
                        args: vec!["a".to_string(), "b".to_string()],
                    }],
                    terminator: LirTerminator::Return(None),
                }],
                is_extern: false,
                extern_ret32: false,
            },
        ],
    };
    let ll = generate_llvm(&p).expect("生成 LLVM IR");
    assert!(ll.contains("define i64 @add(i64 %a, i64 %b)"));
    assert!(ll.contains("= add i64"));
    assert!(ll.contains("ret i64"));
    assert!(ll.contains("call i64 @add(i64"));
}

#[test]
fn gen_compare_and_cond_jump() {
    let p = simple_main(
        vec![
            LirStmt::Assign {
                target: "x".to_string(),
                value: LirOperand::Int(5),
            },
            LirStmt::Binary {
                target: "_c".to_string(),
                op: HirBinaryOp::Gt,
                ty: LirType::I64,
                lhs: LirOperand::Local("x".to_string()),
                rhs: LirOperand::Int(10),
            },
        ],
        LirTerminator::CondJump {
            cond: "_c".to_string(),
            then: 0,
            otherwise: 0,
        },
        vec![("x", LirType::I64), ("_c", LirType::Bool)],
    );
    let ll = generate_llvm(&p).expect("生成 LLVM IR");
    // 比较用操作数类型 i64，结果存到 i1 槽
    assert!(ll.contains("icmp sgt i64"));
    assert!(ll.contains("store i1"));
    assert!(ll.contains("br i1"));
}

#[test]
fn gen_unary_not_and_bool_select() {
    let p = simple_main(
        vec![
            LirStmt::Assign {
                target: "b".to_string(),
                value: LirOperand::Bool(false),
            },
            LirStmt::Unary {
                target: "nb".to_string(),
                op: HirUnaryOp::Not,
                ty: LirType::Bool,
                operand: LirOperand::Local("b".to_string()),
            },
            LirStmt::Call {
                target: Some("_t".to_string()),
                callee: "println".to_string(),
                args: vec!["nb".to_string()],
            },
        ],
        LirTerminator::Return(None),
        vec![
            ("b", LirType::Bool),
            ("nb", LirType::Bool),
            ("_t", LirType::Unit),
        ],
    );
    let ll = generate_llvm(&p).expect("生成 LLVM IR");
    assert!(ll.contains("xor i1 true"));
    assert!(ll.contains("select i1"));
    assert!(ll.contains("c\"true\\00\""));
    assert!(ll.contains("c\"false\\00\""));
}

#[test]
fn gen_escape_string() {
    let p = simple_main(
        vec![LirStmt::Assign {
            target: "_s".to_string(),
            value: LirOperand::String("a\"b\\c\nd".to_string()),
        }],
        LirTerminator::Return(None),
        vec![("_s", LirType::Str)],
    );
    let ll = generate_llvm(&p).expect("生成 LLVM IR");
    // 输入 `a"b\c<LF>d` → 转义为 `a\22b\\c\0Ad`（`"` 用十六进制 \22，
    // 因 `\"` 会被 clang 误解析为 [1 x i8] 导致长度不匹配）
    assert!(ll.contains("a\\22b\\\\c\\0Ad"));
}

#[test]
fn gen_rejects_undefined_function() {
    let p = simple_main(
        vec![LirStmt::Call {
            target: Some("_t".to_string()),
            callee: "nope".to_string(),
            args: vec![],
        }],
        LirTerminator::Return(None),
        vec![("_t", LirType::I64)],
    );
    let err = generate_llvm(&p).unwrap_err();
    assert!(err.to_string().contains("nope"));
}
