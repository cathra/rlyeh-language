# Rlyeh 语言语法规范 (EBNF)

> 版本：0.1.0  
> 最后更新：2026-08-31

> **⚠️ 实现状态**：本文为**目标语法规范**（EBNF），其中部分语法为规划特性，MVP 编译器尚未实现：
> 宏系统（§2.14 `macro_rules!` 已实现：`$x:expr`/`ident`/`ty`/`tt` + `$(`...`)` 重复，parse 期 AST 展开；
> 内置格式化宏 `println!`/`print!`/`format!`/`dbg!` 与集合宏 `arr!`/`vec!`/`map!` 均已实现（I3 ✅，parse 期 desugar，见 §2.14 下方说明）、
> 闭包 `|x| ...`（H2/H3/H5 ✅ 已实现，见 guide/03-basic-syntax.md §3.8；typecheck 按预期 fn 签名/捕获检查）、引用类型 `&T` 已实现
> （G1 ✅：`&x`/`&mut x` 表达式、`&T`/`&mut T` 参数与返回、`*` 解引用；`&str` 只读借用视图已实现
> （G2 ✅：`as_str()` + `&str` 参数/返回/索引 + `String::from(&str)`），裸指针 `*const T`/`*mut T` 已实现（G3 ✅））、
> `dyn Trait`（H4 ✅）、`?` 运算符（K1 ✅）、`expr as T` 数值转换（U6 ✅：≤64 位整族 + 浮点 + bool + char
> 间转换，`fptosi`/`sitofp`/`trunc`/`sext`/`zext`/`icmp ne 0` 等，见 guide/03-basic-syntax.md §3.2）、生命周期参数 `'a` 等。
> 普通函数 `async fn`/`await` 已实现（S1c ✅：`FnDecl`/`ActorMethod` 的 `async?` 与 `expr.await` 语法全程接受，
> 状态机 desugar——`async fn` 编译为 Future 结构体 + poll 状态机 + 构造器，`expr.await` 经状态机轮询子 future，
> 支持 `Poll::Pending` 挂起/恢复与跨 await 变量提升；参数 `i64`、返回 `i64`/`()`；控制流块内 / 表达式嵌套 await 规划）。
> JSON 序列化已实现（L2 ✅：`json::stringify(v)` 与 `json::parse::<T>(s)` 内建，turbofish 泛型实参
> `::<T>`（PostfixOp `'::' '<' TypeList '>' '(' ArgList? ')'`，parser 三 token 前瞻检测；嵌套泛型
> `>>` 拆分层）；typecheck 期 desugar 为 String 构建/解析表达式，零新增 IR 节点；支持标量/数组/struct/Vec/
> HashMap 序列化（L2f）与 i64/bool/String/HashMap 反序列化（L2g）；自定义 `Serialize`/`Deserialize` 仍规划）。
> 平台加固已实现（L4 ✅：WASI（`__rlyeh_target_os` 码 5）下 net 模块网络函数明确禁用短路返回；actor 交叉编译 /
> WASM 支持——`wasm32-wasip1` 目标下 driver 注入静态 `rlyeh_actor_resolve` 符号表替代 dlsym，actor 语法
> （§2.7）与受监督语义全程可用，详见 guide/11-targets-toolchain.md §11.3）。
> **已实现子集的教程与可运行示例见 [`guide/index.md`](./guide/index.md)，已知限制见 [`guide/13-references-limits.md`](./guide/13-references-limits.md) §13。**

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [01_词法分析器](design/01_词法分析器.md) / [02_语法分析器](design/02_语法分析器.md) / [06_比较链与条件判断](design/06_比较链与条件判断.md) | 模块设计 |
| 实现纪要 | [附录 A](#附录-a实现纪要)（原 P001–P003，已归档至 [design/prompts/](design/prompts/)） | 词法/语法/比较链落地状态 |

---

## 1. 词法规则

### 1.1 空白与注释

```
Whitespace  ::= ' ' | '\t' | '\n' | '\r'
Comment     ::= '//' ~[\n]* '\n'
BlockComment ::= '/*' (BlockComment | ~'*/')* '*/'
```

### 1.2 标识符

```
Ident       ::= (Letter | '_') (Letter | Digit | '_')*
Letter      ::= 'a'..'z' | 'A'..'Z'
Digit       ::= '0'..'9'
RawIdent    ::= 'r#' Ident
```

> `RawIdent`（原始标识符 `r#foo`）双重语义：
> 1. **关键字转义**：`send`/`recv`/`type` 等保留字可用作标识符（如 `extern fn r#send`、调用 `r#send(...)`）；
> 2. **根命名空间显式引用**：在模块内部调用 `r#foo(...)` 时跳过模块内优先解析（`resolve_callable`），
>    直接绑定**根命名空间**的 `foo`——模块内 `fn foo` 与之同名不构成遮蔽
>    （例：fs 模块内 `fn rename` 与根 extern `r#rename` 同名，`r#rename(...)` 绑定根 extern，
>    裸名 `rename(...)` 绑定 `fs::rename`）。函数声明名（`fn r#foo`）与 import 导入路径在
>    符号注册时归一化为无前缀名。

### 1.3 字面量

```
IntegerLit  ::= Digit (Digit | '_')*
FloatLit    ::= Digit+ '.' Digit+ (['e'|'E'] ['+'|'-']? Digit+)?
StringLit   ::= '"' (Escape | ~["\\\n])* '"'
CharLit     ::= ''' (Escape | ~['\\\n]) '''
BoolLit     ::= 'true' | 'false'
TimeLit     ::= Digit{1,2} 'am' | Digit{1,2} 'pm'
TimeHLit    ::= Digit{1,2} ':' Digit{2} (am|pm)?

Escape      ::= '\n' | '\t' | '\r' | '\\' | '\"' | '\'' | '\x' Hex Hex
```

### 1.4 运算符与分隔符

```
OpAdd       ::= '+'
OpSub       ::= '-'
OpMul       ::= '*'
OpDiv       ::= '/'
OpMod       ::= '%'
OpEq        ::= '=='
OpNe        ::= '!='
OpLt        ::= '<'
OpLe        ::= '<='
OpGt        ::= '>'
OpGe        ::= '>='
OpAnd       ::= '&&' | 'and'
OpOr        ::= '||' | 'or'
OpNot       ::= '!' | 'not'
OpAssign    ::= '='
OpAddAssign ::= '+='
OpSubAssign ::= '-='
OpMulAssign ::= '*='
OpDivAssign ::= '/='

LParen      ::= '('
RParen      ::= ')'
LBrace      ::= '{'
RBrace      ::= '}'
LBracket    ::= '['
RBracket    ::= ']'
Comma       ::= ','
Colon       ::= ':'
Semicolon   ::= ';'
Dot         ::= '.'
DotDotLt    ::= '..<'   // 左闭右开 [a, b)
Ellipsis    ::= '...'   // 闭区间 [a, b]
LtDotDot    ::= '<..'   // 左开右闭 (a, b]
Arrow       ::= '->'
FatArrow    ::= '=>'
```

---

## 2. 语法规则

### 2.1 程序结构

```
Program     ::= ModuleItem*
ModuleItem  ::= FnDecl
              | StructDecl
              | EnumDecl
              | TraitDecl
              | ImplBlock
              | ModuleDecl
              | ImportDecl
              | ConstDecl
              | StaticDecl
              | ActorDecl
              | MacroDecl
              | Attr* Statement
```

### 2.2 模块系统

```
ModuleDecl  ::= 'module' Ident ';'                              <!-- 外部文件模块 -->
              | 'module' Ident '{' ModuleItem* '}'              <!-- 内联模块 -->
              | 'pub' ModuleDecl                                <!-- 规划：公开模块 -->
ImportDecl  ::= 'import' ImportTree ';'
              | 'pub' 'import' ImportTree ';'                   <!-- 规划：再导出（re-export） -->
ImportTree  ::= Path                                            <!-- 单路径导入（含 as 别名） -->
              | Path ':' ':' '{' ImportList '}'                 <!-- 规划：组导入 -->
              | Path ':' ':' '*'                                <!-- 规划：glob 导入 -->
ImportList  ::= ImportTree (',' ImportTree)* ','?
Path        ::= Ident (':' ':' Ident)*                          <!-- 扁平路径：模块名::item（无 crate/super/self） -->
```

> 详细语义与编译模型设计见 [`module-system.md`](./module-system.md)。

### 2.3 函数

```
FnDecl      ::= Attr* 'pub'? 'unsafe'? 'const'? 'async'? 'fn' Ident
                GenParams? '(' ParamList? ')' RetType? WhereClause? Block
ParamList   ::= Param (',' Param)* ','?
Param       ::= 'mut'? Ident ':' Type | '...'
RetType     ::= '->' Type
```

### 2.4 类型系统

```
Type        ::= PrimType
              | Ident (':' ':' Ident)* GenArgs?
              | '&' Lifetime? 'mut'? Type
              | '*' 'const' Type
              | '*' 'mut' Type
              | '[' Type ';' Expr ']'
              | '[' Type ']'
              | '(' TypeList? ')'
              | '!'  // never type
              | 'dyn' TraitBound

PrimType    ::= 'i8' | 'i16' | 'i32' | 'i64' | 'i128' | 'isize'
              | 'u8' | 'u16' | 'u32' | 'u64' | 'u128' | 'usize'
              | 'f32' | 'f64'
              | 'bool' | 'char' | 'str'

TypeList    ::= Type (',' Type)* ','?
```

> **切片类型 `[T]`（S1/S2，2026-08-30 落地）**：`[T]` 为**无大小类型（DST）**——MVP 中
> 不能独立存储、也不能作为值类型，仅以引用形式使用：`&[T]` / `&mut [T]`。
> 切片引用的值是**胖指针**（2 槽：槽 0 = data 指针、槽 1 = 长度），布局与 `&str`
> 的 `StrFat` 同构（`{ i8*, i64 }`）。
>
> - **unsize coercion**：`&[T; N]` / `&mut [T; N]` 可自动转为 `&[T]` / `&mut [T]`
>   （数组长度在编译期已知，调用点构造胖指针 `{data, N}`）。
> - **支持的操作**：索引 `s[i]`（`&mut` 亦可写入）、再切片 `s[a..<b]`（零拷贝子区间，
>   边界 clamp 到 `[0, len]`）、方法 `.len()` / `.first()` / `.last()` / `.iter()` /
>   `.as_ptr()` / `.as_mut_ptr()`（`Vec<T>` 另有 `.as_slice()` / `.as_mut_slice()`
>   产出切片视图）。
> - **MVP 限制**：裸切片 `[T]` 不能作局部变量类型、结构体字段或函数返回值的独立类型；
>   不支持 `split_at`（返回切片二元组）；`&[u8]` 与 `&str` 间无视角转换。
>
> 详见 [`semantics.md`](semantics.md) 切片类型规则与 [`tasks/slice-type-system.md`](tasks/slice-type-system.md)。

> **类型联合 `T | U`（U1/U2，2026-08-30 落地）**：类型上下文允许 `|` 链，
> 成员必须**两两互不相交**（`resolve` 期校验，重叠报 `UnionMembersNotDisjoint`）。
>
> ```rlyeh
> let x: i64 | String = 5;              // 成员值可直接构造联合（协变）
> fn f(p: i64 | f64) {}                 // 非法：数值类型互通，视为重叠
> struct S { id: i64 | String }         // 字段级联合（U4）
> let y: (i64 | String) = 5;            // 括号内可写联合（见下方优先级说明）
> ```
>
> **优先级**：`&` / `*` 前缀构造的内层**不含**联合，故 `&T | &mut U` 解析为
> `(&T) | (&mut U)`（而非 `&(T | &mut U)`）。括号 / 泛型实参 / 元组 / 数组 / fn 签名
> 内的类型仍可写联合，故 `Vec<i64 | String>` 合法。
>
> **与闭包的歧义**：闭包参数类型注解后的 `|` 是**参数列表结束符**，不作联合运算符——
> `|x: i64| x + 1` 正常；需联合时写 `|x: (i64 | String)| ..`。
>
> 使用：`match x { i64 => .., String => .. }` 按**类型臂**收窄（见 `semantics.md` §8.4）。

> **受限标量枚举（U3 核心项，2026-08-30 落地）**：枚举所有变体均为单元变体且
> 无泛型参数时归类为受限标量枚举，**单标量存储（值即 tag）**——构造 `E::B` 产出
> 判别值而非对象指针，`match` 收窄为整型比较。
>
> - **显式判别式**：变体可带判别值 `enum Code { Ok = 200, NotFound = 404, Error = 500 }`，
>   判别值即该变体 tag，构造与 `match` 复用之；未标注时按声明序号。
> - **整数上下文**：标量枚举可作数组索引、位运算（结果 `i64`）、与裸整数双向比较
>   （`c == 1` / `1 == c`）、`let i: i64 = c` 值即整数；反向 `let c: Color = 1` 禁止。
> - 带负载变体（`Circle(f64)`）/ 泛型枚举 / 含数据字段的变体回归常规对象指针表示。
>
> ```rlyeh
> enum Code { Ok = 200, NotFound = 404, Error = 500 }
> let i: i64 = Code::NotFound;   // i == 404
> ```

### 2.5 结构体与枚举

```
StructDecl  ::= 'pub'? 'struct' Ident GenParams? WhereClause?
                '{' FieldDecl* '}'
FieldDecl   ::= 'pub'? Ident ':' Type ';'

EnumDecl    ::= 'pub'? 'enum' Ident GenParams? WhereClause?
                '{' EnumVariant (',' EnumVariant)* ','? '}'
EnumVariant ::= Ident ( '(' TypeList ')' | '{' FieldDecl* '}' )?
```

### 2.6 Trait 与实现

```
TraitDecl   ::= 'pub'? 'trait' Ident GenParams? ':' TraitBound? WhereClause?
                '{' TraitItem* '}'
TraitItem   ::= FnDecl | TypeAlias | ConstDecl

ImplBlock   ::= 'impl' GenParams? Type 'for' Type WhereClause?
                '{' ImplItem* '}'
              | 'impl' GenParams? Type WhereClause?
                '{' ImplItem* '}'
ImplItem    ::= FnDecl | TypeAlias | ConstDecl
```

### 2.7 泛型与生命周期

```
GenParams   ::= '<' GenParam (',' GenParam)* '>'
GenParam    ::= LifetimeParam | TypeParam
LifetimeParam ::= Lifetime (':' Lifetime)?
Lifetime    ::= ''' Ident
TypeParam   ::= Ident (':' TraitBound)?

WhereClause ::= 'where' WherePred (',' WherePred)*
WherePred   ::= Type ':' TraitBound (',' TraitBound)*
```

### 2.8 语句

```
Statement   ::= ';'  // empty statement
              | LetStmt
              | ExprStmt
              | ItemStmt
              | Attr* Statement

LetStmt     ::= 'let' 'mut'? Pattern ':'? Type? '=' Expr ';'
```

### 2.9 表达式（优先级递增）

```
Expr        ::= AssignExpr
AssignExpr  ::= LValue AssignOp Expr
              | LogicExpr

LogicExpr   ::= CompareExpr (LogicOp CompareExpr)*
LogicOp     ::= '&&' | '||' | 'and' | 'or'

CompareExpr ::= RangeExpr (CompareOp RangeExpr)*
              | RangeExpr 'in' InTarget
              | RangeExpr 'not' 'in' InTarget

CompareOp   ::= '<' | '<=' | '>' | '>=' | '==' | '!='
InTarget    ::= '(' InList ')'     // 集合：成员判断，范围元素离散展开
              | RangeExpr          // 裸范围：a..<b / a...b / a<..b 区间判断
InList      ::= InItem (',' InItem)*
InItem      ::= Expr
              | RangeExpr          // 范围元素：展开为离散成员（要求整数常量）

RangeExpr   ::= BitOrExpr ('..<' | '...' | '<..' BitOrExpr)?

BitOrExpr   ::= BitXorExpr ('|' BitXorExpr)*
BitXorExpr  ::= BitAndExpr ('^' BitAndExpr)*
BitAndExpr  ::= ShiftExpr ('&' ShiftExpr)*
ShiftExpr   ::= AddExpr (('<<' | '>>') AddExpr)*
AddExpr     ::= MulExpr (('+' | '-') MulExpr)*
MulExpr     ::= CastExpr (('*' | '/' | '%') CastExpr)*

CastExpr    ::= UnaryExpr ('as' Type)?

UnaryExpr   ::= ('!' | '-' | '*' | '&' 'mut'?) UnaryExpr
              | PostfixExpr

PostfixExpr ::= PrimaryExpr PostfixOp*
PostfixOp   ::= '.' Ident
              | '.' Ident '(' ArgList? ')'
              | '[' Expr ']'
              | '(' ArgList? ')'
              | '?'
              | 'in' RegionName
              | '::' '<' TypeList '>' '(' ArgList? ')'   (* L2 ✅ turbofish 泛型实参：`json::parse::<i64>(s)`、
                                                            `json::parse::<HashMap<i64, i64>>(s)`；parser 对 `::<` 三 token
                                                            前瞻检测，类型实参经子 Parser 解析（支持嵌套泛型，
                                                            收尾 `>>` 按 pending 深度拆分为逐层 `>`） *)

PrimaryExpr ::= Literal
              | Ident
              | 'self' | 'Self'
              | '(' Expr ')'
              | '(' ExprList? ')'
              | '[' ExprList? ']'
              | '[' Expr ';' Expr ']'
              | Block
              | 'if' IfExpr
              | 'match' MatchExpr
              | 'for' ForExpr
              | 'while' WhileExpr
              | 'loop' LoopExpr
              | 'region' RegionExpr
              | 'transfer' TransferExpr
              | ClosureExpr
```

### 2.9.1 控制流表达式（`if` / `while` / `loop` / `match`，含 `if let` / `while let`）

```
IfExpr      ::= 'let' Pattern '=' Expr Block ('else' (Block | 'if' IfExpr))?   (* `if let` *)
              | Expr Block ('else' (Block | 'if' IfExpr))?

WhileExpr   ::= 'let' Pattern '=' Expr Block      (* `while let` *)
              | Expr Block

LoopExpr    ::= Block
MatchExpr   ::= '{' MatchArm* '}'
MatchArm    ::= Pattern ('if' Expr)? '=>' Expr ','?

Pattern      ::= OrPattern
OrPattern    ::= PatternNoOr ('|' PatternNoOr)*        (* `A | B`，SH-P0-7 P-M3 *)
PatternNoOr  ::= PatternAtom (RangeOp PatternAtom)?    (* 范围模式 *)
RangeOp      ::= '..<' | '...' | '<..'                 (* 上开 / 双闭 / 下开 *)
PatternAtom  ::= Literal
               | 'true' | 'false'
               | '_'
               | Ident                                  (* 绑定 / 无参变体 *)
               | Ident '(' PatternList? ')'             (* 变体模式 *)
               | Ident ('::' Ident)+ ('(' PatternList? ')')?   (* 路径变体 `Option::Some(x)` *)
               | Ident '{' FieldPatList '}'             (* 结构体模式 *)
               | '(' PatternList? ')'                   (* 元组模式 *)
               | 'ref' 'mut'? Pattern                   (* 引用模式 *)
PatternList  ::= Pattern (',' Pattern)*
FieldPatList ::= Ident (':' Pattern)? (',' Ident (':' Pattern)?)*
```

**或模式的 `|` 只在 match 臂与 `if let` / `while let` 的模式位置生效**（解析器入口
`parse_or_pattern`）——闭包参数列表以 `|` 作分隔符与结束符（`|x, y| ..`），模式解析
吞 `|` 会误食参数列表结束符。`||` 是独立 token，不会被 `|` 误匹配。

**范围模式边界须为字面量**；语义与 `x in lo..<hi` 完全一致（排序仅放行数值与字符）。
**或模式各备选必须绑定同名同序的变量集**（`Shape::Circle(x) | Shape::Square(y)` 非法）。

`if let` / `while let` 为**纯语法糖**（parser 层展开，零新增 IR 节点）：

```text
if let Pat = e { A } else { B }   ⟶   match e { Pat => { A }, _ => { B } }
while let Pat = e { A }           ⟶   loop { match e { Pat => { A }, _ => break } }
```

缺 `else` 时兜底空块（求值 `()`），保证 `match` 穷尽；`else if` / `else if let`
链由 `IfExpr` 递归处理。模式能力继承 `MatchArm`——元组 / 结构体模式尚不支持
（`if let (a, b) = t` 报 `unsupported syntax: 元组 / 结构体模式在 MVP 阶段`）；
无 let 链（`if let a = .. && let b = ..`）。

### 2.10 比较链语义规则

```
# 连续比较运算符构成比较链
CompareChain ::= Expr CompareOp Expr (CompareOp Expr)*

# 语义：a OP1 b OP2 c ... → a OP1 b && b OP2 c ...
# 所有运算符方向必须一致

# 正向链（所有 < 或 <=）→ 区间内
0 < x < 10        →  x > 0 && x < 10
0 <= x <= 10     →  x >= 0 && x <= 10

# 反向链（所有 > 或 >=）→ 区间外
0 > x > 10       →  x < 0 || x > 10
0 >= x >= 10     →  x <= 0 || x >= 10

# 非法：方向不一致
0 < x > 10       →  COMPILE ERROR
```

### 2.11 区域系统

```
RegionExpr  ::= RegionName? RegionOptions? Block
RegionName  ::= ''' Ident
RegionOptions ::= 'with_size' '(' Expr ')'
                | 'allow_growth' '(' 'growth_factor' '=' Expr ')'
                | 'exact' '(' Expr ')'
                | 'adaptive'
                | 'strategy' '(' RegionStrategy ')'
                | 'hint' '(' RegionHint ')'

TransferExpr ::= Expr 'out' 'of' RegionName
```

### 2.12 Actor 模型

```
ActorDecl   ::= 'actor' Ident GenParams? WhereClause?
                '{' ActorField* ActorMethod* '}'
ActorField  ::= 'pub'? Ident ':' Type '=' Expr ';'
ActorMethod ::= 'pub'? 'async'? FnDecl
```

### 2.13 模式匹配

```
Pattern     ::= Literal
              | Ident
              | '_'
              | '(' PatternList? ')'
              | '[' PatternList? ']'
              | StructPattern
              | EnumPattern
              | RangePattern
              | 'ref' 'mut'? Pattern
              | Pattern '|' Pattern

RangePattern ::= Expr '..<' Expr   // 左闭右开
               | Expr '...' Expr   // 闭区间
               | Expr '<..' Expr   // 左开右闭
```

### 2.14 宏系统

```
MacroDecl   ::= 'macro_rules' '!' Ident '{' MacroRule* '}'
MacroRule    ::= MacroMatcher '=>' MacroTranscriber ';'
MacroMatcher ::= '(' MacroItem* ')'
MacroItem   ::= '$' Ident ':' MacroClass
              | '[' MacroItem* ']'
              | '(' MacroItem* ')'
              | '{' MacroItem* '}'
              | ~['$', '(', ')', '[', ']', '{', '}']
```

**集合宏**（I3 ✅，parse 期 desugar，零新增 IR）：

```
ArrMacro ::= 'arr' '!' '(' Expr (',' Expr)* ')'
VecMacro ::= 'vec' '!' '(' Expr (',' Expr)* ')'
MapMacro ::= 'map' '!' '(' Expr '=>' Expr (',' Expr '=>' Expr)* ')'
```

- `arr![a, b, c]` → 数组字面量（元素类型须统一，长度编译期已知）
- `vec![a, b, c]` → 块表达式 `let mut __vec_N = Vec::with_capacity(n); __vec_N.push(a); ...; __vec_N`（空 → `Vec::new()`）
- `map![k1 => v1, k2 => v2]` → 块表达式 `let mut __map_N = HashMap::with_capacity(n); __map_N.insert(k1, v1); ...; __map_N`（空 → `HashMap::new()`；元素缺 `=>` 报 parse 错）

元素为任意表达式（含嵌套宏调用、绑定变量），经子 Parser 继承宏注册表解析；
`Vec::with_capacity`/`HashMap::with_capacity` 为 typecheck 构造器特判，`push`/`insert` 为 std 方法调用。

---

## 3. 优先级表

| 优先级 | 运算符 | 结合性 |
|--------|--------|----------|
| 1 | `::` | 左 |
| 2 | `.`, `()`，`[]`, `?`, `in` | 左 |
| 3 | `!`, `-`, `*`（解引用）, `&`, `&mut` | 右（一元） |
| 4 | `as` | 左 |
| 5 | `*`, `/`, `%` | 左 |
| 6 | `+`, `-` | 左 |
| 7 | `<<`, `>>` | 左 |
| 8 | `<`, `<=`, `>`, `>=`, `==`, `!=` | 左（比较链） |
| 9 | `in`, `not in` | 左 |
| 10 | `&`（按位与） | 左 |
| 11 | `^` | 左 |
| 12 | `|` | 左 |
| 13 | `&&`, `and` | 左 |
| 14 | `||`, `or` | 左 |
| 15 | `=`, `+=`, `-=`, `*=`, `/=` | 右 |

---

## 4. 特殊语法糖

### 4.1 `in` 表达式

`in` 右侧为**括号集合**时是成员判断（`==` 链），其中范围元素展开为离散成员；
`in` 右侧为**裸范围**时是区间判断（`>= && <` 等）。

```
in (1, 3, 5)              →  x == 1 || x == 3 || x == 5
in (0..<10)               →  x == 0 || x == 1 || ... || x == 9   // 范围元素展开为离散成员
in 0..<10                 →  x >= 0 && x < 10                     // 裸范围 = 区间判断 [0, 10)
in (0...10)               →  x == 0 || x == 1 || ... || x == 10
in 0...10                 →  x >= 0 && x <= 10                    // [0, 10]
in (0<..10)               →  x == 1 || x == 2 || ... || x == 10
in 0<..10                 →  x > 0 && x <= 10                     // (0, 10]
in ('a'..<'z', 'A'..<'Z') →  (x == 'a' || ... || x == 'y') || (x == 'A' || ... || x == 'Y')
not in (1, 2, 3)          →  x != 1 && x != 2 && x != 3
not in (0..<10)           →  x != 0 && x != 1 && ... && x != 9
not in 0..<10             →  x < 0 || x >= 10
```

> 注：集合内范围元素的离散展开要求上下界为编译期整数常量（时间字面量可归一化为分钟值）。

### 4.2 时间字面量

```
9am                       →  9 * 60 = 540 (分钟)
6pm                       →  18 * 60 = 1080
22:00                     →  22 * 60 = 1320
09:30am                   →  9 * 60 + 30 = 570

// 时间集合判断（in 右侧括号集合，范围元素展开为分钟值离散成员）
if hour in (9am...6pm) {}          //  hour == 9:00 || hour == 9:01 || ... || hour == 18:00
if hour not in (6am..<10pm) {}     //  6:00 ~ 21:59 分钟值之外的成员

// 时间区间判断（in 右侧裸范围）
if hour in 9am...6pm {}            //  9:00 ≤ hour ≤ 18:00
```

---

## 5. 完整示例

```rlyeh
// hello-world.rl
fn main() {
    println("Hello, Rlyeh!");
}

// 比较链
fn check_range(x: i32) {
    if 0 < x < 10 {
        println("x is in 0..<10");
    }
    if 0 > x > 10 {
        println("x is outside 0...10");
    }
}

// 集合判断
fn is_vowel(ch: char) -> bool {
    ch in ('a', 'e', 'i', 'o', 'u', 'A', 'E', 'I', 'O', 'U')
}

// 区域系统
fn process_data() {
    region 'r adaptive {
        for i in 0..<10000 {
            let item = Item::new(i) in 'r;
            process(item);
        }
    }
}

// Transfer
fn create_user() -> User {
    region 'r {
        let user = User::new("Alice") in 'r;
        return transfer user out of 'r;
    }
}

// Actor
actor Counter {
    value: u32 = 0,
    
    pub fn increment(amount: u32) -> u32 {
        self.value += amount;
        self.value
    }
}
```

---

## 附录 A：实现纪要

> **说明**：本节提炼自开发任务书（原 `prompts/P001–P003`，2026-08-24 归档至 [`design/prompts/`](design/prompts/)），
> 记录语法解析的实际落地状态、关键决策与已知限制，供后续维护参考。语法设计原文见本规范正文与设计稿。

### A.1 词法分析器（对应 P001，2026-08-19 ✅）

- **Token 分类**：关键字、标识符、整数字面量（`i64`）、浮点字面量（`f64`）、字符串字面量（转义）、
  字符字面量、时间字面量（`9am`/`6pm`，分钟单位）、运算符、分隔符、注释（`//`、`/* */`）。
- **性能目标**：10K 行源码 < 5ms（实测通过）。
- **质量**：20 单元测试 + 14 集成测试 + 1 文档测试；fuzz 测试发现并修复**常量 EOF 时 panic**。

### A.2 语法分析器（对应 P002，2026-08-19 ✅）

- **表达式解析**：Pratt 优先级爬升，与正文 §3 优先级表一一对应；支持前缀一元（`-`/`!`/`not`/`&`/`*`）、
  中缀二元、后缀调用/索引/成员/`?`。
- **比较链解析**：连续二元比较操作符（`0 < x < 10`）合并为单条比较链节点（见 §4.1 与 semantics.md 附录 A.1）。
- **`in` 表达式**：右操作数为括号集合或裸范围时分别解析为成员判断 / 区间判断（§4.1）。
- **语句级语法**：`region`/`transfer`/`actor`/`match`/`for`/`while`/`loop`、宏调用 `macro_rules!` 与集合宏
  `arr!`/`vec!`/`map!`（§2.14）。
- **质量**：44 单元测试 + 15 集成测试 + 3 伪模糊（pseudo-fuzz）；clippy 零警告。

### A.3 比较链与 `in` 表达式解析（对应 P003，2026-08-19 ✅）

- 比较链方向规则、混合比较符组合、`in` 集合（离散成员展开）/ 范围（区间判断）的 parse 期归类与
  typecheck 期语义展开均已实现，详见 semantics.md 附录 A.1。

---

> **维护者**：Rlyeh Language Team  
> **License**：MIT / Apache-2.0
