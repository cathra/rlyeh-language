// 阶段 Q4 验收：toml 模块（轻量 MVP）。
// - Q4a：`toml::to_string(v)` / `toml::stringify(v)`：基础标量（i64/bool/String/&str）/
//   嵌套表（struct 顶层多行 `key = value` + 嵌套字段内联表 `{...}`）/ 数组（数组字面量 /
//   Vec）/ HashMap（内联表，键带引号）序列化
// - Q4b：`toml::from_str::<T>(s)` / `toml::parse::<T>(s)`：round-trip 反序列化（对齐
//   stringify 的紧凑输出；字段序无关、缺失字段零值、未知字段忽略；嵌套 struct 为内联表；
//   Vec/HashMap 反序列化）
// MVP 签名降级：无泛型 trait 约束（`T: Serialize` / `T: Deserialize` bound 不支持），
// 退化为无 bound turbofish 形式（同 Q2 json）。
// 输出与 toml_io.out 精确对比
struct Point { x: i64, y: i64 }
struct Config { name: String, enabled: bool, scores: Vec<i64>, point: Point }

fn main() {
    // Q4a：基础标量
    println(toml::to_string(42));            // 42
    println(toml::to_string(true));          // true
    println(toml::to_string("hi"));          // "hi"
    println(toml::to_string(String::from("s")));  // "s"
    // 数组 / Vec
    let a2: [i64; 2] = [4, 5];
    println(toml::to_string(a2));            // [4,5]
    let v: Vec<i64> = vec![1, 2, 3];
    println(toml::to_string(v));             // [1,2,3]
    let vs: Vec<String> = vec!["x", "y"];
    println(toml::to_string(vs));            // ["x","y"]
    // 嵌套表：顶层多行 + 嵌套字段内联表
    let c = Config { name: String::from("z"), enabled: true, scores: vec![1, 2], point: Point { x: 7, y: 9 } };
    let s = toml::to_string(c);
    println(s);                              // name = "z"\nenabled = true\nscores = [1,2]\npoint = {x = 7,y = 9}
    // Q4b：round-trip
    let c2 = toml::from_str::<Config>(s);
    println(c2.name);                        // z
    if c2.enabled { println(1) } else { println(0) }   // 1
    println(c2.scores.len());                // 2
    println(c2.scores[0] + c2.scores[1]);    // 3
    println(c2.point.x);                     // 7
    println(c2.point.y);                     // 9
    // 字段序无关 / 缺失零值 / 嵌套内联表直接解析（紧凑格式，与 to_string 输出一致）
    let c3 = toml::from_str::<Config>("point={x=1,y=2}\nname=\"q\"");
    println(c3.name);                        // q
    println(c3.point.x + c3.point.y);        // 3
    if c3.enabled { println(1) } else { println(0) }   // 0（缺失零值）
    // 标量 parse
    println(toml::from_str::<i64>("123"));   // 123
    let bt = toml::from_str::<bool>("true");
    if bt { println(1) } else { println(0) } // 1
    println(toml::from_str::<String>("\"hi\""));  // hi
    // Vec parse
    let vp = toml::from_str::<Vec<i64>>("[1,2,3]");
    println(vp.len());                       // 3
    // HashMap（单元素顺序唯一）+ round-trip
    let m1: HashMap<String, i64> = map!["only" => 9];
    let ms = toml::to_string(m1);
    println(ms);                             // {"only" = 9}
    let m2 = toml::from_str::<HashMap<String, i64>>(ms);
    println(m2.len());                       // 1
    match m2.get("only") {
        Option::Some(x) => println(x),       // 9
        Option::None => println(0),
    }
    // 双元素 HashMap round-trip（遍历顺序不确定，只验证内容）
    let m3: HashMap<String, i64> = map!["a" => 1, "b" => 2];
    let m4 = toml::from_str::<HashMap<String, i64>>(toml::to_string(m3));
    println(m4.len());                       // 2
    match m4.get("b") {
        Option::Some(x) => println(x),       // 2
        Option::None => println(0),
    }
    // i64 键 HashMap round-trip
    let mk: HashMap<i64, i64> = map![1 => 10, 2 => 20];
    let mk2 = toml::from_str::<HashMap<i64, i64>>(toml::to_string(mk));
    println(mk2.len());                      // 2
    match mk2.get(1) {
        Option::Some(x) => println(x),       // 10
        Option::None => println(0),
    }
}
