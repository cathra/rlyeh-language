// benchmark: 256x256 f64 矩阵乘法（一维 Vec 模拟）—— 浮点运算 + 内存访问
fn main() {
    let n = 256;
    let total = n * n;
    let mut a: Vec<f64> = Vec::new();
    let mut b: Vec<f64> = Vec::new();
    let mut c: Vec<f64> = Vec::new();
    let mut i = 0;
    while i < total {
        a.push(1.0001);
        b.push(1.0001);
        c.push(0.0);
        i = i + 1;
    }
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
    // 校验和（防止结果被丢弃）
    let mut sum = 0.0;
    let mut idx = 0;
    while idx < total {
        sum = sum + c[idx];
        idx = idx + 1;
    }
    println(sum);
}
