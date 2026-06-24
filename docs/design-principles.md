# 复刻设计原则

> HOI4 是一个经过多年迭代的、庞大的数据驱动引擎。它的设计远比第一眼看上去精巧。
> 本文件记录复刻过程中沉淀的设计原则, 避免重复踩坑。

---

## 原则 1: 原版设计是首要参考对象

**在做任何系统设计之前, 先去理解原版为什么这么做。**

HOI4 的数据驱动逻辑经过 Paradox 团队多年打磨, 每一个看似"奇怪"的约定背后都有原因。
凭直觉或通用工程经验去设计, 几乎一定会偏离原版, 然后在对接原版数据时被迫返工。

### 反面教训(已发生)

**modifier 叠加规则**(2026-06-24 设计阶段):

我最初设计了"双模式"——脚本命令让作者显式标记 `op = add/multiply`, 数据文件由 loader 按
"文件类型"猜 op(ideas 一律 Add, terrain 一律 Multiply)。

这个设计是错的。去查证原版后发现: **原版根本不需要标记, 也不猜文件类型**。op 是从属性名后缀
自动判断的:
- `soft_attack = 0.05`(无后缀) → Add(直接加)
- `soft_attack_factor = 0.05`(`_factor` 后缀) → Multiply(独立乘一层)

属性名本身就编码了 op。这是 Paradox 脚本的全局约定, 一个 idea 可以同时写
`soft_attack = 10`(加固定值) 和 `soft_attack_factor = 0.05`(乘百分比), 两种语义都合法且共存。

**如果我当时直接照自己的"双模式"实现, 后续加载真实 ideas 文件时就会发现: 文件里写的
是 `soft_attack_factor`, 但我的 loader 按"ideas 一律 Add"错误归类, 导致数值全错。**

### 验证方法(优先级从高到低)

1. **看原版数据文件**(`common/` 下对应目录)——真实数据是最终事实。一个属性在文件里怎么写,
   引擎就必须怎么解析。
2. **查 defines**(`common/defines/00_defines.lua`)——数值常量和开关, 揭示机制边界。
3. **查 wiki**([hoi4.paradoxwikis.com](https://hoi4.paradoxwikis.com))——机制说明和公式。
4. **查社区讨论**(Paradox 论坛 / Reddit r/hoi4)——模糊规则的澄清, 但要交叉验证。

### 实践准则

- 设计新系统时, 先打开原版对应的 `common/` 子目录, 看数据文件的真实结构。
- 遇到"原版这里设计得好奇怪"的想法时, **先假设是自己没理解透**, 去查证, 不要轻易改造。
- 遇到 parser/loader 加载失败时, 多半是原版有我们没料到的语法约定(BOM、日期格式、
  命名空间限定 `xxx:yyy`、裸 ident 列表等)——**逐个修 parser 去适配原版, 而非改数据**。
- "简化"要谨慎: 原版的某些复杂性(modifier 的 add/multiply 区分、装备的三层继承)
  是支撑整个数据生态的, 简化掉会在后续系统连环出问题。

---

## 原则 2: 数据驱动优先, 硬编码是债

师的属性、装备数值、模板结构——都应该从原版数据文件加载, 而非硬编码在 Rust 里。
硬编码的数值(equipment_data.rs 的 static 表、resolve.rs 的常量)是"还没接通数据驱动"的债,
后续应逐步替换为从 GameData 读取。

---

## 原则 3: 骨架要先于内容

搭系统骨架(数据结构 + 接口 + 加载管线)时, 要预留后续系统的接入点, 但**不实现后续系统本身**。
例如 modifier 层先建好 CombatContext + ModifierStack 接口, 但科技/国策/堑壕的具体 modifier
内容等对应系统做时再加。

判断"骨架是否完整"的标准: **后续加任何系统时, 是否需要改现有结算/加载代码?**
如果需要, 说明骨架有缺口(如 modifier 层缺失); 如果只需"往接口塞数据", 说明骨架完整。

---

## 附: 已查证的原版规则备忘

| 规则 | 结论 | 来源 |
|---|---|---|
| modifier 叠加 | 同类属性无后缀(`soft_attack`)相加; `_factor` 后缀(`soft_attack_factor`)独立乘 | Paradox 脚本全局约定 + wiki |
| 装甲/穿甲汇总 | 60%平均 + 40%最高(营层) | defines `ARMOR_VS_AVERAGE=0.4` |
| 装备继承 | archetype(槽位+default_modules) → 具体型号(inherit+预填数值) → 装备变体 | `tank_chassis.txt` 实证 |
| org_loss_when_moving | 可配加法/乘法(`USE_MULTIPLICATIVE_ORG_LOSS_WHEN_MOVING`, 默认乘) | defines 第770行 |
