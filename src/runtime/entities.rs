//! 游戏实体结构(M3)
use std::collections::HashMap;

#[derive(Debug, Clone, Default)]
pub struct Province {
    pub id: u32,
    /// 所属 State id(归属从 State 派生, Province 不再存 owner/controller)
    pub state_id: u32,
    pub terrain: String,
    /// 邻接省份 id 列表(行军/撤退的基础设施)
    pub neighbors: Vec<u32>,
}

/// 州/地区(Province 的上级容器, 归属/建筑/人力的唯一权威源)
/// 可变运行时状态(进 World, 不进 GameData)
/// 设计见 docs/superpowers/specs/2026-06-24-state-concept-design.md
#[derive(Debug, Clone, Default)]
pub struct State {
    pub id: u32,
    pub name: String,              // "STATE_1"(本地化 key)
    pub owner: String,             // 法理归属(谁拥有这片领土)
    pub controller: String,        // 实际控制(可能被占领, ≠ owner)
    pub manpower: f64,             // 人力(征兵来源)
    pub state_category: String,    // "town"/"city"/"megalopolis"(决定建筑槽位)
    pub cores: Vec<String>,        // 核心国 tag(谁有合法领土声索)
    pub buildings: HashMap<String, f64>,  // 建筑占位映射(后续建筑系统升级)
    pub provinces: Vec<u32>,       // 这个 State 包含哪些省份(正向映射)
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
    /// modifier 汇总(科技/精神/ideas 等国家级修正)
    pub modifiers: crate::combat::modifier::ModifierStack,
}

/// 行动状态机(替代原 7 个扁平字段 retreating/destination/move_progress/attacking/
/// origin_province/pending_arrival/supporting)。
///
/// 设计要点:
/// - Retreating 期间对其他战斗系统(check_engagements/占地)不可见
/// - location_province 在 Retreating/Moving 期间保持出发地原值, 到达才更新
/// - 攻方失败回 origin 时若 origin 已非己方 → 找邻省 → 都没有则歼灭(根治瞬移)
#[derive(Debug, Clone, Default)]
pub enum OrderState {
    /// 静止、非战斗(可作守方被拉入战斗, 但本身无主动指令)
    #[default]
    Idle,
    /// 主动行军: dest=当前段终点, progress=0..1, hostile=是否进军敌方地块(红箭头),
    /// origin=当前段出发地, remaining=dest 之后还要去的省(多段路径, 不含 dest)
    Moving { dest: u32, progress: f64, hostile: bool, origin: u32, remaining: Vec<u32> },
    /// 撤退行军: dest=撤退目标(己方省), progress=0..1
    /// 对其他战斗系统不可见(check_engagements/占地判定跳过此状态的师)
    /// location_province 在 Retreating 期间保持撤退开始时的原值, 到达才改
    Retreating { dest: u32, progress: f64 },
    /// 到达目标但战斗未胜, 等战斗胜利才结算归属。remaining=战斗胜后续走的剩余路径
    Pending { dest: u32, remaining: Vec<u32> },
    /// 支援攻击: 不移动, 作为攻方远程参战 target 省的战斗
    Supporting { target: u32 },
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
    /// 行动状态机(替代原 7 个扁平字段)
    pub order: OrderState,
    /// modifier 汇总(堑壕/计划/经验等师自身修正)
    pub modifiers: crate::combat::modifier::ModifierStack,
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
    /// 是否满足进入撤退的条件(瞬时判定: org 归零 + HP 有余)。
    /// 注意: 这是"应否撤退"的触发条件, 与"是否已在 Retreating 状态"(is_withdrawing)不同。
    pub fn should_withdraw(&self) -> bool {
        self.org <= 0.0 && self.strength > 0.0
    }
    /// 兼容别名(= should_withdraw)。迁移期保留, 迁移完成后改调用点为 should_withdraw。
    pub fn is_retreating(&self) -> bool {
        self.should_withdraw()
    }
    /// 当前是否处于撤退行军状态(读 enum)
    pub fn is_withdrawing(&self) -> bool {
        matches!(self.order, OrderState::Retreating { .. })
    }
    /// 当前是否在主动行军(Moving)
    pub fn is_moving(&self) -> bool {
        matches!(self.order, OrderState::Moving { .. })
    }
    pub fn is_supporting(&self) -> bool {
        matches!(self.order, OrderState::Supporting { .. })
    }
    pub fn is_pending(&self) -> bool {
        matches!(self.order, OrderState::Pending { .. })
    }
    pub fn is_idle(&self) -> bool {
        matches!(self.order, OrderState::Idle)
    }
    /// 撤退目的地(Retreating 时有值)
    pub fn retreat_dest(&self) -> Option<u32> {
        if let OrderState::Retreating { dest, .. } = self.order {
            Some(dest)
        } else {
            None
        }
    }
    pub fn move_dest(&self) -> Option<u32> {
        if let OrderState::Moving { dest, .. } = self.order {
            Some(dest)
        } else {
            None
        }
    }
    pub fn pending_dest(&self) -> Option<u32> {
        if let OrderState::Pending { dest, .. } = self.order {
            Some(dest)
        } else {
            None
        }
    }
    /// 当前行军的出发地(Moving 时有值)
    pub fn move_origin(&self) -> Option<u32> {
        if let OrderState::Moving { origin, .. } = self.order {
            Some(origin)
        } else {
            None
        }
    }
    /// 当前是否在进军敌方地块(红箭头)
    pub fn is_attacking_move(&self) -> bool {
        matches!(self.order, OrderState::Moving { hostile: true, .. })
    }
    /// 行军进度(0..1), Moving/Retreating 有值
    pub fn move_progress(&self) -> f64 {
        match self.order {
            OrderState::Moving { progress, .. } | OrderState::Retreating { progress, .. } => progress,
            _ => 0.0,
        }
    }
    /// 已退出战斗(撤退中 或 歼灭)
    pub fn is_broken(&self) -> bool {
        self.is_withdrawing() || self.is_annihilated()
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
    // 有效属性 = 面板值 × 综合补给充足度 × modifier
    pub fn effective_soft_attack(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.soft_attack * self.supply_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::SoftAttack)
    }
    pub fn effective_hard_attack(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.hard_attack * self.supply_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::HardAttack)
    }
    pub fn effective_defense(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.defense * self.equipment_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::Defense)
    }
    pub fn effective_breakthrough(&self, mods: &crate::combat::modifier::ModifierStack) -> f64 {
        self.breakthrough * self.equipment_ratio()
            * mods.multiplier(crate::combat::modifier::ModifierStat::Breakthrough)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Battle {
    pub id: u64,
    pub province: u32,
    pub attackers: Vec<u64>,
    pub defenders: Vec<u64>,
    /// 预备队(超宽度的师在此等候补位)
    pub reserve_attackers: Vec<u64>,
    pub reserve_defenders: Vec<u64>,
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
