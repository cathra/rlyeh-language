// `str` 值（字符串字面量 / 绑定字面量的变量）作为一等字符串类型：
// 方法调用（升级为 String 对象）、拼接、比较均按字符串内容语义解析。
fn main() {
    let s = "hi";

    // 1. 方法调用（Str 值接收者 → String impl，desugar 升级为 String 对象）
    println(s.len());                        // 2
    println(s.substring(0, 1));              // h
    println(s.contains(String::from("h")));  // true

    // 2. 拼接（左 / 右操作数为 Str 值 / String 均可）
    println(s + "!");                        // hi!
    let t = s + "!";                         // 拼接结果为 String
    println(t.len());                        // 3
    println("ab" + "cd");                    // abcd（两字面量）
    println(String::from("xy") + "z");       // xyz（String + 字面量）
    println(s + s);                          // hihi（Str 值 + Str 值）

    // 3. 比较（Str 值 vs 字面量 / Str 值 / String：内容比较）
    println(s == "hi");                      // true
    println(s != "hi");                      // false
    println(s < "zz");                       // true（字典序）
    println(s == "he");                      // false
    println("ab" < "b");                     // true（字面量直接比较）
    println(s == String::from("hi"));        // true（Str 值 vs String）
    println(s.substring(0, 1) == "h");       // true（子串结果比较）

    // 4. &str 视图与升级组合
    println(s.as_str().len());               // 2（Str 值经升级调用 as_str）
    let mut m = String::from(s);             // Str 值 → String（内容升级）
    m.push_str(String::from("!"));
    println(m);                              // hi!
}
