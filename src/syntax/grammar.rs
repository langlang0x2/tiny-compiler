use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SymbolType {
    Terminal,
    NonTerminal,
    Null,
}

#[derive(Clone, Debug)]
pub struct GrammarSymbol {
    pub name: String,
    pub symbol_type: SymbolType,
    pub terminal_id: Option<usize>,
    pub non_terminal_id: Option<usize>,
}

#[derive(Clone, Debug)]
pub struct TerminalSymbol {
    pub symbol_id: usize,
}

#[derive(Clone, Debug)]
pub struct NonTerminalSymbol {
    pub symbol_id: usize,
    pub production_ids: Vec<usize>,
    pub first_set: HashSet<usize>,
    pub follow_set: HashSet<usize>,
}

#[derive(Clone, Debug)]
pub struct Production {
    pub production_id: usize,
    pub head: usize,
    pub body: Vec<usize>,
    pub first_set: HashSet<usize>,
}

#[derive(Clone, Debug)]
pub struct Grammar {
    pub symbols: Vec<GrammarSymbol>,
    pub terminals: Vec<TerminalSymbol>,
    pub non_terminals: Vec<NonTerminalSymbol>,
    pub productions: Vec<Production>,
    pub start_non_terminal: usize,
    pub epsilon_terminal: usize,
    pub eof_terminal: usize,
    symbol_lookup: HashMap<String, usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProductionInfo {
    pub index_id: usize,
    pub head_name: String,
    pub body_size: usize,
}

impl Grammar {
    pub fn new() -> Self {
        let mut grammar = Self {
            symbols: Vec::new(),
            terminals: Vec::new(),
            non_terminals: Vec::new(),
            productions: Vec::new(),
            start_non_terminal: 0,
            epsilon_terminal: 0,
            eof_terminal: 0,
            symbol_lookup: HashMap::new(),
        };
        let epsilon = grammar.add_terminal_internal("e", SymbolType::Null);
        let eof = grammar.add_terminal("EOF");
        grammar.epsilon_terminal = epsilon;
        grammar.eof_terminal = eof;
        grammar
    }

    pub fn add_terminal(&mut self, name: &str) -> usize {
        self.add_terminal_internal(name, SymbolType::Terminal)
    }

    fn add_terminal_internal(&mut self, name: &str, symbol_type: SymbolType) -> usize {
        if let Some(&symbol_id) = self.symbol_lookup.get(name) {
            let symbol = &self.symbols[symbol_id];
            return symbol
                .terminal_id
                .unwrap_or_else(|| panic!("{name} already exists and is not a terminal"));
        }

        let symbol_id = self.symbols.len();
        let terminal_id = self.terminals.len();
        self.symbols.push(GrammarSymbol {
            name: name.to_string(),
            symbol_type,
            terminal_id: Some(terminal_id),
            non_terminal_id: None,
        });
        self.terminals.push(TerminalSymbol { symbol_id });
        self.symbol_lookup.insert(name.to_string(), symbol_id);
        terminal_id
    }

    pub fn add_non_terminal(&mut self, name: &str) -> usize {
        if let Some(&symbol_id) = self.symbol_lookup.get(name) {
            let symbol = &self.symbols[symbol_id];
            return symbol
                .non_terminal_id
                .unwrap_or_else(|| panic!("{name} already exists and is not a non-terminal"));
        }

        let symbol_id = self.symbols.len();
        let non_terminal_id = self.non_terminals.len();
        self.symbols.push(GrammarSymbol {
            name: name.to_string(),
            symbol_type: SymbolType::NonTerminal,
            terminal_id: None,
            non_terminal_id: Some(non_terminal_id),
        });
        self.non_terminals.push(NonTerminalSymbol {
            symbol_id,
            production_ids: Vec::new(),
            first_set: HashSet::new(),
            follow_set: HashSet::new(),
        });
        self.symbol_lookup.insert(name.to_string(), symbol_id);
        non_terminal_id
    }

    pub fn set_start_symbol(&mut self, name: &str) {
        self.start_non_terminal = self.non_terminal_id(name);
    }

    pub fn add_production(&mut self, head_name: &str, body_names: &[&str]) -> usize {
        let head = self.non_terminal_id(head_name);
        let body = body_names
            .iter()
            .map(|name| self.symbol_id(name))
            .collect::<Vec<_>>();
        let production_id = self.productions.len();
        self.productions.push(Production {
            production_id,
            head,
            body,
            first_set: HashSet::new(),
        });
        self.non_terminals[head].production_ids.push(production_id);
        production_id
    }

    pub fn symbol_id(&self, name: &str) -> usize {
        *self
            .symbol_lookup
            .get(name)
            .unwrap_or_else(|| panic!("unknown grammar symbol: {name}"))
    }

    pub fn has_symbol(&self, name: &str) -> bool {
        self.symbol_lookup.contains_key(name)
    }

    pub fn non_terminal_id(&self, name: &str) -> usize {
        let symbol_id = self.symbol_id(name);
        self.symbols[symbol_id]
            .non_terminal_id
            .unwrap_or_else(|| panic!("{name} is not a non-terminal"))
    }

    pub fn symbol_name(&self, symbol_id: usize) -> &str {
        &self.symbols[symbol_id].name
    }

    pub fn terminal_name(&self, terminal_id: usize) -> &str {
        self.symbol_name(self.terminals[terminal_id].symbol_id)
    }

    pub fn non_terminal_name(&self, non_terminal_id: usize) -> &str {
        self.symbol_name(self.non_terminals[non_terminal_id].symbol_id)
    }

    pub fn symbol_type(&self, symbol_id: usize) -> SymbolType {
        self.symbols[symbol_id].symbol_type
    }

    pub fn production_info_table(&self) -> Vec<ProductionInfo> {
        self.productions
            .iter()
            .map(|production| ProductionInfo {
                index_id: production.production_id,
                head_name: self.non_terminal_name(production.head).to_string(),
                body_size: production.body.len(),
            })
            .collect()
    }

    pub fn format_production(&self, production_id: usize) -> String {
        let production = &self.productions[production_id];
        let body = if production.body.is_empty() {
            "e".to_string()
        } else {
            production
                .body
                .iter()
                .map(|symbol_id| self.symbol_name(*symbol_id).to_string())
                .collect::<Vec<_>>()
                .join(" ")
        };
        format!("{} -> {}", self.non_terminal_name(production.head), body)
    }

    pub fn format_terminal_set(&self, terminals: &HashSet<usize>) -> String {
        let mut names = terminals
            .iter()
            .map(|terminal| self.terminal_name(*terminal).to_string())
            .collect::<Vec<_>>();
        names.sort();
        format!("{{{}}}", names.join(", "))
    }

    pub fn augmented(&self) -> Self {
        let mut grammar = self.clone();
        let original_start_name = grammar
            .non_terminal_name(grammar.start_non_terminal)
            .to_string();
        let augmented_name = format!("{}'", original_start_name);
        let augmented_start = grammar.add_non_terminal(&augmented_name);
        grammar.start_non_terminal = augmented_start;

        let old_productions = grammar.productions.clone();
        for non_terminal in &mut grammar.non_terminals {
            non_terminal.production_ids.clear();
            non_terminal.first_set.clear();
            non_terminal.follow_set.clear();
        }
        grammar.productions.clear();

        grammar.add_production(&augmented_name, &[&original_start_name]);
        for production in old_productions {
            let head_name = grammar.non_terminal_name(production.head).to_string();
            let body_names = production
                .body
                .iter()
                .map(|symbol_id| grammar.symbol_name(*symbol_id).to_string())
                .collect::<Vec<_>>();
            let body_refs = body_names.iter().map(String::as_str).collect::<Vec<_>>();
            grammar.add_production(&head_name, &body_refs);
        }

        grammar.compute_first_sets();
        grammar.compute_follow_sets();
        grammar
    }
}
