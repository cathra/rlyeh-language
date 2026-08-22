# Zeta 语言语法规范 (EBNF)

> 版本：v2.0  
> 最后更新：2026-08-22

> **⚠️ 实现状态**：本文为**目标语法规范**（EBNF），其中部分语法为规划特性，MVP 编译器尚未实现：
> 宏调用（`name!`，`!` 为 `not` 运算符）、闭包 `|x| ...`（typecheck 报 Unsupported）、引用类型 `&T` 已实现
> （G1 ✅：`&x`/`&mut x` 表达式、`&T`/`&mut T` 参数与返回、`*` 解引用；`&str` 只读借用视图已实现
> （G2 ✅：`as_str()` + `&str` 参数/返回/索引 + `String::from(&str)`），裸指针 `*T` 仍规划）、
> `dyn Trait`、`?` 运算符、生命周期参数 `'a`、`macro_rules` 等。
> **已实现子集的教程与可运行示例见 [`guide.md`](./guide.md)，已知限制见其 §13。**

## 相关文档

| 类型 | 文档 | 说明 |
|------|------|------|
| 项目总纲 | [CODEBUDDY.md](../CODEBUDDY.md) | 项目全景 |
| 设计文档 | [01_词法分析器](../design/01_词法分析器.md) / [02_语法分析器](../design/02_语法分析器.md) / [06_比较链与条件判断](../design/06_比较链与条件判断.md) | 模块设计 |
| 实现任务 | [P001](../prompts/P001_词法分析器核心.md) / [P002](../prompts/P002_语法分析器核心.md) / [P003](../prompts/P003_比较链语义分析.md) | CodeBuddy 任务 |

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
              | ModDecl
              | UseDecl
              | ConstDecl
              | StaticDecl
              | ActorDecl
              | MacroDecl
              | Attr* Statement
```

### 2.2 模块系统

```
ModDecl     ::= 'mod' Ident ';'
              | 'mod' Ident '{' ModuleItem* '}'
UseDecl     ::= 'use' UsePath ';'
UsePath     ::= Path (':' ':' '{' UseList '}')?
UseList     ::= UsePath (',' UsePath)* (','? | ',')
```

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

```zeta
// hello-world.zeta
fn main() {
    println("Hello, Zeta!");
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

> **维护者**：Zeta Language Team  
> **License**：MIT / Apache-2.0
