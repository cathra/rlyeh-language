// SH-P1-1 A4：函数级 `where` 子句的约束必须**强制校验**——与被约束的内联
// bound（`fn f<T: Bound>(..)`）同一套诊断，不可静默放行。
// expect: does not implement protocol `Speak`
protocol Speak {
    fn speak(&self) -> i64;
}

struct Rock { id: i64 }

fn loud<T>(x: T) -> i64 where T: Speak { x.speak() }

fn main() {
    let r = Rock { id: 2 };
    println(loud(r));
}
