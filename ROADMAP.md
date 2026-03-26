# Radar Roadmap

## v0.1 — CLI 骨架与基础分析 ✅

- CLI 子命令框架（`analyze` / `graph` / `cycles`）
- 依赖图数据结构（petgraph）
- **Rust 分析器**：`use`/`mod`/`extern crate`
- **JS/TS 分析器**：ESM/CJS/动态 import/re-export
- 输出格式：JSON / DOT / Mermaid / Tree

---

## v0.2 — 多语言支持 ✅

- [x] **Python 分析器**：`import`/`from...import`，相对包导入（`.`/`..`）
- [x] **Go 分析器**：`import` 块、别名/blank/dot 导入，`go.mod` 本地包路径解析
- [x] **Java 分析器**：`import`/`import static`/通配符，package 声明过滤
- [x] **Vue 分析器**：`<script setup>` 块 import + `<template>` 大写组件识别
- [x] **`impact` 子命令**：反向 BFS，`--depth` 限制，JSON 输出

---

## v0.3 — 分析质量与函数级分析 ✅

**文件级增强**
- [x] **`--focus` 过滤**：正向 BFS 提取目标文件出向子图
- [x] **`--depth` 限制**：`analyze --focus` 和 `impact` 均已支持
- [x] **`--include`/`--exclude`** glob 过滤（基于 `globset`）
- [x] **路径别名解析**：TS `tsconfig.json` paths（含注释剥离）、Vue `@/` 别名、Go `go.mod` 模块路径
- [x] **`analyze --summary`**：输出节点数、边数、循环数、最高入/出度节点、孤立节点数

**函数级分析（tree-sitter）**
- [x] **函数定义提取**：Rust / Go / JS / TS / Python，基于 tree-sitter AST
- [x] **函数调用图构建**：同文件精确匹配 + 跨文件唯一名优先
- [x] **`radar impact --function <fn>`**：函数级影响分析，BFS 反向追踪所有调用者
- [x] **`radar functions <dir>`**：列出所有函数定义（JSON/DOT/Mermaid/Tree）
- **边界**：不覆盖动态派发、闭包传递、反射调用

---

## v0.4 — 分析能力扩展 ✅

- [x] **`unused` 子命令**：检测未被引用的文件（in-degree == 0）和未被调用的函数；支持 `--functions`、`--include-entry`；自动跳过 main/test/bench 等
- [x] **`hotspot` 子命令**：按 in-degree 降序列出高风险核心节点；支持 `--top N`、JSON 输出
- [x] **`path` 子命令**：BFS 查找两文件间最短依赖路径；支持 JSON/Mermaid/Tree 输出；无路径时明确提示
- [x] **`impact --text`**：函数级影响分析新增人类可读输出
- [x] **`functions --output`**：函数调用图支持 DOT/Mermaid/Tree 可视化输出

---

## v0.5 — 可用性与集成

- [ ] **`diff` 子命令**：对比两次分析，输出新增/删除的依赖边（适合 PR review）
- [ ] **CI 模式**：`cycles --fail-on-cycle` 检测到循环时非零退出
- [ ] **`report` 子命令**：一键生成 Markdown 全量报告（摘要 + 循环 + hotspot + unused）
- [ ] **`.radarrc.toml` 配置文件**：固化 `--lang`/`--include`/`--exclude` 等项目级默认参数

---

## v1.0 — 稳定版

- [ ] 完整测试覆盖（当前仅 `rust_lang.rs`、`js_ts.rs`、`fn_analyzer.rs` 等有单元测试）
- [ ] 大型 monorepo 性能基准（10K+ 文件）
- [ ] `cargo install` / Homebrew 发布
