# M2 完成报告 — 运行时重构 + 主循环

**日期**: 2026-06-20
**结论**: ✅ 3 个 P0 缺陷全部修复,主循环骨架就位,M3 地基已稳

## P0 修复验证(来自 M1 review)

### P0-1 结构化命令参数
- `Arg` 增加 `Block(Vec<(String, Arg)>)` 变体,嵌套块递归
- `Effect::Command` 用 `params: Vec<(String, Arg)>` 替代扁平 `args`
- `lower.rs` 新增 `parse_value` 递归转换,**嵌套块参数不再丢数据**
- 回归测试: `t_lower_nested_block_param_no_data_loss` 验证嵌套块两字段都在

### P0-2 Trigger Registry
- `Registry` 新增 `triggers: HashMap<String, TriggerFn>`
- `Trigger::Check` 求值时查表,未注册返回 **false**(保守,比 M1 的恒 true 更安全)
- 此变化让 afghanistan 集成测试暴露真实依赖:需注册 trigger 才执行

### P0-3 可失败命令签名
- `EffectFn`/`TriggerFn` 返回 `Result<(), CmdError>` / `Result<bool, CmdError>`
- `CmdError` 三变体: UnknownCommand / BadParam / RuntimeError
- Interpreter 收集错误到 `World.error_log`(不中止整体执行)
- 测试: `t_command_returns_error_on_bad_param` 验证坏参数返回 Err

## 主循环(spec §4.2.1)
- `GameClock::tick` 实现 hourly/daily/weekly/monthly 钩子分发
- `World` 加 `hour`/`player_tag`/`event_bus` 字段, `on()`/`fire_event()` 方法
- 测试: `t_daily_hook_fires_after_24_ticks`(24h 后 on_daily 触发)、`t_hourly_fires_every_tick`

## 额外清理
- 删除 M1 review P3-12 的死字段 `scope_stack`
- 修复 M1 review P3-11 死导入(已彻底删除而非 allow 压制)

## 测试结果
```
25 passed (21 单元 + 4 集成), 0 failed
cargo clippy --all-targets: 0 警告
cargo run --bin hoi4_demo: political_power=150, stability=0.05, industry_level=1
```

## M3 准备就绪
地基已稳:
- 命令可携带结构化嵌套参数
- trigger 可注册并真实求值
- 命令失败有错误语义和审计
- 主循环就位,战斗/生产逻辑可挂载为 hourly 钩子

M3 可在健壮的地基上实现战斗引擎(docs/formulas/land-combat.md 的公式)和生产系统。

## 已知简化(M3+ 解决)
- ForEach 仍简化为单次执行(M3 接入实体存储后枚举省份/国家)
- on_actions 脚本文件未自动加载(M3 内容转译)
- 实体存储仍是 vars/flags(M3 引入 Province/Country/Division 实体)
