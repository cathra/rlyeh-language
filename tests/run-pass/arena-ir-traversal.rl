// SH-P2-3 (0.2.0-I) 验证：索引式 arena + NodeId 表示树形 IR，
// 经显式工作栈完成一次可变重写遍历（等价于 Rust 版 arena 遍历）。
// 对应 I2：arena + 整数索引（避免运行时引用计数开销，契合 Rlyeh 索引式倾向）。

// 节点句柄：1-based 下标，0 表示空（无效）。
struct NodeId {
    raw: i64,
}

// 索引式存储：底层 Vec<T> 槽区，NodeId = 下标 + 1。
struct Arena<T> {
    slots: Vec<T>,
}

impl<T> Arena<T> {
    // 注：泛型 impl 静态方法 MVP 不支持（`Vec::new()` 特判在泛型关联函数内
    // 无法解析元素类型），故空 Arena 在调用方顶层以 `Arena { slots: Vec::new() }`
    // 构造（Vec::new() 经 `Arena<IrNode>` 注解推断元素类型，见 main）。
    // 分配一个节点，返回稳定句柄（Vec 扩容不影响既有句柄的有效性语义）。
    fn alloc(&mut self, x: T) -> NodeId {
        self.slots.push(x);
        NodeId { raw: self.slots.len() }
    }
    // 按句柄读（Copy 节点零开销按值拷贝）。
    fn get(&self, id: NodeId) -> T {
        self.slots.get(id.raw - 1)
    }
    // 按句柄写（可变重写入口）。
    fn set(&mut self, id: NodeId, x: T) {
        self.slots.set(id.raw - 1, x);
    }
    fn len(&self) -> i64 {
        self.slots.len()
    }
}

// IR 节点：整型二叉树（左右子树以 NodeId 链接）。
struct IrNode {
    value: i64,
    left: NodeId,
    right: NodeId,
}

fn mk_node(v: i64) -> IrNode {
    IrNode { value: v, left: NodeId { raw: 0 }, right: NodeId { raw: 0 } }
}

fn main() {
    let mut arena: Arena<IrNode> = Arena { slots: Vec::new() };

    // 构建树：
    //       10
    //      /  \
    //    20    30
    //   /        \
    //  40         50
    let id1 = arena.alloc(mk_node(10));
    let id2 = arena.alloc(mk_node(20));
    let id3 = arena.alloc(mk_node(30));
    let id4 = arena.alloc(mk_node(40));
    let id5 = arena.alloc(mk_node(50));
    let mut r = arena.get(id1);
    r.left = id2;
    r.right = id3;
    arena.set(id1, r);
    let mut l = arena.get(id2);
    l.left = id4;
    arena.set(id2, l);
    let mut rr = arena.get(id3);
    rr.right = id5;
    arena.set(id3, rr);

    // 可变重写遍历：显式工作栈（避免递归），每个节点 value += 100。
    let mut stack: Vec<NodeId> = Vec::new();
    stack.push(id1);
    while stack.len() > 0 {
        let top = stack.pop();
        if let Option::Some(id) = top {
            let mut n = arena.get(id);
            n.value = n.value + 100;
            arena.set(id, n);
            // 右子树后入栈，保证左子树先处理（前序：根 -> 左 -> 右）。
            if n.right.raw != 0 {
                stack.push(n.right);
            }
            if n.left.raw != 0 {
                stack.push(n.left);
            }
        }
    }

    // 验证：索引式随机访问读取各节点，确认遍历已改写。
    let root = arena.get(id1);
    println(root.value);            // 110
    let lc = arena.get(root.left);
    println(lc.value);              // 120
    let lcl = arena.get(lc.left);
    println(lcl.value);             // 140
    let rc = arena.get(root.right);
    println(rc.value);              // 130
    let rcr = arena.get(rc.right);
    println(rcr.value);             // 150
}
