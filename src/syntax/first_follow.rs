use std::collections::HashSet;

use super::{Grammar, SymbolType};

impl Grammar {
    pub fn compute_first_sets(&mut self) {
        for production in &mut self.productions {
            production.first_set.clear();
        }
        for non_terminal in &mut self.non_terminals {
            non_terminal.first_set.clear();
        }

        loop {
            let snapshot = self
                .non_terminals
                .iter()
                .map(|nt| nt.first_set.clone())
                .collect::<Vec<_>>();
            let mut changed = false;

            for production_id in 0..self.productions.len() {
                let first_set = self.first_of_sequence_with_snapshot(
                    &self.productions[production_id].body,
                    &snapshot,
                );
                if extend_set(&mut self.productions[production_id].first_set, &first_set) {
                    changed = true;
                }
                let head = self.productions[production_id].head;
                if extend_set(&mut self.non_terminals[head].first_set, &first_set) {
                    changed = true;
                }
            }

            if !changed {
                break;
            }
        }
    }

    pub fn compute_follow_sets(&mut self) {
        for non_terminal in &mut self.non_terminals {
            non_terminal.follow_set.clear();
        }
        self.non_terminals[self.start_non_terminal]
            .follow_set
            .insert(self.eof_terminal);

        let snapshot = self
            .non_terminals
            .iter()
            .map(|nt| nt.first_set.clone())
            .collect::<Vec<_>>();
        let mut follow_dependencies = vec![HashSet::new(); self.non_terminals.len()];

        for production in &self.productions {
            let body = &production.body;

            for index in 0..body.len() {
                let current_symbol = body[index];
                if self.symbol_type(current_symbol) != SymbolType::NonTerminal {
                    continue;
                }

                let current_non_terminal = self.symbols[current_symbol].non_terminal_id.unwrap();
                let suffix = &body[index + 1..];
                let suffix_first = self.first_of_sequence_with_snapshot(suffix, &snapshot);

                for terminal in &suffix_first {
                    if *terminal != self.epsilon_terminal {
                        self.non_terminals[current_non_terminal]
                            .follow_set
                            .insert(*terminal);
                    }
                }

                if suffix.is_empty() || suffix_first.contains(&self.epsilon_terminal) {
                    follow_dependencies[current_non_terminal].insert(production.head);
                }
            }
        }

        loop {
            let follow_snapshot = self
                .non_terminals
                .iter()
                .map(|nt| nt.follow_set.clone())
                .collect::<Vec<_>>();
            let mut changed = false;

            for (non_terminal_id, dependencies) in follow_dependencies.iter().enumerate() {
                for dependency in dependencies {
                    if extend_set(
                        &mut self.non_terminals[non_terminal_id].follow_set,
                        &follow_snapshot[*dependency],
                    ) {
                        changed = true;
                    }
                }
            }

            if !changed {
                break;
            }
        }
    }

    pub(crate) fn first_of_sequence_with_snapshot(
        &self,
        sequence: &[usize],
        first_snapshot: &[HashSet<usize>],
    ) -> HashSet<usize> {
        let mut result = HashSet::new();
        if sequence.is_empty() {
            result.insert(self.epsilon_terminal);
            return result;
        }

        let mut all_nullable = true;
        for symbol_id in sequence {
            match self.symbol_type(*symbol_id) {
                SymbolType::Terminal => {
                    let terminal_id = self.symbols[*symbol_id].terminal_id.unwrap();
                    if terminal_id != self.epsilon_terminal {
                        result.insert(terminal_id);
                        all_nullable = false;
                        break;
                    }
                }
                SymbolType::Null => {}
                SymbolType::NonTerminal => {
                    let non_terminal_id = self.symbols[*symbol_id].non_terminal_id.unwrap();
                    let first_set = &first_snapshot[non_terminal_id];
                    for terminal in first_set {
                        if *terminal != self.epsilon_terminal {
                            result.insert(*terminal);
                        }
                    }
                    if !first_set.contains(&self.epsilon_terminal) {
                        all_nullable = false;
                        break;
                    }
                }
            }
        }

        if all_nullable {
            result.insert(self.epsilon_terminal);
        }
        result
    }
}

fn extend_set(target: &mut HashSet<usize>, source: &HashSet<usize>) -> bool {
    let before = target.len();
    target.extend(source.iter().copied());
    target.len() != before
}
