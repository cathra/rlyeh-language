//! `&str` 引用切片集成测试（G2 块二：`String::as_str()` 只读借用视图）。
//!
//! 覆盖：as_str 借用、`&str` 参数（含 `&String` 兼容传参）、返回 `&str`、
//! `&str` 字节索引、`String::from(&str)` 深拷贝、`&str` 切片（substring 拷贝）、
//! 链式调用、`&str` 数据独立性。
//!
//! 表示模型：`&str` 运行时 = 指向 String 对象的瘦指针（G1 聚合引用）；
//! 方法/索引/切片按 String impl 解析（typecheck 归一化）；`String::from(&str)`
//! 运行期读 data/len 槽深拷贝。
//!
//! 需要系统 clang（与 string_concat_test.rs 相同）。

use std::path::PathBuf;

use rlyeh_driver::run_source_file;

/// 独立临时项目目录，避免并行测试互相覆盖。
fn temp_project() -> PathBuf {
    static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("rlyeh-strref-{}-{seq}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("创建临时目录失败");
    dir
}

/// 运行内联源码（自动注入 core.rl），返回程序输出。
fn run(src: &str) -> String {
    let dir = temp_project();
    let file = dir.join("main.rl");
    std::fs::write(&file, src).expect("写入 main.rl 失败");
    let out = run_source_file(&file).expect("&str 集成测试编译运行失败");
    let _ = std::fs::remove_dir_all(&dir);
    out
}

/// as_str 基本借用：len 视图与原串一致。
#[test]
fn as_str_view_len() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello");
    let r = s.as_str();
    println(r.len());                 // 5
    println(s.len());                 // 5（原串不变）
}
"#,
    );
    assert_eq!(out, "5\n5\n");
}

/// `&str` 函数参数：`&String` 与 `&str` 均可传（compatible_with 互视）。
#[test]
fn str_param_accepts_both() {
    let out = run(
        r#"
fn len_of(s: &str) -> i64 {
    s.len()
}

fn main() {
    let s = String::from("rlyeh");
    let r = s.as_str();
    println(len_of(r));               // &str 直接传
    println(len_of(&s));              // &String 兼容传参
}
"#,
    );
    assert_eq!(out, "4\n4\n");
}

/// 返回 `&str` 并继续链式使用。
#[test]
fn str_return_chained() {
    let out = run(
        r#"
fn id(s: &str) -> &str {
    s
}

fn main() {
    let s = String::from("chain");
    let r = id(s.as_str());
    println(r.len());                 // 5
    println(s.as_str().len());        // 链式 as_str
}
"#,
    );
    assert_eq!(out, "5\n5\n");
}

/// `&str` 字节索引：r[i] 经 data 槽取字节。
#[test]
fn str_index_byte() {
    let out = run(
        r#"
fn main() {
    let s = String::from("abc");
    let r = s.as_str();
    println(r[1]);                    // 98 ('b')
    println(s.as_str()[0]);           // 97 ('a')
}
"#,
    );
    assert_eq!(out, "98\n97\n");
}

/// `String::from(&str)` 深拷贝：副本修改不影响原串。
#[test]
fn string_from_str_deep_copy() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello");
    let r = s.as_str();
    let t = String::from(r);          // &str → 深拷贝
    t.push_str(String::from(" world"));
    println(t.len());                 // 11
    println(s.len());                 // 5（原串独立）
    println(s);
}
"#,
    );
    assert_eq!(out, "11\n5\nhello\n");
}

/// `&str` 切片：substring 返回拷贝（API 保持 String，不破坏现有调用者）。
#[test]
fn str_slice_substring() {
    let out = run(
        r#"
fn main() {
    let s = String::from("hello world");
    let r = s.as_str();
    let sub = r.substring(0, 5);
    println(sub);                     // hello（拷贝）
    println(r.len());                 // 11（原视图不变）
}
"#,
    );
    assert_eq!(out, "hello\n11\n");
}

/// `&str` 内容相等：String::from(r) == 原串。
#[test]
fn str_content_equals() {
    let out = run(
        r#"
fn main() {
    let s = String::from("rlyeh");
    let r = s.as_str();
    println(String::from(r) == s);    // true
    println(r == s);                  // 视图与 String 相等（宽松规则）
}
"#,
    );
    assert_eq!(out, "true\ntrue\n");
}

/// `&str` 经函数往返后仍可用（视图数据独立性）。
#[test]
fn str_view_survives_roundtrip() {
    let out = run(
        r#"
fn first_two(s: &str) -> String {
    String::from(s.substring(0, 2))
}

fn main() {
    let s = String::from("roundtrip");
    let r = s.as_str();
    println(first_two(r));            // ro
    println(s.len());                 // 9（原串未被视图操作影响）
}
"#,
    );
    assert_eq!(out, "ro\n9\n");
}

/// `&str` 变量赋值与再次借用。
#[test]
fn str_reborrow() {
    let out = run(
        r#"
fn main() {
    let s = String::from("reborrow");
    let r1 = s.as_str();
    let r2 = r1;                      // &str 拷贝（瘦指针复制）
    println(r1.len());
    println(r2.len());
}
"#,
    );
    assert_eq!(out, "8\n8\n");
}
