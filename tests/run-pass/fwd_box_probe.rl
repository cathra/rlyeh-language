// 探针：struct 自引用（Box 打破）——第一层 next 指向自己
struct Node { x: i64, next: Option<Box<Node>> }
fn main() {
    let mut head = Node { x: 1, next: None };
    head.next = Some(Box::new(head));
    println(head.x);
    match head.next {
        Some(n) => println(n.x),
        None => println(-1),
    }
}
