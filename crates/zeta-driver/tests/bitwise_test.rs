//! 位运算全链路集成测试（文件入口 API，自动注入 `zeta-std/zeta/core.zeta`）。
//!
//! 覆盖：`&`/`|`/`^` 基础运算（常量折叠 + 变量两条路径）、`<<`/`>>` 移位
//! （含负数算术右移）、与算术/比较的优先级混合、字节打包/解包
//! （sockaddr_in 端口/IP 场景）、掩码应用、循环累积移位。
//!
//! 需要系统 clang（与 std_test.rs / agg_test.rs 相同）。

use std::path::PathBuf;

use zeta_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("zeta-bitwise-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.zeta），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.zeta");
    std::fs::write(&file, src).expect("写入 main.zeta 失败");
    let out = run_source_file(&file).expect("位运算测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// 基础位运算：`&`/`|`/`^`（常量折叠 + 变量两条路径）。
#[test]
fn bitwise_basic() {
    let out = run(
        r#"
fn main() {
    // 常量折叠路径（const_fold 直接求值）
    println(5 & 3);   // 1
    println(5 | 3);   // 7
    println(5 ^ 3);   // 6
    // 变量路径（真实 LLVM 指令）
    let a = 12;
    let b = 10;
    println(a & b);   // 8  (1100 & 1010)
    println(a | b);   // 14 (1100 | 1010)
    println(a ^ b);   // 6  (1100 ^ 1010)
    // 混合：常量 + 变量
    println(a & 0xFF);   // 12
    println(b | 0x80);   // 138
    println(a ^ 0xFF);   // 243
}
"#,
    );
    assert_eq!(out, "1\n7\n6\n8\n14\n6\n12\n138\n243\n");
}

/// 移位：`<<`/`>>`（右移为算术右移，负数符号保持）。
#[test]
fn shift_ops() {
    let out = run(
        r#"
fn main() {
    println(1 << 4);    // 16
    println(255 >> 4);  // 15
    let x = 1;
    println(x << 10);   // 1024
    println(7 << 2);    // 28（常量折叠）
    // 算术右移：负数保持符号（-8 >> 1 = -4）
    let neg = -8;
    println(neg >> 2);  // -2
    println(-16 >> 3);  // -2
    println(-1 >> 4);   // -1（全 1 右移仍为 -1）
    println(0x1F << 3); // 248
}
"#,
    );
    assert_eq!(out, "16\n15\n1024\n28\n-2\n-2\n-1\n248\n");
}

/// 位运算与算术 / 比较混合（依赖 parser 优先级表：`*` > `+` > `<<` > `&` > `^` > `|`）。
#[test]
fn mixed_priority() {
    let out = run(
        r#"
fn main() {
    let x = 6;
    println((x & 3) + 1);   // 3（括号内先算 &）
    println(1 | 2 * 2);     // 5（* 优先于 |：1 | 4）
    println((1 << 4) / 4);  // 4
    println(2 ^ 3 + 1);     // 6（+ 优先于 ^：2 ^ 4）
    // 位运算结果参与比较（Zeta 优先级：COMPARE > BIT_AND，
    // 裸写 `x & 3 == 2` 会解析为 `x & (3==2)` 即 bool 参与位运算而报错，需括号）
    println((x & 3) == 2);  // 1（(6 & 3) == 2）
}
"#,
    );
    assert_eq!(out, "3\n5\n4\n6\ntrue\n");
}

/// 字节打包 / 解包（sockaddr_in 端口 + IPv4 场景）。
#[test]
fn byte_pack_unpack() {
    let out = run(
        r#"
fn main() {
    // 端口 8080 = 0x1F90：高字节 0x1F=31，低字节 0x90=144
    let port = 8080;
    let hi = port >> 8;
    let lo = port & 0xFF;
    println(hi);             // 31
    println(lo);             // 144
    println((hi << 8) | lo); // 8080（打包往返）
    // 32 位 IPv4：192.168.1.10 → 0xC0A8010A
    let ip = (192 << 24) | (168 << 16) | (1 << 8) | 10;
    println(ip);                 // 3232235786
    println(ip & 0xFF);          // 10（第 4 字节）
    println((ip >> 8) & 0xFF);   // 1（第 3 字节）
    println((ip >> 16) & 0xFF);  // 168（第 2 字节）
    println((ip >> 24) & 0xFF);  // 192（第 1 字节）
    // 往返：解包再打包
    println(((ip >> 24) & 0xFF) << 24 | ((ip >> 16) & 0xFF) << 16
        | ((ip >> 8) & 0xFF) << 8 | (ip & 0xFF));  // 3232235786
}
"#,
    );
    assert_eq!(out, "31\n144\n8080\n3232235786\n10\n1\n168\n192\n3232235786\n");
}

/// 掩码应用：奇偶判断 / 取低 4 位 / 置位 / 翻转。
#[test]
fn bitmask_patterns() {
    let out = run(
        r#"
fn main() {
    let x = 17;
    println(x & 1);   // 1（奇数）
    let y = 20;
    println(y & 1);   // 0（偶数）
    println(x & 15);  // 1（取低 4 位：17 & 0xF）
    // 置位：flags 或上某一位
    let mut flags = 0;
    flags = flags | 4;
    println(flags);       // 4
    println(flags | 2);   // 6
    // 异或翻转低 8 位
    println(x ^ 0xFF);    // 238（17 ^ 255）
    // 清位：与反掩码（~ 未实现，用异或构造）
    let mask = 0x0F;
    println(x & (255 ^ mask));  // 16（清除低 4 位：17 & 0xF0）
}
"#,
    );
    assert_eq!(out, "1\n0\n1\n4\n6\n238\n16\n");
}

/// 循环累积移位：字节流拼接（网络协议报文构造场景）。
#[test]
fn shift_accumulate_loop() {
    let out = run(
        r#"
fn main() {
    // 模拟拼接 3 字节 [1, 2, 3] → 0x010203
    let mut acc = 0;
    let mut i = 0;
    while i < 3 {
        acc = (acc << 8) | (i + 1);
        i = i + 1;
    }
    println(acc);       // 66051
    println(acc >> 16); // 1（还原第 1 字节）
    println((acc >> 8) & 0xFF);  // 2（第 2 字节）
    println(acc & 0xFF);         // 3（第 3 字节）
    // 奇偶过滤循环
    let mut sum = 0;
    let mut j = 1;
    while j <= 8 {
        if (j & 1) == 1 {
            sum = sum + j;
        }
        j = j + 1;
    }
    println(sum);       // 1+3+5+7 = 16
}
"#,
    );
    assert_eq!(out, "66051\n1\n2\n3\n16\n");
}
