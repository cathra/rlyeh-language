//! 类型检查上下文（符号表）。

use std::collections::HashMap;

use rlyeh_hir::HirExpr;
use rlyeh_lexer::Span;

use rlyeh_ast::{AstActorDecl, AstExpr, AstFnDecl};

use crate::error::TypeError;
use crate::types::{EnumDef, FnSignature, ImplDef, StructDef, TraitDef, Type};

/// 延迟闭包值绑定记录（H5 补全：非注解闭包 `let f = |x| body;`）。
///
/// 绑定处闭包参数类型未知，仅记录 AST 与变量名，绑定变量类型为未固化
/// `Type::Closure`（`fn_name` 为空）；首次调用点 `f(args)` 由实参类型
/// 推断参数类型后固化（检查闭包体、生成匿名函数与捕获聚合对象）。
#[derive(Debug, Clone)]
pub struct DeferredClosure {
    /// 绑定的变量名（`let f = ..` 中的 `f`）
    pub var_name: String,
    /// 闭包 AST（`ExprKind::Closure`）
    pub closure: AstExpr,
    /// 绑定语句 span（错误定位用）
    pub span: Span,
}

/// 泛型函数模板（实例化前不生成 HIR，调用点按实参类型实例化）。
#[derive(Debug, Clone)]
pub struct FnTemplate {
    /// 模板名（未实例化的原始名）
    #[allow(dead_code)]
    pub name: String,
    /// 泛型参数名
    pub type_params: Vec<String>,
    /// 泛型参数 → 约束 trait 名列表（U3：`fn f<T: Bound1 + Bound2>`）
    pub bounds: HashMap<String, Vec<String>>,
    /// 签名（参数 / 返回类型中可含 [`Type::Generic`]）
    pub sig: FnSignature,
    /// 原始函数 AST（实例化时克隆并替换类型参数）
    pub ast: AstFnDecl,
}

/// 类型检查上下文。
///
/// 维护函数签名、局部变量、结构体定义与类型别名。
/// 函数签名在检查函数体之前先收集完毕，因此支持函数间互相调用。

/// 局部变量作用域（U1：`TypeContext::scopes` 栈的一层）。
#[derive(Debug, Clone, Default)]
pub struct Scope {
    /// 变量表（原名 → (存储槽名, 类型)）。同名遮蔽时槽名 mangle 为 `name$N`，
    /// 保证下游 HIR/MIR/LIR/codegen 按槽名区分变量（下游按字符串名分配存储槽，
    /// 遮蔽必须靠槽名隔离，否则同名槽互相覆盖）。
    pub vars: HashMap<String, (String, Type)>,
    /// 初始化表达式表（原名 → 初始化 HIR，供 `String::from(s)` 追踪字面量值）。
    pub inits: HashMap<String, HirExpr>,
    /// 是否函数边界（隔离：变量查找不穿透该层，函数/闭包体看不到外层局部变量）。
    pub is_fn: bool,
    /// H4 去虚拟化表（dyn 绑定变量原名 → 具体类型，变量被重新赋值或离开作用域时失效）。
    pub dyn_concrete: HashMap<String, Type>,
}

#[derive(Debug, Clone, Default)]
pub struct TypeContext {
    /// 函数签名表（函数名 → 签名）
    pub fn_signatures: HashMap<String, FnSignature>,
    /// 泛型函数模板表（泛型函数实例化前的原始定义）
    pub fn_templates: HashMap<String, FnTemplate>,
    /// 泛型实例化缓存（实例键 → 实例函数名）
    pub mono_instances: HashMap<String, String>,
    /// 泛型实例化生成的函数项（检查过程中追加）
    pub mono_items: Vec<rlyeh_hir::HirItem>,
    /// 局部变量作用域栈（U1：函数/块/循环体/match 臂/闭包体各一层）。
    ///
    /// - 查找从栈顶向下，遇 `is_fn` 边界层即停（函数/闭包体隔离）；
    /// - 块级遮蔽：内层同名绑定 mangle 存储槽名（`name$N`），下游按槽名区分；
    /// - 同层重复 `let x` 为覆盖语义（保持原槽名，现状行为）。
    pub scopes: Vec<Scope>,
    /// 结构体定义表
    pub structs: HashMap<String, StructDef>,
    /// 枚举定义表
    pub enum_defs: HashMap<String, EnumDef>,
    /// trait 定义表
    pub trait_defs: HashMap<String, TraitDef>,
    /// P7d-1（2026-08-29）：当前正在收集的 trait 完整名（含模块前缀）。
    /// 用于自引用 trait（如 `trait Error { fn source(&self) -> Option<&dyn Error> }`）
    /// 在尚未注册进 trait_defs 前，让 dyn 解析回退到自身名字。
    pub collecting_trait: Option<String>,
    /// impl 块列表（inherent 与 trait impl 统一存放）
    pub impl_defs: Vec<ImplDef>,
    /// 变体名索引（变体名 → (枚举名, 变体名)，支持裸名 `Some(x)` 构造）
    pub variant_index: HashMap<String, (String, String)>,
    /// 类型别名表
    pub type_aliases: HashMap<String, Type>,
    /// use 导入别名表（本地名 → 完整符号名，如 `"add"` → `"math::add"`）
    pub use_aliases: HashMap<String, String>,
    /// 模块常量表（完整符号名 → (HIR 值, 类型)，如 `"math::MAX"`）
    pub constants: HashMap<String, (HirExpr, Type)>,
    /// Actor 定义表（完整符号名 → 声明，如 `"Counter"` / `"math::Counter"`）
    pub actors: HashMap<String, AstActorDecl>,
    /// 已生成的 `rlyeh_actor_*` extern 声明名（actor 展开去重用）
    pub generated_actor_externs: std::collections::HashSet<String>,
    /// 已生成的 `rlyeh_gc_*` extern 声明名（K4 追踪 GC 展开去重用）
    pub generated_gc_externs: std::collections::HashSet<String>,
    /// 当前检查的模块前缀（顶层为空串，`mod math` 内为 `"math"`）
    pub module_prefix: String,
    /// 当前作用域的泛型参数名（如 `["T"]`）
    pub type_params: Vec<String>,
    /// 当前泛型替换表（泛型参数名 → 具体类型，实例化 body 检查时有效）
    pub generic_subst: HashMap<String, Type>,
    /// 当前 impl 的关联类型映射（关联类型名 → 具体类型，U2）。
    ///
    /// `collect_impl` 解析 `type Item = Concrete;` 后填充，方法签名中
    /// `Self::Item` 经 `resolve_ast_type` 查本表替换；trait 声明收集时为空，
    /// `Self::Item` 退化为占位 `Type::Generic("Self::Item")`。
    pub assoc_types: HashMap<String, Type>,
    /// 当前 impl 的目标类型（U4：方法签名/body 中 `Self` 解析为它；
    /// trait 上下文未设置时 `Self` 退化为占位 `Generic("Self")`）
    pub self_type: Option<Type>,
    /// 遮蔽槽名计数器（生成 `name$N` 唯一槽名）
    pub shadow_seq: usize,
    /// 临时变量名计数器
    pub temp_counter: usize,
    /// H2 无捕获闭包匿名函数名计数器（`__closure_{n}` 全局唯一）
    pub closure_seq: usize,
    /// 延迟闭包值绑定表（H5 补全：非注解闭包 `let f = |x| body;`）。
    ///
    /// 绑定处注册（变量类型为未固化 `Type::Closure`，`fn_name` 为空），
    /// 首次调用点 `f(args)` 由实参类型推断参数类型后固化并移除记录。
    pub deferred_closures: Vec<DeferredClosure>,
    /// 顶层裸名 fn 被遮蔽的重命名表（裸名 → mangle 名）。
    ///
    /// 用户顶层函数与 std 预置根函数重名时（如用户 `fn read` 与 std extern `read`），
    /// 用户声明注册为 `read@shadow<N>`，std 原名保留——模块内部裸名调用仍绑定 std 版本，
    /// 用户顶层代码经本表绑定用户自身版本；同时避免下游 MIR/LIR/codegen 同名符号冲突。
    pub fn_shadow_of: HashMap<String, String>,
    /// 被遮蔽声明的重命名记录（声明 Span(start,end) → mangle 名）。
    ///
    /// 收集阶段（第一遍）决定重命名并记录，检查阶段（第二遍）生成 HIR 项时
    /// 按同一 Span 取出 mangle 名，保证两遍一致。
    pub fn_decl_shadow: HashMap<(usize, usize), String>,
    /// 同名遮蔽计数器（裸名 → 已遮蔽次数，用于生成唯一 mangle 名）。
    pub fn_shadow_seq: HashMap<String, usize>,
    /// L3 PGO 回灌：区域名 → 推荐初始容量（字节）。
    ///
    /// 由 `rlyeh build --profile` 读取 `.rl_profile` 后注入；
    /// `adaptive` 区域在检查时优先采用该容量作为 `HirRegionOptions.size`。
    pub region_hints: HashMap<String, usize>,
    /// 当前函数/方法声明返回类型（P6c，`?` 运算符 From 自动转换用于确定目标错误类型）。
    /// 在函数/actor 方法体检查入口设置，退出时恢复。
    pub current_return_type: Option<Type>,
    /// 当前是否处于 `unsafe` 块内（SH-P0-1 E3：extern 调用门禁上下文）
    pub in_unsafe: bool,
    /// 标准库预置（prelude）源码字节长度（含末尾换行）：
    /// 偏移量 `<` 此值的调用视为受信任的 std 内部 FFI，豁免 unsafe 门禁。
    pub prelude_len: usize,
    /// extern 函数名集合（SH-P0-1 E3：门禁查表，键与 `fn_signatures` 一致）
    pub extern_fns: std::collections::HashSet<String>,
}

impl TypeContext {
    /// 创建空上下文（含根作用域，保证 `insert_variable` 等 API 始终有可写层）。
    pub fn new() -> Self {
        let mut ctx = Self::default();
        ctx.push_scope(true);
        ctx
    }

    /// 进入新作用域。`is_fn=true` 表示函数/闭包边界（变量查找不穿透，实现隔离，
    /// 函数体内看不到调用方局部变量）；块/循环体/match 臂为 `false`（查找穿透，
    /// 块内可访问外层变量）。
    pub fn push_scope(&mut self, is_fn: bool) {
        self.scopes.push(Scope { is_fn, ..Default::default() });
    }

    /// 离开作用域（同时丢弃其中的变量/初始化表达式/dyn 具体类型）。
    pub fn pop_scope(&mut self) {
        self.scopes.pop();
    }

    /// 计算变量在当前作用域应使用的存储槽名：
    /// - 当前层已有同名绑定 → 复用其槽名（同层 `let x` 覆盖语义，现状保持）；
    /// - 外层（块层可穿透所属函数层；起始层为 fn 边界时本层查完即止）已有
    ///   同名绑定 → mangle 为 `name$N`（块级遮蔽，下游按槽名区分，避免同名
    ///   存储槽互相覆盖）；
    /// - 否则 → 原名。
    fn stored_name(&mut self, name: &str) -> String {
        let cur = self.scopes.last_mut().expect("作用域栈为空");
        if let Some((slot, _)) = cur.vars.get(name) {
            return slot.clone();
        }
        // U1 修复：起始层若为 fn 边界，本层查完即止（函数体隔离，不穿透外层
        // fn 层——否则嵌套编译（如 impl 方法体实例化）时外层同名参数触发误
        // mangle，导致参数槽名 `self$0` 与 codegen 入口槽 `self` 错位）；
        // 块层可穿透，但越过外层 fn 边界后停止（块内可见所属函数全部局部变量）。
        let mut crossed_fn = cur.is_fn;
        for s in self.scopes.iter().rev().skip(1) {
            if crossed_fn {
                break;
            }
            if s.vars.contains_key(name) {
                let uname = format!("{name}${}", self.shadow_seq);
                self.shadow_seq += 1;
                return uname;
            }
            crossed_fn = s.is_fn;
        }
        name.to_string()
    }

    /// 记录一个变量绑定，返回存储槽名（块级遮蔽时 mangle 为 `name$N`）。
    pub fn insert_variable(&mut self, name: String, type_: Type) -> String {
        let stored = self.stored_name(&name);
        self.scopes
            .last_mut()
            .expect("作用域栈为空")
            .vars
            .insert(name, (stored.clone(), type_));
        stored
    }

    /// 查找变量的（存储槽名, 类型）。fn 边界不穿透（函数/闭包体隔离）。
    pub fn resolve_variable(&self, name: &str) -> Option<(&str, &Type)> {
        let mut crossed_fn = false;
        for s in self.scopes.iter().rev() {
            if crossed_fn {
                break;
            }
            if let Some((slot, ty)) = s.vars.get(name) {
                return Some((slot.as_str(), ty));
            }
            crossed_fn = s.is_fn;
        }
        None
    }

    /// 查找变量的类型（兼容便捷接口）。
    pub fn lookup_variable(&self, name: &str) -> Option<&Type> {
        self.resolve_variable(name).map(|(_, ty)| ty)
    }

    /// 记录变量绑定的初始化表达式，返回存储槽名（与 `insert_variable` 同步遮蔽）。
    pub fn insert_local_init(&mut self, name: String, init: HirExpr) -> String {
        let stored = self.stored_name(&name);
        self.scopes
            .last_mut()
            .expect("作用域栈为空")
            .inits
            .insert(name, init);
        stored
    }

    /// 查找变量绑定的初始化表达式。fn 边界不穿透。
    pub fn lookup_local_init(&self, name: &str) -> Option<&HirExpr> {
        let mut crossed_fn = false;
        for s in self.scopes.iter().rev() {
            if crossed_fn {
                break;
            }
            if let Some(h) = s.inits.get(name) {
                return Some(h);
            }
            crossed_fn = s.is_fn;
        }
        None
    }

    /// 记录 dyn 绑定变量的具体类型（H4 去虚拟化：`let d: dyn Trait = &obj;`）。
    pub fn insert_dyn_concrete(&mut self, name: String, type_: Type) {
        self.scopes
            .last_mut()
            .expect("作用域栈为空")
            .dyn_concrete
            .insert(name, type_);
    }

    /// 查找 dyn 绑定变量的具体类型。fn 边界不穿透。
    pub fn get_dyn_concrete(&self, name: &str) -> Option<&Type> {
        let mut crossed_fn = false;
        for s in self.scopes.iter().rev() {
            if crossed_fn {
                break;
            }
            if let Some(t) = s.dyn_concrete.get(name) {
                return Some(t);
            }
            crossed_fn = s.is_fn;
        }
        None
    }

    /// 移除 dyn 绑定变量的具体类型（变量被重新赋值时失效）。
    pub fn remove_dyn_concrete(&mut self, name: &str) {
        let mut crossed_fn = false;
        for s in self.scopes.iter_mut().rev() {
            if crossed_fn {
                break;
            }
            if s.dyn_concrete.remove(name).is_some() {
                return;
            }
            crossed_fn = s.is_fn;
        }
    }

    /// 记录一个函数签名。
    pub fn insert_fn_signature(&mut self, name: String, signature: FnSignature) {
        self.fn_signatures.insert(name, signature);
    }

    /// 查找函数签名。
    pub fn lookup_fn_signature(&self, name: &str) -> Option<&FnSignature> {
        self.fn_signatures.get(name)
    }

    /// 记录一个结构体定义。
    pub fn insert_struct(&mut self, name: String, def: StructDef) {
        self.structs.insert(name, def);
    }

    /// 查找结构体定义。
    pub fn lookup_struct(&self, name: &str) -> Option<&StructDef> {
        self.structs.get(name)
    }

    /// 记录一个类型别名。
    #[allow(dead_code)]
    pub fn insert_type_alias(&mut self, name: String, type_: Type) {
        self.type_aliases.insert(name, type_);
    }

    /// 记录一个 use 导入别名（本地名 → 完整符号名）。
    pub fn insert_use_alias(&mut self, local: String, full: String) {
        self.use_aliases.insert(local, full);
    }

    /// 记录一个模块常量（完整符号名 → (HIR 值, 类型)）。
    pub fn insert_constant(&mut self, name: String, value: HirExpr, type_: Type) {
        self.constants.insert(name, (value, type_));
    }

    /// 查找模块常量（支持短名 → 完整名解析，与 struct/actor 一致：
    /// 裸名查表，失败后回退 `import` 导入别名）。
    pub fn lookup_constant(&self, name: &str) -> Option<&(HirExpr, Type)> {
        if let Some(v) = self.constants.get(name) {
            return Some(v);
        }
        self.resolve_full_name(name)
            .and_then(|full| self.constants.get(&full))
    }

    /// 将局部名解析为完整符号名。
    ///
    /// 优先级：直接存在的符号名（函数 / 结构体 / actor / 常量 / 枚举）→ use 导入别名。
    /// 无法解析时返回 `None`（由调用方决定如何报错）。
    pub fn resolve_full_name(&self, name: &str) -> Option<String> {
        if self.structs.contains_key(name)
            || self.fn_signatures.contains_key(name)
            || self.actors.contains_key(name)
            || self.constants.contains_key(name)
            || self.enum_defs.contains_key(name)
        {
            return Some(name.to_string());
        }
        if let Some(a) = self.use_aliases.get(name) {
            return Some(a.clone());
        }
        // Q3a 修复：模块内 trait/impl 方法签名在收集阶段解析参数类型时 use 段
        // 尚未注册，模块内短名须按 `module::Name` 前缀定位（如 `fmt/module.rl` 中
        // `trait Display { fn fmt(&self, f: &mut Formatter) }`）。
        // 枚举同样按前缀定位（`protocol::Msg`），否则模块内裸名枚举类型注解
        // （`fn encode(m: Msg)`）与 match 模式解析失败。
        if !name.contains("::") && !self.module_prefix.is_empty() {
            let full = format!("{}::{}", self.module_prefix, name);
            if self.structs.contains_key(&full)
                || self.fn_signatures.contains_key(&full)
                || self.actors.contains_key(&full)
                || self.constants.contains_key(&full)
                || self.enum_defs.contains_key(&full)
            {
                return Some(full);
            }
        }
        None
    }

    /// 解析 trait 名的完整符号键（trait 未纳入 `resolve_full_name`，单独处理）。
    ///
    /// 依次尝试：1) 精确键；2) 当前模块前缀；3) 以 `::name` 结尾的 trait（如
    /// 用户写 `From` 引用 `io::error::From`）。P6c（2026-08-29）。
    pub(crate) fn resolve_trait_key(&self, name: &str) -> Option<String> {
        if self.trait_defs.contains_key(name) {
            return Some(name.to_string());
        }
        if let Some(full) = self.resolve_full_name(name) {
            if self.trait_defs.contains_key(&full) {
                return Some(full);
            }
        }
        self.trait_defs
            .keys()
            .find(|k| k == &name || k.ends_with(&format!("::{name}")))
            .cloned()
    }

    /// 按名字查找 actor 定义（支持短名 → 完整名解析，与 struct 一致）。
    pub fn lookup_actor(&self, name: &str) -> Option<&AstActorDecl> {
        self.resolve_full_name(name)
            .and_then(|full| self.actors.get(&full))
    }

    /// 解析一个具名类型（查结构体 / 枚举 / 别名 / 预置内置类型名）。
    ///
    /// 内置类型名（`i64`、`bool` 等）直接映射到 [`Type`]；
    /// 其余名称查结构体 / 枚举表与别名表，并支持 `math::Point` 形式的模块路径
    /// 与 `import` 导入的别名。当前作用域的泛型参数返回 [`Type::Generic`]；
    /// 若存在泛型替换表则返回替换后的具体类型。查不到时返回
    /// [`TypeError::UndefinedType`]。
    pub fn resolve_named_type(&self, name: &str, span: Span) -> Result<Type, TypeError> {
        // 泛型替换优先（实例化 body 检查时 `T` → 具体类型）
        if let Some(t) = self.generic_subst.get(name) {
            return Ok(t.clone());
        }
        // U4：`Self` 解析为当前 impl 目标类型（方法签名/body 收集时设置）；
        // trait 上下文（未设置）退化为占位 `Generic("Self")`
        if name == "Self" {
            if let Some(t) = &self.self_type {
                return Ok(t.clone());
            }
            return Ok(Type::Generic(name.to_string()));
        }
        if self.type_params.iter().any(|p| p == name) {
            return Ok(Type::Generic(name.to_string()));
        }
        let builtin = builtin_type(name);
        if let Some(t) = builtin {
            return Ok(t);
        }
        if let Some(alias) = self.type_aliases.get(name) {
            return Ok(alias.clone());
        }
        if self.structs.contains_key(name) {
            return Ok(Type::Named(name.to_string(), Vec::new()));
        }
        if self.enum_defs.contains_key(name) {
            let full = name.to_string();
            // U3 核心项（2026-08-30）：受限标量枚举（全单元变体、无泛型）紧凑为
            // 单标量存储，值即 tag。此处即唯一判定点——收集阶段 enum 已注册，
            // 故 `is_scalar_enum` 时序正确；主 ctx 与 `collect_fn_signatures` 接口路径
            // 共用本函数，保证两种路径产出的类型表示一致。
            if self.is_scalar_enum(&full) {
                return Ok(Type::ScalarEnum(full));
            }
            return Ok(Type::Named(full, Vec::new()));
        }
        // 模块路径（如 `math::Point`）或 use 导入的别名
        if let Some(full) = self.resolve_full_name(name) {
            if self.structs.contains_key(&full) {
                return Ok(Type::Named(full, Vec::new()));
            }
            if self.enum_defs.contains_key(&full) {
                if self.is_scalar_enum(&full) {
                    return Ok(Type::ScalarEnum(full));
                }
                return Ok(Type::Named(full, Vec::new()));
            }
        }
        Err(TypeError::UndefinedType {
            name: name.to_string(),
            span,
        })
    }

    /// 记录一个枚举定义。
    pub fn insert_enum(&mut self, name: String, def: EnumDef) {
        for variant in &def.variants {
            self.variant_index
                .insert(variant.name.clone(), (name.clone(), variant.name.clone()));
        }
        self.enum_defs.insert(name, def);
    }

    /// 查找枚举定义。
    pub fn lookup_enum(&self, name: &str) -> Option<&EnumDef> {
        self.enum_defs.get(name)
    }

    /// U3（2026-08-30）：判定具名枚举是否为**受限标量枚举**——无泛型参数，
    /// 且所有变体均为单元变体（不携带负载）。
    ///
    /// 收集阶段 enum 已注册，故本判定在 `resolve_named_type` 解析类型时
    /// 时序正确；主类型检查 ctx 与 `collect_fn_signatures` 接口路径共用同一
    /// `resolve_named_type`，保证两种路径产出的类型表示一致（均为 `Type::ScalarEnum`）。
    pub fn is_scalar_enum(&self, name: &str) -> bool {
        let Some(def) = self.enum_defs.get(name) else {
            return false;
        };
        def.type_params.is_empty() && def.variants.iter().all(|v| v.fields.is_empty())
    }

    /// 记录一个 trait 定义。
    pub fn insert_trait(&mut self, name: String, def: TraitDef) {
        self.trait_defs.insert(name, def);
    }

    /// 查找 trait 定义。
    #[allow(dead_code)]
    pub fn lookup_trait(&self, name: &str) -> Option<&TraitDef> {
        self.trait_defs.get(name)
    }

    /// 记录一个 impl 块。
    pub fn insert_impl(&mut self, def: ImplDef) {
        self.impl_defs.push(def);
    }

    /// 按目标类型解析变体名：`EnumName::Variant` 或裸 `Variant`。
    ///
    /// 返回 `(枚举名, 变体名)`。
    pub fn resolve_variant(&self, enum_name: Option<&str>, variant: &str) -> Option<(String, String)> {
        if let Some(en) = enum_name {
            if self.enum_defs.contains_key(en) {
                let def = self.enum_defs.get(en)?;
                if def.variants.iter().any(|v| v.name == variant) {
                    return Some((en.to_string(), variant.to_string()));
                }
            }
            return None;
        }
        self.variant_index
            .get(variant)
            .map(|(e, v)| (e.clone(), v.clone()))
    }

    /// 分配一个临时变量名（用于 match 展开 / 聚合构造）。
    pub fn fresh_temp(&mut self) -> String {
        let n = self.temp_counter;
        self.temp_counter += 1;
        format!("__tmp{n}")
    }

    /// 在 inherent impl 中按目标类型查找方法所属的 impl 块。
    #[allow(dead_code)]
    pub fn find_impl(&self, self_type: &Type) -> Option<&ImplDef> {
        self.impl_defs
            .iter()
            .find(|d| d.trait_name.is_none() && type_matches(d, self_type))
    }

    /// 在 trait impl 中按目标类型查找方法所属的 impl 块。
    #[allow(dead_code)]
    pub fn find_trait_impl(&self, self_type: &Type) -> Option<&ImplDef> {
        self.impl_defs
            .iter()
            .find(|d| d.trait_name.is_some() && type_matches(d, self_type))
    }

    /// 按目标类型查找含指定方法的 impl 块（inherent 优先，trait 次之）。
    pub fn find_impl_for_method(&self, self_type: &Type, method: &str) -> Option<&ImplDef> {
        self.impl_defs
            .iter()
            .find(|d| type_matches(d, self_type) && d.methods.iter().any(|m| m.sig.name == method))
    }

    /// X4：按目标类型 + trait 名查找含指定方法的 trait impl 块。
    /// 用于同名方法分属不同 trait 时（如 `Display::fmt` 与 `Debug::fmt`），
    /// 按 trait 名精确区分；`trait_name` 为解析后的完整符号名（如 `fmt::Display`）。
    pub fn find_impl_for_trait_method(
        &self,
        self_type: &Type,
        trait_name: &str,
        method: &str,
    ) -> Option<&ImplDef> {
        self.impl_defs.iter().find(|d| {
            d.trait_name.as_deref() == Some(trait_name)
                && type_matches(d, self_type)
                && d.methods.iter().any(|m| m.sig.name == method)
        })
    }

    /// V3 trait 默认方法回退（2026-08-26）：`find_impl_for_method` 找不到"实现了
    /// 该方法的 impl"时，寻找类型匹配且是 trait impl、且该 trait 声明了 `method`
    /// 默认实现的 impl。返回的 impl 用于确定 self 类型与泛型统一，方法定义
    /// （trait 默认 body）由 `check_method_call` 的 `trait_default_method` 构造。
    pub fn find_trait_default_impl(&self, self_type: &Type, method: &str) -> Option<&ImplDef> {
        self.impl_defs.iter().find(|d| {
            if !type_matches(d, self_type) {
                return false;
            }
            let Some(tname) = d.trait_name.as_deref() else {
                return false;
            };
            self.trait_defs.get(tname).is_some_and(|t| {
                t.methods
                    .iter()
                    .any(|m| m.name == method && m.default_body.is_some())
            })
        })
    }

    /// 当前作用域是否为泛型参数。
    #[allow(dead_code)]
    pub fn is_type_param(&self, name: &str) -> bool {
        self.type_params.iter().any(|p| p == name)
    }
}

/// impl 块目标类型与具体类型匹配（未含泛型参数的 impl 需精确匹配；
/// 含泛型参数的 impl 匹配同名类型，参数在调用点替换）。
pub(crate) fn type_matches(imp: &ImplDef, concrete: &Type) -> bool {
    // P6c（2026-08-29）：blanket impl（`impl<T, U> Into<U> for T`——self_type 为裸
    // 泛型参数）可匹配任意具体类型；类型参数在调用点按 turbofish / 实参替换。
    // （`impl<T> Bag<T>` 的 self_type 是 `Named("Bag",[T])`，不受本分支影响。）
    if matches!(&imp.self_type, Type::Generic(_)) {
        return true;
    }
    let Type::Named(name, _) = &imp.self_type else {
        return false;
    };
    match concrete {
        Type::Named(cname, _) => name == cname,
        Type::Ref(inner, _) => type_matches(imp, inner),
        _ => false,
    }
}

/// 将内置类型名解析为 [`Type`]。
fn builtin_type(name: &str) -> Option<Type> {
    Some(match name {
        "i8" => Type::I8,
        "i16" => Type::I16,
        "i32" => Type::I32,
        "i64" => Type::I64,
        "i128" => Type::I128,
        "isize" => Type::ISize,
        "u8" => Type::U8,
        "u16" => Type::U16,
        "u32" => Type::U32,
        "u64" => Type::U64,
        "u128" => Type::U128,
        "usize" => Type::USize,
        "f32" => Type::F32,
        "f64" => Type::F64,
        "bool" => Type::Bool,
        "char" => Type::Char,
        "string" => Type::Str,
        "()" | "unit" => Type::Unit,
        _ => return None,
    })
}
