mod first_follow;
mod grammar;
mod lr0;
mod slr;

#[allow(unused_imports)]
pub use grammar::{
    Grammar, GrammarSymbol, NonTerminalSymbol, Production, ProductionInfo, SymbolType,
    TerminalSymbol,
};
#[allow(unused_imports)]
pub use lr0::{
    build_lr0_dfa, exhaust_transition, get_closure, Dfa, ItemCategory, ItemSet, LR0Item,
    TransitionEdge,
};
#[allow(unused_imports)]
pub use slr::{
    build_slr_parse_table, is_slr1, ActionCategory, ActionCell, GotoCell, ParseTable,
};

pub fn arithmetic_expression_grammar() -> Grammar {
    let mut grammar = Grammar::new();
    for terminal in ["id", "+", "*", "(", ")"] {
        grammar.add_terminal(terminal, "TOKEN");
    }
    for non_terminal in ["E", "T", "F"] {
        grammar.add_non_terminal(non_terminal);
    }
    grammar.set_start_symbol("E");
    grammar.add_production("E", &["E", "+", "T"]);
    grammar.add_production("E", &["T"]);
    grammar.add_production("T", &["T", "*", "F"]);
    grammar.add_production("T", &["F"]);
    grammar.add_production("F", &["(", "E", ")"]);
    grammar.add_production("F", &["id"]);
    grammar.compute_first_sets();
    grammar.compute_follow_sets();
    grammar
}

pub fn tiny_grammar() -> Grammar {
    let mut grammar = Grammar::new();
    for terminal in [
        "if", "then", "else", "end", "repeat", "until", "read", "write", "id", "num", ":=",
        ";", "<", "=", "+", "-", "*", "/", "(", ")",
    ] {
        grammar.add_terminal(terminal, "TOKEN");
    }
    for non_terminal in [
        "program",
        "stmt_seq",
        "statement",
        "if_stmt",
        "repeat_stmt",
        "assign_stmt",
        "read_stmt",
        "write_stmt",
        "exp",
        "simple_exp",
        "term",
        "factor",
    ] {
        grammar.add_non_terminal(non_terminal);
    }

    grammar.set_start_symbol("program");
    grammar.add_production("program", &["stmt_seq"]);
    grammar.add_production("stmt_seq", &["stmt_seq", ";", "statement"]);
    grammar.add_production("stmt_seq", &["statement"]);
    grammar.add_production("statement", &["if_stmt"]);
    grammar.add_production("statement", &["repeat_stmt"]);
    grammar.add_production("statement", &["assign_stmt"]);
    grammar.add_production("statement", &["read_stmt"]);
    grammar.add_production("statement", &["write_stmt"]);
    grammar.add_production("if_stmt", &["if", "exp", "then", "stmt_seq", "end"]);
    grammar.add_production(
        "if_stmt",
        &["if", "exp", "then", "stmt_seq", "else", "stmt_seq", "end"],
    );
    grammar.add_production("repeat_stmt", &["repeat", "stmt_seq", "until", "exp"]);
    grammar.add_production("assign_stmt", &["id", ":=", "exp"]);
    grammar.add_production("read_stmt", &["read", "id"]);
    grammar.add_production("write_stmt", &["write", "exp"]);
    grammar.add_production("exp", &["exp", "<", "simple_exp"]);
    grammar.add_production("exp", &["exp", "=", "simple_exp"]);
    grammar.add_production("exp", &["simple_exp"]);
    grammar.add_production("simple_exp", &["simple_exp", "+", "term"]);
    grammar.add_production("simple_exp", &["simple_exp", "-", "term"]);
    grammar.add_production("simple_exp", &["term"]);
    grammar.add_production("term", &["term", "*", "factor"]);
    grammar.add_production("term", &["term", "/", "factor"]);
    grammar.add_production("term", &["factor"]);
    grammar.add_production("factor", &["(", "exp", ")"]);
    grammar.add_production("factor", &["num"]);
    grammar.add_production("factor", &["id"]);

    grammar.compute_first_sets();
    grammar.compute_follow_sets();
    grammar
}

pub fn run_demo() {
    for (label, grammar) in [
        ("Arithmetic Expression", arithmetic_expression_grammar()),
        ("TINY", tiny_grammar()),
    ] {
        println!("================ {} Grammar ================", label);
        print_first_and_follow(&grammar);

        let augmented = grammar.augmented();
        let dfa = build_lr0_dfa(&augmented);
        let parse_table = build_slr_parse_table(&augmented, &dfa);

        println!("LR(0) item sets: {}", dfa.item_sets.len());
        println!("LR(0) transitions: {}", dfa.edges.len());
        println!("Is SLR(1): {}", if is_slr1(&parse_table) { "YES" } else { "NO" });

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
        println!();
    }
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
    use super::*;
    use std::collections::{BTreeSet, HashSet};

    fn set_of(grammar: &Grammar, names: &[&str]) -> BTreeSet<String> {
        let expected = names
            .iter()
            .map(|name| name.to_string())
            .collect::<BTreeSet<_>>();
        let available = grammar
            .terminals
            .iter()
            .enumerate()
            .map(|(terminal_id, _)| grammar.terminal_name(terminal_id).to_string())
            .collect::<BTreeSet<_>>();
        expected.intersection(&available).cloned().collect()
    }

    fn terminal_names(grammar: &Grammar, set: &HashSet<usize>) -> BTreeSet<String> {
        set.iter()
            .map(|terminal| grammar.terminal_name(*terminal).to_string())
            .collect()
    }

    #[test]
    fn arithmetic_first_and_follow_are_correct() {
        let grammar = arithmetic_expression_grammar();
        let e = grammar.non_terminal_id("E");
        let t = grammar.non_terminal_id("T");
        let f = grammar.non_terminal_id("F");

        assert_eq!(
            terminal_names(&grammar, &grammar.non_terminals[e].first_set),
            set_of(&grammar, &["(", "id"])
        );
        assert_eq!(
            terminal_names(&grammar, &grammar.non_terminals[t].first_set),
            set_of(&grammar, &["(", "id"])
        );
        assert_eq!(
            terminal_names(&grammar, &grammar.non_terminals[f].first_set),
            set_of(&grammar, &["(", "id"])
        );
        assert_eq!(
            terminal_names(&grammar, &grammar.non_terminals[e].follow_set),
            set_of(&grammar, &[")", "EOF", "+"])
        );
        assert_eq!(
            terminal_names(&grammar, &grammar.non_terminals[t].follow_set),
            set_of(&grammar, &[")", "*", "EOF", "+"])
        );
        assert_eq!(
            terminal_names(&grammar, &grammar.non_terminals[f].follow_set),
            set_of(&grammar, &[")", "*", "EOF", "+"])
        );
    }

    #[test]
    fn arithmetic_grammar_is_slr1() {
        let grammar = arithmetic_expression_grammar().augmented();
        let dfa = build_lr0_dfa(&grammar);
        let table = build_slr_parse_table(&grammar, &dfa);

        assert!(is_slr1(&table));
        assert!(!table.action_cells.is_empty());
        assert!(!table.goto_cells.is_empty());
    }

    #[test]
    fn tiny_grammar_is_slr1() {
        let grammar = tiny_grammar().augmented();
        let dfa = build_lr0_dfa(&grammar);
        let table = build_slr_parse_table(&grammar, &dfa);

        assert!(is_slr1(&table), "conflicts: {:?}", table.conflicts);
    }
}
