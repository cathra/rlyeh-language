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

// EH-4（0.2.0-AA，2026-09-21）：**拥有型动态错误** `DynError`（RFC §4.4 / anyhow 式便捷）。
// 语义："我不关心具体错误类型、只想传播"的统一错误抽象——内部持有 `Box<dyn Error>`。
// 布局：`Box` 持有 2 槽胖指针 `{data 指针, vtable 指针}`，data 指向**堆上拥有**的具体
// 错误对象（由 `IoError::into_dyn` 等构造器先 `Box::new` 装箱后取 `&*b` 得到）。
// 编译器侧配套（2026-09-21）：① `type_slot_count` 为 `dyn Protocol` 返回 2 槽
// （此前 `Box::new(<dyn 值>)` 报「该类型不支持堆装箱」）；② `check_method_call` 的
// dyn 虚调用识别**堆包裹**的 protocol 对象（`Box<dyn P>` 接收者先解出指向胖指针的
// 对象指针，再按槽 0 = data / 槽 1 = vtable 间接调用）。
// 用法：
//   let d: DynError = IoError::from_kind(IoErrorKind::NotFound).into_dyn();
//   println(d.message());            // 转发 → Box<dyn Error> 虚调用 → IoError::message
//   fn load() -> Result<i64, DynError> { ... }
struct DynError {
    inner: Box<dyn Error>,
}

// 装箱为拥有型动态错误。四步：① `Box::new(self)` 把具体错误移入堆（拥有）；
// ② `&*b` 取该堆对象引用；③ `let d: dyn Error = r` 构造 2 槽胖指针
// `{data=堆对象, vtable}`；④ `Box::new(d)` 把胖指针装箱。
// 注：MVP 下外层箱释放不回收内层箱（同 `Box` 既有析构约定，见 `box_leak.rl`）。
// （独立 impl 块：须位于 `struct DynError` 之后，故不能并入上方 `impl IoError`。）
impl IoError {
    fn into_dyn(self) -> io::error::DynError {
        let b: Box<io::error::IoError> = Box::new(self);
        let r: &io::error::IoError = &*b;
        let d: dyn Error = r;
        io::error::DynError { inner: Box::new(d) }
    }
}

impl DynError {
    // 具体错误 → DynError（`into_dyn` 的等价静态构造形式，便于链式构造 / `From`）。
    fn from_io(e: io::error::IoError) -> io::error::DynError {
        e.into_dyn()
    }
    // 转发到内层 `Box<dyn Error>` 的虚调用（`message` 为主要用途）。
    fn message(&self) -> String {
        self.inner.message()
    }
    fn source(&self) -> Option<&dyn Error> {
        self.inner.source()
    }
}

// 使 `DynError` 自身也是 `Error`——可再参与 `source()` 背链 / `&dyn Error` 上转。
impl DynError: Error {
    fn message(&self) -> String {
        self.inner.message()
    }
    fn source(&self) -> Option<&dyn Error> {
        self.inner.source()
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

// EH-4 M3（0.2.0-AA，2026-09-21）：便捷早退宏 `bail!` / `ensure!` **未在此提供**——
// 宏注册表按**编译单元**隔离，std 模块文件（本文件）中 `macro_rules!` 定义的宏
// **不对用户代码可见**（实测：用户处报「未定义的宏 `ensure`（内置宏：println!/…）」）。
// 故 M3 以**用户侧宏**形式落地（见 tests/run-pass/eh_dyn_error.rl），并登记宏系统缺口：
//   ① 宏不可从 std / 模块文件导出（无 `pub macro` 或等价机制）；
//   ② `$e:expr` 元变量**不匹配含 `::` 的路径调用**（`bail!(IoError::from_kind(..))`
//      报「规则均不匹配」）——须用 `$($t:tt)*` 重复；
//   ③ transcriber 须展开为**单个表达式**：`return Result::Err(..)` 不能带结尾分号；
//   ④ 条件须显式加括号 `!($cond)`（元变量展开的优先级不高于一元 `!`）。
