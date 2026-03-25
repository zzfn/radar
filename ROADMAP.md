# Radar Roadmap

## 现状（v0.1 - 已完成）

- CLI 骨架（`analyze` / `graph` / `cycles`）
- **Rust 分析器** ✅（`use`/`mod`/`extern crate`）
- **JS/TS 分析器** ✅（ESM/CJS/动态 import/re-export）
- 依赖图数据结构（petgraph）
- 输出格式：JSON / DOT / Mermaid / Tree

---

## v0.2 — 语言分析器补全

- [ ] **Python 分析器**：`import`/`from...import`，相对包导入（`.foo`），标准库 vs 第三方区分
- [ ] **Go 分析器**：`import "pkg"` / `import ( "pkg1" \n "pkg2" )`，module 路径解析（读 `go.mod`）
- [ ] **Java 分析器**：`import com.example.Foo;`，`import static`，包声明
- [ ] **Vue 分析器**：解析 `<script>`/`<script setup>` 块内的 import，复用 JS/TS 分析器逻辑；识别 `<template>` 中的组件引用
- [ ] **Language 枚举扩展**：`Graph::Language` 和 `CLI::Lang` 增加 `Go` / `Java` / `Vue`
- [ ] **`detect_language` 更新**：加入 `.go`、`.java`、`.vue` 扩展名识别

---

## v0.3 — 分析质量提升

- [ ] **`--focus` 过滤**（`main.rs:88` TODO）：N 跳依赖展开
- [ ] **`--depth` 限制**接入（参数已有，逻辑未用）
- [ ] **`--include`/`--exclude`** glob 过滤接入
- [ ] **路径别名解析**：TS `tsconfig.json` paths、Go module replace、Vue `@/` 别名
- [ ] **`GraphSummary` 统计输出**：最高入/出度节点、孤立节点、循环数

---

## v0.4 — 可用性与集成

- [ ] **`watch` 子命令**：文件变更时增量更新图
- [ ] **`diff` 子命令**：对比两次分析，输出依赖变化
- [ ] **CI 模式**：`--fail-on-cycle` 非零退出
- [ ] **`.radarrc.toml` 配置文件**

---

## v1.0 — 稳定版

- [ ] 完整测试覆盖（当前仅 `rust_lang.rs` 和 `js_ts.rs` 有测试）
- [ ] 大型 monorepo 性能基准（10K+ 文件）
- [ ] README 重写 + 安装文档
- [ ] `cargo install` / Homebrew 发布
