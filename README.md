# tiny-compiler

本仓库为课程实验项目（词法与语法分析练习），本 README 主要说明如何构建与测试代码。

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
程序通过命令行参数接收测试文件路径：

- 语法分析：传入 `tests/syntax` 下的语法文件
  ```bash
  cargo run -- tests/syntax/exp.txt
  ```
  示例文件： [tests/syntax/exp.txt](tests/syntax/exp.txt)

- 词法分析：传入 `tests/lexer` 下的规则文件（可选地再传入源文件）
  ```bash
  # 仅规则文件
  cargo run -- tests/lexer/test1.txt

  # 规则文件 + 源文件（.tny 或其他样例）
  cargo run -- tests/lexer/test1.txt tests/lexer/tiny.txt
  ```
  示例文件： [tests/lexer/test1.txt](tests/lexer/test1.txt)、[tests/lexer/tiny.txt](tests/lexer/tiny.txt)

程序会根据传入路径自动分发到 `lexer` 或 `syntax` 模块（见 `src/main.rs`）。

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

## 验证通过的标志
- 程序正常退出（退出码 0）且在终端输出解析或分析结果。
- 若发生错误，程序会输出错误信息并以非零退出码结束。

## 代码位置参考
- 可执行入口：`src/main.rs`
- 词法模块：`src/lexer/`
- 语法模块：`src/syntax/`
- 实验报告：`docs/实验报告3/`

---
如需我帮助运行一次示例（在当前环境执行 `cargo run` 并展示输出），或者将 README 一并提交到 Git，请告诉我。 
