---
name: radar
description: >
  代码依赖分析工具。修改文件/函数前评估影响范围（blast radius），检测循环依赖，
  定位死代码（unused files/functions），识别高风险核心节点（hotspot），
  查找两文件间依赖路径（why does A depend on B）。
  Use when: modifying files/functions, refactoring, reviewing PRs for dependency changes,
  finding dead code / unused code, understanding why two modules are coupled,
  assessing change risk before editing shared utilities.
  主动触发：用户准备修改文件或函数时、询问依赖关系或改动风险时、开始大范围重构前、
  询问某个函数被谁调用或调用了谁时、需要了解项目整体结构或模块关系时、
  需要可视化依赖图时。
license: MIT
metadata:
  author: nio-wad
  version: "0.6.0"
---

# radar

二进制：`./scripts/radar`

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
- **NEVER 把 radar 输出重定向到 /tmp 文件**：直接从 stdout 读取即可，禁止使用
  `> /tmp/xxx.json` 或 `--out-file /tmp/...`。输出过大时改用 `--depth 2` 裁剪。
- **Java Maven/Gradle 项目**：radar 会自动从传入目录向上找 `pom.xml` / `build.gradle`
  定位项目根，再扫描其下所有 `src/main/java` 作为解析候选，跨模块 import 自动覆盖。
  传任意目录（模块子目录或项目根）均可，无需手动指定每个模块的 source root。

---

## 修改前的决策框架

**优先使用 `context`**：绝大多数修改前评估，一条命令即可完成（影响范围 + 可选函数调用者 + 循环检测）。只在需要特殊分析时才降级到单一子命令。

| 修改类型 | 首选分析 | 原因 |
|---------|---------|------|
| 任意文件（通用） | `context <file>` | 一次拿到影响范围 + 循环，减少工具调用 |
| 业务逻辑函数 | `context <file> --function <fn>` | 额外追加函数级调用者，精确定位 |
| Java 文件 | `context <file>`（不加 --function） | tree-sitter 不支持 Java 函数级 |
| Vue 文件 | `context <file>`（不加 --function） | tree-sitter 不支持 Vue 函数级 |
| 删除或重命名 | 先 `unused`，再 `context` | 先确认引用状态，再评估影响 |
| 跨模块重构 | `hotspot` → `path` → `context` | 先摸清架构，再逐一评估 |

---

## 分析粒度选择

函数名的唯一性决定函数级分析的可信度：

| 情况 | 可信度 | 策略 |
|------|--------|------|
| 函数名在项目内唯一 | 高 | 函数级结果可直接使用 |
| 函数名在项目内重复 | 低 | 保守跳过，降级为文件级分析 |
| 语言不受 tree-sitter 支持（Java/Vue） | 无 | 只用文件级 |

先用 `./scripts/radar functions <dir> --output json` 确认函数名唯一性，再决定是否信任函数级结果。

---

## 执行流程

### 场景零：修改前全量上下文（首选）

绝大多数情况下，直接用 `context` 替代分别调用 `impact` + `cycles`：

```bash
# 只评估文件影响
./scripts/radar context <文件绝对路径> --root <项目根目录>

# 同时评估某个函数的调用者
./scripts/radar context <文件绝对路径> --root <项目根目录> --function <函数名>
```

输出示例（markdown 格式，默认）：
```
## Context: src/auth/validator.rs
**Language:** Rust
**File impact:** 3 file(s) affected

### Affected Files
- `middleware/guard.rs` (depth=1)
- `api/routes.rs` (depth=1)
- `tests/integration.rs` (depth=2)

### Function: `validate_token`
**Callers:** 2
- `check_auth` in `middleware/guard.rs` (depth=1)
- `handle_login` in `api/routes.rs` (depth=2)

### Cycles
none
```

决策（同 impact）：
- `File impact: none` → 低风险，直接修改
- `1-4 files affected` → 中风险，告知受影响文件后修改
- `5+ files affected` → 高风险，展示影响链，建议用户确认
- `Cycles ⚠` → 依赖链已有循环，修改可能扩大循环范围

**异常处理：**

| 情况 | 原因 | 处理 |
|------|------|------|
| 结果为空（0 节点） | 语言检测失败 | 显式传 `--lang <lang>` |
| 分析耗时极长 | 目录含 vendor/生成文件 | 改为分析 `src/` 子目录 |
| binary 找不到 | skill 未编译 | `cargo build --release && cp target/release/radar skills/radar/scripts/` |

### 场景一：修改文件前（降级方案）

仅在需要 JSON 格式或与其他工具集成时才用 `impact` 替代 `context`：

```bash
./scripts/radar impact <文件绝对路径> --root <项目根目录>
```

### 场景二：修改函数前（降级方案）

仅在需要确认函数名唯一性时才单独用 `functions` + `impact`：

```bash
# 步骤一：确认函数名存在 + 检查唯一性
./scripts/radar functions <目录> --lang <语言>

# 步骤二（已被 context --function 覆盖，通常不需要单独跑）
./scripts/radar impact <文件绝对路径> --function <函数名> --root <项目根目录>
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
./scripts/radar cycles <项目根目录> --json
```

空数组 `[]` → 安全；非空 → 列出循环路径，建议处理后再提交。

### 场景四：大范围重构前扫描

```bash
./scripts/radar hotspot <项目根目录> --top 10   # 高风险核心节点
./scripts/radar unused <项目根目录> --functions  # 死代码候选
```

解读：
- hotspot 入度 > 10 → 架构核心，修改需完整 impact 评估
- hotspot 入度 1-3  → 普通模块，正常修改
- unused 文件 out_degree > 0 → 孤立模块（依赖别人但无人依赖），候选清理
- unused 文件 out_degree == 0 → 完全孤立，可安全删除

### 场景五：追踪依赖来源

```bash
./scripts/radar path <项目根目录> --from <文件A绝对路径> --to <文件B绝对路径>
```

有路径 → 按路径逐跳解释耦合原因；无路径 → 两模块独立，可分别修改。

### 场景六：生成依赖图并在浏览器打开

将依赖图编码进 mermaid.live URL，直接在浏览器渲染，无需安装任何工具。

```bash
MERMAID=$(./scripts/radar analyze <src目录> --output mermaid)

URL=$(echo "$MERMAID" | python3 -c "
import sys, zlib, base64, json, re
raw = sys.stdin.read()
m = re.search(r'\`\`\`mermaid\n(.*?)\`\`\`', raw, re.DOTALL)
code = m.group(1).strip() if m else raw.strip()
obj = json.dumps({'code': code, 'mermaid': {'theme': 'default'}})
compressed = zlib.compress(obj.encode('utf-8'))
encoded = base64.urlsafe_b64encode(compressed).decode()
print(f'https://mermaid.live/edit#pako:{encoded}')
")

echo "依赖图：$URL"
open "$URL"
```

执行步骤：
1. 运行 `./scripts/radar analyze <src目录> --output mermaid` 获取图内容
2. Python 压缩编码成 mermaid.live URL
3. `open "$URL"` 在浏览器打开
4. 同时把 URL 输出给用户，方便分享

### 场景七：生成 JSON 依赖图供 AI 读取

在开始分析或修改前，先生成结构化依赖图帮助理解项目模块关系。

```bash
./scripts/radar analyze <src目录> --output json 2>/dev/null
```

输出为纯 JSON（诊断信息走 stderr），路径为相对路径，可直接读取：

```json
{
  "meta": { "node_count": 20, "edge_count": 19 },
  "nodes": [{ "id": 0, "path": "error.rs", "kind": "File", "language": "Rust" }],
  "edges": [{ "from": 5, "to": 6, "kind": "Import", "line": 2 }]
}
```

使用时机：
- 用户让 AI 理解项目结构、模块划分时
- 开始大范围重构前建立全局视图
- 回答"这个模块负责什么"、"哪些模块相互依赖"等架构问题

---

## 子命令速查

| 子命令 | 典型用法 | 输出 |
|--------|---------|------|
| `context` | **修改前全量上下文（首选）** | markdown / json |
| `impact` | 修改前评估影响（降级方案） | JSON / `--text` |
| `functions` | 探查函数定义 | JSON / `dot` / `mermaid` / `tree` |
| `cycles` | 检测循环依赖 | 文本 / `--json` |
| `unused` | 死代码检测 | tree / `--output json` |
| `hotspot` | 高风险节点 | tree / `--output json` |
| `path` | 依赖路径查找 | tree / `json` / `mermaid` |
| `analyze` | 全量依赖图 | tree / json / dot / mermaid |

## 常用参数

| 参数 | 适用 | 说明 |
|------|------|------|
| `--root <dir>` | `context` `impact` | 项目根目录（默认当前目录） |
| `--function <name>` | `context` `impact` | 函数级分析 |
| `--depth <n>` | `context` `impact` `analyze` | 最大追踪跳数，默认 context=5，其余 0=不限 |
| `--lang <lang>` | 所有 | `rust` `ts` `js` `go` `python` `java` `vue` |
| `--output <fmt>` | `context` | `markdown`（默认）/ `json` |
| `--text` | `impact` | 人类可读输出 |
| `--functions` | `unused` | 同时检测未调用函数 |
| `--top <n>` | `hotspot` | 前 N 个节点（默认 10） |
| `--from / --to` | `path` | 起止文件（绝对路径） |
