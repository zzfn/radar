---
name: radar
description: >
  代码依赖分析工具。修改文件/函数前评估影响范围（blast radius），检测循环依赖，
  定位死代码（unused files/functions），识别高风险核心节点（hotspot），
  查找两文件间依赖路径（why does A depend on B）。
  Use when: modifying files/functions, refactoring, reviewing PRs for dependency changes,
  finding dead code / unused code, understanding why two modules are coupled,
  assessing change risk before editing shared utilities.
  主动触发：用户准备修改文件或函数时、询问依赖关系或改动风险时、开始大范围重构前。
license: MIT
metadata:
  author: nio-wad
  version: "0.4.0"
---

# radar

二进制：`./skills/radar/scripts/radar`

---

## NEVER

- **NEVER 用相对路径**：radar 不推断上下文，相对路径静默失败（返回空结果，不报错）。
  用 `$(realpath <path>)` 或 `$(pwd)/<path>` 转换。
- **NEVER 在含 vendor/node_modules 的目录直接运行**：分析数千无关文件，噪声极大。
  改为分析 `src/` 子目录，或加 `--exclude "node_modules/**"`。
- **NEVER 把 unused 函数结果当作可以安全删除的确定依据**：JS 回调、Python 装饰器、
  Go 接口实现、Rust trait impl 均无法被静态调用图覆盖，结果是"候选"而非"确定"。
- **NEVER 跳过 functions 验证直接运行 impact --function**：函数名拼错或语言不支持
  tree-sitter 时，impact 静默返回 `total_callers=0`，被误判为安全。
- **NEVER 把 total_affected > 30 的结果直接展示给用户**：是底层公共模块，先用
  `--depth 2` 聚焦直接依赖层，否则信息过载反而无用。

---

## 修改前的决策框架

运行 radar 之前，先判断修改的性质，选择正确的分析路径：

| 修改类型 | 首选分析 | 原因 |
|---------|---------|------|
| 工具函数/utils | 文件级 `impact` | 调用者广，函数图噪声更大 |
| 业务逻辑函数 | 函数级 `impact --function` | 精确定位，避免过度告警 |
| Java / Vue 文件 | 只用文件级 | tree-sitter 不支持这两种语言 |
| 删除或重命名 | 先 `unused`，再 `impact` | 确认无遗漏引用后再操作 |
| 跨模块重构 | `hotspot` → `path` → `impact` | 先摸清架构，再逐一评估 |
| hotspot 列表中的文件 | `impact` + `cycles` | 核心节点改动需同时检查是否新增循环 |

---

## 分析粒度选择

函数名的唯一性决定函数级分析的可信度：

| 情况 | 可信度 | 策略 |
|------|--------|------|
| 函数名在项目内唯一 | 高 | 函数级结果可直接使用 |
| 函数名在项目内重复 | 低 | 保守跳过，降级为文件级分析 |
| 语言不受 tree-sitter 支持（Java/Vue） | 无 | 只用文件级 |

先用 `radar functions <dir> --output json` 确认函数名唯一性，再决定是否信任函数级结果。

---

## 执行流程

### 场景一：修改文件前

```bash
radar impact <文件绝对路径> --root <项目根目录>
```

决策：
- `total_affected == 0` → 低风险，直接修改
- `total_affected < 5`  → 中风险，告知受影响文件列表后修改
- `total_affected >= 5` → 高风险，展示影响链，建议用户确认
- `total_affected > 30` → 底层公共模块，改用 `--depth 2` 聚焦直接影响层
- `has_cycles: true`    → 依赖链已有循环，修改可能扩大循环范围

**异常处理：**

| 情况 | 原因 | 处理 |
|------|------|------|
| 结果为空（0 节点） | 语言检测失败 | 显式传 `--lang <lang>` |
| 分析耗时极长 | 目录含 vendor/生成文件 | 改为分析 `src/` 子目录 |
| binary 找不到 | skill 未编译 | `cargo build --release && cp target/release/radar skills/radar/scripts/` |

### 场景二：修改函数前

**必须两步走**，不可跳过步骤一：

```bash
# 步骤一：确认函数名存在 + 检查唯一性
radar functions <目录> --lang <语言>

# 步骤二：查询调用者
radar impact <文件绝对路径> --function <函数名> --root <项目根目录>
```

决策：
- `total_callers == 0` → 无静态调用者（注意精度边界）
- `total_callers > 0`  → 列出所有调用者，提示可能需同步修改

精度边界（必须告知用户）：
- 同文件调用、跨文件唯一名 → **准确**
- 同名函数多个 → 保守跳过，实际影响**可能更大**
- 动态派发/回调/接口实现 → **无法覆盖**

步骤一返回空（语言不支持）→ 降级为文件级 `impact`。

### 场景三：修改完成后检查循环

```bash
radar cycles <项目根目录> --json
```

空数组 `[]` → 安全；非空 → 列出循环路径，建议处理后再提交。

### 场景四：大范围重构前扫描

```bash
radar hotspot <项目根目录> --top 10   # 高风险核心节点
radar unused <项目根目录> --functions  # 死代码候选
```

解读：
- hotspot 入度 > 10 → 架构核心，修改需完整 impact 评估
- hotspot 入度 1-3  → 普通模块，正常修改
- unused 文件 out_degree > 0 → 孤立模块（依赖别人但无人依赖），候选清理
- unused 文件 out_degree == 0 → 完全孤立，可安全删除

### 场景五：追踪依赖来源

```bash
radar path <项目根目录> --from <文件A绝对路径> --to <文件B绝对路径>
```

有路径 → 按路径逐跳解释耦合原因；无路径 → 两模块独立，可分别修改。

---

## 子命令速查

| 子命令 | 典型用法 | 输出 |
|--------|---------|------|
| `impact` | 修改前评估影响 | JSON / `--text` |
| `functions` | 探查函数定义 | JSON / `dot` / `mermaid` / `tree` |
| `cycles` | 检测循环依赖 | 文本 / `--json` |
| `unused` | 死代码检测 | tree / `--output json` |
| `hotspot` | 高风险节点 | tree / `--output json` |
| `path` | 依赖路径查找 | tree / `json` / `mermaid` |
| `analyze` | 全量依赖图 | tree / json / dot / mermaid |

## 常用参数

| 参数 | 适用 | 说明 |
|------|------|------|
| `--root <dir>` | `impact` | 项目根目录（默认当前目录） |
| `--function <name>` | `impact` | 函数级分析 |
| `--depth <n>` | `impact` `analyze` | 最大追踪跳数，0=不限 |
| `--lang <lang>` | 所有 | `rust` `ts` `js` `go` `python` `java` `vue` |
| `--text` | `impact` | 人类可读输出 |
| `--output <fmt>` | 多数 | `json` / `dot` / `mermaid` / `tree` |
| `--functions` | `unused` | 同时检测未调用函数 |
| `--top <n>` | `hotspot` | 前 N 个节点（默认 10） |
| `--from / --to` | `path` | 起止文件（绝对路径） |
