//! 寻路: BFS 找两省间最短路径(跳数最少)。
//!
//! 设计(决策1+2):
//! - `is_passable` 插槽: 当前恒 true(任何省可穿)。未来加"未开战不得入境""绕开驻军省"
//!   等避让规则, 只改这个函数, BFS 主体不动。
//! - `edge_weight` 插槽: 当前权重全 1(=BFS, 跳数最少)。将来给省份加距离数据后,
//!   把权重换成实际距离 → 寻路自动升级为 Dijkstra(距离之和最短)。见 spec §3.4。
//!   (本文件用简单队列实现 BFS; 升级 Dijkstra 时改 find_path 主体 + edge_weight 取值。)
use crate::runtime::World;

/// 从 from 寻路到 to, 返回路径(含 to, 不含 from)。
/// - 路径第一个 = 下一站, 最后一个 = 最终目标
/// - from == to 返回 None(无意义命令)
/// - 起点或终点不在地图里 / 不连通 → 返回 None
pub fn find_path(world: &World, from: u32, to: u32) -> Option<Vec<u32>> {
    use std::collections::{HashMap, HashSet, VecDeque};
    if from == to {
        return None;
    }
    // 起点或终点不在地图里 → 无法寻路
    if !world.provinces.contains_key(&from) || !world.provinces.contains_key(&to) {
        return None;
    }
    let mut queue: VecDeque<u32> = VecDeque::new();
    let mut visited: HashSet<u32> = HashSet::new();
    let mut came_from: HashMap<u32, u32> = HashMap::new();
    queue.push_back(from);
    visited.insert(from);
    while let Some(cur) = queue.pop_front() {
        if cur == to {
            break;
        }
        let neighbors = world.provinces.get(&cur).map(|p| p.neighbors.clone()).unwrap_or_default();
        for n in neighbors {
            if !visited.contains(&n) && is_passable(world, n) {
                visited.insert(n);
                came_from.insert(n, cur);
                queue.push_back(n);
            }
        }
    }
    // to 未被访问到 → 不连通
    if !visited.contains(&to) {
        return None;
    }
    // 从 to 回溯到 from, 得到 [下一站, ..., to](不含 from)
    let mut path = Vec::new();
    let mut cur = to;
    while cur != from {
        path.push(cur);
        cur = *came_from.get(&cur).expect("came_from 应完整连通");
    }
    path.reverse();
    Some(path)
}

/// 判断一个省能否作为寻路中转(穿过)。
///
/// 当前实现: 恒 true(任何省可穿; 穿过敌方驻军省时由运行时战斗逻辑处理)。
/// 未来扩展点(决策1):
/// - "未开战不得入境": 检查 prov.controller 是否己方或已与该 owner 开战
/// - "绕开驻军省": 检查 prov 有无敌方非撤退师
/// 改这个函数即可, find_path 主体不动。
fn is_passable(_world: &World, _prov: u32) -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::{Province, World};

    /// 链式拓扑 1-2-3-4(双向邻接)
    fn chain_world() -> World {
        let mut w = World::new();
        w.provinces.insert(1, Province { id: 1, neighbors: vec![2], ..Default::default() });
        w.provinces.insert(2, Province { id: 2, neighbors: vec![1, 3], ..Default::default() });
        w.provinces.insert(3, Province { id: 3, neighbors: vec![2, 4], ..Default::default() });
        w.provinces.insert(4, Province { id: 4, neighbors: vec![3], ..Default::default() });
        w
    }

    #[test]
    fn t_find_path_adjacent() {
        let w = chain_world();
        // 1→2 相邻, 返回单元素 [2]
        assert_eq!(find_path(&w, 1, 2), Some(vec![2]));
    }

    #[test]
    fn t_find_path_multi_hop() {
        let w = chain_world();
        // 1→4 经 2,3, 返回 [2,3,4]
        assert_eq!(find_path(&w, 1, 4), Some(vec![2, 3, 4]));
    }

    #[test]
    fn t_find_path_same_province() {
        let w = chain_world();
        assert_eq!(find_path(&w, 2, 2), None, "同省应返回 None");
    }

    #[test]
    fn t_find_path_disconnected() {
        let mut w = chain_world();
        // 加一个孤立省 9
        w.provinces.insert(9, Province { id: 9, neighbors: vec![], ..Default::default() });
        assert_eq!(find_path(&w, 1, 9), None, "不连通应返回 None");
    }

    #[test]
    fn t_find_path_missing_endpoint() {
        let w = chain_world();
        // 省不在地图里 → None
        assert_eq!(find_path(&w, 1, 99), None, "终点不在地图应返回 None");
        assert_eq!(find_path(&w, 99, 1), None, "起点不在地图应返回 None");
    }
}
