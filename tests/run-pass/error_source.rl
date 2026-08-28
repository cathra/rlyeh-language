// Y6a：Error::source() 非 dyn 退化（Option<String>）——链式源错误
// `trait Error { fn message(&self) -> String; fn source(&self) -> Option<String>; }`
// 包装错误返回底层源错误 message；无源错误返回 None。

trait Error {
    fn message(&self) -> String;
    fn source(&self) -> Option<String>;
}

// 底层错误：无源
struct IoFailure { msg: String }

impl Error for IoFailure {
    fn message(&self) -> String {
        self.msg
    }
    fn source(&self) -> Option<String> {
        Option::None
    }
}

// 包装错误：source 返回底层 message（链式）
struct AppError { msg: String, source_msg: String }

impl Error for AppError {
    fn message(&self) -> String {
        self.msg
    }
    fn source(&self) -> Option<String> {
        Option::Some(self.source_msg)
    }
}

fn main() {
    // 1. 无源错误：source() = None
    let io_e = IoFailure { msg: String::from("file not found") };
    match io_e.source() {
        Option::Some(_) => println(0),
        Option::None => println(1), // 1
    }
    // 2. 包装错误：source() = Some(底层 message)
    let app_e = AppError { msg: String::from("load failed"), source_msg: String::from("file not found") };
    match app_e.source() {
        Option::Some(s) => println(s), // file not found
        Option::None => println(0),
    }
    // 3. message() 保持独立
    println(app_e.message()); // load failed
}
