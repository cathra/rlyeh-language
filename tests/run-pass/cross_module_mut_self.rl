// 回归（lang-defects #10，已修复 2026-09-06）：跨模块调用 `&mut self` 方法应自动
// 借用接收者，无需显式 `(&mut x)`。此处 `c` 为不可变绑定，`c.inc()` 经自动借用
// `&mut c` 正确变更内部状态（输出 2 = 0+1+1）。若自动借用失效，typecheck 会报
// `expected (), found counter::Counter`（接收者类型泄漏）。

module counter {
    pub struct Counter {
        v: i64,
    }
    impl Counter {
        pub fn new() -> Counter {
            Counter { v: 0 }
        }
        pub fn inc(&mut self) {
            self.v = self.v + 1;
        }
        pub fn get(&self) -> i64 {
            self.v
        }
    }
}

import counter::Counter;

fn main() {
    let c = Counter { v: 0 };
    c.inc();
    c.inc();
    println(c.get());
}
