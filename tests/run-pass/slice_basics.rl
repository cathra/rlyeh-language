// S4（2026-08-30）：切片类型系统 run-pass 用例——
// `&[T]` / `&mut [T]` 形参（含跨函数非内联传参）、数组→切片 unsize coercion、
// 索引 / 再切片 / `.len()` / `first` / `last` / `iter`、越界 clamp、
// `Vec<u8>` 切片视图（紧凑字节）与 `File` 切片 IO（二进制安全，含 NUL 字节）。
fn sum(xs: &[i64]) -> i64 {
    let mut s = 0;
    let mut i = 0;
    while i < xs.len() {
        s = s + xs[i];
        i = i + 1;
    }
    s
}

fn sum_iter(xs: &[i64]) -> i64 {
    let mut s = 0;
    for r in xs.iter() {
        s = s + *r;
    }
    s
}

fn sum_range(xs: &[i64]) -> i64 {
    let sub = xs[1..<3];
    let mut s = 0;
    let mut i = 0;
    while i < sub.len() {
        s = s + sub[i];
        i = i + 1;
    }
    s
}

fn sum_clamped(xs: &[i64]) -> i64 {
    let sub = xs[2..<99];
    let mut s = 0;
    let mut i = 0;
    while i < sub.len() {
        s = s + sub[i];
        i = i + 1;
    }
    s
}

fn ends(xs: &[i64]) -> i64 {
    xs.first() + xs.last()
}

fn bump(xs: &mut [i64]) {
    let mut i = 0;
    while i < xs.len() {
        xs[i] = xs[i] + 1;
        i = i + 1;
    }
}

fn sum_bytes(xs: &[u8]) -> i64 {
    let mut s = 0;
    let mut i = 0;
    while i < xs.len() {
        s = s + xs[i];
        i = i + 1;
    }
    s
}

fn main() {
    let xs = [10, 20, 30, 40, 50];
    println(sum(&xs));          // 150：索引求和
    println(sum_iter(&xs));     // 150：iter() 零拷贝迭代
    println(sum_range(&xs));    // 50：再切片 [1..<3] = 20+30
    println(sum_clamped(&xs));  // 120：上界越界 clamp 到 len → 30+40+50
    println(ends(&xs));         // 60：first() + last() = 10+50

    let mut ys = [1, 2, 3];
    bump(&mut ys);
    println(ys[0] + ys[1] + ys[2]);   // 9：&mut [i64] 写入回原数组 2+3+4

    // Vec<u8> 紧凑字节缓冲的切片视图（as_slice / as_mut_slice）
    let mut v: Vec<u8> = Vec::new();
    v.push(1);
    v.push(2);
    v.push(3);
    println(sum_bytes(v.as_slice()));   // 6：1+2+3（步长 1 字节）

    // File 切片 IO：二进制安全（内容含 NUL 字节 0）
    let path = String::from("/tmp/rlyeh_slice_s4.bin");
    let mut content = String::new();
    content.push_byte(65);
    content.push_byte(0);
    content.push_byte(66);
    let _w = write_file(path, content);
    match File::open(path) {
        Ok(file) => {
            let mut file = file;
            let mut buf: Vec<u8> = Vec::new();
            let mut i = 0;
            while i < 8 {
                buf.push(0);
                i = i + 1;
            }
            match file.read_slice(buf.as_mut_slice()) {
                Ok(n) => {
                    println(n);                          // 3：读入 3 字节
                    println(buf[0] + buf[1] + buf[2]);   // 131：65+0+66
                }
                Err(e) => println(-1),
            }
            file.close();
        }
        Err(e) => println(-1),
    }
}
