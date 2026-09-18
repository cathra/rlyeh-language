// 阶段 Q3 / X4 验收：Display / Debug trait + Formatter + 格式化引擎接入。
// - Q3a/Q3b：std `fmt/module.rl` 定义 `trait Display { fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> }`
//   与 `trait Debug { fn fmt(&self, f: &mut Formatter) -> Result<(), FmtError> }`（X4：`Debug::fmt_debug`
//   改名 `Debug::fmt`，同名经 impl 查找按 trait 区分）；`Formatter { buf, fill, width, align }`
//   + `Formatter::new()` + `write_str`/`result`。`fmt` 返回 `Result<(), FmtError>`（写缓冲 + 错误返回）。
// - Q3b：`{}` 查 `fmt::Display::fmt`，`{:?}` 查 `fmt::Debug::fmt`；`dbg!` 用 Debug 格式。
//   内建类型（i64/bool/String/&str）走内建转换。
// 输出与 display_fmt.out 精确对比
struct Point { x: i64, y: i64 }

impl Point: Display {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::FmtError> {
        f.write_str(String::from("P(") + int_to_string(self.x) + String::from(",") + int_to_string(self.y) + String::from(")"));
        Result::Ok(())
    }
}
impl Point: Debug {
    fn fmt(&self, f: &mut Formatter) -> Result<(), fmt::FmtError> {
        f.write_str(String::from("Point{x:") + int_to_string(self.x) + String::from(",y:") + int_to_string(self.y) + String::from("}"));
        Result::Ok(())
    }
}

fn main() {
    let p = Point { x: 1, y: 2 };
    // Q3b：{} 走 Display，{:?} 走 Debug
    println!("{}", p);       // P(1,2)
    println!("{:?}", p);     // Point{x:1,y:2}
    // format! 返回 String
    let s = format!("{}", p);
    println(s);              // P(1,2)
    // dbg! 用 Debug 格式
    dbg!(p);                 // dbg: Point{x:1,y:2}
    // 内建类型行为不变
    println!("{} {}", 1, true);     // 1 true
    println!("{:?} {:?}", 2, "x");  // 2 x
}
