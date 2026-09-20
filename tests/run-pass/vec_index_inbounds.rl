// 边界硬化回归测试：Vec 在界内索引必须正常工作（不触发越界 abort）。
// 对应 docs/rfc/tcl2-certification.md §5.2 与 docs/tasks/leaf/eh-8-panic-policy.md M4：
// 越界索引经 codegen 注入 `icmp + br + call @abort()` 确定性中止（消除 UB）；
// 本测试锁定「界内索引不被错误中止、返回值正确」这一不变量。
fn main() -> i64 {
    let v: Vec<i64> = Vec::new();
    v.push(10);
    v.push(20);
    v[0] + v[1]
}
