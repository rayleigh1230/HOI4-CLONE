# hoi4-clone

HOI4 风格脚本运行时 —— 完整复刻项目的 M1(脚本引擎骨架)。

## 现状(M1)

- ✅ HOI4 脚本词法/语法解析(token → Block 树),支持注释/字符串/负数/布尔/裸比较
- ✅ Block → 有类型 AST(Effect/Trigger)降级,识别 if/limit/every_/random_events
- ✅ 最小 World 状态 + 命令注册 + 解释执行
- ✅ 端到端验证:加载真实国策脚本(afghanistan.txt 片段)并执行
- ✅ 19 个测试全部通过(16 单元 + 3 集成)

## 运行

```bash
cargo run --bin hoi4_demo                       # 跑内置 demo
cargo run --bin hoi4_demo -- path/to/foo.txt    # 跑自定义脚本
cargo test                                      # 全部测试
```

> 注:本项目使用 `stable-x86_64-pc-windows-gnu` 工具链(已通过 rustup override 绑定),
> 因当前环境无 MSVC 链接器。其他平台默认工具链即可。

## 架构

```
src/
├── parser/    词法+语法: HOI4 脚本 → Block 树
├── ast/       Block → Effect/Trigger AST 降级
├── runtime/   World 状态 + Registry 命令注册 + Interpreter 解释执行
└── commands/  具体命令实现(M1: 变量类)
```

详细架构见 `docs/specs/2026-06-20-architecture-design.md`。
陆战公式见 `docs/formulas/land-combat.md`。

## 下一里程碑

M2:核心机制层 —— 战斗引擎(骰子/防御池/装甲/宽度)、生产系统、科技树加载。
