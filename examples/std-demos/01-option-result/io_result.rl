// M3a：io 自由函数 Result 化——成功路径解包 / 失败 Err(IoError) / `?` 传播
fn kind_code(k: IoErrorKind) -> i64 {
    match k {
        IoErrorKind::NotFound => 1,
        IoErrorKind::PermissionDenied => 2,
        IoErrorKind::AlreadyExists => 3,
        IoErrorKind::InvalidInput => 4,
        IoErrorKind::WouldBlock => 5,
        IoErrorKind::TimedOut => 6,
        IoErrorKind::Other => 7,
    }
}

// `?` 传播（成功路径）：写入字节数 = "hello m3a" 长度
fn ok_bytes() -> Result<i64, IoError> {
    let n = write_file(String::from("/tmp/rlyeh_m3a.txt"), String::from("hello m3a"))?;
    Result::Ok(n)
}

// `?` 传播（失败路径）：读不存在的文件 → Err 上抛
fn fail_read() -> Result<i64, IoError> {
    let content = read_file(String::from("/tmp/rlyeh_no_such_x.txt"))?;
    Result::Ok(content.len)
}

fn main() {
    // 成功路径：write_file 返回 Ok(字节数)
    let w = write_file(String::from("/tmp/rlyeh_m3a.txt"), String::from("hello m3a"));
    println(w.is_ok());                    // 1
    match w {
        Ok(n) => println(n),               // 10
        Err(e) => println(-1),
    }
    // 成功路径：read_file 读回
    let r = read_file(String::from("/tmp/rlyeh_m3a.txt"));
    println(r.is_ok());                    // 1
    match r {
        Ok(v) => println(v),               // hello m3a
        Err(e) => println(-1),
    }
    // `?` 传播成功
    match ok_bytes() {
        Ok(n) => println(n),               // 10
        Err(e) => println(-1),
    }
    // 失败路径：Err(IoError) + kind 匹配
    let m = read_file(String::from("/tmp/rlyeh_no_such_x.txt"));
    println(m.is_err());                   // 1
    match m {
        Ok(v) => println(0),
        Err(e) => println(kind_code(e.kind())),   // 1（NotFound）
    }
    // 失败路径：`?` 上抛 Err
    println(fail_read().is_err());         // 1
}
