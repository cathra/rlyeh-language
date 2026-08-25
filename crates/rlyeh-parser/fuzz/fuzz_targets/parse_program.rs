//! 模糊测试目标：任意 UTF-8 输入不应使解析器 panic。
//!
//! 运行方式（需要 nightly + cargo-fuzz）：
//! ```text
//! cargo +nightly install cargo-fuzz
//! cargo +nightly fuzz run parse_program
//! ```

#![no_main]

use libfuzzer_sys::fuzz_target;
use rlyeh_parser::Parser;

fuzz_target!(|data: &[u8]| {
    let Ok(source) = std::str::from_utf8(data) else {
        return; // 非 UTF-8 输入直接跳过
    };
    let mut parser = match Parser::new(source) {
        Ok(p) => p,
        Err(_) => return, // 词法错误不算解析器缺陷
    };
    // 解析器在任何输入下都不得 panic
    let _ = parser.parse_program();
});
