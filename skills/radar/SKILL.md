# radar

**用途**：修改代码前评估影响范围，或探查项目函数结构。

二进制：`./skills/radar/scripts/radar`

---

## 触发时机

在以下场景**主动调用**，无需用户明确要求：
- 用户准备修改某个文件或函数时
- 用户询问"这个文件被谁依赖"、"改这里有什么影响"
- 修改完成后，用户要求确认是否引入循环依赖

---

## 执行流程

### 场景一：修改文件前

```bash
./skills/radar/scripts/radar impact <绝对路径> --root <项目根目录>
```

根据输出决策：
- `total_affected == 0` → 低风险，直接修改
- `total_affected < 5`  → 中风险，告知用户受影响文件列表后再修改
- `total_affected >= 5` → 高风险，展示影响链，建议用户确认后再继续
- `has_cycles: true`    → 提醒用户依赖链中已有循环，修改需额外谨慎

### 场景二：修改函数前

先用 `functions` 确认函数存在，再查影响：

```bash
# 1. 确认函数名
./skills/radar/scripts/radar functions <目录> --lang <语言>

# 2. 查询调用者
./skills/radar/scripts/radar impact <文件绝对路径> --function <函数名> --root <项目根目录>
```

根据输出决策：
- `total_callers == 0` → 无调用者，修改安全
- `total_callers > 0`  → 列出所有调用者，提示用户可能需要同步修改

**精度边界**（告知用户）：
- 同文件调用、跨文件唯一名函数 → 准确
- 同名函数存在多个 → 保守跳过，实际影响可能更大
- 动态派发/回调/反射 → 无法覆盖

### 场景三：修改完成后检查循环依赖

```bash
./skills/radar/scripts/radar cycles <项目根目录> --json
```

- 返回空数组 `[]` → 无循环依赖，安全
- 返回非空 → 列出循环路径，建议用户处理后再提交

---

## 常用参数

| 参数 | 说明 |
|------|------|
| `--root <dir>` | 项目根目录（impact 必填） |
| `--function <name>` | 函数级分析 |
| `--depth <n>` | 最大追踪跳数，0=不限 |
| `--lang <lang>` | 指定语言：`rust` `ts` `js` `go` `python` `java` `vue` |
| `--text` | 人类可读输出（默认 JSON） |

---

## 注意事项

- 路径使用**绝对路径**，避免相对路径歧义
- `--root` 应为包含源码的项目根目录，不是文件所在目录
- 语言检测失败时显式传 `--lang`
