# 项目约束(hoi4-clone)

> 本文件是项目级硬约束, 在本项目下的**所有对话自动加载**, 优先级高于通用行为。
> 详细原则见 `docs/design-principles.md`; 本文件只列不可违反的红线。

## 红线 1: 原版定义 > 直觉, 必须先查证再实现

本项目是 HOI4 复刻。**任何涉及游戏机制的设计, 实现前必须查证原版真实定义, 不能凭直觉/通用经验推断。**

凭直觉推断几乎一定会偏离原版, 然后在对接原版数据或被玩家验证时返工。原版的机制经过 Paradox 多年打磨, 看似"奇怪"的约定背后都有原因。

### 查证优先级(从高到低)

1. **原版数据文件**(`src/data_raw/` 已嵌入的; 或原版 `common/` 目录) — 真实数据是最终事实
2. **defines**(`common/defines/00_defines.lua`) — 数值常量和机制边界
3. **wiki**([hoi4.paradoxwikis.com](https://hoi4.paradoxwikis.com)) — 机制说明和公式
4. **社区讨论** — 模糊规则澄清, 但要交叉验证

### 必查清单(实现新机制前)

- [ ] 这个机制在原版数据文件里**怎么写**?(字段名、值、结构)
- [ ] defines 里有没有相关常量?
- [ ] 作用对象是谁?(攻方/守方/双方?哪个 stat?)
- [ ] 触发时机?(结算时/读取时/每日?)

**任一项答不上来 = 还没查证够, 不能动手写实现。**

### 反面教训(已发生, 引以为戒)

- **地形惩罚的攻守归属**(2026-06-26): 我凭直觉认为"谁开火罚谁", 把地形惩罚同时乘到攻方正向和守方反击。**查证后发现原版只罚攻方身份**(攻守身份整场战斗固定, 不随反击翻转), 且还漏罚了 breakthrough。返工修正。
- **modifier 的 add/multiply**(2026-06-24): 最初设计"双模式"(显式标记 op), 查证后发现原版用属性名后缀(`_factor`)自动推导, 根本不需要标记。

### 实践准则

- 遇到"原版这里设计得好奇怪" → **先假设是自己没理解透**, 去查证, 不要改造。
- parser/loader 加载失败 → 多半是原版有没料到的语法约定(BOM/日期/命名空间/裸列表), **逐个修 parser 适配, 不改数据**。
- "简化"要谨慎: 原版的某些复杂性(modifier 的 add/multiply 区分、装备三层继承)是支撑数据生态的, 简化掉会连环出问题。

---

## 红线 2: 改完跑全量测试

改 `struct` 字段或核心公式后, 必须跑**全量** `cargo test`(含 `tests/` 集成目标), 不能只看 `src/` 内联测试。`tests/` 集成测试容易因 struct 加字段而编译失败, 但内联测试发现不了。

```bash
cargo test                    # 全量(含 tests/ 集成)
# integration 偶发 flaky(TEST_BLOCKED 跨测试泄漏, 既有问题), 用单线程稳定:
cargo test -- --test-threads=1
```

当前基线: **206 测试全绿**。见 `docs/HANDOFF.md`。

---

## 技术约束(踩过的坑)

- **工具链**: `stable-x86_64-pc-windows-gnu`(rustup override 绑定, 无 MSVC 链接器)
- **WASM FFI**: u64 参数在 JS 侧要 BigInt, 用 u32 避免
- **WASM 更新后**: fetch 加 `?v=Date.now()` 防缓存
- **借用冲突**: `get_mut` 持借用时不能再 `divisions.values()`, 用快照→计算→写回
- **敌人判定**: 用 `are_at_war`/`enemies_of`, 不能用 `owner_tag != owner`(旧的全员敌对)
- **资源命令**: 需国家作用域(player_tag 兜底); 无国家时报错(非静默)
- **ParamGet::get**: 要全限定调用(`ParamGet::get(p, key)`), slice 的 inherent `get(usize)` 会遮蔽

## 文档导航

- `docs/HANDOFF.md` — 项目全貌 + 当前状态(新会话先读这个)
- `docs/design-principles.md` — 复刻设计原则(详细版)
- `docs/formulas/land-combat.md` — 陆战公式
- `docs/superpowers/specs/` — 各系统的设计文档
- `docs/superpowers/plans/` — 各系统的实现计划
