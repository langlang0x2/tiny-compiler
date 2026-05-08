use std::{env, fs, io, process};

mod lexer;
mod syntax;

fn main() {
    let mut args = env::args().skip(1);
    let first_arg = args.next().unwrap_or_else(|| {
        eprintln!("usage:");
        eprintln!("  cargo run -- <rule-file>");
        eprintln!("  cargo run -- syntax");
        process::exit(1);
    });

    if first_arg == "syntax" {
        syntax::run_demo();
        return;
    }

    let path = first_arg;

    let content = fs::read_to_string(&path).unwrap_or_else(|err| {
        eprintln!("failed to read file: {path} ({err})");
        process::exit(1);
    });

    let rules = lexer::parse_rules(&content).unwrap_or_else(|err| {
        eprintln!("failed to parse rules: {err}");
        process::exit(1);
    });

    let tables = lexer::build_token_regular_tables(&rules).unwrap_or_else(|err| {
        eprintln!("failed to build regular tables: {err}");
        process::exit(1);
    });
    let (mut charset_table, _) = lexer::build_charset_table(&rules).unwrap_or_else(|err| {
        eprintln!("failed to build charset table: {err}");
        process::exit(1);
    });

    println!("loaded: {}", path);
    let mut token_nfas = Vec::new();

    for table in &tables {
        println!("token {}:", table.token_name);
        for row in table.rows() {
            println!(
                "  r{} {} op1={} op2={:?}",
                row.regular_id, row.operator_symbol, row.operand_id1, row.operand_id2
            );
        }
        let nfa = table.build_nfa(&mut charset_table).unwrap_or_else(|err| {
            eprintln!("failed to build NFA for {}: {err}", table.token_name);
            process::exit(1);
        });
        println!("  nfa states={} edges={}", nfa.states.len(), nfa.edges.len());
        token_nfas.push(nfa);
    }

    let merged_nfa = lexer::merge_nfas(&token_nfas);
    let merged_dfa = lexer::nfa_to_dfa(&merged_nfa, &mut charset_table).unwrap_or_else(|err| {
        eprintln!("failed to build DFA: {err}");
        process::exit(1);
    });

    println!("merged nfa states={} edges={}", merged_nfa.states.len(), merged_nfa.edges.len());
    println!("merged dfa states={} edges={}", merged_dfa.states.len(), merged_dfa.edges.len());
    println!("input a string to match, empty line to quit:");

    let mut line = String::new();
    loop {
        line.clear();
        io::stdin().read_line(&mut line).unwrap_or_else(|err| {
            eprintln!("failed to read input: {err}");
            process::exit(1);
        });

        let input = line.trim();
        if input.is_empty() {
            break;
        }

        match lexer::dfa_match(&merged_dfa, &charset_table, input).unwrap_or_else(|err| {
            eprintln!("failed to match input: {err}");
            process::exit(1);
        }) {
            Some(token) => println!("token: {}", token),
            None => println!("token: <no match>"),
        }
    }
}
