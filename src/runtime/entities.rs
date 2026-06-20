//! 游戏实体结构(M3)
#[derive(Debug, Clone, Default)]
pub struct Province {
    pub id: u32,
    pub owner: String,
    pub controller: String,
    pub terrain: String,
    /// 邻接省份 id 列表(行军/撤退的基础设施)
    pub neighbors: Vec<u32>,
}

#[derive(Debug, Clone, Default)]
pub struct Country {
    pub tag: String,
    pub owned_states: Vec<u32>,
    pub capital_state: u32,
    /// 装备库存(M4a): equipment_type → 数量
    pub equipment_stockpile: std::collections::HashMap<String, f64>,
    /// 人力池(陆战循环): 国家征召的兵员储备
    pub manpower_pool: f64,
}

#[derive(Debug, Clone, Default)]
pub struct Division {
    pub id: u64,
    pub owner_tag: String,
    pub location_province: u32,
    // 战斗属性(M3 硬编码; M4 由装备+营汇总)
    pub soft_attack: f64,
    pub hard_attack: f64,
    pub defense: f64,
    pub breakthrough: f64,
    pub armor: f64,
    pub piercing: f64,
    pub hardness: f64,
    pub combat_width: f64,
    // 当前状态
    pub max_org: f64,
    pub org: f64,
    pub max_strength: f64,
    pub strength: f64,
    // 装备(M4a): need=满编需求, held=当前持有
    pub equipment_need: std::collections::HashMap<String, f64>,
    pub equipment_held: std::collections::HashMap<String, f64>,
    // 人力(陆战循环): 独立于装备的兵员资源
    pub manpower_need: f64,
    pub manpower_held: f64,
    // 撤退标志: org 归零但 HP 有余 → 撤退(保留师, 移出战斗, 待撤邻省)
    pub retreating: bool,
    // 行军状态: 目标省(None=静止), 进度(0-1, 到1完成移动)
    pub destination: Option<u32>,
    pub move_progress: f64,
}

impl Division {
    pub fn org_ratio(&self) -> f64 {
        if self.max_org > 0.0 {
            self.org / self.max_org
        } else {
            0.0
        }
    }
    /// HP 归零 → 歼灭(番号撤销, 师彻底消失)
    pub fn is_annihilated(&self) -> bool {
        self.strength <= 0.0
    }
    /// 组织度归零 + HP 有余 → 撤退(保留师, 移出战斗恢复)
    /// 注意: 这是触发撤退的瞬间条件; 撤退后 retreating 标志持续, org 可能恢复
    pub fn is_retreating(&self) -> bool {
        self.org <= 0.0 && self.strength > 0.0
    }
    /// 已退出战斗(撤退中 或 歼灭) — 兼容旧调用
    pub fn is_broken(&self) -> bool {
        self.retreating || self.is_annihilated()
    }
    /// 综合补给充足度(0-1): min(装备比, 人力比)。木桶效应, 短板决定。
    /// (原名 equipment_ratio, 保留以兼容调用; 实为四量模型的综合充足度)
    pub fn equipment_ratio(&self) -> f64 {
        self.supply_ratio()
    }
    /// 综合补给充足度 = min(装备充足度, 人力充足度)
    pub fn supply_ratio(&self) -> f64 {
        let eq = self.equipment_ratio_only();
        let mp = self.manpower_ratio();
        eq.min(mp)
    }
    /// 仅装备充足度
    pub fn equipment_ratio_only(&self) -> f64 {
        let need: f64 = self.equipment_need.values().sum();
        let held: f64 = self.equipment_held.values().sum();
        if need > 0.0 {
            (held / need).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
    /// 仅人力充足度
    pub fn manpower_ratio(&self) -> f64 {
        if self.manpower_need > 0.0 {
            (self.manpower_held / self.manpower_need).clamp(0.0, 1.0)
        } else {
            1.0
        }
    }
    // 有效属性 = 面板值 × 综合补给充足度
    pub fn effective_soft_attack(&self) -> f64 {
        self.soft_attack * self.supply_ratio()
    }
    pub fn effective_hard_attack(&self) -> f64 {
        self.hard_attack * self.supply_ratio()
    }
    pub fn effective_defense(&self) -> f64 {
        self.defense * self.equipment_ratio()
    }
    pub fn effective_breakthrough(&self) -> f64 {
        self.breakthrough * self.equipment_ratio()
    }
}

#[derive(Debug, Clone)]
pub struct Battle {
    pub id: u64,
    pub province: u32,
    pub attackers: Vec<u64>,
    pub defenders: Vec<u64>,
}

/// 作用域(M3: 枚举栈)
#[derive(Debug, Clone)]
pub enum Scope {
    Root,
    Country(String),
    Province(u32),
    Division(u64),
    Battle(u64),
}

impl Scope {
    pub fn country_tag(&self) -> Option<&str> {
        if let Scope::Country(t) = self {
            Some(t)
        } else {
            None
        }
    }
    pub fn province_id(&self) -> Option<u32> {
        if let Scope::Province(p) = self {
            Some(*p)
        } else {
            None
        }
    }
    pub fn division_id(&self) -> Option<u64> {
        if let Scope::Division(d) = self {
            Some(*d)
        } else {
            None
        }
    }
}
