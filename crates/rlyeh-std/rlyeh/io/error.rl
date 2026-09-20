// io/error.rl：IO 错误分类（std-lib.md §12）。
// 目录化（2026-08）：由原 io.rl 拆分。符号完整路径 io::error::IoError 等，
// core.rl 经 `import io::error::...` 重导出到根命名空间（裸名 IoError 即用）。

// M1a（2026-08）：IO 错误分类枚举（C-like，无数据载荷）。
// 对齐 std-lib.md §12：M1b 起由 IoError { kind, message } 携带。
enum IoErrorKind {
    NotFound,
    PermissionDenied,
    AlreadyExists,
    InvalidInput,
    WouldBlock,
    TimedOut,
    Other,
}

// M1b（2026-08）：IO 错误结构（kind 分类 + 人类可读 message）。
// 访问器返回分类 / 消息；正式 Display protocol 随 Q3（格式化），MVP 用 message()。
struct IoError {
    kind: io::error::IoErrorKind,
    message: String,
}

impl IoError {
    fn new(kind: io::error::IoErrorKind, message: String) -> io::error::IoError {
        io::error::IoError { kind: kind, message: message }
    }
    // M2b：错误转换约定（From/Into MVP 回退，见 protocol 声明处注释）
    fn from_kind(kind: io::error::IoErrorKind) -> io::error::IoError {
        io::error::IoError { kind: kind, message: io::error::kind_message(kind) }
    }
    fn kind(self) -> io::error::IoErrorKind {
        self.kind
    }
    fn message(&self) -> String {
        self.message
    }
}

// M2a（2026-08）：通用错误 protocol（std-lib.md §12）。
// MVP 仅 message()；正式 Display/Debug 随 Q3（格式化 protocol）。
// H4 限制：dyn Protocol 仅可作局部变量绑定（不可作函数参数），
// describe 类调用在函数体内构造 `let d: dyn Error = ...`。
// Y6a（2026-08-28）：补 `source()`——初版因 `&dyn Error` 上转型未实现退化为
// `Option<String>`（仅返回源错误 message 字符串）。
// P7d-1（2026-08-29）：利用 P4 已完成的 `&dyn Error` 上转型，升级为
// `Option<&dyn Error>`，形成真实错误链（逐层 source 追溯，而非字符串拷贝）。
protocol Error {
    fn message(&self) -> String;
    fn source(&self) -> Option<&dyn Error>;
}

impl IoError: Error {
    fn message(&self) -> String {
        self.message
    }
    fn source(&self) -> Option<&dyn Error> {
        Option::None
    }
}

// M2b / Y6b（2026-08）：错误转换约定（std-lib.md §12 From/Into protocol）。
// Y6b（2026-08-28）：泛型 protocol `From<T>`/`Into<T>` 声明 + 类型实参支持 `::` 路径
// （`impl From<io::error::IoErrorKind> for IoError`，P6a）与 `where` 子句（P6b）。
// `IoError` 经 `From<IoErrorKind>` 接入 `from_kind`。
// P6c（2026-08-29）：`From::from` + `?` 运算符 From 自动转换已落地；
// P6c-1/2（2026-08-29）：`Into::into` 经 blanket 语义实现——`Into::<U>::into(x)`
// 约束求解确认 `impl From<A> for U` 存在后改写 `From::from(x)`（std 无需注册 blanket impl）。
protocol From<T> {
    fn from(v: T) -> Self;
}

protocol Into<T> {
    fn into(self) -> T;
}

// IoErrorKind → IoError（复用 from_kind 的默认 message 生成）。
impl IoError: From<IoErrorKind> {
    fn from(v: IoErrorKind) -> io::error::IoError {
        IoError::from_kind(v)
    }
}

fn kind_message(kind: io::error::IoErrorKind) -> String {
    match kind {
        io::error::IoErrorKind::NotFound => String::from("entity not found"),
        io::error::IoErrorKind::PermissionDenied => String::from("permission denied"),
        io::error::IoErrorKind::AlreadyExists => String::from("entity already exists"),
        io::error::IoErrorKind::InvalidInput => String::from("invalid input"),
        io::error::IoErrorKind::WouldBlock => String::from("operation would block"),
        io::error::IoErrorKind::TimedOut => String::from("operation timed out"),
        io::error::IoErrorKind::Other => String::from("io error"),
    }
}
