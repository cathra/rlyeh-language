//! 类型检查上下文（符号表）。

use std::collections::HashMap;

use rlyeh_hir::HirExpr;
use rlyeh_lexer::Span;

use rlyeh_ast::{AstActorDecl, AstExpr, AstFnDecl};

use crate::error::TypeError;
use crate::types::{EnumDef, FnSignature, ImplDef, StructDef, TraitDef, Type};

/// 可见性检查模式（P2 灰度开关，由 `--visibility` 控制）。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VisibilityMode {
    /// 关闭（默认）：跨模块私有符号可自由访问，兼容存量代码 / 标准库。
    Off,
    /// 告警：发现私有符号跨模块访问时仅警告，不报错（std 迁移期收集缺口用）。
    Warn,
    /// 严格：私有符号跨模块访问报错 `PrivateItem`。
    Error,
}

impl Default for VisibilityMode {
    fn default() -> Self {
        VisibilityMode::Off
    }
}

impl VisibilityMode {
    /// 从 CLI 字符串解析（`off`/`warn`/`error`）。
    pub fn parse(s: &str) -> Option<VisibilityMode> {
        match s {
            "off" | "Off" | "0" => Some(VisibilityMode::Off),
            "warn" | "Warn" | "1" => Some(VisibilityMode::Warn),
            "error" | "Error" | "2" => Some(VisibilityMode::Error),
            _ => None,
        }
    }
}
use crate::Warning;

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
    /// Q-M2（SH-P0-8）：本层变量的**声明顺序**（原名）。`vars` 是 `HashMap`
    /// 无序，而析构必须按逆声明序（Rust 语义），故另存一份顺序表。
    pub decl_order: Vec<String>,
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
    /// 全局变量表（`static` / `static mut`；完整符号名 → (类型, 是否可变)）。
    /// 与 `constants` 分离：`static` 需要 data 段符号（可寻址、可赋值），
    /// 而 `const` 仅在引用处内联。
    pub globals: HashMap<String, (Type, bool)>,
    /// Actor 定义表（完整符号名 → 声明，如 `"Counter"` / `"math::Counter"`）
    pub actors: HashMap<String, AstActorDecl>,
    /// 已生成的 `rlyeh_actor_*` extern 声明名（actor 展开去重用）
    pub generated_actor_externs: std::collections::HashSet<String>,
    /// 已生成的 `rlyeh_gc_*` extern 声明名（K4 追踪 GC 展开去重用）
    pub generated_gc_externs: std::collections::HashSet<String>,
    /// 当前检查的模块前缀（顶层为空串，`mod math` 内为 `"math"`）
    pub module_prefix: String,
    /// B-6：标注 `#[memory(gc)]` 的模块前缀集合（含嵌套继承），供引用→`Gc<T>`
    /// 默认映射判定（见 `in_gc_module`）。
    pub gc_modules: std::collections::HashSet<String>,
    /// 已声明的模块路径集合（`"io"` / `"io::base"` 等），供 `resolve_import_path`
    /// 区分相对子模块导入与跨模块绝对路径导入（见 `register_use`）。在 `module X;`
    /// 声明即登记，不依赖符号是否已被收集，从而解耦注册时机。
    pub modules: std::collections::HashSet<String>,
    /// 对外公共模块面（`pub module` 声明的模块前缀集合），供可见性检查（P2）判定
    /// 模块本身是否可被外部导入者经 `import parent::name::item` 访问。增量放开语义下
    /// A 阶段仅记录、不强制；C 阶段 `--visibility=error` 时据此收紧。
    pub pub_module_prefixes: std::collections::HashSet<String>,
    /// glob 导入来源记录（`name` → 导入它的模块前缀列表）；同名来自 ≥2 个模块即歧义。
    pub glob_exports: std::collections::HashMap<String, Vec<String>>,
    /// 显式 import 的本地别名 → 是否为 `pub` 导入（`pub import` 重导出链不计入冲突，
    /// 仅非 `pub` 的同名冲突才报 `NameConflict`，避免误伤标准库的重导出链）。用于 glob
    /// 歧义判定时排除显式命名（显式优先，不视为歧义）。
    pub explicit_import_sources: std::collections::HashMap<String, bool>,
    /// import 别名记录（`local` → `full` 目标 + 源位置），供收集全部完成后的延迟校验
    /// （`verify_imports`）判定目标符号 / 模块是否存在，避免模块收集时序导致的误报。
    pub import_alias_spans: Vec<(String, String, Span)>,
    /// 可见性检查模式（P2），默认 `Off`（兼容存量代码 / 标准库）。
    pub visibility: VisibilityMode,
    /// 对外公共符号面（`pub fn`/`struct`/`enum`/`const`/`static`/`actor`/`trait`/`import` 的
    /// 全名）；跨模块访问私有符号（`--visibility=error`）时据此外部可达性判定。
    pub pub_symbols: std::collections::HashSet<String>,
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
    /// 当前 `let`/`return` 等上下文期望的类型（U-M3：下传到 trait 关联函数调用，
    /// 作为协议静态方法 `Self` 的候选 `self_target`，使其可按 `let x: T = Trait::f()`
    /// 的 `T` 对齐返回类型；无期望时保持 `None`，退化为从 impl 自推断）。
    pub expected_type: Option<Type>,
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
    /// 类型检查期间收集的建议性警告（非致命；见 [`crate::Warning`]）。
    pub warnings: Vec<Warning>,
}

/// 编辑距离（Levenshtein，截断到 0..=2 以适配候选建议）。
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let n = a.len();
    let m = b.len();
    if n == 0 {
        return m;
    }
    if m == 0 {
        return n;
    }
    let mut prev: Vec<usize> = (0..=m).collect();
    let mut cur = vec![0usize; m + 1];
    for i in 1..=n {
        cur[0] = i;
        for j in 1..=m {
            let cost = if a[i - 1] == b[j - 1] { 0 } else { 1 };
            cur[j] = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[m]
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

    /// 提交一条建议性警告。
    ///
    /// 按 `(位置, 种类)` 去重，避免泛型实例化对同一源码位置重复触发
    /// （实例化 AST 与原模板共享 `span`）。
    pub fn emit_warning(&mut self, w: Warning) {
        let dup = self.warnings.iter().any(|e| {
            e.span == w.span && std::mem::discriminant(&e.kind) == std::mem::discriminant(&w.kind)
        });
        if !dup {
            self.warnings.push(w);
        }
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
        let scope = self.scopes.last_mut().expect("作用域栈为空");
        // 同层重复绑定（`let x` 覆盖）沿用同一槽名，声明顺序不重复登记
        if !scope.vars.contains_key(&name) {
            scope.decl_order.push(name.clone());
        }
        scope.vars.insert(name, (stored.clone(), type_));
        stored
    }

    /// Q-M2（SH-P0-8）：当前作用域**本层**声明的变量，按声明顺序返回
    /// `(原名, 存储槽名, 类型)`。
    ///
    /// 块尾析构据此按逆声明序插入 `x.drop()`；作用域弹出时本层变量随之消失，
    /// 故外层变量不会被误析构。
    pub fn current_scope_decls(&self) -> Vec<(String, String, Type)> {
        let Some(scope) = self.scopes.last() else {
            return Vec::new();
        };
        scope
            .decl_order
            .iter()
            .filter_map(|n| {
                scope
                    .vars
                    .get(n)
                    .map(|(slot, ty)| (n.clone(), slot.clone(), ty.clone()))
            })
            .collect()
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

    /// 记录一个显式 import 的本地别名及其 `pub` 属性（用于 glob 歧义 / 冲突判定）。
    pub fn record_explicit_import(&mut self, local: String, is_pub: bool) {
        self.explicit_import_sources.insert(local, is_pub);
    }

    /// 返回 `name` 因两个 glob 导入同名而歧义的源模块列表；无歧义或已被显式
    /// import 消除时返回 `None`（显式优先于 glob）。
    pub fn glob_ambiguity_sources(&self, name: &str) -> Option<Vec<String>> {
        if self.explicit_import_sources.contains_key(name) {
            return None;
        }
        let srcs = self.glob_exports.get(name)?;
        if srcs.len() >= 2 {
            Some(srcs.clone())
        } else {
            None
        }
    }

    /// 为未找到的名称生成拼写相近候选（编辑距离 ≤ 2 的已知符号 / 模块名，取前 3）。
    pub fn name_candidates(&self, name: &str) -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        all.extend(self.structs.keys().cloned());
        all.extend(self.fn_signatures.keys().cloned());
        all.extend(self.fn_templates.keys().cloned());
        all.extend(self.enum_defs.keys().cloned());
        all.extend(self.constants.keys().cloned());
        all.extend(self.actors.keys().cloned());
        all.extend(self.modules.iter().cloned());
        all.extend(self.trait_defs.keys().cloned());
        let mut cand: Vec<(usize, String)> = all
            .into_iter()
            .filter(|s| s != name)
            .map(|s| (edit_distance(name, &s), s))
            .filter(|(d, _)| *d <= 2)
            .collect();
        cand.sort_by_key(|(d, _)| *d);
        cand.into_iter().map(|(_, s)| s).take(3).collect()
    }

    /// 可见性检查（P2）：跨模块访问私有符号时按 `--visibility` 模式报错。
    /// `def_full` 为解析后的完整符号名（如 `m::secret`）；本地 / 同模块符号直接放行。
    /// `Off` 模式为 no-op（兼容存量代码与标准库）。
    pub fn check_visibility(&self, def_full: &str, span: Span) -> Result<(), TypeError> {
        use crate::VisibilityMode::*;
        if matches!(self.visibility, Off) {
            return Ok(());
        }
        // 本地 / 同作用域符号（无 `::`）不参与跨模块可见性检查。
        let Some(pos) = def_full.rfind("::") else {
            return Ok(());
        };
        let def_module = &def_full[..pos];
        let caller = &self.module_prefix;
        let same_or_descendant =
            caller == def_module || caller.starts_with(&format!("{def_module}::"));
        if same_or_descendant || self.pub_symbols.contains(def_full) {
            return Ok(());
        }
        Err(TypeError::PrivateItem {
            name: def_full.to_string(),
            span,
        })
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

    /// 记录一个全局变量（`static` / `static mut`；完整符号名 → (类型, 是否可变)）。
    pub fn insert_global(&mut self, name: String, type_: Type, is_mut: bool) {
        self.globals.insert(name, (type_, is_mut));
    }

    /// 查找全局变量（支持短名 → 完整名解析，与常量一致）。
    ///
    /// 返回 `(类型, 是否可变)`；`static mut` 为 `true`。
    pub fn lookup_global(&self, name: &str) -> Option<(Type, bool)> {
        if let Some(v) = self.globals.get(name) {
            return Some(v.clone());
        }
        self.resolve_full_name(name)
            .and_then(|full| self.globals.get(&full).cloned())
    }

    /// 将局部名解析为完整符号名。
    ///
    /// 优先级：直接存在的符号名（函数 / 结构体 / actor / 常量 / 枚举）→ use 导入别名
    /// （含 `pub use` 重导出的多级链，如 `c → M::c → a::b`，经 `use_aliases` 传递追踪
    /// 直至命中真实符号或回退模块前缀；`visited` 防环）。
    /// 无法解析时返回 `None`（由调用方决定如何报错）。
    /// B-6：当前 `module_prefix` 是否处于 `#[memory(gc)]` 模块（含嵌套继承）。
    pub fn in_gc_module(&self) -> bool {
        self.gc_modules.iter().any(|p| {
            self.module_prefix == *p || self.module_prefix.starts_with(&format!("{p}::"))
        })
    }

    pub fn resolve_full_name(&self, name: &str) -> Option<String> {
        let is_direct = |k: &str| {
            self.structs.contains_key(k)
                || self.fn_signatures.contains_key(k)
                || self.actors.contains_key(k)
                || self.constants.contains_key(k)
                || self.enum_defs.contains_key(k)
                || self.fn_templates.contains_key(k)
        };
        let mut current = name.to_string();
        let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
        loop {
            if is_direct(&current) {
                return Some(current);
            }
            match self.use_aliases.get(&current) {
                Some(next) if !visited.contains(next) => {
                    visited.insert(next.clone());
                    current = next.clone();
                }
                _ => break,
            }
        }
        // Q3a 修复：模块内 trait/impl 方法签名在收集阶段解析参数类型时 use 段
        // 尚未注册，模块内短名须按 `module::Name` 前缀定位（如 `fmt/module.rl` 中
        // `trait Display { fn fmt(&self, f: &mut Formatter) }`）。
        // 枚举同样按前缀定位（`protocol::Msg`），否则模块内裸名枚举类型注解
        // （`fn encode(m: Msg)`）与 match 模式解析失败。
        if !name.contains("::") && !self.module_prefix.is_empty() {
            let full = format!("{}::{}", self.module_prefix, name);
            if is_direct(&full) {
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
        // glob 同名歧义：两个 `import a::*` 均导出 `name` 时，作为类型使用处直接报错（惰性）。
        // 置于类型实参 / 别名解析之后，使同名类型参数优先（不被误判为歧义）。
        if let Some(srcs) = self.glob_ambiguity_sources(name) {
            return Err(TypeError::GlobAmbiguity {
                name: name.to_string(),
                sources: srcs,
                span,
            });
        }
        // P2 可见性：跨模块且私有的具名类型，`--visibility=error` 报 PrivateItem。
        self.check_visibility(name, span)?;
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
        // 模块化兜底：参见 `resolve_named_type_suffix`。
        if let Some(k) = self.resolve_named_type_suffix(name) {
            if self.enum_defs.contains_key(&k) && self.is_scalar_enum(&k) {
                return Ok(Type::ScalarEnum(k));
            }
            return Ok(Type::Named(k, Vec::new()));
        }
        Err(TypeError::UndefinedType {
            name: name.to_string(),
            span,
        })
    }

    /// 模块化后缀兜底（2026-09-18）：标准库按子模块拆分后，跨模块引用可能仍写拆分前的
    /// 短路径（如 `fmt::FmtError` 实际注册为 `fmt::error::FmtError`），而签名收集阶段
    /// 早于模块内 `pub import` 的别名生效。此处在**唯一**后缀匹配时接受该名，返回规范类型
    /// 的全名，避免为每个子模块强绑别名注册时机。歧义（多个同名）返回 None。
    pub(crate) fn resolve_named_type_suffix(&self, name: &str) -> Option<String> {
        if !name.contains("::") {
            return None;
        }
        let suffix = format!("::{}", name.rsplit("::").next().unwrap_or_default());
        if suffix == "::" {
            return None;
        }
        let mut hit: Option<String> = None;
        let mut ambiguous = false;
        for k in self.structs.keys().chain(self.enum_defs.keys()) {
            if k.ends_with(&suffix) {
                if hit.is_some() {
                    ambiguous = true;
                    break;
                }
                hit = Some(k.clone());
            }
        }
        if ambiguous {
            return None;
        }
        hit
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
            .find(|d| d.trait_name.is_none() && type_matches(d, &self_type))
    }

    /// 在 trait impl 中按目标类型查找方法所属的 impl 块。
    #[allow(dead_code)]
    pub fn find_trait_impl(&self, self_type: &Type) -> Option<&ImplDef> {
        self.impl_defs
            .iter()
            .find(|d| d.trait_name.is_some() && type_matches(d, &self_type))
    }

    /// 按目标类型查找含指定方法的 impl 块（inherent 优先，trait 次之）。
    /// 把类型名规范化：经别名链 + 后缀兜底（见 `resolve_named_type_suffix`）映射到
    /// 规范符号名。用于 impl / 方法查找时统一查询类型与注册 impl 的 `self_type`
    /// （标准库按子模块拆分后，查询方常持别名 `sync::Mutex`，而 impl 注册为
    /// `sync::mutex::Mutex`；不规范化则 `type_matches` 直比失配）。
    pub(crate) fn canonical_type(&self, t: &Type) -> Type {
        match t {
            Type::Named(name, args) => {
                let c = self
                    .resolve_full_name(name)
                    .or_else(|| self.resolve_named_type_suffix(name))
                    .unwrap_or_else(|| name.clone());
                Type::Named(c, args.clone())
            }
            Type::ScalarEnum(name) => {
                let c = self
                    .resolve_full_name(name)
                    .or_else(|| self.resolve_named_type_suffix(name))
                    .unwrap_or_else(|| name.clone());
                Type::ScalarEnum(c)
            }
            _ => t.clone(),
        }
    }

    pub fn find_impl_for_method(&self, self_type: &Type, method: &str) -> Option<&ImplDef> {
        let self_type = self.canonical_type(self_type);
        self.impl_defs
            .iter()
            .find(|d| type_matches(d, &self_type) && d.methods.iter().any(|m| m.sig.name == method))
    }

    /// A2（SH-P1-1，2026-09-02）：按目标类型 + 方法名查找**全部**匹配的 impl
    /// （inherent 优先于 trait）。用于同一 `self_type` 上同一泛型 trait 的**多
    /// impl**（如 `impl Wrap<i64> for W` 与 `impl Wrap<bool> for W`），解析点
    /// 需按 trait 类型实参 / 实参类型选取正确的 impl，而非首匹配。
    pub fn find_impl_candidates(&self, self_type: &Type, method: &str) -> Vec<ImplDef> {
        let self_type = self.canonical_type(self_type);
        let mut inherent = Vec::new();
        let mut trait_impls = Vec::new();
        for d in &self.impl_defs {
            if type_matches(d, &self_type) && d.methods.iter().any(|m| m.sig.name == method) {
                if d.trait_name.is_none() {
                    inherent.push(d.clone());
                } else {
                    trait_impls.push(d.clone());
                }
            }
        }
        inherent.extend(trait_impls);
        inherent
    }

    /// A2：按 trait 名 + 目标类型 + 方法名查找全部匹配的 trait impl（X4
    /// `trait_hint` 路径的候选集）。
    pub fn find_trait_method_candidates(
        &self,
        self_type: &Type,
        trait_name: &str,
        method: &str,
    ) -> Vec<ImplDef> {
        let self_type = self.canonical_type(self_type);
        self.impl_defs
            .iter()
            .filter(|d| {
                names_match(d.trait_name.as_deref().unwrap_or(""), trait_name)
                    && type_matches(d, &self_type)
                    && d.methods.iter().any(|m| m.sig.name == method)
            })
            .cloned()
            .collect()
    }

    /// X4：按目标类型 + trait 名查找含指定方法的 trait impl 块。
    /// 用于同名方法分属不同 trait 时（如 `Display::fmt` 与 `Debug::fmt`），
    /// 按 trait 名精确区分；`trait_name` 为解析后的完整符号名（如 `fmt::Display`）。
    ///
    /// 匹配经 [`names_match`]：先**精确**、未命中再**短名等价**——标准库按
    /// 子模块拆分后，impl 注册名可能是 `fmt::display::Display`，而调用方（如
    /// 占位符引擎）仍以 `fmt::Display` 查询。
    pub fn find_impl_for_trait_method(
        &self,
        self_type: &Type,
        trait_name: &str,
        method: &str,
    ) -> Option<&ImplDef> {
        let self_type = self.canonical_type(self_type);
        // 精确匹配优先（避免同名 trait 串味）
        if let Some(d) = self.impl_defs.iter().find(|d| {
            d.trait_name.as_deref() == Some(trait_name)
                && type_matches(d, &self_type)
                && d.methods.iter().any(|m| m.sig.name == method)
        }) {
            return Some(d);
        }
        // 短名等价兜底（模块化后注册名可能是 `fmt::display::Display`）
        self.impl_defs.iter().find(|d| {
            names_match(d.trait_name.as_deref().unwrap_or(""), trait_name)
                && type_matches(d, &self_type)
                && d.methods.iter().any(|m| m.sig.name == method)
        })
    }

    /// V3 trait 默认方法回退（2026-08-26）：`find_impl_for_method` 找不到"实现了
    /// 该方法的 impl"时，寻找类型匹配且是 trait impl、且该 trait 声明了 `method`
    /// 默认实现的 impl。返回的 impl 用于确定 self 类型与泛型统一，方法定义
    /// （trait 默认 body）由 `check_method_call` 的 `trait_default_method` 构造。
    pub fn find_trait_default_impl(&self, self_type: &Type, method: &str) -> Option<&ImplDef> {
        self.impl_defs.iter().find(|d| {
            if !type_matches(d, &self_type) {
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
/// trait 名等价判定：**精确相等**，或**最后一段相同**（模块化兼容）。
///
/// 标准库把 `fmt::Display` 下沉为子模块 `fmt::display::Display` 后，impl 的注册名与
/// 调用方的查询名（占位符引擎、`#[derive]` 展开仍写 `fmt::Display`）可能处于不同
/// 层级，故提供短名兜底；精确比较在调用点优先尝试
/// （见 [`TypeContext::find_impl_for_trait_method`]）。
pub(crate) fn names_match(registered: &str, query: &str) -> bool {
    if registered == query {
        return true;
    }
    if registered.is_empty() || query.is_empty() {
        return false;
    }
    registered.rsplit("::").next() == query.rsplit("::").next()
}

pub(crate) fn type_matches(imp: &ImplDef, concrete: &Type) -> bool {
    // P6c（2026-08-29）：blanket impl（`impl<T, U> Into<U> for T`——self_type 为裸
    // 泛型参数）可匹配任意具体类型；类型参数在调用点按 turbofish / 实参替换。
    // （`impl<T> Bag<T>` 的 self_type 是 `Named("Bag",[T])`，不受本分支影响。）
    if matches!(&imp.self_type, Type::Generic(_)) {
        return true;
    }
    // U-M3：按「类型名键」做等价比较——既保留 `Named` 泛型 impl 的精确匹配
    // （名 + 类型实参递归），又使原始类型 impl（如 `impl i64: Default` 的
    // self_type=`Named("i64")` 与注解 / 字面量解析出的 `Type::I64`）互通。
    same_type(&imp.self_type, concrete)
}

/// 类型名键：命名类型取类型名，原始类型取规范名（如 `I64`→`"i64"`）。
/// 用于 `type_matches` 的等价比较，使 `Named("i64")` 与 `Type::I64` 互通。
fn type_name_key(t: &Type) -> Option<String> {
    match t {
        Type::Named(n, _) => Some(n.clone()),
        Type::I8 => Some("i8".into()),
        Type::I16 => Some("i16".into()),
        Type::I32 => Some("i32".into()),
        Type::I64 => Some("i64".into()),
        Type::I128 => Some("i128".into()),
        Type::ISize => Some("isize".into()),
        Type::U8 => Some("u8".into()),
        Type::U16 => Some("u16".into()),
        Type::U32 => Some("u32".into()),
        Type::U64 => Some("u64".into()),
        Type::U128 => Some("u128".into()),
        Type::USize => Some("usize".into()),
        Type::F32 => Some("f32".into()),
        Type::F64 => Some("f64".into()),
        Type::Bool => Some("bool".into()),
        Type::Char => Some("char".into()),
        Type::Str => Some("string".into()),
        Type::Unit => Some("()".into()),
        _ => None,
    }
}

/// 两个类型在 `type_matches` 意义上等价：解 Ref 层后，命名类型按名 + 类型实参递归
/// 比较；原始类型按规范名与同名 `Named` 互通；其余（元组 / 数组 / 函数等）无名称键，不等。
fn strip_ref<'a>(t: &'a Type) -> &'a Type {
    match t {
        Type::Ref(x, _, _) => &**x,
        o => o,
    }
}

fn same_type(a: &Type, b: &Type) -> bool {
    // 泛型参数作通配（如 `impl Vec<T>` 的 `Vec<T>` 匹配具体 `Vec<String>`，T 在调用点统一）
    if matches!(a, Type::Generic(_)) || matches!(b, Type::Generic(_)) {
        return true;
    }
    let a = strip_ref(a);
    let b = strip_ref(b);
    match (a, b) {
        // 命名类型按名匹配（兼容旧行为：`impl<T> MyMutex<T>` 依名字匹配 `MyMutex`，
        // 类型实参在调用点统一；实参精确比较会破坏泛型 impl 匹配，故仅比名）。
        (Type::Named(n1, _), Type::Named(n2, _)) => n1 == n2,
        // 命名类型 ↔ 原始变体 / 原始变体 ↔ 原始变体：按规范名键互通
        // （`impl i64: Trait` 的 self_type=`Named("i64")` 与注解 / 字面量解析出的 `Type::I64`）。
        _ => {
            let k1 = type_name_key(a);
            let k2 = type_name_key(b);
            k1.is_some() && k1 == k2
        }
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
