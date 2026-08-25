// 期望编译失败：闭包值被调用时，闭包体内的未定义变量必须报错。
// 惰性检查仅适用于从未被调用的闭包（见 run-pass/closure_lazy.zeta）。
// expect: undefined variable `missing`
fn main() {
    let f = |x: i64| x + missing;
    println(f(1));
}
