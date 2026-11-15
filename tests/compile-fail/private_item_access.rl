// 工作流 C（P2 可见性）验收：跨模块访问非 pub 函数应报 PrivateItem。
// flag: --visibility=error --no-std
module m {
    fn secret() -> i64 { 1 }
}

fn main() {
    println(m::secret());
}
// expect: m::secret
// expect: not accessible
