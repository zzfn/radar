# Radar

多语言代码依赖分析工具。扫描源码目录，构建文件级与函数级依赖图，支持循环依赖检测、影响范围追踪、死代码检测等分析能力。

## 特性

- **多语言支持**：Rust、JavaScript、TypeScript、Vue、Python、Go、Java（7 种）
- **循环依赖检测**：基于强连通分量算法（Kosaraju SCC）
- **影响范围分析**：反向 BFS 追踪，支持文件级和函数级
- **函数级分析**：基于 tree-sitter AST，提取函数定义与调用图，支持 DOT/Mermaid 可视化
- **死代码检测**：找出未被引用的文件和未被调用的函数
- **高风险节点**：按被依赖数排序，定位改动影响最大的文件
- **依赖路径查找**：BFS 找两文件间最短依赖链，回答"A 为什么依赖 B"
- **路径别名解析**：TS `tsconfig.json` paths、Vue `@/`、Go `go.mod`
- **多种输出格式**：终端树形、JSON、Graphviz DOT、Mermaid
- **Glob 过滤**：`--include`/`--exclude` 精确控制分析范围
- **自动语言检测**：按文件数量投票推断主要语言
- **并行分析**：基于 rayon，大型项目也能快速完成
- **尊重 `.gitignore`**：自动跳过忽略文件

## 安装

```bash
cargo install --path .
```

## 子命令一览

| 子命令 | 用途 |
|--------|------|
| `analyze` | 分析目录依赖关系，支持聚焦、过滤、统计摘要 |
| `graph` | 生成完整依赖图（DOT/Mermaid） |
| `cycles` | 检测循环依赖 |
| `impact` | 修改某文件/函数后的影响范围（反向 BFS） |
| `unused` | 死代码检测：未引用文件 + 未调用函数 |
| `hotspot` | 高风险节点：按被依赖数降序列出 |
| `path` | 两文件间最短依赖路径查找 |
| `functions` | 列出所有函数定义（JSON/DOT/Mermaid/Tree） |

## 用法

### analyze — 分析依赖关系

```bash
# 自动检测语言，树形输出
radar analyze ./src

# 指定语言，输出为 JSON
radar analyze ./src --lang ts --output json

# 聚焦某个文件（正向 BFS 子图，限制深度）
radar analyze ./src --focus src/main.rs --depth 3

# Glob 过滤
radar analyze ./src --include "**/*.ts" --exclude "**/*.test.ts"

# 输出统计摘要
radar analyze ./src --summary

# 输出到文件
radar analyze ./src --output dot --out-file deps.dot
```

### graph — 生成完整依赖图

```bash
# 默认输出 DOT 格式
radar graph ./src

# 输出 Mermaid，写入文件
radar graph ./src --output mermaid --out-file deps.md
```

### cycles — 检测循环依赖

```bash
# 文本格式输出
radar cycles ./src

# JSON 格式输出
radar cycles ./src --json
```

### impact — 影响范围分析

```bash
# 分析修改某文件会影响哪些文件（反向 BFS）
radar impact src/core.rs --root ./src

# 限制传播深度
radar impact src/core.rs --root ./src --depth 3

# 文件级纯文本输出（适合 shell 管道）
radar impact src/core.rs --root ./src --text

# 函数级影响分析（JSON）
radar impact src/core.rs --root ./src --function parse_config

# 函数级影响分析（人类可读）
radar impact src/core.rs --root ./src --function parse_config --text
```

### unused — 死代码检测

```bash
# 检测未被任何文件引用的文件（自动跳过入口文件）
radar unused ./src

# 同时检测未被调用的函数
radar unused ./src --functions

# 包含入口文件（main.rs、index.ts 等）
radar unused ./src --include-entry

# JSON 输出
radar unused ./src --functions --output json
```

### hotspot — 高风险节点

```bash
# 列出被最多文件依赖的 Top 10 节点
radar hotspot ./src

# 自定义数量
radar hotspot ./src --top 20

# JSON 输出
radar hotspot ./src --output json
```

### path — 依赖路径查找

```bash
# 查找两个文件之间的最短依赖路径
radar path ./src --from src/auth.ts --to src/db.ts

# JSON 输出
radar path ./src --from src/auth.ts --to src/db.ts --output json

# Mermaid 格式
radar path ./src --from src/auth.ts --to src/db.ts --output mermaid
```

### functions — 列出函数定义

```bash
# 列出目录内所有函数（默认 JSON）
radar functions ./src

# 树形展示（按文件分组）
radar functions ./src --output tree

# 生成函数调用图（DOT 格式，可用 Graphviz 渲染）
radar functions ./src --output dot --out-file fn_graph.dot

# Mermaid 格式，嵌入 Markdown
radar functions ./src --output mermaid --out-file fn_graph.md
```

## 支持的语言

| 语言 | 扩展名 | 文件级分析 | 函数级分析 |
|------|--------|-----------|-----------|
| Rust | `.rs` | ✅ | ✅ |
| JavaScript | `.js` `.jsx` `.mjs` `.cjs` | ✅ | ✅ |
| TypeScript | `.ts` `.tsx` `.mts` `.cts` | ✅ | ✅ |
| Vue | `.vue` | ✅ | — |
| Python | `.py` | ✅ | ✅ |
| Go | `.go` | ✅ | ✅ |
| Java | `.java` | ✅ | — |

> 函数级分析基于 tree-sitter AST，不覆盖动态派发、闭包回调、反射调用。

## 输出格式

| 格式 | 说明 | 适用场景 |
|------|------|----------|
| `tree` | 终端彩色树形（默认） | 快速查看 |
| `json` | 结构化 JSON | 工具链集成、AI 调用 |
| `dot` | Graphviz DOT 格式 | 渲染矢量图（`dot -Tsvg`） |
| `mermaid` | Mermaid 流程图语法 | 嵌入 Markdown |

## 开发

```bash
# 构建
cargo build

# 运行测试
cargo test

# 分析自身（dogfooding）
radar analyze ./src --lang rust
radar unused ./src --lang rust --functions
radar hotspot ./src --lang rust
```

## Roadmap

详见 [ROADMAP.md](./ROADMAP.md)。
