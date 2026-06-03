# tiny-compiler

本仓库为课程实验项目（词法分析、语法分析与 TINY 编译器），本 README 主要说明如何构建、测试与运行。

## 前提
- 已安装 Rust（推荐稳定版，Rust 1.70+ 或更高）。

## 构建
在仓库根目录运行：

```bash
cargo build --release
# 或用于调试构建
cargo build
```

构建成功后可在 `target/debug/` 或 `target/release/` 下找到可执行文件。

## 运行与测试（样例）

程序通过命令行参数接收测试文件路径，根据路径自动分发到对应模块（见 `src/main.rs`）。

### 实验一/二：词法分析

传入 `tests/lexer` 下的规则文件，可选再传入源文件：

```bash
# 仅规则文件（交互匹配模式）
cargo run -- tests/lexer/test1.txt

# 规则文件 + 源文件（扫描并输出 token 序列）
cargo run -- tests/lexer/test1.txt tests/lexer/tiny.txt
```

### 实验二/三：语法分析

传入 `tests/syntax` 下的语法文件，输出 FIRST/FOLLOW 集、LR(0) 项目集族、SLR(1) 分析表：

```bash
cargo run -- tests/syntax/exp.txt
cargo run -- tests/syntax/tiny.txt
```

### 实验四：TINY 语言编译器

将 `.tny` 源程序编译为 TM 中间代码：

```bash
# 编译 sample.tny，生成 sample.tm
cargo run -- tests/lexer/sample.tny

# 查看生成的 TM 代码
cat tests/lexer/sample.tm
```

编译流程为：`.tny` 源文件 → 词法 DFA 扫描（读 `tests/lexer/tiny.txt` 规则）→ SLR(1) 语法分析（读 `tests/syntax/tiny.txt` 语法）→ 单趟 SDT 生成 TM 指令 → 输出 `.tm` 文件。

生成的 `sample.tm` 可用 TM 虚拟机运行，得到 TINY 程序的执行结果。

### 运行所有测试

```bash
cargo test
```

当前共 26 个测试（词法 16 + 语法 2 + 编译器 7 + 原有 1），全部通过。

## 常见命令

- 运行并显示回溯信息（调试用）：

```bash
RUST_BACKTRACE=1 cargo run -- tests/syntax/exp.txt
```

- 仅构建并运行可执行文件（无 cargo 包裹的开销）：

```bash
cargo build
./target/debug/regex-dfa-engine tests/syntax/exp.txt
```

## 代码位置参考

```
src/
├── main.rs              # 入口：按路径或后缀名 .tny 分发
├── compiler.rs          # 【实验四】SLR(1) 解析 + 单趟 SDT 代码生成
├── lexer/               # 【实验一/二】词法分析器构造工具
│   ├── mod.rs           #   主流程
│   ├── rule.rs          #   命名正则表达式规则解析
│   ├── regex.rs         #   正则语言解析与 NFA 构造
│   ├── nfa.rs           #   NFA 图结构与基本运算
│   ├── dfa.rs           #   NFA 转 DFA、DFA 匹配与扫描
│   └── minimize.rs      #   DFA 最小化预留
└── syntax/              # 【实验三】语法分析器构造工具
    ├── mod.rs           #   主流程
    ├── grammar.rs       #   文法数据结构、FIRST/FOLLOW
    ├── grammar_file.rs  #   文法文件解析
    ├── lr0.rs           #   LR(0) 项目集族构造
    ├── slr.rs           #   SLR(1) 分析表构造
    └── first_follow.rs  #   FIRST/FOLLOW 辅助

tests/
├── lexer/
│   ├── tiny.txt         # TINY 语言词法规则
│   ├── sample.tny       # TINY 源程序样例（阶乘）
│   └── sample.tm        # 编译生成的 TM 中间代码
└── syntax/
    └── tiny.txt         # TINY 语言语法规则

docs/
├── 实验报告1/
├── 实验报告2/
├── 实验报告3/
└── 实验报告4/
```

---

如需我帮助运行一次示例（在当前环境执行 `cargo run` 并展示输出），或者将 README 一并提交到 Git，请告诉我。
