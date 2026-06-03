mod first_follow;
pub mod grammar;
pub mod grammar_file;
mod lr0;
mod slr;

pub use grammar::{Grammar, SymbolType};
pub use lr0::{build_lr0_dfa, Dfa};
pub use slr::{build_slr_parse_table, is_slr1, ActionCategory};

pub fn run(path: &str) -> Result<(), String> {
    let grammar = grammar_file::load_grammar(path)?;
    println!("================ Grammar: {} ================", path);
    print_first_and_follow(&grammar);

    let augmented = grammar.augmented();
    let dfa = build_lr0_dfa(&augmented);
    let parse_table = build_slr_parse_table(&augmented, &dfa);

    println!("LR(0) item sets: {}", dfa.item_sets.len());
    println!("LR(0) transitions: {}", dfa.edges.len());
    println!(
        "Is SLR(1): {}",
        if is_slr1(&parse_table) { "YES" } else { "NO" }
    );

    if !parse_table.conflicts.is_empty() {
        println!("Conflicts:");
        for conflict in &parse_table.conflicts {
            println!("  {conflict}");
        }
    }

    println!("Production Info:");
    for info in augmented.production_info_table() {
        println!(
            "  [{}] {} body_size={}",
            info.index_id, info.head_name, info.body_size
        );
    }

    println!("ACTION Table:");
    for cell in &parse_table.action_cells {
        let action_text = match cell.action_type {
            ActionCategory::Shift => format!("s{}", cell.id),
            ActionCategory::Reduce => format!("r{}", cell.id),
            ActionCategory::Accept => "acc".to_string(),
        };
        println!(
            "  ACTION[{}, {}] = {}",
            cell.state_id, cell.terminal_symbol_name, action_text
        );
    }

    println!("GOTO Table:");
    for cell in &parse_table.goto_cells {
        println!(
            "  GOTO[{}, {}] = {}",
            cell.state_id, cell.non_terminal_symbol_name, cell.next_state_id
        );
    }

    Ok(())
}

fn print_first_and_follow(grammar: &Grammar) {
    println!("Productions:");
    for production in &grammar.productions {
        println!(
            "  [{}] {} FIRST={}",
            production.production_id,
            grammar.format_production(production.production_id),
            grammar.format_terminal_set(&production.first_set)
        );
    }

    println!("Non-terminal FIRST/FOLLOW:");
    for non_terminal_id in 0..grammar.non_terminals.len() {
        let non_terminal = &grammar.non_terminals[non_terminal_id];
        println!(
            "  {} FIRST={} FOLLOW={}",
            grammar.non_terminal_name(non_terminal_id),
            grammar.format_terminal_set(&non_terminal.first_set),
            grammar.format_terminal_set(&non_terminal.follow_set)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::grammar_file::parse_grammar;
    use super::*;
    use std::{fs, path::PathBuf};

    fn syntax_test_files() -> Vec<PathBuf> {
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/syntax");
        let mut files = fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", dir.display()))
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "txt"))
            .collect::<Vec<_>>();
        files.sort();
        assert!(
            !files.is_empty(),
            "no syntax test files in {}",
            dir.display()
        );
        files
    }

    #[test]
    fn syntax_test_files_can_be_parsed() {
        for path in syntax_test_files() {
            let content = fs::read_to_string(&path).unwrap();
            let grammar = parse_grammar(&content)
                .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()));

            assert!(
                !grammar.productions.is_empty(),
                "{} has no productions",
                path.display()
            );
            assert!(
                grammar.non_terminals[grammar.start_non_terminal]
                    .follow_set
                    .contains(&grammar.eof_terminal),
                "{} start symbol FOLLOW set is missing EOF",
                path.display()
            );
        }
    }

    #[test]
    fn syntax_test_files_are_slr1() {
        for path in syntax_test_files() {
            let content = fs::read_to_string(&path).unwrap();
            let grammar = parse_grammar(&content)
                .unwrap_or_else(|err| panic!("failed to parse {}: {err}", path.display()))
                .augmented();
            let dfa = build_lr0_dfa(&grammar);
            let table = build_slr_parse_table(&grammar, &dfa);

            assert!(
                is_slr1(&table),
                "{} conflicts: {:?}",
                path.display(),
                table.conflicts
            );
        }
    }
}
