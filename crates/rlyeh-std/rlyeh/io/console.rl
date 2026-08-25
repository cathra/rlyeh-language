// io/console.rl：控制台 IO（stdout/stderr + stdin 读取，std-lib.md §4.2）。
// 目录化（2026-08）：由原 io.rl 拆分。符号完整路径 io::console::Stdout 等。

// N2a（2026-08）：stdout/stderr 句柄对象（std-lib.md §4.2）。
// POSIX fd 1/2 封装；`println!` 内建宏之外的程序化输出通道。
// flush() MVP 为 no-op（stdio 退出时自动刷盘，同 C 语义）。
struct Stdout {
    fd: i64,
}

struct Stderr {
    fd: i64,
}

// 获取 stdout 句柄（fd 1）。
fn stdout() -> io::console::Stdout {
    io::console::Stdout { fd: 1 }
}

// 获取 stderr 句柄（fd 2）。
fn stderr() -> io::console::Stderr {
    io::console::Stderr { fd: 2 }
}

impl Stdout {
    // 写入 buf 全部字节；返回写入字节数。
    fn write(self, buf: String) -> Result<i64, io::error::IoError> {
        let n = write(self.fd, buf, buf.len);
        Result::Ok(n)
    }
    // 写入一行（buf + "\n"）；返回总字节数。
    fn writeln(self, buf: String) -> Result<i64, io::error::IoError> {
        let n1 = write(self.fd, buf, buf.len);
        let n2 = write(self.fd, String::from("\n"), 1);
        Result::Ok(n1 + n2)
    }
    // 刷新（MVP no-op：stdio 退出时自动刷盘）。
    fn flush(self) -> Result<i64, io::error::IoError> {
        Result::Ok(1)
    }
}

impl Stderr {
    // 写入 buf 全部字节；返回写入字节数。
    fn write(self, buf: String) -> Result<i64, io::error::IoError> {
        let n = write(self.fd, buf, buf.len);
        Result::Ok(n)
    }
    // 写入一行（buf + "\n"）；返回总字节数。
    fn writeln(self, buf: String) -> Result<i64, io::error::IoError> {
        let n1 = write(self.fd, buf, buf.len);
        let n2 = write(self.fd, String::from("\n"), 1);
        Result::Ok(n1 + n2)
    }
    // 刷新（MVP no-op）。
    fn flush(self) -> Result<i64, io::error::IoError> {
        Result::Ok(1)
    }
}

// N2b（2026-08）：stdin 增强（std-lib.md §4.2）。
// 从 stdin 读取全部内容直到 EOF；返回 Result<String, IoError>。
// 注意：与 read_line/lines 混用会互相消费 stdin 缓冲（MVP 无独立行缓冲）。
fn read_to_string() -> Result<String, io::error::IoError> {
    let mut buf = String::new();
    loop {
        let mut tmp = String::with_capacity(256);
        let n = read(0, tmp, 256);
        if n < 0 {
            return Result::Err(IoError::new(
                io::error::IoErrorKind::Other,
                String::from("read from stdin failed"),
            ));
        }
        if n == 0 {
            break;   // EOF
        }
        let mut i = 0;
        while i < n {
            buf.push_byte(tmp.data[i]);
            i = i + 1;
        }
    }
    Result::Ok(buf)
}

// N2b：stdin 行迭代器（缓冲 + EOF 状态；J2 形态接入 `for line in lines()`）。
struct StdinLines {
    done: i64,
    buf: String,
    pos: i64,
}

// 获取 stdin 行迭代器。
fn lines() -> io::console::StdinLines {
    io::console::StdinLines { done: 0, buf: String::new(), pos: 0 }
}

impl StdinLines {
    // 取下一行（不含换行符）；EOF 返回 None。
    fn next(&mut self) -> Option<String> {
        loop {
            if self.done == 1 {
                return Option::None;
            }
            // 在现有缓冲内查找换行符
            let mut i = self.pos;
            while i < self.buf.len {
                if self.buf.data[i] == 10 {   // '\n'
                    break;
                }
                i = i + 1;
            }
            if i < self.buf.len {
                // 找到 \n：返回 [pos, i)
                let mut line = String::with_capacity(i - self.pos);
                let mut j = self.pos;
                while j < i {
                    line.push_byte(self.buf.data[j]);
                    j = j + 1;
                }
                self.pos = i + 1;
                return Option::Some(line);
            }
            // 缓冲不足：补读 256 字节
            let mut tmp = String::with_capacity(256);
            let n = read(0, tmp, 256);
            if n == 0 {
                self.done = 1;
                // EOF：剩余内容作为最后一行
                if self.pos < self.buf.len {
                    let mut line = String::with_capacity(self.buf.len - self.pos);
                    let mut j = self.pos;
                    while j < self.buf.len {
                        line.push_byte(self.buf.data[j]);
                        j = j + 1;
                    }
                    self.pos = self.buf.len;
                    return Option::Some(line);
                }
                return Option::None;
            }
            if n < 0 {
                self.done = 1;
                return Option::None;
            }
            // 追加 tmp 到缓冲后继续循环
            let mut k = 0;
            while k < n {
                self.buf.push_byte(tmp.data[k]);
                k = k + 1;
            }
        }
    }
}

// 从 stdin（fd 0）读取一行，不含换行符；读取失败返回 Err(IoError)。
fn read_line() -> Result<String, io::error::IoError> {
    let mut tmp = String::with_capacity(256);
    let n = read(0, tmp, 256);
    if n < 0 {
        return Result::Err(IoError::new(
            io::error::IoErrorKind::Other,
            String::from("failed to read from stdin"),
        ));
    }
    let mut buf = String::new();
    let mut i = 0;
    while i < n {
        if tmp.data[i] == 10 {   // '\n'
            break;
        }
        buf.push_byte(tmp.data[i]);
        i = i + 1;
    }
    Result::Ok(buf)
}
