# Radar

**AI 代码修改前的安全网。** 一条命令告诉 Claude 改这里会波及什么，有没有循环依赖，谁在调用这个函数。

支持 7 种语言，基于 tree-sitter AST 静态分析，零配置，零依赖，纯本地运行。

---

## 快速开始

```bash
# 修改某文件前，一条命令获取完整上下文
radar context $(realpath src/auth/validator.rs) --root $(pwd)

# 同时分析某个函数的调用者
radar context $(realpath src/auth/validator.rs) --root $(pwd) --function validate_token
```

输出示例：

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

---

## 安装

```bash
cargo install --path .

# 作为 Claude Code skill 使用时，binary 已内置于 skills/radar/scripts/
```

---

## 核心命令

### `context` — 修改前全量上下文（首选）

整合文件影响范围、函数调用者、循环检测，一次完成，减少 AI 工具调用次数。

```bash
# 文件级评估（绝对路径）
radar context $(realpath src/core.rs) --root $(pwd)

# 同时评估函数调用者
radar context $(realpath src/core.rs) --root $(pwd) --function parse_config

# JSON 格式（供程序解析，含 via 调用链）
radar context $(realpath src/core.rs) --root $(pwd) --output json

# 限制追踪深度（默认 5）
radar context $(realpath src/core.rs) --root $(pwd) --depth 3
```

**结果解读：**

| 输出 | 含义 | 建议 |
|------|------|------|
| `File impact: none` | 无文件依赖此目标 | 低风险，直接修改 |
| `1–4 files affected` | 少量直接依赖者 | 中风险，告知用户后修改 |
| `5+ files affected` | 广泛影响 | 高风险，逐一确认后修改 |
| `⚠ (target is in a cycle)` | 目标文件自身在循环中 | 修改可能扩大循环范围，谨慎 |
| `No callers found` | 函数无静态调用者 | 注意精度边界（见下） |

> **精度边界**：动态派发、闭包回调、接口实现、反射调用不在静态分析覆盖范围内，结果是"已知调用者"而非"全部调用者"。

---

### `impact` — 文件/函数影响范围

```bash
# 文件级影响（JSON，AI 调用默认格式）
radar impact $(realpath src/core.rs) --root $(pwd)

# 人类可读文本
radar impact $(realpath src/core.rs) --root $(pwd) --text

# 函数级影响
radar impact $(realpath src/core.rs) --root $(pwd) --function parse_config

# 限制深度
radar impact $(realpath src/core.rs) --root $(pwd) --depth 3
```

---

### `cycles` — 循环依赖检测

```bash
# 文本输出
radar cycles $(pwd)/src

# JSON 输出（适合 CI 检查）
radar cycles $(pwd)/src --json
```

---

### `unused` — 死代码检测

```bash
# 检测未被任何文件引用的文件
radar unused $(pwd)/src

# 同时检测未被调用的函数
radar unused $(pwd)/src --functions

# JSON 输出
radar unused $(pwd)/src --functions --output json
```

> 结果是"候选"，不是"确定可删"。JS 回调、Python 装饰器、Go 接口实现、Rust trait impl 均无法被静态调用图覆盖。

---

### `hotspot` — 高风险核心节点

```bash
# Top 10 被依赖最多的文件
radar hotspot $(pwd)/src

# 自定义数量
radar hotspot $(pwd)/src --top 20

# JSON 输出
radar hotspot $(pwd)/src --output json
```

---

### `path` — 依赖路径查找

回答"A 为什么依赖 B"。

```bash
radar path $(pwd)/src \
  --from $(realpath src/auth.ts) \
  --to $(realpath src/db.ts)

# JSON / Mermaid 格式
radar path $(pwd)/src \
  --from $(realpath src/auth.ts) \
  --to $(realpath src/db.ts) \
  --output mermaid
```

---

### `analyze` — 全量依赖图

```bash
# 终端树形（默认）
radar analyze $(pwd)/src

# 聚焦某文件的出向依赖子图
radar analyze $(pwd)/src --focus $(realpath src/main.rs) --depth 3

# 统计摘要
radar analyze $(pwd)/src --summary

# 生成 Mermaid 图并在浏览器打开
MERMAID=$(radar analyze $(pwd)/src --output mermaid)
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
open "$URL"
```

---

### `functions` — 函数定义与调用图

```bash
# 列出所有函数（JSON）
radar functions $(pwd)/src

# 按文件树形展示
radar functions $(pwd)/src --output tree

# 生成函数调用图（Graphviz）
radar functions $(pwd)/src --output dot --out-file fn_graph.dot

# Mermaid 格式
radar functions $(pwd)/src --output mermaid
```

---

## 支持的语言

| 语言 | 扩展名 | 文件级依赖 | 函数级分析 | 路径别名解析 |
|------|--------|:---:|:---:|------|
| Rust | `.rs` | ✅ | ✅ | — |
| TypeScript | `.ts` `.tsx` | ✅ | ✅ | `tsconfig.json` paths |
| JavaScript | `.js` `.jsx` `.mjs` | ✅ | ✅ | — |
| Vue | `.vue` | ✅ | — | `@/` 别名 |
| Python | `.py` | ✅ | ✅ | 相对包导入 |
| Go | `.go` | ✅ | ✅ | `go.mod` 模块路径 |
| Java | `.java` | ✅ | — | Maven/Gradle 多模块自动发现 |

**Java 说明**：传入任意目录（模块子目录或项目根均可），radar 自动向上找 `pom.xml` / `build.gradle` 定位项目根，跨模块 import 自动覆盖。

---

## 常用参数

| 参数 | 适用命令 | 说明 |
|------|---------|------|
| `--root <dir>` | `context` `impact` | 项目根目录（默认当前目录） |
| `--function <name>` | `context` `impact` | 函数级分析 |
| `--depth <n>` | `context` `impact` `analyze` | 追踪深度，`context` 默认 5，其余默认不限 |
| `--lang <lang>` | 所有 | `rust` `ts` `js` `go` `python` `java` `vue` |
| `--output <fmt>` | `context` | `markdown`（默认）/ `json` |
| `--include / --exclude` | `analyze` | Glob 过滤，如 `"**/*.test.ts"` |
| `--text` | `impact` | 人类可读输出（替代 JSON） |
| `--functions` | `unused` | 同时检测未调用函数 |
| `--top <n>` | `hotspot` | 前 N 个节点，默认 10 |
| `--from / --to` | `path` | 起止文件（绝对路径） |
| `--summary` | `analyze` | 输出统计摘要 |

---

## 使用场景速查

| 场景 | 推荐命令 |
|------|---------|
| 修改任意文件前 | `context <file>` |
| 修改某个函数前 | `context <file> --function <fn>` |
| Java / Vue 文件 | `context <file>`（不加 `--function`） |
| 删除或重命名文件 | `unused` → `context` |
| 大范围重构前 | `hotspot` → `path` → `context` |
| CI 循环检测 | `cycles --json` |
| 理解模块依赖关系 | `analyze --summary` |
| 可视化依赖图 | `analyze --output mermaid` + mermaid.live |

---

## 已知限制

- **动态派发不覆盖**：虚函数、接口实现、反射调用、事件回调均无法被静态调用图捕获
- **函数级分析精度**：同名函数跨文件时保守跳过，实际影响可能更大
- **Java/Vue 无函数级分析**：tree-sitter 当前不支持，只能用文件级
- **生成文件噪声**：`node_modules/`、`vendor/`、`dist/` 等目录建议用 `--exclude` 过滤

---

## 开发

```bash
cargo build
cargo test
cargo build --release && cp target/release/radar skills/radar/scripts/
```

Roadmap 详见 [ROADMAP.md](./ROADMAP.md)。
