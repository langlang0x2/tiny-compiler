use std::{env, path::Path, process};

mod lexer;
mod syntax;

fn main() {
    // 读取测试文件路径：tests/syntax 下走语法分析，tests/lexer 下走词法分析。
    let mut args = env::args().skip(1);
    let path = args.next().unwrap_or_else(|| {
        eprintln!("usage:");
        eprintln!("  cargo run -- tests/lexer/<rule-file>.txt");
        eprintln!("  cargo run -- tests/syntax/<grammar-file>.txt");
        process::exit(1);
    });

    // 主函数只负责分发，具体流程封装在 lexer / syntax 模块里。
    let result = if is_under(&path, "tests", "syntax") {
        syntax::run(&path)
    } else if is_under(&path, "tests", "lexer") {
        lexer::run(&path)
    } else {
        Err(format!(
            "unknown input path: {path}\nexpected tests/lexer/*.txt or tests/syntax/*.txt"
        ))
    };

    if let Err(err) = result {
        eprintln!("{err}");
        process::exit(1);
    }
}

fn is_under(path: &str, first: &str, second: &str) -> bool {
    let mut components = Path::new(path)
        .components()
        .filter_map(|component| component.as_os_str().to_str());
    matches!(
        (components.next(), components.next()),
        (Some(a), Some(b)) if a == first && b == second
    )
}
