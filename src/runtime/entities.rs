//! 游戏实体结构(M3)
#[derive(Debug, Clone)]
pub struct Province {
    pub id: u32,
    pub owner: String,
    pub controller: String,
    pub terrain: String,
}

#[derive(Debug, Clone, Default)]
pub struct Country {
    pub tag: String,
    pub owned_states: Vec<u32>,
    pub capital_state: u32,
}

#[derive(Debug, Clone)]
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
}

impl Division {
    pub fn org_ratio(&self) -> f64 {
        if self.max_org > 0.0 {
            self.org / self.max_org
        } else {
            0.0
        }
    }
    pub fn is_broken(&self) -> bool {
        self.org <= 0.0
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
