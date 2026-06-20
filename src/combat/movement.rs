//! 行军: 师在省份间移动(陆战循环)
//!
//! 每小时推进有 destination 的师。撤退师有速度加成(RETREAT_SPEED_FACTOR=0.25)。
//! 到达后更新 location, 清 destination; 若 retreating 则视为脱险恢复中。
use crate::runtime::World;

/// 每小时移动进度基准(约5小时到达一个省)
const MOVE_RATE: f64 = 0.2;
/// 撤退速度加成(原版 RETREAT_SPEED_FACTOR)
const RETREAT_SPEED_BONUS: f64 = 0.25;

/// 推进所有正在移动的师(每小时调用)
pub fn advance_movement(world: &mut World) {
    // 收集需要更新的师(避免迭代时借用 world)
    let moving: Vec<u64> = world
        .divisions
        .iter()
        .filter_map(|(id, d)| d.destination.map(|_| *id))
        .collect();

    for id in moving {
        let Some(d) = world.divisions.get_mut(&id) else { continue };
        let rate = if d.retreating {
            MOVE_RATE * (1.0 + RETREAT_SPEED_BONUS)
        } else {
            MOVE_RATE
        };
        d.move_progress += rate;
        if d.move_progress >= 1.0 {
            // 到达
            if let Some(dest) = d.destination.take() {
                d.location_province = dest;
                d.move_progress = 0.0;
                // 撤退师到达己方省后, retreating 保持(org 恢复满后由 recover 清除)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::entities::Division;

    #[test]
    fn t_division_moves_to_destination() {
        let mut w = World::new();
        let d = Division {
            id: 0,
            owner_tag: "GER".into(),
            location_province: 1,
            destination: Some(2),
            move_progress: 0.0,
            ..Default::default()
        };
        let did = w.add_division(d.clone());
        // 推进 4 次(80%)
        for _ in 0..4 {
            advance_movement(&mut w);
        }
        assert!((w.divisions.get(&did).unwrap().move_progress - 0.8).abs() < 1e-9);
        assert_eq!(w.divisions.get(&did).unwrap().location_province, 1, "未到不应换省");
        // 第5次到达
        advance_movement(&mut w);
        assert_eq!(w.divisions.get(&did).unwrap().location_province, 2, "应到达省2");
        assert!(w.divisions.get(&did).unwrap().destination.is_none());
        let _ = d;
    }

    #[test]
    fn t_retreat_moves_faster() {
        let mut w = World::new();
        let d1 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            destination: Some(2), move_progress: 0.0, retreating: false,
            ..Default::default()
        };
        let d2 = Division {
            id: 0, owner_tag: "X".into(), location_province: 1,
            destination: Some(2), move_progress: 0.0, retreating: true,
            ..Default::default()
        };
        let id1 = w.add_division(d1);
        let id2 = w.add_division(d2);
        advance_movement(&mut w);
        let p1 = w.divisions.get(&id1).unwrap().move_progress;
        let p2 = w.divisions.get(&id2).unwrap().move_progress;
        assert!(p2 > p1, "撤退应更快: normal={p1} retreat={p2}");
    }
}
