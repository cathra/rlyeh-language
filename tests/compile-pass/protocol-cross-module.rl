// 跨模块协议引用语法：协议父约束、struct/impl 一致性列表均支持 :: 限定名（已知限制 #2 闭合）。
// flag: --no-std
module m {
    pub protocol Base {
        fn base(&self) -> i64;
    }
}
pub protocol Derived: m::Base {
    fn derived(&self) -> i64;
}
struct S { v: i64 }
impl S: m::Base {
    fn base(&self) -> i64 { self.v }
}
fn main() -> i64 { 0 }
