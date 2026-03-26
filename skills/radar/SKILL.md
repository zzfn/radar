---
name: radar
description: >
  代码依赖分析工具。修改文件/函数前评估影响范围（blast radius），检测循环依赖，
  定位死代码（unused files/functions），识别高风险核心节点（hotspot），
  查找两文件间依赖路径（why does A depend on B）。
  Use when: modifying files/functions, refactoring, reviewing PRs for dependency changes,
  finding dead code / unused code, understanding why two modules are coupled,
  assessing change risk before editing shared utilities.
license: MIT
metadata:
  author: nio-wad
  version: "0.4.0"
---

# radar

**用途**：修改代码前评估影响范围，探查项目结构，定位死代码和高风险依赖节点。

二进制：`./skills/radar/scripts/radar`

---

## NEVER

- **NEVER 用相对路径**：radar 不推断上下文，相对路径会静默失败（返回空结果，不报错），
  必须传绝对路径。用 `$(realpath <path>)` 或 `$(pwd)/<path>` 转换。
- **NEVER 在含 vendor/node_modules 的目录直接运行**：会分析数千个无关文件，
  结果噪声极大。改为分析 `src/` 子目录，或加 `--exclude "node_modules/**"`。
- **NEVER 把 unused 函数结果当作可以安全删除的确定依据**：JS 回调、Python 装饰器、
  Go 接口实现、Rust trait impl 均无法被静态调用图覆盖，tree-sitter 只识别直接调用。
  unused 结果是"候选"，不是"确定"。
- **NEVER 跳过 functions 验证步骤直接运行 impact --function**：函数名拼写错误或
  语言不受 tree-sitter 支持时，impact 会静默返回 `total_callers=0`，被误判为安全。
- **NEVER 把 total_affected 极高（>30）时的结果直接展示给用户**：说明修改的是底层
  公共模块，先用 `--depth 2` 限制层级，聚焦直接依赖者，否则信息过载反而无用。

---

## 触发时机

在以下场景**主动调用**，无需用户明确要求：
- 用户准备修改某个文件或函数时
- 用户询问"这个文件被谁依赖"、"改这里有什么影响"
- 用户询问"哪些代码没人用"、"哪个文件改动风险最高"
- 用户想知道"A 为什么依赖 B"或"A 到 B 的依赖链"
- 修改完成后，确认是否引入循环依赖

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
- `total_affected > 30` → 底层公共模块，改用 `--depth 2` 聚焦直接影响层
- `has_cycles: true`    → 依赖链已有循环，修改可能扩大循环范围，额外谨慎

**异常情况：**

| 情况 | 可能原因 | 处理方式 |
|------|---------|---------|
| 结果为空（0 节点） | 语言检测失败 | 显式传 `--lang <lang>` |
| 分析耗时极长 | 目录含 vendor/生成文件 | 改为分析 `src/` 子目录 |
| binary 找不到 | skill 未编译 | `cargo build --release && cp target/release/radar skills/radar/scripts/` |

### 场景二：修改函数前（函数级影响）

**必须两步走**，不可跳过步骤一：

```bash
# 步骤一：确认函数名存在（防止静默失败）
radar functions <目录> --lang <语言>

# 步骤二：查询调用者
radar impact <文件绝对路径> --function <函数名> --root <项目根目录>
```

根据输出决策：
- `total_callers == 0` → 无静态调用者，修改安全（但见下方精度说明）
- `total_callers > 0`  → 列出所有调用者，提示用户可能需要同步修改

**精度边界**（必须告知用户）：
- 同文件调用、跨文件唯一名函数 → **准确**
- 同名函数存在多个 → 保守跳过，实际影响**可能更大**
- 动态派发/回调/反射/接口实现 → **无法覆盖**，结果仅供参考

**步骤一返回空时的降级策略：**
该语言不受 tree-sitter 支持（如 Java、Vue），降级为文件级分析：
```bash
radar impact <文件绝对路径> --root <项目根目录>
```

### 场景三：修改完成后检查循环依赖

```bash
radar cycles <项目根目录> --json
```

- 返回空数组 `[]` → 无循环依赖，安全
- 返回非空 → 列出循环路径，建议用户处理后再提交

### 场景四：探查项目整体健康度

大范围重构前，先扫描风险分布：

```bash
# 高风险核心节点（入度越高，改动影响越广）
radar hotspot <项目根目录> --top 10

# 未被引用的死代码候选
radar unused <项目根目录> --functions
```

**解读规则：**
- hotspot 入度 > 10 → 该文件是架构核心，修改需完整的 impact 评估
- hotspot 入度 1-3 → 普通模块，正常修改
- unused 文件 out_degree > 0 → 该文件依赖其他模块但无人依赖它，是典型孤立模块
- unused 文件 out_degree == 0 → 完全孤立，可安全删除

### 场景五：追踪依赖来源

解释"为什么 A 会依赖 B"：

```bash
radar path <项目根目录> --from <文件A绝对路径> --to <文件B绝对路径>
```

- 返回路径和跳数 → 按路径逐跳解释耦合原因
- 无路径 → 两模块无依赖关系，告知用户可以独立修改

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

## 常用参数

| 参数 | 适用命令 | 说明 |
|------|---------|------|
| `--root <dir>` | `impact` | 项目根目录（可选，默认当前目录） |
| `--function <name>` | `impact` | 函数级影响分析 |
| `--depth <n>` | `impact` `analyze` | 最大追踪跳数，0=不限 |
| `--lang <lang>` | 所有命令 | `rust` `ts` `js` `go` `python` `java` `vue` |
| `--text` | `impact` | 人类可读输出（文件级和函数级均支持） |
| `--output <fmt>` | 多数命令 | `json` / `dot` / `mermaid` / `tree` |
| `--functions` | `unused` | 同时检测未调用函数 |
| `--top <n>` | `hotspot` | 显示前 N 个节点（默认 10） |
| `--from / --to` | `path` | 指定起止文件（绝对路径） |
