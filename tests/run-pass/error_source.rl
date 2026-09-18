// Y6a + P7d-1（2026-08-29）：Error::source() 真实错误链（Option<&dyn Error>）
// 利用 P4 已完成的 &dyn Error 上转型，source() 返回底层错误的 &dyn Error 引用，
// 形成可逐层追溯的错误链（替代 Y6a 的 message 字符串拷贝退化）。
//
// 表示法：包装错误持有底层错误的「引用字段」（`source: &IoFailure`），source()
// 上转该引用值返回（而非取 `&self.field`——后者在 MVP 下会触发 `&self.struct_field`
// 返回引用的 codegen 悬空缺陷，见 lang-defects）。即 idiomatic 的 `source: &dyn Error`
// 退化到具体引用 `&IoFailure`（MVP 暂不支持 `&dyn Error` 作 struct 字段）。

protocol Error {
    fn message(&self) -> String;
    fn source(&self) -> Option<&dyn Error>;
}

// 底层错误：无源（source = None）
struct IoFailure { msg: String }

impl IoFailure: Error {
    fn message(&self) -> String {
        self.msg
    }
    fn source(&self) -> Option<&dyn Error> {
        Option::None
    }
}

// 包装错误：持有底层错误的引用（真实错误链载体），source() 上转该引用返回
struct AppError { msg: String, source: &IoFailure }

impl AppError: Error {
    fn message(&self) -> String {
        self.msg
    }
    fn source(&self) -> Option<&dyn Error> {
        let d: &dyn Error = self.source; // 上转存储的引用（非 &self.field）
        Option::Some(d)
    }
}

fn main() {
    // 1. 底层错误 source() = None
    let io_e = IoFailure { msg: String::from("file not found") };
    match io_e.source() {
        Option::Some(_) => println(0),
        Option::None => println(1), // 1
    }
    // 2. 包装错误 source() 返回底层 &dyn Error，链式 .message() 取到真实 message
    let r = &io_e;                  // &IoFailure
    let app_e = AppError { msg: String::from("load failed"), source: r };
    match app_e.source() {
        Option::Some(s) => println(s.message()), // file not found
        Option::None => println(0),
    }
    // 3. message() 保持独立
    println(app_e.message());       // load failed
    // 4. 链可遍历：app_e.source() -> io_failure；io_failure.source() = None（链尾）
    match app_e.source() {
        Option::Some(s) => match s.source() {
            Option::Some(_) => println(2),
            Option::None => println(3), // 3（链到 IoFailure 终止）
        },
        Option::None => println(0),
    }
}
