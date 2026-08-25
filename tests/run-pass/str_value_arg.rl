// Str 值实参自动升级：`String` 形参位置传入字符串字面量（或绑定字面量的
// 变量）自动构造 String 对象，无需 `String::from`（与 IIFE / 闭包值实参
// 的升级语义一致）。覆盖：实例方法 / 普通函数 / 函数指针 / 静态方法 /
// 泛型函数 / HashMap 泛型 impl 方法。
fn main() {
    // 1. 实例方法实参（push_str / contains / starts_with / ends_with / find）
    let mut m = String::from("hello");
    m.push_str(" world");
    m.push_str("!");
    println(m);                       // hello world!
    println(m.contains("world"));     // true
    println(m.contains("rlyeh"));      // false
    println(m.starts_with("hello"));  // true
    println(m.ends_with("!"));        // true
    println(m.find("world"));         // 6

    // 2. replace / split / strip_prefix / strip_suffix
    let r = m.replace("world", "rlyeh");
    println(r);                       // hello rlyeh!
    let parts = "a,b,c".split(",");
    println(parts.len());             // 3
    match m.strip_prefix("hello") {
        Option::Some(s) => println(s),  // world!
        Option::None => println("none"),
    }
    match m.strip_suffix("!") {
        Option::Some(s) => println(s),  // hello world
        Option::None => println("none"),
    }

    // 3. 绑定字面量的变量作实参（local_inits 追踪）
    let sub = "world";
    println(m.contains(sub));         // true

    // 4. 普通函数调用实参
    println(exclaim("hi"));           // hi!

    // 5. 函数指针调用实参
    let f: fn(String) -> String = exclaim;
    println(f("fn"));                 // fn!

    // 6. 静态方法实参（自定义类型）
    let w = Wrap::new("wrap");
    println(w.v);                     // wrap

    // 7. 泛型函数混合参数（String 形参升级，T 正常推断）
    println(pick_string("gen", 42));  // gen!

    // 8. HashMap 无类型注解 + 字面量键（contains_infer 路径）
    let mut map = HashMap::new();
    map.insert("key1", 1);
    map.insert("key2", 2);
    println(map.len());               // 2
    match map.get("key1") {
        Option::Some(x) => println(x),  // 1
        Option::None => println(0),
    }
}

fn exclaim(s: String) -> String {
    s + "!"
}

fn pick_string<T>(s: String, x: T) -> String {
    s + "!"
}

struct Wrap { v: String }
impl Wrap {
    fn new(s: String) -> Wrap { Wrap { v: s } }
}
