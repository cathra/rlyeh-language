// serde/module.rl：序列化框架——只做「子模块声明 + 暴露内容导出」（2026-09-18 重整）。
//
// 组成：
//   serde/protocols.rl        protocol Serialize / Deserialize      → 子模块 serde::protocols
//   serde/error.rl         enum JsonError / TomlError            → 子模块 serde::error
//   serde/serializer.rl    struct Serializer + impl              → 子模块 serde::serializer
//   serde/deserializer.rl  struct Deserializer + impl            → 子模块 serde::deserializer
//   serde/impls.rl         impl i64 / bool / String: Serialize   → 子模块 serde::impls
//
// `pub import` 登记 `serde::Xxx → serde::<mod>::Xxx` 重导出别名，使既有引用（用户代码
// `impl T: Serialize`、std 内部）无需改动。裸名导出见标准库根 module.rl。
//
// 说明：`json::stringify` / `json::parse::<T>` 为编译器内建（L2），不经本框架；
// 本模块提供手写 `impl Serialize` / `impl Deserialize` 的协议载体与访问器框架。
//
// module 声明顺序 = 收集期注册顺序：protocols / error 先于 serializer / deserializer /
// impls（后者引用协议与工具函数）。

module protocols;
pub import protocols::Serialize;
pub import protocols::Deserialize;
module error;
pub import error::JsonError;
pub import error::TomlError;
module serializer;
pub import serializer::Serializer;
module deserializer;
pub import deserializer::Deserializer;
module impls;
