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
///
/// 决策14 路径失效应对依赖此函数: 当某省变不可进入(如投降后), 调用方(movement.rs)
/// 据此停止师的行军。测试用 set_test_blocked 模拟省份不可进入。
pub fn is_passable(world: &World, prov: u32) -> bool {
    #[cfg(test)]
    {
        let blocked_now = TEST_BLOCKED.with(|b| b.borrow().as_ref().map(|set| set.contains(&prov)));
        if let Some(true) = blocked_now {
            return false;
        }
    }
    let _ = (world, prov);
    true
}

#[cfg(test)]
thread_local! {
    /// 测试用: 设为 Some(HashSet) 时, 集合中的省视为不可进入(模拟投降/停战)。
    static TEST_BLOCKED: std::cell::RefCell<Option<std::collections::HashSet<u32>>> =
        std::cell::RefCell::new(None);
}

#[cfg(test)]
pub fn set_test_blocked(provs: &[u32]) {
    TEST_BLOCKED.with(|b| *b.borrow_mut() = Some(provs.iter().copied().collect()));
}
#[cfg(test)]
pub fn clear_test_blocked() {
    TEST_BLOCKED.with(|b| *b.borrow_mut() = None);
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

    // ===== 决策14: 路径中途失效应对 =====

    #[test]
    fn t_path_stops_when_dest_inaccessible() {
        // 机制1: 多段行军途中, dest 突然不可进入 → 师转 Idle, remaining 清空
        use crate::combat::movement::advance_movement;
        use crate::runtime::entities::{Division, OrderState};
        let mut w = chain_world(); // 1-2-3-4
        let d = Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: OrderState::Moving {
                dest: 2, progress: 0.5, hostile: false, origin: 1, remaining: vec![3],
            },
            ..Default::default()
        };
        let did = w.add_division(d);
        // 设置省2 不可进入(模拟投降/停战)
        super::set_test_blocked(&[2]);
        advance_movement(&mut w);
        super::clear_test_blocked();
        let div = w.divisions.get(&did).unwrap();
        assert!(div.is_idle(), "dest 不可进入应停止, order={:?}", div.order);
        assert_eq!(div.move_dest(), None, "应无 dest(转 Idle)");
    }

    #[test]
    fn t_invalidate_paths_clears_blocked() {
        // 机制2: invalidate 扫描 dest+remaining, 任一不可进入则整条路径停止
        let mut w = chain_world();
        let d = crate::runtime::entities::Division {
            id: 0, owner_tag: "GER".into(), location_province: 1,
            order: crate::runtime::entities::OrderState::Moving {
                dest: 2, progress: 0.3, hostile: false, origin: 1, remaining: vec![3, 4],
            },
            ..Default::default()
        };
        let did = w.add_division(d);
        // 省4 不可进入(remaining 里的中转省)
        super::set_test_blocked(&[4]);
        crate::combat::movement::invalidate_paths_to_inaccessible(&mut w);
        super::clear_test_blocked();
        assert!(w.divisions.get(&did).unwrap().is_idle(), "remaining 含不可进入省应整条停止");
    }
}
