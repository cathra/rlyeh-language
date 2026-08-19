# P005: Transfer 语义实现

> **模块路径**：`crates/zeta-regionck/` (扩展)  
> **预估工期**：3-5 天  
> **前置依赖**：P004（区域系统）  
> **输出**：完整的 transfer 语义——从区域安全转移所有权  
> **状态**：✅ MVP 已实现（2026-08-20）

---

## 任务描述

实现 `transfer x out of 'r` 的完整语义：
1. 从区域的析构列表中移除对象
2. 标记内存块中的"已迁出"位置
3. 将所有权转移到接收方
4. 验证安全性（不能 transfer 引用、不能部分 transfer、不能重复 transfer）

---

## 核心算法

### Transfer 操作流程

```rust
// crates/zeta-regionck/src/transfer.rs

#![warn(missing_docs)]

use std::ptr::NonNull;
use thiserror::Error;

/// Transfer 安全检查器
pub struct TransferChecker {
    /// 记录每个区域中已 transfer 的对象
    transferred: HashMap<String, Vec<NonNull<u8>>>,
    /// 当前活跃区域
    active_regions: Vec<String>,
}

/// Transfer 错误
#[derive(Debug, Error)]
pub enum TransferError {
    #[error("cannot transfer reference at {line}:{col}: references cannot escape regions")]
    CannotTransferReference { line: usize, col: usize },
    
    #[error("object already transferred at {line}:{col}")]
    AlreadyTransferred { line: usize, col: usize },
    
    #[error("cannot partially transfer '{field}' from struct at {line}:{col}")]
    PartialTransfer { field: String, line: usize, col: usize },
    
    #[error("region '{name}' not found at {line}:{col}")]
    RegionNotFound { name: String, line: usize, col: usize },
    
    #[error("transfer from outer region not allowed at {line}:{col}: {detail}")]
    OuterRegionTransfer { detail: String, line: usize, col: usize },
    
    #[error("type '{ty}' does not implement 'Sized' at {line}:{col}, cannot transfer")]
    UnsizedTransfer { ty: String, line: usize, col: usize },
}

impl TransferChecker {
    pub fn new() -> Self {
        Self {
            transferred: HashMap::new(),
            active_regions: vec![],
        }
    }
    
    /// 验证 transfer 表达式的合法性
    pub fn validate_transfer(
        &mut self,
        expr: &HirExpr,
        region_name: &str,
        span: Span,
    ) -> Result<(), TransferError> {
        // 1. 验证区域存在
        self.check_region_exists(region_name, span)?;
        
        // 2. 验证不是引用
        self.check_not_reference(expr, span)?;
        
        // 3. 验证不是部分转移
        self.check_not_partial(expr, span)?;
        
        // 4. 验证没有重复转移
        self.check_not_already_transferred(expr, region_name, span)?;
        
        Ok(())
    }
    
    fn check_region_exists(&self, name: &str, span: Span) -> Result<(), TransferError> {
        if !self.active_regions.iter().any(|r| r == name) {
            return Err(TransferError::RegionNotFound {
                name: name.to_string(),
                line: span.line,
                col: span.col,
            });
        }
        Ok(())
    }
    
    fn check_not_reference(&self, expr: &HirExpr, span: Span) -> Result<(), TransferError> {
        match &expr.kind {
            // &T 或 &mut T 不能 transfer
            HirExprKind::Reference { .. } => {
                Err(TransferError::CannotTransferReference {
                    line: span.line,
                    col: span.col,
                })
            }
            _ => Ok(()),
        }
    }
    
    fn check_not_partial(&self, expr: &HirExpr, span: Span) -> Result<(), TransferError> {
        // 不能 transfer 结构体的单个字段
        // 不能 transfer 元组的部分元素
        match &expr.kind {
            HirExprKind::FieldAccess { base, field, .. } => {
                Err(TransferError::PartialTransfer {
                    field: field.clone(),
                    line: span.line,
                    col: span.col,
                })
            }
            HirExprKind::TupleIndex { .. } => {
                Err(TransferError::PartialTransfer {
                    field: "tuple_element".to_string(),
                    line: span.line,
                    col: span.col,
                })
            }
            _ => Ok(()),
        }
    }
    
    fn check_not_already_transferred(
        &self,
        expr: &HirExpr,
        region_name: &str,
        span: Span,
    ) -> Result<(), TransferError> {
        // 获取 expr 对应的内存地址
        let ptr = self.get_expr_ptr(expr);
        
        if let Some(transferred) = self.transferred.get(region_name) {
            if transferred.contains(&ptr) {
                return Err(TransferError::AlreadyTransferred {
                    line: span.line,
                    col: span.col,
                });
            }
        }
        Ok(())
    }
    
    fn get_expr_ptr(&self, expr: &HirExpr) -> NonNull<u8> {
        // 从 HIR 中获取表达式对应的内存地址
        // 实际实现中，这个信息在代码生成阶段才会确定
        // 在语义分析阶段，我们用符号名代替
        todo!("返回表达式的内存地址或符号标识")
    }
    
    /// 执行 transfer（在运行时调用）
    /// 返回转移后的对象引用
    pub unsafe fn execute_transfer<T>(
        region: &mut Region,
        ptr: NonNull<T>,
    ) -> &'static mut T {
        let ptr_u8 = ptr.cast();
        
        // 1. 从析构列表中移除
        region.remove_destructor(ptr_u8);
        
        // 2. 标记内存为"已迁出"
        region.mark_transferred(ptr_u8);
        
        // 3. 返回对象引用（生命周期变为 'static，由接收方管理）
        &mut *ptr.as_ptr()
    }
}
```

### Region 侧的配合修改

```rust
// crates/zeta-region-alloc/src/region.rs (扩展)

impl Region {
    /// 从析构列表中移除指定对象
    pub fn remove_destructor(&mut self, ptr: NonNull<u8>) {
        if let Some(pos) = self.destructors.iter().position(|e| e.ptr == ptr) {
            self.destructors.swap_remove(pos);
        }
    }
    
    /// 标记对象为已迁出
    /// 区域销毁时跳过这些位置
    pub fn mark_transferred(&mut self, ptr: NonNull<u8>) {
        self.transferred.push(ptr);
    }
    
    /// 销毁区域时，跳过已迁出的对象
    pub unsafe fn destroy(&mut self) {
        // 1. 调用剩余析构函数（逆序）
        for entry in self.destructors.iter().rev() {
            (entry.drop_fn)(entry.ptr);
        }
        self.destructors.clear();
        
        // 2. 释放所有内存块
        for block in self.blocks.drain(..) {
            block.deallocate();
        }
        
        // 3. 清空 transferred 列表
        self.transferred.clear();
    }
    
    /// 检查对象是否已迁出
    pub fn is_transferred(&self, ptr: NonNull<u8>) -> bool {
        self.transferred.contains(&ptr)
    }
}
```

---

## 必须实现的功能清单

### 1. 语义检查
- [x] 区域存在性验证（`RegionNotFound`）✅
- [x] 非引用检查（`CannotTransferReference`，防御性：typecheck 已在 MVP 拒绝 `&` 语法）✅
- [x] 非部分转移检查（`PartialTransfer`：transfer 非变量表达式直接拒绝；字段/索引形态防御性预留）✅
- [x] 非重复转移检查（`DoubleTransfer`）✅
- [x] 嵌套区域中的转移方向检查（`OuterRegionTransfer`：源区域必须是当前活跃栈顶）✅

### 2. 运行时操作
- [x] 从析构列表移除（`mark_transferred` 内部完成）✅
- [x] 标记已迁出（`transferred` 列表）✅
- [x] 返回所有权句柄（`execute_transfer<T>` → `&'static mut T`）✅
- [x] 区域销毁时跳过已迁出对象（`destroy` + `is_transferred` 查询）✅

### 3. HIR 转换
- [x] Transfer 节点生成（P004 已实现 `HirExpr::Transfer`）✅
- [ ] 类型调整为 `'static` 或调用者生命周期（HIR 无类型标注，待类型系统扩展）
- [ ] 插入析构列表操作（MIR 阶段）

### 4. MIR  lowering
- [ ] Transfer 变为 `region_remove_destructor` + `bitcast`（依赖 `zeta-mir`，当前占位）
- [ ] 调用约定调整（依赖 MIR）

---

## 测试用例

```rust
// crates/zeta-regionck/tests/transfer_test.rs

use zeta_regionck::{TransferChecker, TransferError};
use zeta_parser::Parser;

fn check(source: &str) -> Result<(), Vec<TransferError>> {
    let mut parser = Parser::new(source).unwrap();
    let ast = parser.parse_program().unwrap();
    // ... 经过类型检查得到 HIR
    let mut checker = TransferChecker::new();
    checker.validate_program(hir)
}

#[test]
fn test_basic_transfer() {
    let src = r#"
        fn create() -> BigStruct {
            region 'r {
                let data = BigStruct::new() in 'r;
                transfer data out of 'r
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn test_transfer_reference_error() {
    let src = r#"
        region 'r {
            let data = BigStruct::new() in 'r;
            let ref = &data;
            transfer ref out of 'r;  // 错误：不能 transfer 引用
        }
    "#;
    let errs = check(src).unwrap_err();
    assert!(errs.iter().any(|e| matches!(e, TransferError::CannotTransferReference { .. })));
}

#[test]
fn test_partial_transfer_error() {
    let src = r#"
        region 'r {
            let pair = (1, 2) in 'r;
            transfer pair.0 out of 'r;  // 错误：不能部分转移
        }
    "#;
    let errs = check(src).unwrap_err();
    assert!(errs.iter().any(|e| matches!(e, TransferError::PartialTransfer { .. })));
}

#[test]
fn test_double_transfer_error() {
    let src = r#"
        region 'r {
            let data = BigStruct::new() in 'r;
            let a = transfer data out of 'r;
            let b = transfer data out of 'r;  // 错误：重复转移
        }
    "#;
    let errs = check(src).unwrap_err();
    assert!(errs.iter().any(|e| matches!(e, TransferError::AlreadyTransferred { .. })));
}

#[test]
fn test_transfer_from_wrong_region() {
    let src = r#"
        region 'r1 { let x = 1 in 'r1; }
        transfer x out of 'r1;  // 错误：区域已结束
    "#;
    let errs = check(src).unwrap_err();
    assert!(!errs.is_empty());
}

#[test]
fn test_nested_region_transfer_inner() {
    let src = r#"
        region 'outer {
            let a = Data::new() in 'outer;
            region 'inner {
                let b = Data::new() in 'inner;
                transfer b out of 'inner;  // OK
            }
        }
    "#;
    assert!(check(src).is_ok());
}

#[test]
fn test_nested_region_transfer_outer_from_inner() {
    let src = r#"
        region 'outer {
            let a = Data::new() in 'outer;
            region 'inner {
                transfer a out of 'outer;  // 错误：从内层转移外层对象
            }
        }
    "#;
    let errs = check(src).unwrap_err();
    assert!(!errs.is_empty());
}

#[test]
fn test_transfer_struct_field_error() {
    let src = r#"
        struct Pair { x: i32, y: i32 }
        region 'r {
            let p = Pair { x: 1, y: 2 } in 'r;
            transfer p.x out of 'r;  // 错误：部分转移
        }
    "#;
    let errs = check(src).unwrap_err();
    assert!(errs.iter().any(|e| matches!(e, TransferError::PartialTransfer { .. })));
}

#[test]
fn test_transfer_then_use_in_region() {
    let src = r#"
        region 'r {
            let data = BigStruct::new() in 'r;
            let moved = transfer data out of 'r;
            data.use();  // 错误：data 已被转移
        }
    "#;
    // 这应该被借用检查器捕获（use-after-move）
    // 但 TransferChecker 也应该检测到
}
```

```rust
// crates/zeta-region-alloc/tests/transfer_runtime_test.rs

use zeta_region_alloc::Region;
use zeta_region_alloc::GrowthStrategy;

#[test]
fn test_runtime_transfer() {
    let mut region = Region::new(Some("test".to_string()), 
        GrowthStrategy::Exact(1024)).unwrap();
    
    // 在区域内分配
    let obj = region.allocate(Box::new(42i32)).unwrap();
    let ptr = std::ptr::NonNull::from(obj);
    
    // 执行 transfer
    unsafe {
        let transferred = region.execute_transfer(ptr);
        assert_eq!(*transferred, 42);
    }
    
    // 验证析构列表已更新
    assert!(region.is_transferred(ptr.cast()));
    
    // 销毁区域（不应该 double free）
    unsafe { region.destroy(); }
    
    // transferred 对象仍然有效（但需要我们手动管理）
    // 在实际使用中，它会被移交给调用者
}

#[test]
fn test_destructor_not_called_for_transferred() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    
    static DROP_COUNT: AtomicUsize = AtomicUsize::new(0);
    
    struct TrackDrop;
    impl Drop for TrackDrop {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, Ordering::SeqCst);
        }
    }
    
    let mut region = Region::new(None, GrowthStrategy::Exact(256)).unwrap();
    
    // 分配 10 个对象
    let mut ptrs = vec![];
    for _ in 0..10 {
        let obj = region.allocate(TrackDrop).unwrap();
        ptrs.push(std::ptr::NonNull::from(obj).cast());
    }
    
    // Transfer 5 个
    for &ptr in &ptrs[0..5] {
        unsafe { region.execute_transfer(ptr); }
    }
    
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 0);
    
    // 销毁区域
    unsafe { region.destroy(); }
    
    // 只有 5 个未被 transfer 的对象应该被析构
    assert_eq!(DROP_COUNT.load(Ordering::SeqCst), 5);
}
```

---

## 性能基准

```rust
// crates/zeta-region-alloc/benches/transfer_bench.rs
use criterion::{black_box, Criterion};
use zeta_region_alloc::Region;
use zeta_region_alloc::GrowthStrategy;

fn bench_transfer(c: &mut Criterion) {
    c.bench_function("transfer_1k_objects", |b| {
        b.iter(|| {
            let mut region = Region::new(None, GrowthStrategy::Exact(64000)).unwrap();
            let mut ptrs = vec![];
            
            for i in 0..1000 {
                let p = region.allocate(black_box(i)).unwrap();
                ptrs.push(std::ptr::NonNull::from(p).cast());
            }
            
            // Transfer all
            for ptr in ptrs {
                unsafe { region.execute_transfer(ptr); }
            }
            
            unsafe { region.destroy(); }
        })
    });
}

// 目标：transfer 1000 个对象的开销 < 50μs
// （主要是从 Vec 中 swap_remove 的开销）
```

---

## 验收标准

| 标准 | 要求 |
|------|------|
| 所有测试通过 | 100% |
| 零警告 | `clippy -- -D warnings` |
| 安全检查 | 4 种错误类型全覆盖 |
| 运行时正确 | 无 double free、无泄漏 |
| 性能 | 1000 次 transfer < 50μs |
| 文档 | 公共 API 100% 有文档 |

---

## 交付文件

```
crates/zeta-regionck/src/
├── lib.rs
├── checker.rs       ← 主检查器
├── transfer.rs      ← Transfer 语义（本 Prompt 重点）
├── escape.rs        ← 逃逸分析
└── error.rs

crates/zeta-region-alloc/src/
├── lib.rs
├── region.rs        ← 扩展：remove_destructor, mark_transferred
├── destructor.rs
└── strategy.rs
```

---

## 完成后下一步

进入 **P006_Actor运行时.md**，实现 Actor 并发模型。

## 实施记录（2026-08-20）

- **运行时**（`zeta-region-alloc`）：`Region` 新增 `execute_transfer<T>(&mut self, ptr) -> &'static mut T`
  （移除析构 + 标记已迁出 + 返回所有权句柄）与 `is_transferred(ptr)` 查询；
  `mark_transferred` 保留为底层簿记入口。
- **语义**（`zeta-regionck`）：新增 `OuterRegionTransfer` 检查——`transfer v out of 'r`
  的源区域必须是**当前活跃区域栈栈顶**，从内层区域转移外层区域对象报错；
  transfer 非变量表达式（调用结果、复合表达式等无法静态判定归属）报 `PartialTransfer`。
- **错误类型**：`RegionError` 新增 `CannotTransferReference` / `OuterRegionTransfer` /
  `UnsizedTransfer` 变体（`PartialTransfer` 已有）；`line`/`col` 仍为占位 0（HIR 无 Span）。

## 落地偏差说明

- `zeta-regionck` 未新建 `transfer.rs`，Transfer 检查并入 `checker.rs` 的 `check_transfer`（与 P004 一致）。
- `execute_transfer` 归属 `zeta-region-alloc::region`（P005 原稿放在 regionck 的 TransferChecker 内；
  运行时操作涉及 `DestructorRegistry` 内部字段，放 Region 侧更内聚）。
- 未公开 `remove_destructor`：`mark_transferred` 已内部完成"移除析构 + 标记"；
  单独公开会导致"析构被移除但未标记"的泄漏状态，保留单一入口。
- **非引用 / 字段级部分转移检查为防御性**：typecheck 在 MVP 阶段对 `&`、字段访问、元组索引
  直接返回 `Unsupported`，这些语法无法到达 regionck。`CannotTransferReference` / `UnsizedTransfer`
  保留变体与 Display，待 HIR 引入引用节点与类型标注后启用。
- **transfer 后的内存语义**：bump 区域销毁时整体释放全部块（`deallocate_all`），
  transferred 对象仍位于区域块内（零拷贝，ADR-003）；接收方须在 `destroy` **之前**
  读取/修改/把值 move 出区域，销毁后句柄失效。真实编译器中的"搬移到外部堆"由 MIR/代码生成阶段完成。
- `test_transfer_then_use_in_region`（use-after-move）依赖借用检查器（P007），本阶段未实现。
- 性能基准 `benches/transfer_bench.rs` 未建（可选交付），核心路径已在单元测试覆盖。

## 测试清单（新增 9 项）

- `zeta-region-alloc/tests/transfer_runtime_test.rs`（3 项）：所有权句柄读写、10 对象 transfer 5 个
  后销毁仅析构 5 个（无 double free）、`is_transferred` 状态查询。
- `zeta-regionck/tests/transfer_test.rs`（6 项）：内层 transfer 内层对象 OK、内层 transfer 外层对象
  报 `OuterRegionTransfer`、内层结束后外层直接作用域 transfer OK、transfer 复合表达式/调用结果报
  `PartialTransfer`、防御性变体 Display。

全 workspace 176 项测试通过，clippy 0 警告，`cargo fmt --check` 通过。
