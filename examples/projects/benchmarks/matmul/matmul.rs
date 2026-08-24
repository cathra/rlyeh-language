// benchmark: 256x256 f64 矩阵乘法 —— 与 matmul.zeta 同逻辑
fn main() {
    let n = 256;
    let total = n * n;
    let mut a = vec![1.0001; total];
    let mut b = vec![1.0001; total];
    let mut c = vec![0.0; total];
    let mut x = 0;
    while x < n {
        let mut y = 0;
        while y < n {
            let mut s = 0.0;
            let mut k = 0;
            while k < n {
                s = s + a[x * n + k] * b[k * n + y];
                k = k + 1;
            }
            c[x * n + y] = s;
            y = y + 1;
        }
        x = x + 1;
    }
    let mut sum = 0.0;
    let mut idx = 0;
    while idx < total {
        sum = sum + c[idx];
        idx = idx + 1;
    }
    println!("{}", sum);
}
