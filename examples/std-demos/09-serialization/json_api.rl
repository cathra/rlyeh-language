// 阶段 Q2 验收：json 泛型 API 入口（Q2a）+ 流式 writer/reader（Q2b）。
// - Q2a：`json.to_string(v)` / `json.from_str::<T>(s)`——编译器内建别名
//   （≡ `json.stringify` / `json.parse::<T>`）。MVP 无泛型 protocol 约束
//   （`T: Serialize` / `T: Deserialize` bound 未支持），签名退化为无 bound
//   turbofish 形式：序列化类型由实参推断、反序列化经 turbofish 指定。
// - Q2b：`json.to_writer(w, v)` → `w.write_all(json.stringify(v))`（返回
//   `Result<i64, io::error::IoError>`）；`json.from_reader::<T>(r)` →
//   `json.parse::<T>(r.read_to_string().unwrap())`（读失败经 `unwrap` 死循环，
//   MVP 语义，与 std `Result::unwrap` 一致）。目标 `File`；TcpStream 留待流式
//   read_all 方法化（docs/std-lib.md §9.2）。
// 输出与 json_api.out 精确对比
#[derive(Serialize, Deserialize)]
struct Point { x: i64, y: i64 }

fn main() {
    let p = Point { x: 1, y: 2 };
    // Q2a：to_string / from_str（turbofish 泛型实参）
    let s1 = json::to_string(p);
    println(s1);                          // {"x":1,"y":2}
    let q1 = json::from_str::<Point>(s1);
    println(q1.x + q1.y);                 // 3

    // 既有内建 API（stringify / parse）保持兼容
    let s2 = json::stringify(p);
    println(s2);                          // {"x":1,"y":2}
    let q2 = json::parse::<Point>(s2);
    println(q2.x + q2.y);                 // 3

    // Q2b：to_writer 写文件（返回 Result<i64, IoError> = write_all 的返回）
    let f = File::create(String::from("/tmp/rlyeh_q2.json"));
    match f {
        Ok(file) => {
            let mut file = file;
            let w = json::to_writer(&mut file, p);
            match w {
                Ok(n) => println(n),       // 13（{"x":1,"y":2} 长度）
                Err(e) => println(-1),
            }
            file.close();
        }
        Err(e) => println(-1),
    }

    // Q2b：from_reader 读文件（round-trip）
    let r = File::open(String::from("/tmp/rlyeh_q2.json"));
    match r {
        Ok(file) => {
            let mut file = file;
            let q3 = json::from_reader::<Point>(&mut file);
            println(q3.x + q3.y);          // 3
            file.close();
        }
        Err(e) => println(-1),
    }
}
