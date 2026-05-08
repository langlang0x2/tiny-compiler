pub mod dfa;
pub mod minimize;
pub mod nfa;
pub mod regex;
pub mod rule;

use std::{fs, io};

pub use dfa::{dfa_match, nfa_to_dfa};
pub use nfa::merge_nfas;
pub use regex::{build_charset_table, build_token_regular_tables};
pub use rule::parse_rules;

pub fn run(path: &str) -> Result<(), String> {
    let content =
        fs::read_to_string(path).map_err(|err| format!("failed to read file: {path} ({err})"))?;
    let rules = parse_rules(&content).map_err(|err| format!("failed to parse rules: {err}"))?;
    let tables = build_token_regular_tables(&rules)
        .map_err(|err| format!("failed to build regular tables: {err}"))?;
    let (mut charset_table, _) = build_charset_table(&rules)
        .map_err(|err| format!("failed to build charset table: {err}"))?;

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

        let nfa = table
            .build_nfa(&mut charset_table)
            .map_err(|err| format!("failed to build NFA for {}: {err}", table.token_name))?;
        println!(
            "  nfa states={} edges={}",
            nfa.states.len(),
            nfa.edges.len()
        );
        token_nfas.push(nfa);
    }

    let merged_nfa = merge_nfas(&token_nfas);
    let merged_dfa = nfa_to_dfa(&merged_nfa, &mut charset_table)
        .map_err(|err| format!("failed to build DFA: {err}"))?;

    println!(
        "merged nfa states={} edges={}",
        merged_nfa.states.len(),
        merged_nfa.edges.len()
    );
    println!(
        "merged dfa states={} edges={}",
        merged_dfa.states.len(),
        merged_dfa.edges.len()
    );
    println!("input a string to match, empty line to quit:");

    let mut line = String::new();
    loop {
        line.clear();
        io::stdin()
            .read_line(&mut line)
            .map_err(|err| format!("failed to read input: {err}"))?;

        let input = line.trim();
        if input.is_empty() {
            break;
        }

        match dfa_match(&merged_dfa, &charset_table, input)
            .map_err(|err| format!("failed to match input: {err}"))?
        {
            Some(token) => println!("token: {}", token),
            None => println!("token: <no match>"),
        }
    }

    Ok(())
}
