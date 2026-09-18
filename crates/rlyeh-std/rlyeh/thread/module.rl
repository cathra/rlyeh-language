// thread/module.rl：线程模块——只做「子模块声明 + 暴露内容导出」（2026-09-18 重整）。
//
// 组成：
//   thread/handle.rl    struct Thread + impl（start / join / current）  → 子模块 thread::handle
//   thread/entry.rl     fn __start_with_input（闭包跨线程捕获辅助）      → 子模块 thread::entry
//   thread/builder.rl   struct Builder + impl（栈大小定制）             → 子模块 thread::builder
//   thread/ops.rl       fn sleep / join_all                            → 子模块 thread::ops
//
// `pub import` 登记 `thread::Xxx → thread::<mod>::Xxx` 重导出别名，使既有引用
// （用户代码 `Thread::start`、std 内部、编译器特判）无需改动：
// - `thread::Thread`：typecheck 方法特判按短名等价匹配（check_expr/method.rs）；
// - `thread::__start_with_input`：编译器经 `resolve_full_name` 定位本别名
//   （check_expr/method/thread.rs）。
//
// module 声明顺序 = 收集期注册顺序：handle 先于 entry / builder / ops
// （后者签名引用 `thread::handle::Thread`）。

module handle;
pub import handle::Thread;
module entry;
pub import entry::__start_with_input;
module builder;
pub import builder::Builder;
module ops;
pub import ops::sleep;
pub import ops::join_all;
