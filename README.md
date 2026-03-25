# Radar

项目依赖关系分析工具。扫描源码目录，构建文件级依赖图，支持循环依赖检测和多种格式输出。

## 特性

- **多语言支持**：Rust、JavaScript、TypeScript、Vue、Python、Go、Java
- **循环依赖检测**：基于强连通分量算法（Kosaraju SCC）
- **多种输出格式**：终端树形、JSON、Graphviz DOT、Mermaid
- **自动语言检测**：按文件数量投票推断主要语言
- **并行分析**：基于 rayon，大型项目也能快速完成
- **尊重 `.gitignore`**：自动跳过忽略文件

## 安装

```bash
cargo install --path .
```

## 用法

### analyze — 分析依赖关系

```bash
# 自动检测语言，树形输出
radar analyze ./src

# 指定语言，输出为 JSON
radar analyze ./src --lang ts --output json

# 输出到文件
radar analyze ./src --output dot --out-file deps.dot

# 聚焦某个文件
radar analyze ./src --focus src/main.rs
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

## 支持的语言

| 语言 | 扩展名 | 状态 |
|------|--------|------|
| Rust | `.rs` | ✅ |
| JavaScript | `.js` `.jsx` `.mjs` `.cjs` | ✅ |
| TypeScript | `.ts` `.tsx` `.mts` `.cts` | ✅ |
| Vue | `.vue` | 🚧 开发中 |
| Python | `.py` | 🚧 开发中 |
| Go | `.go` | 🚧 开发中 |
| Java | `.java` | 🚧 开发中 |

## 输出格式

| 格式 | 说明 | 适用场景 |
|------|------|----------|
| `tree` | 终端彩色树形 | 快速查看 |
| `json` | 结构化 JSON | 二次处理 |
| `dot` | Graphviz DOT | 渲染矢量图 |
| `mermaid` | Mermaid 语法 | 嵌入 Markdown |

## 开发

```bash
# 构建
cargo build

# 运行测试
cargo test

# 分析自身（dogfooding）
cargo run -- analyze ./src --lang rust
```

## Roadmap

详见 [ROADMAP.md](./ROADMAP.md)。
