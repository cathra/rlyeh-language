// Y1（2026-08）：File::open_with（§4.1 目标签名）+ open 兼容壳（默认 Read）
// + 完整 Metadata（size/mtime/is_file/is_dir，经 driver 注入 __rlyeh_file_* stat 内建）。
fn main() {
    // open_with(Append)：追加写入（"a"，定位文件末尾）
    let _ = write_file(String::from("/tmp/rlyeh_y1_n1.txt"), String::from("rlyeh"));
    match File::open_with(String::from("/tmp/rlyeh_y1_n1.txt"), OpenMode::Append) {
        Ok(file) => {
            let mut file = file;
            match file.write_all(String::from("-y1")) {
                Ok(n) => println(n),             // 3
                Err(e) => println(-1),
            }
            file.close();
        }
        Err(e) => println(-1),
    }
    // open 兼容壳（默认 Read）：读回 + 完整元数据
    match File::open(String::from("/tmp/rlyeh_y1_n1.txt")) {
        Ok(file) => {
            let mut file = file;
            match file.read_to_string() {
                Ok(v) => println(v),             // rlyeh-y1
                Err(e) => println(-1),
            }
            match file.metadata() {
                Ok(m) => {
                    println(m.size());           // 8
                    println(m.is_file());        // true（S_IFREG）
                    println(m.is_dir());         // false
                    println(m.mtime() > 0);      // true（epoch 秒恒正）
                }
                Err(e) => println(-1),
            }
            file.close();
        }
        Err(e) => println(-1),
    }
    // 失败路径：open 不存在的文件 → Err(NotFound)
    match File::open(String::from("/tmp/rlyeh_y1_no_such_n1.txt")) {
        Ok(file) => {
            file.close();
            println(0);
        }
        Err(e) => println(-1),
    }
}
