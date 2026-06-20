# M3 完成报告 — 实体存储 + 作用域 + 陆战引擎

**日期**: 2026-06-20
**结论**: ✅ "两个师能打仗"目标达成,M3 核心验收通过

## 交付物

| 组件 | 文件 | 说明 |
|---|---|---|
| 实体结构 | `src/runtime/entities.rs` | Province/Country/Division/Battle + Scope 枚举 |
| World 扩展 | `src/runtime/world.rs` | 实体存储 + scope_stack + divisions_of/add_division |
| 作用域枚举 | `src/runtime/interp.rs` | run_for_each: every_owned_state/all_army/every_country/random_country |
| 陆战引擎 | `src/combat/resolve.rs` | resolve_hour(公式见 land-combat.md) + resolve_all_battles |
| 战斗命令 | `src/combat/commands.rs` | create_division/start_battle/is_broken |
| 主循环挂载 | `src/runtime/clock.rs` | on_hourly → resolve_all_battles |

## M3 核心验收

**"两个师能打仗"** — 端到端验证:
```
脚本建两国→各建一师→开战→tick 24h→守方 org 下降 ✅
高强度攻击→守方破阵→is_broken 触发 ✅
无战斗时 org 不变(防误伤) ✅
```

## 作用域框架(M2 review 的核心阻塞已解除)

- `every_owned_state`: 遍历当前国家的省
- `all_army`: 遍历当前国家的师
- `every_country`: 遍历所有国家
- `random_country`: 取首个(M5 接真随机)
- 嵌套作用域靠 scope_stack 压栈/出栈支持
- `current_country` 回退 player_tag(顶层默认玩家)

## 陆战公式实现(对照 docs/formulas/land-combat.md)

| 公式要素 | 实现 | 测试 |
|---|---|---|
| 攻击点(软/硬×硬度) | compute via soft/hard attack | t_inf_vs_inf |
| 防御池(10%/40% 命中) | compute_hits | t_high_defense_reduces_damage |
| 装甲碾压(+6/+2 骰) | armor_outclass check | t_armor_outclass_deals_damage |
| 掷骰(d4/d2 × 系数) | org/str dice | t_inf_vs_inf |
| 多师分摊(35%/65%) | DAMAGE_SPLIT_FIRST | (单师测试, M4 加多师) |

## 关键技术决策

- **避免 unsafe**: resolve_all_battles 用两阶段(读快照→计算→写回)解决 HashMap 多可变借用,无 unsafe
- **resolve_hour 签名**: `&[Division], &mut [&mut Division]` 兼容 HashMap get_mut 收集的引用

## 测试结果
```
36 passed (26 单元 + 4 集成 + 3 作用域 + 3 战斗), 0 failed
cargo clippy --all-targets: 0 警告
```

## 已知简化(M4+ 解决)

- Division 属性硬编码(M4 接装备+营汇总, create_division 已预留参数)
- 单省单战斗(M4 加多方向/宽度)
- 无移动/战略部署(M4)
- 无生产系统(M4)
- ForEach 的 filter 用 Check trigger(已查表, M4 接入真实判定)

## M3 review 反馈处理(NEEDS CHANGES → 已修复关键项)

reviewer 指出 2 个 P0 + 4 个 P1, 已修复:

- ✅ **P0-1 分摊 bug**: 单目标原本只吃 35% 伤害。修正公式: 首要 = 35% + 65%/n, 单目标 = 100%
- ✅ **P0-2 无反击**: 战斗原为单向(攻方无敌)。改为对称结算(攻→守用 defense 池 + 守→攻用 breakthrough 池)
- ✅ **P1-3 骰子期望偏差**: 1dN 期望应为 (N+1)/2, 原误用 N/2(低估 ~20-33%)
- ✅ **P1-4 作用域栈泄漏**: filter 求值失败时 `?` 提前返回未 pop。改为 match + 显式 pop
- ✅ **P1-7 测试强度**: 加 `exact_org_after_one_hour`(精确数值 49.4 锁定公式) + `counter_attack_damages_attacker`
- ✅ **P2-9 armor_deflect**: 原只作用于 str, 现同时作用于 org 和 str
- ✅ **P1-6 丢失更新**: resolve_all_battles 用 HashMap 聚合同一师在多场战斗的结果, 避免覆盖

**保留为 M4 已知简化**(reviewer 标注, 非阻塞 M3 验收):
- P1-5 防御池多攻击者共享消耗(M4 多师宽度战斗时必修)
- P2-8 穿甲系数表(当前二元穿透, M4 加阈值表)
- P2-10 random_country 确定性(HashMap 序, M5 接真随机)
- P2-11 owned_states/省 ID 混淆(M4 接地图时厘清 state vs province)
- P2-14 战斗生命周期(破阵战斗未清理, M4 加)

## M4 准备
战斗引擎就位且公式经精确数值验证, M4 可接入: 装备数据加载(49 文件→Division 属性)、生产系统、多师宽度战斗。
