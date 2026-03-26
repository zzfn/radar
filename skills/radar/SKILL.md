---
name: radar
description: 代码依赖分析工具。修改文件/函数前评估影响范围，探查函数结构，检测循环依赖，定位死代码和高风险节点。Use when modifying code files or functions to assess blast radius.
license: MIT
metadata:
  author: nio-wad
  version: "0.4.0"
---

# radar

**用途**：修改代码前评估影响范围，探查项目结构，定位死代码和高风险依赖节点。

二进制：`./skills/radar/scripts/radar`

---

## 触发时机

在以下场景**主动调用**，无需用户明确要求：
- 用户准备修改某个文件或函数时
- 用户询问"这个文件被谁依赖"、"改这里有什么影响"
- 用户询问"哪些代码没人用"、"哪个文件改动风险最高"
- 用户想知道"A 为什么依赖 B"或"A 到 B 的依赖链"
- 修改完成后，用户要求确认是否引入循环依赖

---

## 执行流程

### 场景一：修改文件前（评估影响范围）

```bash
radar impact <文件绝对路径> --root <项目根目录>
```

根据输出决策：
- `total_affected == 0` → 低风险，直接修改
- `total_affected < 5`  → 中风险，告知用户受影响文件列表后再修改
- `total_affected >= 5` → 高风险，展示影响链，建议用户确认后再继续
- `has_cycles: true`    → 提醒用户依赖链中已有循环，修改需额外谨慎

### 场景二：修改函数前（函数级影响）

先用 `functions` 确认函数存在，再查影响：

```bash
# 1. 确认函数名
radar functions <目录> --lang <语言>

# 2. 查询调用者
radar impact <文件绝对路径> --function <函数名> --root <项目根目录>
```

根据输出决策：
- `total_callers == 0` → 无调用者，修改安全
- `total_callers > 0`  → 列出所有调用者，提示用户可能需要同步修改

**精度边界**（告知用户）：
- 同文件调用、跨文件唯一名函数 → 准确
- 同名函数存在多个 → 保守跳过，实际影响可能更大
- 动态派发/回调/反射 → 无法覆盖

### 场景三：修改完成后检查循环依赖

```bash
radar cycles <项目根目录> --json
```

- 返回空数组 `[]` → 无循环依赖，安全
- 返回非空 → 列出循环路径，建议用户处理后再提交

### 场景四：探查项目整体健康度

开始大范围重构前，快速扫描风险：

```bash
# 找高风险核心节点（改动影响最大的文件）
radar hotspot <项目根目录> --top 10

# 找未被引用的死代码
radar unused <项目根目录> --functions
```

- hotspot 入度越高 → 改动该文件影响越广，需更谨慎
- unused 文件/函数 → 可候选清理，修改无风险

### 场景五：追踪依赖来源

当需要解释"为什么 A 会依赖 B"时：

```bash
radar path <项目根目录> --from <文件A绝对路径> --to <文件B绝对路径>
```

- 返回最短依赖路径和跳数
- 无路径时明确提示"无依赖关系"

---

## 子命令速查

| 子命令 | 典型用法 | 输出 |
|--------|---------|------|
| `impact` | 修改前评估影响 | JSON（默认）/ `--text` |
| `functions` | 探查函数定义 | JSON（默认）/ `dot` / `mermaid` / `tree` |
| `cycles` | 检测循环依赖 | 文本 / `--json` |
| `unused` | 死代码检测 | tree（默认）/ `--output json` |
| `hotspot` | 高风险节点 | tree（默认）/ `--output json` |
| `path` | 依赖路径查找 | tree（默认）/ `json` / `mermaid` |
| `analyze` | 全量依赖图 | tree / json / dot / mermaid |

---

## 常用参数

| 参数 | 适用命令 | 说明 |
|------|---------|------|
| `--root <dir>` | `impact` | 项目根目录（可选，默认当前目录） |
| `--function <name>` | `impact` | 函数级影响分析 |
| `--depth <n>` | `impact` `analyze` | 最大追踪跳数，0=不限 |
| `--lang <lang>` | 所有命令 | 指定语言：`rust` `ts` `js` `go` `python` `java` `vue` |
| `--text` | `impact` | 人类可读输出（文件级和函数级均支持） |
| `--output <fmt>` | 多数命令 | `json` / `dot` / `mermaid` / `tree` |
| `--functions` | `unused` | 同时检测未调用函数 |
| `--top <n>` | `hotspot` | 显示前 N 个节点（默认 10） |
| `--from / --to` | `path` | 指定起止文件 |

---

## 注意事项

- 路径使用**绝对路径**，避免相对路径歧义
- `impact` 的第一个参数是**目标文件**，`--root` 是项目根目录
- `--root` 应为包含源码的项目根目录，不是文件所在目录
- 语言检测失败时显式传 `--lang`
