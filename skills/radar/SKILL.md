# radar — 代码依赖分析工具

## 用途

分析代码库的文件级和函数级依赖关系，用于：
- 修改代码前评估影响范围（blast radius）
- 探查项目结构和函数定义
- 检测循环依赖

支持语言：Rust · TypeScript · JavaScript · Vue · Go · Python · Java

---

## 调用方式

二进制位于项目的 `scripts/radar`，直接调用：

```bash
./skills/radar/scripts/radar --version
./skills/radar/scripts/radar impact <file> --root .
```

---

## 核心命令

### 1. `impact` — 文件级影响范围（最常用）

修改某个文件前，查询哪些文件会受影响：

```bash
# 基本用法（JSON 输出，AI 友好）
./skills/radar/scripts/radar impact <target-file> --root <project-root>

# 限制影响链深度
./skills/radar/scripts/radar impact src/auth/jwt.ts --root . --depth 3

# 人类可读格式
./skills/radar/scripts/radar impact src/auth/jwt.ts --root . --text
```

**输出结构：**
```json
{
  "target": "/abs/path/to/file",
  "affected": [
    { "path": "src/api/login.ts", "depth": 1, "via": ["src/auth/jwt.ts"] },
    { "path": "src/app.ts",       "depth": 2, "via": ["src/auth/jwt.ts", "src/api/login.ts"] }
  ],
  "total_affected": 2,
  "has_cycles": false
}
```

**使用时机：** 每次修改文件前调用，根据 `total_affected` 和 `depth` 判断改动风险。

---

### 2. `impact --function` — 函数级影响范围

修改某个具体函数前，查询所有调用它的函数：

```bash
./skills/radar/scripts/radar impact src/auth.rs --function verify_token --root .
./skills/radar/scripts/radar impact src/utils/helper.ts --function formatDate --root . --depth 5
```

**输出结构：**
```json
{
  "target_file": "/abs/path/to/file",
  "target_function": "verify_token",
  "callers": [
    { "function": "handle_request", "file": "src/handler.rs", "depth": 1, "via": [] },
    { "function": "middleware",     "file": "src/mid.rs",     "depth": 2, "via": ["handle_request"] }
  ],
  "total_callers": 2
}
```

**精度说明：**
- 同文件调用 → 准确
- 跨文件、函数名全局唯一 → 准确
- 跨文件、同名函数多个 → 保守跳过（不猜测）
- 动态派发/闭包/反射 → 不覆盖

---

### 3. `functions` — 列出所有函数定义

在修改前探查项目结构，或确认函数名是否存在：

```bash
./skills/radar/scripts/radar functions <dir>
./skills/radar/scripts/radar functions src/ --lang rust
```

**输出结构（JSON 数组）：**
```json
[
  { "name": "verify_token", "file": "src/auth.rs", "start_line": 42, "end_line": 58, "language": "Rust" },
  { "name": "handle_request", "file": "src/handler.rs", "start_line": 12, "end_line": 30, "language": "Rust" }
]
```

**使用时机：** 不确定函数名是否存在，或需要了解函数分布时。

---

### 4. `analyze` — 完整依赖分析

```bash
# 快速查看目录依赖结构
./skills/radar/scripts/radar analyze ./src

# 输出统计摘要
./skills/radar/scripts/radar analyze . --summary

# 聚焦某文件的出向依赖（最多 3 跳）
./skills/radar/scripts/radar analyze . --focus src/main.rs --depth 3

# 排除测试文件
./skills/radar/scripts/radar analyze . --exclude "**/*.test.ts" --exclude "**/__tests__/**"

# 只分析 src 目录
./skills/radar/scripts/radar analyze . --include "src/**"

# 输出 JSON 供进一步处理
./skills/radar/scripts/radar analyze . --output json
```

---

### 5. `cycles` — 循环依赖检测

```bash
./skills/radar/scripts/radar cycles .                  # 文本格式
./skills/radar/scripts/radar cycles . --json           # JSON 格式
```

---

## 语言自动检测

不指定 `--lang` 时，按文件数量投票推断主要语言。混合项目建议显式指定：

```bash
--lang ts        # TypeScript（含 .tsx）
--lang js        # JavaScript
--lang rust      # Rust
--lang go        # Go
--lang python    # Python
--lang java      # Java
--lang vue       # Vue SFC
```

---

## AI 调用建议流程

```
1. 确认要修改的目标（文件 or 函数）
2. 调用 radar impact 评估影响范围
   - total_affected == 0 → 低风险，直接修改
   - total_affected < 5  → 中风险，查看 affected 列表后修改
   - total_affected >= 5 → 高风险，逐层分析再修改
3. 修改完成后，再次调用 radar cycles 确认没有引入循环依赖
```

---

## 选项速查

| 选项 | 适用命令 | 说明 |
|------|---------|------|
| `--root <dir>` | impact | 项目根目录（默认当前目录） |
| `--function <name>` | impact | 函数级分析 |
| `--depth <n>` | impact, analyze | 最大跳数（0=不限） |
| `--lang <lang>` | 全部 | 指定语言 |
| `--focus <file>` | analyze | 聚焦文件子图 |
| `--include <glob>` | analyze | 只包含匹配文件 |
| `--exclude <glob>` | analyze | 排除匹配文件 |
| `--summary` | analyze | 输出统计摘要 |
| `--text` | impact | 人类可读输出 |
| `--output json\|dot\|mermaid\|tree` | analyze, graph | 输出格式 |
