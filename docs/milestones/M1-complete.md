# M1 完成报告 — 脚本引擎骨架

**日期**: 2026-06-20
**结论**: ✅ 方案验证通过,核心风险解除

## 交付物

| 组件 | 文件 | 说明 |
|---|---|---|
| 词法分析 | `src/parser/lexer.rs` | HOI4 脚本 → token(支持注释/字符串/负数/布尔/比较符) |
| 语法分析 | `src/parser/block.rs` | token → 嵌套 Block 树(支持裸比较 `var >= N`) |
| AST 类型 | `src/ast/{effect,trigger}.rs` | Effect/Trigger/Arg/CompareOp |
| 降级转换 | `src/ast/lower.rs` | Block → AST(识别 if/limit/every_/random_events) |
| 状态存储 | `src/runtime/world.rs` | 变量/flags/strings/作用域栈 |
| 命令注册 | `src/runtime/registry.rs` + `src/commands/` | 7 个变量类命令 |
| 解释执行 | `src/runtime/interp.rs` | 执行 Effect AST,求值 Trigger |
| Demo | `src/main.rs` + `examples/demo_focus.txt` | `cargo run` 端到端展示 |

## 测试结果

```
16 单元测试 + 3 集成测试 = 19 通过, 0 失败
cargo clippy --all-targets: 0 警告
cargo run --bin hoi4_demo: 正确输出 political_power=150, stability=0.05, industry_level=1
```

## 关键风险验证结果

| 风险(spec §7) | 结果 |
|---|---|
| HOI4 脚本语法可被正确解析 | ✅ afghanistan.txt 真实片段解析通过 |
| effect/trigger DSL 可用 AST 表达 | ✅ if/limit/every_/random_events 全部支持 |
| 命令注册机制可扩展 | ✅ Registry 模式,新增命令只需一行注册 |
| 真实国策脚本能端到端执行 | ✅ completion_reward 正确修改 World |

**意外发现并修复**:HOI4 trigger 大量使用裸比较(`political_power >= 150`)而非 `key = value`,
TDD 集成测试捕获了这一盲点,parser/lower 已扩展支持。

## 已知简化(M2+ 解决)

- ForEach 不实际枚举省份/国家,只执行一次(M2 接入实体存储后修正)
- Trigger.Check 默认返回 true(M2 接入真实判定逻辑)
- ~500 个 effect/trigger 命令仅实现 7 个(M2-M4 渐进补充)
- 主循环(hourly tick)未实现(M2 搭建)

## 环境备注

- 工具链: `stable-x86_64-pc-windows-gnu`(当前环境无 MSVC 链接器,GNU 工具链自带 rust-mingw 解决)
- 零外部依赖: M1 纯标准库实现(serde/thiserror 待 M2 网络稳定后引入)

## 下一步: M2 核心机制层

按 `docs/specs/2026-06-20-architecture-design.md` §6, M2 目标:
- 战斗引擎(用 `docs/formulas/land-combat.md` 的公式)
- 生产系统(IC/工厂效率/资源)
- 科技树加载
- 主循环(hourly tick + on_actions 钩子)
