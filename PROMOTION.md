# Radar —— AI 改代码之前先看清楚

## 一句话

> **改代码之前，先知道会波及什么。**

---

## 问题

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

```bash
radar context $(realpath src/auth/validator.rs) \
  --root $(pwd) \
  --function validate_token
```

输出：

```
## Context: src/auth/validator.rs
**Language:** Rust
**File impact:** 8 file(s) affected

### Affected Files
- `middleware/guard.rs` (depth=1)
- `middleware/rate_limit.rs` (depth=1)
- `api/routes.rs` (depth=2)
- `api/admin.rs` (depth=2)
...

### Function: `validate_token`
**Callers:** 3
- `check_auth` in `middleware/guard.rs` (depth=1)
- `verify_session` in `middleware/session.rs` (depth=1)
- `admin_gate` in `api/admin.rs` (depth=2)

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

文件级分析全部支持，tree-sitter 函数级分析覆盖 Rust / TS / JS / Python / Go。

---

## 技术特点

- **纯本地，零网络请求**：所有分析在本机完成，代码不出境
- **基于 tree-sitter AST**：函数级分析不靠正则，不靠猜
- **尊重 `.gitignore`**：自动跳过构建产物和第三方依赖
- **并行分析**：基于 rayon，万级文件项目也不慢
- **路径别名解析**：`tsconfig.json` paths、`@/`、`go.mod` 均支持

---

## 使用方式

### 集成到 AI coding agent（推荐）

将 radar 作为工具注册给 AI agent 后，AI 会**自动判断时机**——当你提到要修改某个文件或函数时，它主动跑 radar、读结果、再动手，整个过程你不需要输入任何命令。

```
你：把 validate_token 的超时从 30s 改成 60s

AI：[自动运行 radar context，发现 3 个调用者]
    validate_token 有 3 个调用者，修改前确认一下影响范围……
```

触发时机由工具描述定义，包括：修改文件/函数前、询问依赖关系时、开始重构前、需要可视化依赖图时。

### 直接调用

Radar 是标准 CLI，也可以手动跑：

```bash
# 修改文件前
radar context $(realpath <目标文件>) --root $(pwd)

# 修改函数前
radar context $(realpath <目标文件>) --root $(pwd) --function <函数名>
```

---

## 一个比喻

雷达不替你开飞机，但它能告诉你前方有什么。

---

*基于静态分析，动态派发 / 反射调用 / 闭包回调不在覆盖范围内，结果是"已知风险"而非"全部风险"。*
