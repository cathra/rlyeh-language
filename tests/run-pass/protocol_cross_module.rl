// 跨模块协议引用：impl Point: m::Show 与 fn f<T: m::Show>（module 路径 trait 名，
// parse_conformance_list / 泛型 bound 现已支持 :: 限定名）。应输出 7。
// flag: --no-std
module m {
    pub protocol Show {
        fn show(&self) -> i64;
    }
}
struct Point { x: i64, y: i64 }
impl Point: m::Show {
    fn show(&self) -> i64 { self.x + self.y }
}
fn f<T: m::Show>(x: T) -> i64 { x.show() }
fn main() {
    let p = Point { x: 3, y: 4 };
    println(f(p));
}
