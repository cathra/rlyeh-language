//! 确定性伪模糊测试（stable 工具链可用）。
//!
//! 等价于 cargo-fuzz 的验收目标"任意输入不 panic"：用种子 PRNG 生成
//! 大量随机的 UTF-8 文本并喂给解析器，断言解析过程（含词法失败路径）
//! 从不 panic。`libfuzzer` 目标见 `crates/rlyeh-parser/fuzz/`。

use rlyeh_parser::Parser;

/// xorshift64 种子随机数生成器（确定性，无需外部依赖）
struct XorShift(u64);

impl XorShift {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// Rlyeh 语法中常见的字节片段（覆盖关键字、运算符、字面量、标点）
const FRAGMENTS: &[&str] = &[
    "fn", "let", "mut", "if", "else", "while", "loop", "for", "in", "match", "region", "'r",
    "transfer", "out of", "actor", "struct", "enum", "protocol", "impl", "pub", "async", "unsafe",
    "import", "module", "const", "static", "and", "not", "return", "break", "continue", "send", "move",
    "ref", "self", "Self", "as", "0", "42", "3.14", "true", "false", "\"str\"", "'c'", "9am",
    "6pm", "x", "y", "data", "u32", "f64", "Result", "Vec", "String", "+", "-", "*", "/", "%", "=",
    "+=", "-=", "*=", "/=", "==", "!=", "<", "<=", ">", ">=", "&&", "||", "!", "&", "&mut", "|",
    "^", "<<", ">>", "::", "->", "=>", "..<", "...", "<..", "..", ".", ",", ";", ":", "(", ")",
    "[", "]", "{", "}", "_", "?", " ", "\n", "\t",
];

/// 生成一个随机的"token 拼接"文本：`n_frag` 个片段拼接而成
fn gen_source(rng: &mut XorShift, n_frag: usize) -> String {
    let mut src = String::new();
    for _ in 0..n_frag {
        src.push_str(FRAGMENTS[rng.below(FRAGMENTS.len())]);
    }
    src
}

/// 核心不变量：任意输入（词法成功或失败）解析都不应 panic
fn parse_no_panic(src: &str) {
    let mut parser = match Parser::new(src) {
        Ok(p) => p,
        Err(_) => return, // 词法失败路径：不是解析器缺陷
    };
    let _ = parser.parse_program();
}

#[test]
fn fuzz_no_panic_random_garbage() {
    let mut rng = XorShift(0x5EED_CAFE_2026);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..20_000 {
            let n = 2 + rng.below(63);
            parse_no_panic(&gen_source(&mut rng, n));
        }
    }));
    assert!(result.is_ok(), "解析器在随机输入下发生 panic");
}

#[test]
fn fuzz_no_panic_valid_fragments() {
    // 用语法片段做突变拼接，覆盖更"像合法代码"的随机输入
    let mut rng = XorShift(0xDEAD_BEEF_0001);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..10_000 {
            let n = 4 + rng.below(20);
            parse_no_panic(&gen_source(&mut rng, n));
        }
    }));
    assert!(result.is_ok(), "解析器在片段拼接输入下发生 panic");
}

#[test]
fn fuzz_no_panic_empty_and_whitespace() {
    for src in [
        "",
        " ",
        "\n",
        "\t\n",
        "   \n  \t  ",
        "{}",
        ";;;",
        "(((((())))))",
    ] {
        parse_no_panic(src);
    }
}
