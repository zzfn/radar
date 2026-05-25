# Radar —— AI 改代码之前先看清楚

## 一句话

> **改代码之前，先知道会波及什么。**

---

## 问题

语义搜索、代码索引正在成为标配，但索引和当前代码之间永远有一条缝——更新延迟、覆盖不全、改了文件但缓存还在。Radar 不做索引，每次调用直接分析当前文件，结果永远和代码同步。

---

AI 改代码很快，但它不知道你的 `utils.ts` 被 47 个地方引用，也不知道你的 `parseConfig` 函数有一条你完全没注意到的调用链。

改完才发现——测试挂了，同事的模块炸了，线上出了问题。

---

## Radar 做什么

Radar 是一个本地 CLI 工具，任何 AI coding agent 都可以调用。你准备让 AI 修改某个文件或函数之前，它先跑一条命令，告诉你：

- **这次改动会影响哪些文件**，每一层距离多远
- **谁在调用这个函数**，调用链有多深
- **有没有循环依赖**，改了会不会让情况更糟

一条命令，一份报告，AI 拿着这份上下文再动手。

---

## 看个例子

```
你：resolve_relative 这个函数在哪些地方调用了

AI：[调用 radar context --function resolve_relative]

    ## Context: analyzer/file_dep.rs
    **Language:** Rust
    **File impact:** 2 file(s) affected

    ### Affected Files
    - `analyzer/mod.rs` (depth=1)
    - `main.rs` (depth=2)

    ### Function: `resolve_relative`
    **Callers:** 13
    - `resolve_entries` in `analyzer/js_ts.rs` (depth=1)
    - `parse_deps_with_root` in `analyzer/vue.rs` (depth=1)
    - `analyze_file` in `analyzer/js_ts.rs` (depth=2)
    - `analyze_file` in `analyzer/vue.rs` (depth=2)
    ...

    ### Cycles
    none
```

AI 看到这个，就知道这不是一个"随手改"的函数。它会谨慎处理，或者提醒你先确认改动范围。

---

## 核心能力

**`context`** — 修改前全量快照（首选）
一次输出：文件影响范围 + 函数调用者 + 循环检测。AI 一次调用拿到所有决策信息。

**`impact`** — 影响范围分析
反向 BFS 追踪，谁依赖了这个文件/函数，影响链有多深。

**`cycles`** — 循环依赖检测
基于 Kosaraju SCC 算法，找出模块间的环，适合 CI 门禁。

**`unused`** — 死代码检测
找出没有任何引用的文件和函数，重构前先扫一遍。

**`hotspot`** — 高风险节点
按被依赖数排序，一眼看出哪些文件牵一发动全身。

**`path`** — 依赖路径查找
回答"A 为什么会依赖 B"，找到最短依赖路径。

---

## 支持 7 种语言

Rust · TypeScript · JavaScript · Vue · Python · Go · Java

文件级分析全部支持。函数级分析得益于 Rust 生态的 tree-sitter 绑定，覆盖 Rust / TS / JS / Python / Go，解析精度和性能远超正则方案。

---

## 技术特点

- **Rust 实现，性能无妥协**：无运行时、无 JIT 预热，冷启动 < 50ms，不让 AI 等你
- **rayon 多线程并行**：文件解析全程并行，万级文件仓库秒出结果
- **单文件分发**：一个二进制，无依赖，cp 到任何机器即用
- **纯本地，零网络请求**：所有分析在本机完成，代码不出境，适合内网和保密项目
- **基于 tree-sitter AST**：语义级分析，不靠正则，不因注释里的函数名误报
- **尊重 `.gitignore`**：自动跳过 `node_modules`、构建产物、vendor 目录
- **路径别名解析**：`tsconfig.json` paths、`@/`、`go.mod` 均支持，跨模块引用不丢失

---

## 使用方式

### Skill 调用（推荐）

将 Radar 作为 skill 注册给 AI coding agent 后，AI 会自动判断时机——无需任何额外指令，说要改代码它就先跑 radar。

```
你：把 validate_token 的超时从 30s 改成 60s

AI：[自动调用 radar context --function validate_token]
    validate_token 有 3 个调用者，修改前确认一下影响范围……
```

触发时机由 skill 描述定义，包括：修改文件/函数前、询问依赖关系时、开始重构前、需要可视化依赖图时。

### CLI 直接调用

Radar 是标准 CLI，所有分析能力均可手动调用：

```bash
# 修改前全量上下文（首选）
radar context $(realpath <目标文件>) --root $(pwd)
radar context $(realpath <目标文件>) --root $(pwd) --function <函数名>

# 影响范围分析
radar impact $(realpath <目标文件>) --root $(pwd)

# 循环依赖检测
radar cycles $(pwd)

# 死代码扫描
radar unused $(pwd) --functions

# 高风险节点排行
radar hotspot $(pwd) --top 10

# 依赖路径追踪
radar path $(pwd) --from $(realpath <文件A>) --to $(realpath <文件B>)
```

### 基于 CLI 封装自己的 Skill

Radar CLI 输出标准 Markdown / JSON，可以直接作为任何 AI agent 的工具底座。欢迎基于 CLI 封装适合自己工作流的 skill，或集成进 CI/CD 流程。

> 仓库：https://git.nevint.com/wad/pdd/radar

---

## 一个比喻

雷达不替你开飞机，但它能告诉你前方有什么。

---

*基于静态分析，动态派发 / 反射调用 / 闭包回调不在覆盖范围内，结果是"已知风险"而非"全部风险"。*
