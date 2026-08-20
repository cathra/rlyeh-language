//! 模块依赖图：为多模块增量编译提供受影响传播、拓扑排序与循环检测。
//!
//! 当前编译模型为单文件，此结构作为通用基础设施先行落地：
//! - 边 `a → b` 表示模块 `a` 依赖模块 `b`（`b` 的接口变化会传播到 `a`）。
//! - 受影响集合通过传递闭包计算，保证下游模块全部重编译。

use std::collections::{BTreeMap, BTreeSet};

/// 依赖图中的循环。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CycleError {
    /// 环上模块名序列（首尾相同，构成闭环）
    pub cycle: Vec<String>,
}

impl std::fmt::Display for CycleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "模块依赖循环: {}", self.cycle.join(" -> "))
    }
}

impl std::error::Error for CycleError {}

/// 有向图，`a -> b` 表示 a 依赖 b。
#[derive(Debug, Clone, Default)]
pub struct DependencyGraph {
    /// 所有模块节点（有序）
    nodes: BTreeSet<String>,
    /// 邻接表：依赖者 → 被依赖者集合
    edges: BTreeMap<String, BTreeSet<String>>,
}

impl DependencyGraph {
    /// 创建空图。
    pub fn new() -> Self {
        Self::default()
    }

    /// 添加模块节点。
    pub fn add_node(&mut self, name: &str) {
        self.nodes.insert(name.to_string());
        self.edges.entry(name.to_string()).or_default();
    }

    /// 添加依赖边 `from` 依赖 `to`。
    ///
    /// 不存在的节点会被隐式创建。
    pub fn add_edge(&mut self, from: &str, to: &str) {
        self.add_node(from);
        self.add_node(to);
        self.edges
            .entry(from.to_string())
            .or_default()
            .insert(to.to_string());
    }

    /// 是否直接依赖。
    pub fn depends_on(&self, from: &str, to: &str) -> bool {
        self.edges.get(from).is_some_and(|deps| deps.contains(to))
    }

    /// 直接依赖的模块集合。
    pub fn dependencies_of(&self, name: &str) -> BTreeSet<String> {
        self.edges.get(name).cloned().unwrap_or_default()
    }

    /// 模块是否存在于图中。
    pub fn contains(&self, name: &str) -> bool {
        self.nodes.contains(name)
    }

    /// 模块总数。
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// 是否为空。
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    /// 计算受影响模块集合（含 `changed` 自身）的传递闭包。
    ///
    /// 语义：`changed` 中模块的接口变化，会强制所有（直接或间接）
    /// 依赖它们的模块重编译。
    pub fn affected_by(&self, changed: &[String]) -> BTreeSet<String> {
        let mut affected = BTreeSet::new();
        let mut stack: Vec<String> = changed
            .iter()
            .filter(|n| self.contains(n))
            .cloned()
            .collect();
        while let Some(name) = stack.pop() {
            if !affected.insert(name.clone()) {
                continue;
            }
            // 找出依赖 `name` 的模块（反向遍历边表）
            for (from, deps) in &self.edges {
                if deps.contains(&name) {
                    stack.push(from.clone());
                }
            }
        }
        affected
    }

    /// 拓扑排序（Kahn 算法），结果按编译顺序排列（被依赖者在前）。
    ///
    /// 入度定义为"依赖模块数"：无依赖的模块（最上游）先输出，
    /// 依赖者随后。检测到环时返回 [`CycleError`]。
    pub fn topological_order(&self) -> Result<Vec<String>, CycleError> {
        // 入度表：依赖模块数（依赖越多越靠后编译）
        let mut indegree: BTreeMap<&String, usize> = self
            .nodes
            .iter()
            .map(|n| (n, self.dependencies_of(n).len()))
            .collect();
        // 反向边表：被依赖者 → 依赖者集合
        let mut reverse: BTreeMap<&String, Vec<&String>> = BTreeMap::new();
        for (from, deps) in &self.edges {
            for to in deps {
                reverse.entry(to).or_default().push(from);
            }
        }

        let mut queue: Vec<String> = indegree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(n, _)| (*n).clone())
            .collect();
        queue.sort();

        let mut order = Vec::with_capacity(self.nodes.len());
        while let Some(name) = queue.first().cloned() {
            queue.remove(0);
            order.push(name.clone());
            if let Some(dependents) = reverse.get(&name) {
                for from in dependents {
                    if let Some(d) = indegree.get_mut(*from) {
                        *d -= 1;
                        if *d == 0 {
                            queue.push((*from).clone());
                            queue.sort();
                        }
                    }
                }
            }
        }

        if order.len() != self.nodes.len() {
            // 存在环：剩余节点任意取一个起点还原环
            let remaining: BTreeSet<&String> =
                self.nodes.iter().filter(|n| !order.contains(n)).collect();
            let start = remaining.iter().next().copied().unwrap();
            let mut cycle = Vec::new();
            let mut visited = BTreeSet::new();
            let mut stack = vec![(*start).clone()];
            while let Some(name) = stack.last().cloned() {
                if !visited.insert(name.clone()) {
                    // 回到已访问节点：找到环
                    let pos = cycle.iter().position(|c| *c == name).unwrap_or(0);
                    let mut cyc = cycle.split_off(pos);
                    cyc.push(name.clone());
                    return Err(CycleError { cycle: cyc });
                }
                // 无 next 时由下方 match 弹出栈
                cycle.push(name.clone());
                let mut next: Option<String> = None;
                if let Some(deps) = self.edges.get(&name) {
                    for to in deps {
                        if remaining.contains(to) {
                            next = Some(to.clone());
                            break;
                        }
                    }
                }
                match next {
                    Some(n) => stack.push(n),
                    None => {
                        stack.pop();
                        cycle.pop();
                    }
                }
            }
            return Err(CycleError { cycle });
        }
        Ok(order)
    }

    /// 是否含环。
    pub fn has_cycle(&self) -> bool {
        self.topological_order().is_err()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph() -> DependencyGraph {
        let mut g = DependencyGraph::new();
        // a -> b -> c；d -> c
        g.add_edge("a", "b");
        g.add_edge("b", "c");
        g.add_edge("d", "c");
        g
    }

    #[test]
    fn edges_and_dependencies() {
        let g = graph();
        assert!(g.depends_on("a", "b"));
        assert!(!g.depends_on("b", "a"));
        assert_eq!(g.dependencies_of("a"), BTreeSet::from(["b".to_string()]));
        assert_eq!(g.len(), 4);
    }

    #[test]
    fn affected_by_propagates_transitively() {
        let g = graph();
        // c 变化 → b、a 受影响（d 也依赖 c）
        let affected = g.affected_by(&["c".to_string()]);
        assert_eq!(
            affected,
            BTreeSet::from([
                "a".to_string(),
                "b".to_string(),
                "c".to_string(),
                "d".to_string(),
            ])
        );
        // b 变化 → a 受影响；c、d 不受影响
        let affected = g.affected_by(&["b".to_string()]);
        assert_eq!(affected, BTreeSet::from(["a".to_string(), "b".to_string()]));
    }

    #[test]
    fn topological_order_is_dependency_first() {
        let order = graph().topological_order().unwrap();
        // c 必须在 b 之前、d 之前；b 在 a 之前
        let pos = |n: &str| order.iter().position(|x| x == n).unwrap();
        assert!(pos("c") < pos("b"));
        assert!(pos("c") < pos("d"));
        assert!(pos("b") < pos("a"));
        assert_eq!(order.len(), 4);
    }

    #[test]
    fn cycle_detection() {
        let mut g = DependencyGraph::new();
        g.add_edge("a", "b");
        g.add_edge("b", "c");
        g.add_edge("c", "a");
        assert!(g.has_cycle());
        assert!(g.topological_order().is_err());
    }

    #[test]
    fn self_cycle_detected() {
        let mut g = DependencyGraph::new();
        g.add_edge("a", "a");
        assert!(g.has_cycle());
    }

    #[test]
    fn empty_and_isolated_nodes() {
        let g = DependencyGraph::new();
        assert!(g.is_empty());
        assert_eq!(g.affected_by(&["x".to_string()]), BTreeSet::new());

        let mut g = DependencyGraph::new();
        g.add_node("solo");
        assert_eq!(g.len(), 1);
        assert_eq!(g.topological_order().unwrap(), vec!["solo".to_string()]);
    }
}
