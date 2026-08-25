// N1：File 对象化验收（open/create/close + read_to_string/read/write/write_all/flush/size/metadata）
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

fn main() {
    // create + write + flush + size（"w" 截断创建）
    let f = File::create(String::from("/tmp/zeta_n1.txt"));
    match f {
        Ok(file) => {
            let mut file = file;
            match file.mode() {
                OpenMode::Create => println(5),
                _ => println(0),
            }
            match file.write(String::from("hello file")) {
                Ok(n) => println(n),          // 10
                Err(e) => println(-1),
            }
            match file.flush() {
                Ok(_) => println(1),          // 1
                Err(e) => println(0),
            }
            match file.size() {
                Ok(n) => println(n),          // 10
                Err(e) => println(-1),
            }
            file.close();
        }
        Err(e) => println(-1),
    }
    // open(Read) + read_to_string + metadata
    let r = File::open(String::from("/tmp/zeta_n1.txt"), OpenMode::Read);
    match r {
        Ok(file) => {
            let mut file = file;
            match file.read_to_string() {
                Ok(v) => println(v),          // hello file
                Err(e) => println(-1),
            }
            match file.metadata() {
                Ok(n) => println(n),          // 10
                Err(e) => println(-1),
            }
            file.close();
        }
        Err(e) => println(-1),
    }
    // open(ReadWrite, "r+") + read(cap) + write_all（覆盖中间字节）
    let w = File::open(String::from("/tmp/zeta_n1.txt"), OpenMode::ReadWrite);
    match w {
        Ok(file) => {
            let mut file = file;
            match file.read(5) {
                Ok(v) => println(v),          // hello
                Err(e) => println(-1),
            }
            match file.write_all(String::from("!")) {
                Ok(n) => println(n),          // 1
                Err(e) => println(-1),
            }
            file.close();
        }
        Err(e) => println(-1),
    }
    // 读回确认覆盖结果
    match read_file(String::from("/tmp/zeta_n1.txt")) {
        Ok(v) => println(v),                  // hello!file
        Err(e) => println(-1),
    }
    // 失败路径：open 不存在的文件 → Err(NotFound)
    let m = File::open(String::from("/tmp/zeta_no_such_n1.txt"), OpenMode::Read);
    match m {
        Ok(file) => {
            file.close();
            println(0);
        }
        Err(e) => println(kind_code(e.kind())),   // 1
    }
}
