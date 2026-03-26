# Radar Roadmap

## 现状（v0.1 - 已完成）

- CLI 骨架（`analyze` / `graph` / `cycles`）
- **Rust 分析器** ✅（`use`/`mod`/`extern crate`）
- **JS/TS 分析器** ✅（ESM/CJS/动态 import/re-export）
- **Go 分析器** ✅（单行/分组 import，别名/blank/dot，`go.mod` 本地包解析）
- **Python 分析器** ✅（import/from...import，相对导入路径解析）
- **Java 分析器** ✅（普通/static/通配符 import，package 声明过滤）
- **Vue 分析器** ✅（script setup 块 import + template 组件引用）
- **`impact` 子命令** ✅（反向 BFS，JSON 输出，`--depth` 限制，AI 调用友好）
- 依赖图数据结构（petgraph）
- 输出格式：JSON / DOT / Mermaid / Tree

---

## v0.2 — 语言分析器补全 ✅

- [x] **Python 分析器**：`import`/`from...import`，相对包导入（`.`/`..`），绝对导入不解析路径
- [x] **Go 分析器**：`import "pkg"` / `import ( ... )` 分组块，别名/blank/dot 导入，读 `go.mod` 解析本地包路径
- [x] **Java 分析器**：`import com.example.Foo;`，`import static`，通配符，package 声明过滤
- [x] **Vue 分析器**：状态机提取 `<script setup>` 块 import，识别 `<template>` 中大写组件标签
- [x] **Language 枚举扩展**：新增 `Go` / `Java` / `Vue`
- [x] **`detect_language` 更新**：加入 `.go`、`.java`、`.vue` 扩展名识别

---

## v0.3 — 分析质量提升 ✅

- [x] **`--focus` 过滤**：`analyze` 子命令支持，正向 BFS 提取目标文件的出向子图
- [x] **`--depth` 限制**：`analyze --focus` 和 `impact` 均已支持
- [x] **`--include`/`--exclude`** glob 过滤（基于 `globset`，相对根目录匹配）
- [x] **路径别名解析**：TS `tsconfig.json` paths（含注释剥离）、Vue `@/` → `src/` 惯例 + tsconfig fallback
- [x] **统计摘要输出**：`analyze --summary`，输出节点数、边数、循环数、最高入/出度节点、孤立节点数

---

## 函数级分析（tree-sitter）✅

- [x] **函数定义提取**：Rust / Go / JS / TS / Python 五语言，基于 tree-sitter AST
- [x] **函数调用图构建**：同文件调用 + 跨文件最优匹配（同名唯一优先）
- [x] **`radar impact <file> --function <fn>`**：函数级影响分析，BFS 反向追踪所有调用者
- [x] **`radar functions <dir>`**：列出目录内所有函数定义（JSON，供 AI 探查）
- **边界**：不覆盖动态派发、闭包传递、反射调用

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
