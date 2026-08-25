//! 索引访问与数组字面量集成测试。
//!
//! 覆盖：数组读取/写入、动态索引、for 遍历数组参数、字符串索引、
//! 嵌套数组链式索引、结构体字段数组、数组别名共享语义、
//! 以及 compile-fail 场景（非数组索引 / 非整数索引 / 元素类型不一致 / 空数组）。

use rlyeh_driver::run_source;

const ARRAY_READ_SUM: &str = r#"
fn main() {
    let arr = [10, 20, 30];
    let sum = arr[0] + arr[1] + arr[2];
    println(sum);
}
"#;

const ARRAY_WRITE: &str = r#"
fn main() {
    let arr = [1, 2, 3, 4];
    arr[1] = 99;
    let total = arr[0] + arr[1] + arr[2] + arr[3];
    println(total);
}
"#;

const DYNAMIC_INDEX: &str = r#"
fn main() {
    let arr = [5, 6, 7];
    let mut i = 0;
    let mut sum = 0;
    while i < 3 {
        sum += arr[i];
        i += 1;
    }
    println(sum);
}
"#;

const FOR_SUM_ARRAY_PARAM: &str = r#"
fn sum_array(arr: [i64; 4]) -> i64 {
    let mut s = 0;
    for i in 0..<4 {
        s += arr[i];
    }
    s
}

fn main() {
    let t = sum_array([1, 2, 3, 4]);
    println(t);
}
"#;

const STRING_INDEX: &str = r#"
fn main() {
    let s = "hello";
    let ch = s[1];
    println(ch);
}
"#;

const NESTED_ARRAY: &str = r#"
fn main() {
    let m = [[1, 2], [3, 4]];
    let v = m[1][0];
    println(v);
}
"#;

const STRUCT_FIELD_ARRAY: &str = r#"
struct Grid {
    rows: [i64; 3],
}

fn main() {
    let mut g = Grid { rows: [7, 8, 9] };
    g.rows[1] = 99;
    let t = g.rows[0] + g.rows[1] + g.rows[2];
    println(t);
}
"#;

const ARRAY_ALIAS_SHARED: &str = r#"
fn main() {
    // 数组变量绑定复制指针（堆共享），写入经别名可见
    let arr = [1, 2, 3];
    let b = arr;
    b[0] = 10;
    println(arr[0]);
}
"#;

#[test]
fn array_read_sum() {
    assert_eq!(run_source(ARRAY_READ_SUM).expect("运行失败"), "60\n");
}

#[test]
fn array_write() {
    assert_eq!(run_source(ARRAY_WRITE).expect("运行失败"), "107\n");
}

#[test]
fn dynamic_index() {
    assert_eq!(run_source(DYNAMIC_INDEX).expect("运行失败"), "18\n");
}

#[test]
fn for_sum_array_param() {
    assert_eq!(run_source(FOR_SUM_ARRAY_PARAM).expect("运行失败"), "10\n");
}

#[test]
fn string_index() {
    assert_eq!(run_source(STRING_INDEX).expect("运行失败"), "e\n");
}

#[test]
fn nested_array() {
    assert_eq!(run_source(NESTED_ARRAY).expect("运行失败"), "3\n");
}

#[test]
fn struct_field_array() {
    assert_eq!(run_source(STRUCT_FIELD_ARRAY).expect("运行失败"), "115\n");
}

#[test]
fn array_alias_shares_heap_data() {
    assert_eq!(run_source(ARRAY_ALIAS_SHARED).expect("运行失败"), "10\n");
}

#[test]
fn index_rejects_non_array_or_string() {
    let src = r#"
fn main() {
    let x = 5;
    let v = x[0];
    println(v);
}
"#;
    let err = run_source(src).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("array or string"),
        "意外错误: {msg}"
    );
}

#[test]
fn index_rejects_non_integer_index() {
    let src = r#"
fn main() {
    let arr = [1, 2, 3];
    let s = "x";
    let v = arr[s];
    println(v);
}
"#;
    let err = run_source(src).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("expected an integer"),
        "意外错误: {msg}"
    );
}

#[test]
fn array_lit_rejects_type_mismatch() {
    let src = r#"
fn main() {
    let arr = [1, "x"];
    println(arr[0]);
}
"#;
    let err = run_source(src).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("expected `i64`, found `string`"),
        "意外错误: {msg}"
    );
}

#[test]
fn array_lit_rejects_empty() {
    let src = r#"
fn main() {
    let arr = [];
    println(1);
}
"#;
    let err = run_source(src).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("typecheck"), "意外错误: {msg}");
}
