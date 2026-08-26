// W 收尾边界：递归 async 深度（受控中等深度，验证栈使用与逻辑正确）
async fn count(n: i64) -> i64 {
    if n == 0 {
        return 0;
    }
    let r = count(n - 1).await;
    r + 1
}
fn main() {
    let mut f = count(1000);
    println(block_on(&mut f));
}
