//! State 加载器: history/states/*.txt → Vec<State>(初始集合)
//!
//! 原版结构(见 spec §5):
//!   state={ id=1 name="X" manpower=N state_category=town
//!           history={ owner=FRA add_core_of=COR buildings={...} }
//!           provinces={ 3838 9851 } }
//! owner/cores/buildings 在 history={} 子块; provinces 是裸数字列表(Value::List)

use crate::parser::{Block, Value};
use crate::runtime::State;
use std::collections::HashMap;

/// 解析 state 文件, 产出初始 State 集合
pub fn load_states(src: &str) -> Vec<State> {
    let block = match crate::parser::parse(src) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("[data] 警告: state 文件解析失败: {:?}", e);
            return vec![];
        }
    };
    block
        .fields
        .iter()
        .filter(|f| f.key == "state")
        .filter_map(|f| {
            if let Value::Block(sb) = &f.value {
                parse_state_block(sb)
            } else {
                None
            }
        })
        .collect()
}

fn parse_state_block(b: &Block) -> Option<State> {
    let num = |k: &str| {
        b.fields
            .iter()
            .find(|f| f.key == k)
            .and_then(|f| f.value.as_scalar_num())
    };
    let str_val = |k: &str| {
        b.fields
            .iter()
            .find(|f| f.key == k)
            .and_then(|f| f.value.as_scalar_str())
            .unwrap_or("")
            .to_string()
    };

    let id = num("id")? as u32;
    let name = str_val("name");
    let manpower = num("manpower").unwrap_or(0.0);
    let category = str_val("state_category");

    // owner/cores/buildings 在 history={} 子块
    let history = find_block(b, "history")?;
    let owner = history
        .fields
        .iter()
        .find(|f| f.key == "owner")
        .and_then(|f| f.value.as_scalar_str())
        .unwrap_or("")
        .to_string();
    let cores: Vec<String> = history
        .fields
        .iter()
        .filter(|f| f.key == "add_core_of")
        .filter_map(|f| f.value.as_scalar_str().map(String::from))
        .collect();
    let buildings: HashMap<String, f64> = find_block(history, "buildings")
        .map(|bb| {
            bb.fields
                .iter()
                .filter_map(|f| f.value.as_scalar_num().map(|v| (f.key.clone(), v)))
                .collect()
        })
        .unwrap_or_default();

    // resources 在 state 级别(不在 history 内)
    let resources: HashMap<String, f64> = find_block(b, "resources")
        .map(|rb| {
            rb.fields
                .iter()
                .filter_map(|f| f.value.as_scalar_num().map(|v| (f.key.clone(), v)))
                .collect()
        })
        .unwrap_or_default();

    let provinces = parse_provinces_list(b);

    Some(State {
        id,
        name,
        owner: owner.clone(), // 法理归属
        controller: owner,    // 初始 controller = owner(未占领)
        manpower,
        state_category: category,
        cores,
        buildings,
        resources,
        provinces,
    })
}

/// 解析 provinces={ 3838 9851 } 块(裸数字列表)
/// parser 把 { num num } 解析成 Value::List, 不是 Value::Block
fn parse_provinces_list(state_block: &Block) -> Vec<u32> {
    let Some(pf) = state_block.fields.iter().find(|f| f.key == "provinces") else {
        return vec![];
    };
    match &pf.value {
        Value::List(items) => items.iter().filter_map(|s| s.parse::<u32>().ok()).collect(),
        Value::Block(b) => b
            .fields
            .iter()
            .filter_map(|f| f.value.as_scalar_num().map(|v| v as u32))
            .collect(),
        _ => vec![],
    }
}

fn find_block<'a>(block: &'a Block, key: &str) -> Option<&'a Block> {
    block
        .fields
        .iter()
        .find(|f| f.key == key)
        .and_then(|f| if let Value::Block(b) = &f.value { Some(b) } else { None })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn t_load_state_from_block() {
        let src = r#"state={
            id=42
            name="STATE_42"
            manpower = 100000
            state_category = city
            history={
                owner = GER
                add_core_of = GER
                add_core_of = FRA
                buildings = { infrastructure = 3 industrial_complex = 2 }
            }
            provinces={ 100 101 102 }
        }"#;
        let states = load_states(src);
        assert_eq!(states.len(), 1);
        let s = &states[0];
        assert_eq!(s.id, 42);
        assert_eq!(s.owner, "GER");
        assert_eq!(s.controller, "GER"); // 初始 = owner
        assert!((s.manpower - 100000.0).abs() < 1e-9);
        assert_eq!(s.state_category, "city");
        assert_eq!(s.cores, vec!["GER".to_string(), "FRA".to_string()]);
        assert_eq!(s.provinces, vec![100, 101, 102]);
        assert!((s.buildings.get("infrastructure").copied().unwrap_or(0.0) - 3.0).abs() < 1e-9);
    }

    #[test]
    fn t_load_real_state_file() {
        let src = include_str!("../data_raw/states/1-France.txt");
        let states = load_states(src);
        assert!(states.len() >= 1, "应解析出至少 1 个 state");
        let s = states.iter().find(|s| s.id == 1).expect("应有 state id=1");
        assert_eq!(s.owner, "FRA");
        assert!(!s.provinces.is_empty(), "科西嘉应含省份");
    }

    #[test]
    fn t_load_state_with_resources() {
        let src = r#"state={
            id=42
            name="STATE_42"
            manpower = 100000
            state_category = city
            history={ owner = GER }
            resources = { steel = 16 chromium = 3 }
            provinces={ 100 101 }
        }"#;
        let states = load_states(src);
        let s = &states[0];
        assert!(
            (s.resources.get("steel").copied().unwrap_or(0.0) - 16.0).abs() < 1e-9,
            "steel 应 16"
        );
        assert!(
            (s.resources.get("chromium").copied().unwrap_or(0.0) - 3.0).abs() < 1e-9,
            "chromium 应 3"
        );
    }

    #[test]
    fn t_load_state_without_resources_defaults_empty() {
        let src = r#"state={
            id=43 name="X" state_category=town
            history={ owner = GER }
            provinces={ 200 }
        }"#;
        let states = load_states(src);
        assert!(states[0].resources.is_empty(), "无 resources 块应默认空");
    }
}
