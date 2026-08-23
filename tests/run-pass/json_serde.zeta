// L2 serde：json::stringify / json::parse（turbofish 泛型实参）
// - `json::stringify(v)`：i64 / bool / String / &str / 数组 / struct（字段序=定义序）/
//   Vec（元素类型须确定，`vec![...]` 字面量需注解）/ 字符串转义（" \ 换行 制表）/
//   HashMap（L2f：i64/String 键、值递归，`map![...]` 需注解定型；键序确定性）
// - `json::parse::<T>(s)`：i64 / bool / String（转义还原）/ HashMap（L2g：
//   i64/String 键 + 标量值 i64/bool/String，`{"k":v,...}` 格式，空 {} → 空 map）
// - turbofish 语法：`::<T>`（Colon+Colon+Lt 三 token 前瞻；嵌套泛型 `>>` 拆分）
// 输出与 json_serde.out 精确对比

struct Point { x: i64, y: i64 }
struct Line { a: Point, b: Point, name: String }

fn main() {
    // 标量
    println(json::stringify(42));                          // 42
    println(json::stringify(true));                        // true
    println(json::stringify(false));                       // false
    // String / &str（含引号）
    println(json::stringify("hi"));                        // "hi"
    let s = String::from("a\"b");
    println(json::stringify(s));                           // "a\"b"
    // 数组（静态展开）
    println(json::stringify([1, 2, 3]));                   // [1,2,3]
    // struct（字段序 = 定义序）
    let p = Point { x: 10, y: 20 };
    println(json::stringify(p));                           // {"x":10,"y":20}
    // 嵌套 struct
    let l = Line { a: Point { x: 1, y: 2 }, b: Point { x: 3, y: 4 }, name: String::from("L") };
    println(json::stringify(l));                           // {"a":{"x":1,"y":2},"b":{"x":3,"y":4},"name":"L"}
    // Vec（`vec![...]` 绑定后元素类型为 Infer，需注解定型；空 Vec → []）
    let v: Vec<i64> = vec![1, 2, 3];
    println(json::stringify(v));                           // [1,2,3]
    let ve: Vec<i64> = vec![];
    println(json::stringify(ve));                          // []
    // 字面量 &str
    println(json::stringify("lit"));                       // "lit"

    // parse：i64 / bool / String（转义还原）。
    // MVP 语义：直接返回 T（非法输入给默认值：0 / false / 空串），非 Result 包装
    println(json::parse::<i64>("42"));                   // 42
    println(json::parse::<bool>("true"));                // true
    let back = json::parse::<String>("\"a\\\"b\"");
    println(back.len());                                 // 3（"a"b）

    // L2f stringify：HashMap（键序确定性；i64 / String 键 + 标量/嵌套值）
    let m1: HashMap<i64, i64> = map![1 => 10];
    println(json::stringify(m1));                        // {"1":10}
    let m2: HashMap<i64, i64> = map![3 => 30, 1 => 10, 2 => 20];
    println(json::stringify(m2));                        // {"1":10,"2":20,"3":30}
    let m3: HashMap<String, bool> = map![String::from("ok") => true];
    println(json::stringify(m3));                        // {"ok":true}
    let m4: HashMap<String, Vec<i64>> = map![String::from("xs") => vec![1, 2]];
    println(json::stringify(m4));                        // {"xs":[1,2]}
    let m5: HashMap<i64, i64> = map![];
    println(json::stringify(m5));                        // {}

    // L2g parse：HashMap（i64 / String 键 + 标量值；空 {} → 空 map）
    let p1 = json::parse::<HashMap<i64, i64>>("{\"1\":10,\"2\":20}");
    println(p1.len());                                   // 2
    match p1.get(1) {
        Option::Some(v1) => println(v1),                 // 10
        Option::None => println(0),
    }
    match p1.get(9) {
        Option::Some(v2) => println(v2),
        Option::None => println(0),                      // 0（缺失键）
    }
    let p2 = json::parse::<HashMap<i64, i64>>("{}");
    println(p2.len());                                   // 0（空对象）
    let p3 = json::parse::<HashMap<String, bool>>("{\"ok\":true,\"no\":false}");
    println(p3.len());                                   // 2
    match p3.get(String::from("ok")) {
        Option::Some(v3) => println(v3),                 // true
        Option::None => println(false),
    }
    let p4 = json::parse::<HashMap<String, String>>("{\"a\":\"x\",\"b\":\"y\"}");
    match p4.get(String::from("b")) {
        Option::Some(v4) => println(v4),                 // y
        Option::None => println("?"),
    }

    // 往返：stringify → parse
    let rt = json::parse::<HashMap<i64, i64>>(json::stringify(m2));
    println(rt.len());                                   // 3
    match rt.get(3) {
        Option::Some(v5) => println(v5),                 // 30
        Option::None => println(0),
    }
}
