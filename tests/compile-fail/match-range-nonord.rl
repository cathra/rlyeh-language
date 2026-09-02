// SH-P0-7 P-M2：范围模式复用比较链的 `check_comparison`——**排序仅放行数值与
// 字符**，bool 须显式报错而非生成无意义的 `icmp sge`。
// expect: does not support ordering
fn main() {
    let b = true;
    match b {
        false...true => println(1),
        _ => println(0),
    }
}
