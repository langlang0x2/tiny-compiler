#![allow(dead_code)]

use std::collections::HashMap;

use super::{
    nfa::{
        CHARSET_ID_BASE, CharSetTable, DriverType, Graph, closure_nfa, generate_basic_nfa,
        merge_nfas, plus_closure_nfa, product_nfa, union_nfa, zero_or_one_nfa,
    },
    rule::TokenRule,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperandType {
    Char,
    Charset,
    Regular,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LexemeCategory {
    Keyword,
    Identifier,
    Number,
    Operator,
    Delimiter,
    Unknown,
}

#[derive(Debug, Clone)]
pub struct RegularExpression {
    pub regular_id: usize,
    pub name: String,
    pub operator_symbol: char,
    pub operand_id1: usize,
    pub operand_id2: Option<usize>,
    pub type1: OperandType,
    pub type2: Option<OperandType>,
    pub result_type: OperandType,
    pub category: Option<LexemeCategory>,
    pub nfa: Option<Graph>,
}

impl RegularExpression {
    pub fn new_binary(
        regular_id: usize,
        name: impl Into<String>,
        operator_symbol: char,
        operand_id1: usize,
        operand_id2: usize,
        type1: OperandType,
        type2: OperandType,
        result_type: OperandType,
        category: Option<LexemeCategory>,
    ) -> Self {
        Self {
            regular_id,
            name: name.into(),
            operator_symbol,
            operand_id1,
            operand_id2: Some(operand_id2),
            type1,
            type2: Some(type2),
            result_type,
            category,
            nfa: None,
        }
    }

    pub fn new_unary(
        regular_id: usize,
        name: impl Into<String>,
        operator_symbol: char,
        operand_id1: usize,
        type1: OperandType,
        result_type: OperandType,
        category: Option<LexemeCategory>,
    ) -> Self {
        Self {
            regular_id,
            name: name.into(),
            operator_symbol,
            operand_id1,
            operand_id2: None,
            type1,
            type2: None,
            result_type,
            category,
            nfa: None,
        }
    }

    pub fn with_nfa(mut self, nfa: Graph) -> Self {
        self.nfa = Some(nfa);
        self
    }
}

#[derive(Debug, Clone, Default)]
pub struct RegularTable {
    pub token_name: String,
    pub category: Option<LexemeCategory>,
    rows: Vec<RegularExpression>,
    next_id: usize,
}

pub struct LexerProgram {
    pub charset_table: CharSetTable,
    pub nfa: Graph,
}

#[derive(Clone)]
enum SemanticValue {
    Char(char),
    Charset(usize),
    Nfa(Graph),
}

impl SemanticValue {
    fn into_nfa(self) -> Graph {
        match self {
            SemanticValue::Char(ch) => generate_basic_nfa(DriverType::Char, ch as usize),
            SemanticValue::Charset(id) => generate_basic_nfa(DriverType::Charset, id),
            SemanticValue::Nfa(graph) => graph,
        }
    }
}

pub fn build_lexer_program(rules: &[TokenRule]) -> Result<LexerProgram, String> {
    let mut charset_table = CharSetTable::new();
    let mut symbols = HashMap::new();
    let mut token_nfas = Vec::new();

    for rule in rules {
        let tokens = tokenize_regex(&rule.regex)?;
        let mut parser = RegexLanguageParser::new(&tokens, &symbols, &mut charset_table);
        let value = parser.parse_expression()?;
        parser.expect_end()?;

        if is_token_name(&rule.name) {
            let mut nfa = value.into_nfa();
            nfa.mark_accepting(Some(rule.name.clone()));
            symbols.insert(rule.name.clone(), SemanticValue::Nfa(nfa.clone()));
            token_nfas.push(nfa);
        } else {
            symbols.insert(rule.name.clone(), value);
        }
    }

    if token_nfas.is_empty() {
        return Err("no token rules found; token names should be uppercase".to_string());
    }

    Ok(LexerProgram {
        charset_table,
        nfa: merge_nfas(&token_nfas),
    })
}

struct RegexLanguageParser<'a> {
    tokens: &'a [RegexToken],
    position: usize,
    symbols: &'a HashMap<String, SemanticValue>,
    charset_table: &'a mut CharSetTable,
}

impl<'a> RegexLanguageParser<'a> {
    fn new(
        tokens: &'a [RegexToken],
        symbols: &'a HashMap<String, SemanticValue>,
        charset_table: &'a mut CharSetTable,
    ) -> Self {
        Self {
            tokens,
            position: 0,
            symbols,
            charset_table,
        }
    }

    fn parse_expression(&mut self) -> Result<SemanticValue, String> {
        self.parse_union()
    }

    fn expect_end(&self) -> Result<(), String> {
        if self.position == self.tokens.len() {
            Ok(())
        } else {
            Err(format!(
                "unexpected token after expression: {:?}",
                self.tokens[self.position]
            ))
        }
    }

    fn parse_union(&mut self) -> Result<SemanticValue, String> {
        let mut left = self.parse_concat()?;

        while self.consume_operator('|') {
            let right = self.parse_concat()?;
            left = self.apply_union(left, right)?;
        }

        Ok(left)
    }

    fn parse_concat(&mut self) -> Result<SemanticValue, String> {
        let mut left = self.parse_charset_binary()?;

        loop {
            let explicit_concat = self.consume_operator('.');
            if !explicit_concat && !self.peek_starts_primary() {
                break;
            }
            let right = self.parse_charset_binary()?;
            left = SemanticValue::Nfa(product_nfa(&left.into_nfa(), &right.into_nfa()));
        }

        Ok(left)
    }

    fn parse_charset_binary(&mut self) -> Result<SemanticValue, String> {
        let mut left = self.parse_postfix()?;

        loop {
            if self.consume_operator('~') {
                let right = self.parse_postfix()?;
                left = self.apply_range(left, right)?;
            } else if self.consume_operator('-') {
                let right = self.parse_postfix()?;
                left = self.apply_difference(left, right)?;
            } else {
                break;
            }
        }

        Ok(left)
    }

    fn parse_postfix(&mut self) -> Result<SemanticValue, String> {
        let mut value = self.parse_primary()?;

        loop {
            if self.consume_operator('*') {
                value = SemanticValue::Nfa(closure_nfa(&value.into_nfa()));
            } else if self.consume_operator('+') {
                value = SemanticValue::Nfa(plus_closure_nfa(&value.into_nfa()));
            } else if self.consume_operator('?') {
                value = SemanticValue::Nfa(zero_or_one_nfa(&value.into_nfa()));
            } else {
                break;
            }
        }

        Ok(value)
    }

    fn parse_primary(&mut self) -> Result<SemanticValue, String> {
        let Some(token) = self.tokens.get(self.position) else {
            return Err("missing expression operand".to_string());
        };

        self.position += 1;
        match token {
            RegexToken::Char(ch) => Ok(SemanticValue::Char(*ch)),
            RegexToken::Name(name) => self
                .symbols
                .get(name)
                .cloned()
                .ok_or_else(|| format!("unknown or forward reference: {name}")),
            RegexToken::LParen => {
                let value = self.parse_expression()?;
                match self.tokens.get(self.position) {
                    Some(RegexToken::RParen) => {
                        self.position += 1;
                        Ok(value)
                    }
                    _ => Err("mismatched '('".to_string()),
                }
            }
            RegexToken::RParen => Err("unexpected ')'".to_string()),
            RegexToken::Operator(op) => Err(format!("operator '{op}' is missing a left operand")),
        }
    }

    fn apply_union(
        &mut self,
        left: SemanticValue,
        right: SemanticValue,
    ) -> Result<SemanticValue, String> {
        match (left, right) {
            (SemanticValue::Char(c1), SemanticValue::Char(c2)) => Ok(SemanticValue::Charset(
                self.charset_table.union_chars(c1, c2),
            )),
            (SemanticValue::Charset(id), SemanticValue::Char(c))
            | (SemanticValue::Char(c), SemanticValue::Charset(id)) => Ok(SemanticValue::Charset(
                self.charset_table.union_charset_char(id, c)?,
            )),
            (SemanticValue::Charset(left_id), SemanticValue::Charset(right_id)) => Ok(
                SemanticValue::Charset(self.charset_table.union_charsets(left_id, right_id)?),
            ),
            (left, right) => Ok(SemanticValue::Nfa(union_nfa(
                &left.into_nfa(),
                &right.into_nfa(),
            ))),
        }
    }

    fn apply_range(
        &mut self,
        left: SemanticValue,
        right: SemanticValue,
    ) -> Result<SemanticValue, String> {
        match (left, right) {
            (SemanticValue::Char(from), SemanticValue::Char(to)) => {
                Ok(SemanticValue::Charset(self.charset_table.range(from, to)?))
            }
            _ => Err("range '~' only supports cc ~ cc".to_string()),
        }
    }

    fn apply_difference(
        &mut self,
        left: SemanticValue,
        right: SemanticValue,
    ) -> Result<SemanticValue, String> {
        match (left, right) {
            (SemanticValue::Charset(id), SemanticValue::Char(ch)) => Ok(SemanticValue::Charset(
                self.charset_table.difference_charset_char(id, ch)?,
            )),
            _ => Err("difference '-' only supports charset - cc".to_string()),
        }
    }

    fn consume_operator(&mut self, expected: char) -> bool {
        if matches!(
            self.tokens.get(self.position),
            Some(RegexToken::Operator(op)) if *op == expected
        ) {
            self.position += 1;
            true
        } else {
            false
        }
    }

    fn peek_starts_primary(&self) -> bool {
        matches!(
            self.tokens.get(self.position),
            Some(RegexToken::Char(_) | RegexToken::Name(_) | RegexToken::LParen)
        )
    }
}

impl RegularTable {
    pub fn new(token_name: impl Into<String>, category: Option<LexemeCategory>) -> Self {
        Self {
            token_name: token_name.into(),
            category,
            rows: Vec::new(),
            next_id: 1,
        }
    }

    pub fn rows(&self) -> &[RegularExpression] {
        &self.rows
    }

    pub fn push(&mut self, expr: RegularExpression) -> usize {
        let id = expr.regular_id;
        self.rows.push(expr);
        self.next_id = self.next_id.max(id + 1);
        id
    }

    pub fn add_binary(
        &mut self,
        name: impl Into<String>,
        operator_symbol: char,
        operand_id1: usize,
        operand_id2: usize,
        type1: OperandType,
        type2: OperandType,
        result_type: OperandType,
        category: Option<LexemeCategory>,
    ) -> usize {
        let id = self.next_id;
        let expr = RegularExpression::new_binary(
            id,
            name,
            operator_symbol,
            operand_id1,
            operand_id2,
            type1,
            type2,
            result_type,
            category,
        );
        self.push(expr)
    }

    pub fn add_unary(
        &mut self,
        name: impl Into<String>,
        operator_symbol: char,
        operand_id1: usize,
        type1: OperandType,
        result_type: OperandType,
        category: Option<LexemeCategory>,
    ) -> usize {
        let id = self.next_id;
        let expr = RegularExpression::new_unary(
            id,
            name,
            operator_symbol,
            operand_id1,
            type1,
            result_type,
            category,
        );
        self.push(expr)
    }

    pub fn build_nfa(&self, charset_table: &mut CharSetTable) -> Result<Graph, String> {
        let root = self
            .rows
            .last()
            .ok_or_else(|| format!("empty regular table for {}", self.token_name))?;
        let mut graph_cache = HashMap::new();
        let mut charset_cache = HashMap::new();
        let mut graph = self.build_expr_nfa(
            root.regular_id,
            charset_table,
            &mut graph_cache,
            &mut charset_cache,
        )?;
        graph.mark_accepting(Some(self.token_name.clone()));
        Ok(graph)
    }

    fn build_expr_nfa(
        &self,
        regular_id: usize,
        charset_table: &mut CharSetTable,
        graph_cache: &mut HashMap<usize, Graph>,
        charset_cache: &mut HashMap<usize, usize>,
    ) -> Result<Graph, String> {
        if let Some(graph) = graph_cache.get(&regular_id) {
            return Ok(graph.clone());
        }

        let expr = self
            .rows
            .iter()
            .find(|row| row.regular_id == regular_id)
            .ok_or_else(|| format!("unknown regular id: {regular_id}"))?;

        let graph = match expr.operator_symbol {
            '=' => self.resolve_operand_as_nfa(
                expr.operand_id1,
                expr.type1,
                charset_table,
                graph_cache,
                charset_cache,
            )?,
            '.' => {
                let left = self.resolve_operand_as_nfa(
                    expr.operand_id1,
                    expr.type1,
                    charset_table,
                    graph_cache,
                    charset_cache,
                )?;
                let right = self.resolve_operand_as_nfa(
                    expr.operand_id2.unwrap(),
                    expr.type2.unwrap(),
                    charset_table,
                    graph_cache,
                    charset_cache,
                )?;
                product_nfa(&left, &right)
            }
            '|' => {
                let left = self.resolve_operand_as_nfa(
                    expr.operand_id1,
                    expr.type1,
                    charset_table,
                    graph_cache,
                    charset_cache,
                )?;
                let right = self.resolve_operand_as_nfa(
                    expr.operand_id2.unwrap(),
                    expr.type2.unwrap(),
                    charset_table,
                    graph_cache,
                    charset_cache,
                )?;
                union_nfa(&left, &right)
            }
            '*' => {
                let inner = self.resolve_operand_as_nfa(
                    expr.operand_id1,
                    expr.type1,
                    charset_table,
                    graph_cache,
                    charset_cache,
                )?;
                closure_nfa(&inner)
            }
            '+' => {
                let inner = self.resolve_operand_as_nfa(
                    expr.operand_id1,
                    expr.type1,
                    charset_table,
                    graph_cache,
                    charset_cache,
                )?;
                plus_closure_nfa(&inner)
            }
            '?' => {
                let inner = self.resolve_operand_as_nfa(
                    expr.operand_id1,
                    expr.type1,
                    charset_table,
                    graph_cache,
                    charset_cache,
                )?;
                zero_or_one_nfa(&inner)
            }
            op => return Err(format!("operator '{op}' cannot build an NFA directly")),
        };

        graph_cache.insert(regular_id, graph.clone());
        Ok(graph)
    }

    fn resolve_operand_as_nfa(
        &self,
        operand_id: usize,
        operand_type: OperandType,
        charset_table: &mut CharSetTable,
        graph_cache: &mut HashMap<usize, Graph>,
        charset_cache: &mut HashMap<usize, usize>,
    ) -> Result<Graph, String> {
        match operand_type {
            OperandType::Char => Ok(generate_basic_nfa(DriverType::Char, operand_id)),
            OperandType::Charset => {
                if operand_id >= CHARSET_ID_BASE {
                    Ok(generate_basic_nfa(DriverType::Charset, operand_id))
                } else {
                    let charset =
                        self.evaluate_charset_expr(operand_id, charset_table, charset_cache)?;
                    Ok(generate_basic_nfa(DriverType::Charset, charset))
                }
            }
            OperandType::Regular => {
                self.build_expr_nfa(operand_id, charset_table, graph_cache, charset_cache)
            }
        }
    }

    fn evaluate_charset_expr(
        &self,
        regular_id: usize,
        charset_table: &mut CharSetTable,
        charset_cache: &mut HashMap<usize, usize>,
    ) -> Result<usize, String> {
        if let Some(id) = charset_cache.get(&regular_id) {
            return Ok(*id);
        }

        let expr = self
            .rows
            .iter()
            .find(|row| row.regular_id == regular_id)
            .ok_or_else(|| format!("unknown regular id: {regular_id}"))?;

        let charset_id = match expr.operator_symbol {
            '~' => {
                let from = char::from_u32(expr.operand_id1 as u32)
                    .ok_or_else(|| "invalid char id".to_string())?;
                let to = char::from_u32(expr.operand_id2.unwrap() as u32)
                    .ok_or_else(|| "invalid char id".to_string())?;
                charset_table.range(from, to)?
            }
            '|' => {
                let left = self.resolve_charset_operand(
                    expr.operand_id1,
                    expr.type1,
                    charset_table,
                    charset_cache,
                )?;
                let right = self.resolve_charset_operand(
                    expr.operand_id2.unwrap(),
                    expr.type2.unwrap(),
                    charset_table,
                    charset_cache,
                )?;
                match (left, right) {
                    (CharsetValue::Char(c1), CharsetValue::Char(c2)) => {
                        charset_table.union_chars(c1, c2)
                    }
                    (CharsetValue::Charset(id), CharsetValue::Char(c))
                    | (CharsetValue::Char(c), CharsetValue::Charset(id)) => {
                        charset_table.union_charset_char(id, c)?
                    }
                    (CharsetValue::Charset(left_id), CharsetValue::Charset(right_id)) => {
                        charset_table.union_charsets(left_id, right_id)?
                    }
                }
            }
            '-' => {
                let left = self.resolve_charset_operand(
                    expr.operand_id1,
                    expr.type1,
                    charset_table,
                    charset_cache,
                )?;
                let right = self.resolve_charset_operand(
                    expr.operand_id2.unwrap(),
                    expr.type2.unwrap(),
                    charset_table,
                    charset_cache,
                )?;
                match (left, right) {
                    (CharsetValue::Charset(id), CharsetValue::Char(c)) => {
                        charset_table.difference_charset_char(id, c)?
                    }
                    _ => {
                        return Err(format!(
                            "regular id {} does not match charset - char",
                            regular_id
                        ));
                    }
                }
            }
            _ => {
                return Err(format!(
                    "regular id {} does not evaluate to a charset",
                    regular_id
                ));
            }
        };

        charset_cache.insert(regular_id, charset_id);
        Ok(charset_id)
    }

    fn resolve_charset_operand(
        &self,
        operand_id: usize,
        operand_type: OperandType,
        charset_table: &mut CharSetTable,
        charset_cache: &mut HashMap<usize, usize>,
    ) -> Result<CharsetValue, String> {
        match operand_type {
            OperandType::Char => {
                let ch = char::from_u32(operand_id as u32)
                    .ok_or_else(|| "invalid char id".to_string())?;
                Ok(CharsetValue::Char(ch))
            }
            OperandType::Charset => {
                if operand_id >= CHARSET_ID_BASE {
                    Ok(CharsetValue::Charset(operand_id))
                } else {
                    Ok(CharsetValue::Charset(self.evaluate_charset_expr(
                        operand_id,
                        charset_table,
                        charset_cache,
                    )?))
                }
            }
            OperandType::Regular => Err("regular operand cannot be used as charset".to_string()),
        }
    }
}

pub fn build_charset_table(
    rules: &[TokenRule],
) -> Result<(CharSetTable, HashMap<String, usize>), String> {
    let mut table = CharSetTable::new();
    let mut charset_ids = HashMap::new();

    for rule in rules.iter().filter(|rule| is_charset_name(&rule.name)) {
        let charset_id = evaluate_charset_rule(&rule.regex, &charset_ids, &mut table)?;
        charset_ids.insert(rule.name.clone(), charset_id);
    }

    Ok((table, charset_ids))
}

pub fn build_token_regular_tables(rules: &[TokenRule]) -> Result<Vec<RegularTable>, String> {
    let (_, charset_ids) = build_charset_table(rules)?;
    let mut tables = Vec::new();
    for rule in rules.iter().filter(|rule| is_token_name(&rule.name)) {
        tables.push(build_regular_table(rule, &charset_ids)?);
    }

    Ok(tables)
}

pub fn build_regular_table(
    rule: &TokenRule,
    charset_ids: &HashMap<String, usize>,
) -> Result<RegularTable, String> {
    if !is_token_name(&rule.name) {
        return Err(format!("{} is not a token rule", rule.name));
    }

    let tokens = tokenize_regex(&rule.regex)?;
    let tokens = insert_concat(tokens);
    let postfix = infix_to_postfix(&tokens)?;
    let category = infer_category(&rule.name);
    let mut table = RegularTable::new(rule.name.clone(), category.clone());
    let mut stack = Vec::new();

    for token in postfix {
        match token {
            RegexToken::Char(ch) => stack.push(ExprRef::new(ch as usize, OperandType::Char)),
            RegexToken::Name(name) => {
                if let Some(&charset_id) = charset_ids.get(&name) {
                    stack.push(ExprRef::new(charset_id, OperandType::Charset));
                } else {
                    return Err(format!("unknown charset reference: {name}"));
                }
            }
            RegexToken::Operator(op) if is_unary_operator(op) => {
                let operand = stack.pop().ok_or_else(|| {
                    format!("operator '{op}' is missing an operand in {}", rule.name)
                })?;
                let result_type = OperandType::Regular;
                let expr_name = format!("r{}", table.next_id);
                let expr_id = table.add_unary(
                    expr_name,
                    op,
                    operand.id,
                    operand.kind,
                    result_type,
                    category.clone(),
                );
                stack.push(ExprRef::new(expr_id, result_type));
            }
            RegexToken::Operator(op) => {
                let right = stack.pop().ok_or_else(|| {
                    format!("operator '{op}' is missing right operand in {}", rule.name)
                })?;
                let left = stack.pop().ok_or_else(|| {
                    format!("operator '{op}' is missing left operand in {}", rule.name)
                })?;
                let result_type = binary_result_type(op, left.kind, right.kind)?;
                let expr_name = format!("r{}", table.next_id);
                let expr_id = table.add_binary(
                    expr_name,
                    op,
                    left.id,
                    right.id,
                    left.kind,
                    right.kind,
                    result_type,
                    category.clone(),
                );
                stack.push(ExprRef::new(expr_id, result_type));
            }
            RegexToken::LParen | RegexToken::RParen => {
                return Err("parenthesis should not appear in postfix output".to_string());
            }
        }
    }

    if stack.len() != 1 {
        return Err(format!("failed to reduce regex for {}", rule.name));
    }

    let final_expr = stack.pop().unwrap();
    table.add_unary(
        rule.name.clone(),
        '=',
        final_expr.id,
        final_expr.kind,
        OperandType::Regular,
        category,
    );

    Ok(table)
}

fn evaluate_charset_rule(
    regex: &str,
    charset_ids: &HashMap<String, usize>,
    table: &mut CharSetTable,
) -> Result<usize, String> {
    let tokens = infix_to_postfix(&tokenize_regex(regex)?)?;
    let mut stack = Vec::new();

    for token in tokens {
        match token {
            RegexToken::Char(ch) => stack.push(CharsetValue::Char(ch)),
            RegexToken::Name(name) => {
                let id = charset_ids
                    .get(&name)
                    .copied()
                    .ok_or_else(|| format!("unknown charset reference: {name}"))?;
                stack.push(CharsetValue::Charset(id));
            }
            RegexToken::Operator(op) => {
                let right = stack
                    .pop()
                    .ok_or_else(|| format!("operator '{op}' missing operand"))?;
                let left = stack
                    .pop()
                    .ok_or_else(|| format!("operator '{op}' missing operand"))?;
                let id = match (op, left, right) {
                    ('~', CharsetValue::Char(from), CharsetValue::Char(to)) => {
                        table.range(from, to)?
                    }
                    ('|', CharsetValue::Char(c1), CharsetValue::Char(c2)) => {
                        table.union_chars(c1, c2)
                    }
                    ('|', CharsetValue::Charset(id), CharsetValue::Char(c))
                    | ('|', CharsetValue::Char(c), CharsetValue::Charset(id)) => {
                        table.union_charset_char(id, c)?
                    }
                    ('|', CharsetValue::Charset(left_id), CharsetValue::Charset(right_id)) => {
                        table.union_charsets(left_id, right_id)?
                    }
                    ('-', CharsetValue::Charset(id), CharsetValue::Char(c)) => {
                        table.difference_charset_char(id, c)?
                    }
                    _ => return Err(format!("unsupported charset expression: {regex}")),
                };
                stack.push(CharsetValue::Charset(id));
            }
            RegexToken::LParen | RegexToken::RParen => {}
        }
    }

    match stack.pop() {
        Some(CharsetValue::Charset(id)) if stack.is_empty() => Ok(id),
        _ => Err(format!("invalid charset expression: {regex}")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum RegexToken {
    Char(char),
    Name(String),
    Operator(char),
    LParen,
    RParen,
}

#[derive(Debug, Clone, Copy)]
struct ExprRef {
    id: usize,
    kind: OperandType,
}

enum CharsetValue {
    Char(char),
    Charset(usize),
}

impl ExprRef {
    fn new(id: usize, kind: OperandType) -> Self {
        Self { id, kind }
    }
}

fn tokenize_regex(regex: &str) -> Result<Vec<RegexToken>, String> {
    let chars: Vec<char> = regex.chars().collect();
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < chars.len() {
        let ch = chars[i];

        if ch.is_whitespace() {
            i += 1;
            continue;
        }

        if ch == '\'' {
            if i + 2 >= chars.len() || chars[i + 2] != '\'' {
                return Err(format!("invalid char literal in regex: {regex}"));
            }
            tokens.push(RegexToken::Char(chars[i + 1]));
            i += 3;
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            tokens.push(RegexToken::Name(chars[start..i].iter().collect()));
            continue;
        }

        match ch {
            '(' => tokens.push(RegexToken::LParen),
            ')' => tokens.push(RegexToken::RParen),
            '|' | '.' | '*' | '+' | '?' | '~' | '-' => tokens.push(RegexToken::Operator(ch)),
            _ => return Err(format!("unsupported symbol '{ch}' in regex: {regex}")),
        }

        i += 1;
    }

    Ok(tokens)
}

fn insert_concat(tokens: Vec<RegexToken>) -> Vec<RegexToken> {
    let mut result = Vec::new();

    for token in tokens {
        if let Some(prev) = result.last() {
            if needs_concat(prev, &token) {
                result.push(RegexToken::Operator('.'));
            }
        }
        result.push(token);
    }

    result
}

fn infix_to_postfix(tokens: &[RegexToken]) -> Result<Vec<RegexToken>, String> {
    let mut output = Vec::new();
    let mut operators = Vec::new();

    for token in tokens {
        match token {
            RegexToken::Char(_) | RegexToken::Name(_) => output.push(token.clone()),
            RegexToken::LParen => operators.push(token.clone()),
            RegexToken::RParen => {
                let mut found_left_paren = false;
                while let Some(top) = operators.pop() {
                    if top == RegexToken::LParen {
                        found_left_paren = true;
                        break;
                    }
                    output.push(top);
                }
                if !found_left_paren {
                    return Err("mismatched ')'".to_string());
                }
            }
            RegexToken::Operator(op) => {
                while let Some(RegexToken::Operator(top)) = operators.last() {
                    if precedence(*top) >= precedence(*op) {
                        if let Some(last) = operators.pop() {
                            output.push(last);
                        }
                    } else {
                        break;
                    }
                }
                operators.push(token.clone());
            }
        }
    }

    while let Some(top) = operators.pop() {
        if top == RegexToken::LParen {
            return Err("mismatched '('".to_string());
        }
        output.push(top);
    }

    Ok(output)
}

fn needs_concat(left: &RegexToken, right: &RegexToken) -> bool {
    is_expr_end(left) && is_expr_start(right)
}

fn is_expr_start(token: &RegexToken) -> bool {
    matches!(
        token,
        RegexToken::Char(_) | RegexToken::Name(_) | RegexToken::LParen
    )
}

fn is_expr_end(token: &RegexToken) -> bool {
    matches!(
        token,
        RegexToken::Char(_)
            | RegexToken::Name(_)
            | RegexToken::RParen
            | RegexToken::Operator('*' | '+' | '?')
    )
}

fn precedence(op: char) -> usize {
    match op {
        '*' | '+' | '?' => 4,
        '~' | '-' => 3,
        '.' => 2,
        '|' => 1,
        _ => 0,
    }
}

fn is_unary_operator(op: char) -> bool {
    matches!(op, '*' | '+' | '?')
}

fn binary_result_type(
    op: char,
    left: OperandType,
    right: OperandType,
) -> Result<OperandType, String> {
    match op {
        '.' => Ok(OperandType::Regular),
        '~' => match (left, right) {
            (OperandType::Char, OperandType::Char) => Ok(OperandType::Charset),
            _ => Err("range '~' only supports char ~ char".to_string()),
        },
        '-' => match (left, right) {
            (OperandType::Charset, OperandType::Char) => Ok(OperandType::Charset),
            _ => Err("difference '-' only supports charset - char".to_string()),
        },
        '|' => match (left, right) {
            (OperandType::Regular, OperandType::Regular) => Ok(OperandType::Regular),
            (OperandType::Char, OperandType::Char)
            | (OperandType::Char, OperandType::Charset)
            | (OperandType::Charset, OperandType::Char)
            | (OperandType::Charset, OperandType::Charset) => Ok(OperandType::Charset),
            _ => Ok(OperandType::Regular),
        },
        _ => Err(format!("unsupported operator '{op}'")),
    }
}

fn is_token_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|ch| ch.is_ascii_uppercase() || ch == '_')
}

fn is_charset_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(|ch| ch.is_ascii_lowercase() || ch == '_')
}

fn infer_category(name: &str) -> Option<LexemeCategory> {
    match name {
        "ID" => Some(LexemeCategory::Identifier),
        "NUM" => Some(LexemeCategory::Number),
        "IF" | "THEN" | "ELSE" | "END" | "REPEAT" | "UNTIL" | "READ" | "WRITE" => {
            Some(LexemeCategory::Keyword)
        }
        "ASSIGN" | "EQ" | "LT" | "PLUS" | "MINUS" | "TIMES" | "OVER" => {
            Some(LexemeCategory::Operator)
        }
        "LPAREN" | "RPAREN" | "SEMI" => Some(LexemeCategory::Delimiter),
        _ => Some(LexemeCategory::Unknown),
    }
}

#[cfg(test)]
mod tests {
    use super::{OperandType, RegularTable, build_charset_table, build_token_regular_tables};
    use crate::lexer::rule::parse_rules;

    fn parse_table(input: &str, token_name: &str) -> RegularTable {
        let rules = parse_rules(input).unwrap();
        build_token_regular_tables(&rules)
            .unwrap()
            .into_iter()
            .find(|table| table.token_name == token_name)
            .unwrap()
    }

    fn debug_rows(table: &RegularTable) -> Vec<(char, OperandType, Option<OperandType>)> {
        table
            .rows()
            .iter()
            .map(|row| (row.operator_symbol, row.type1, row.type2))
            .collect()
    }

    #[test]
    fn token_tables_only_include_uppercase_rules() {
        let rules = parse_rules(
            r#"
letter 'a'~'z'
digit '0'~'9'
ID letter (letter | digit)*
"#,
        )
        .unwrap();

        let tables = build_token_regular_tables(&rules).unwrap();

        assert_eq!(tables.len(), 1);
        assert_eq!(tables[0].token_name, "ID");
    }

    #[test]
    fn keyword_rule_becomes_concat_chain() {
        let table = parse_table("IF 'i''f'", "IF");
        let rows = debug_rows(&table);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, '.');
        assert_eq!(rows[0].1, OperandType::Char);
        assert_eq!(rows[0].2, Some(OperandType::Char));
        assert_eq!(rows[1].0, '=');
    }

    #[test]
    fn id_rule_uses_charset_and_closure() {
        let table = parse_table(
            r#"
letter 'a'~'z' | 'A'~'Z'
digit '0'~'9'
ID letter (letter | digit)*
"#,
            "ID",
        );

        let rows = debug_rows(&table);

        assert!(rows.iter().any(|row| row.0 == '|'));
        assert!(rows.iter().any(|row| row.0 == '*'));
        assert_eq!(rows.last().unwrap().0, '=');
    }

    #[test]
    fn single_char_token_still_has_a_head_row() {
        let table = parse_table("EQ '='", "EQ");

        assert_eq!(table.rows().len(), 1);
        assert_eq!(table.rows()[0].operator_symbol, '=');
        assert_eq!(table.rows()[0].type1, OperandType::Char);
    }

    #[test]
    fn token_table_can_build_nfa() {
        let rules = parse_rules(
            r#"
letter 'a'~'z' | 'A'~'Z'
digit '0'~'9'
ID letter (letter | digit)*
"#,
        )
        .unwrap();

        let (mut charset_table, _) = build_charset_table(&rules).unwrap();
        let table = build_token_regular_tables(&rules)
            .unwrap()
            .into_iter()
            .find(|table| table.token_name == "ID")
            .unwrap();

        let nfa = table.build_nfa(&mut charset_table).unwrap();

        assert!(!nfa.states.is_empty());
        assert!(!nfa.edges.is_empty());
        assert_eq!(nfa.states[nfa.end_state].category.as_deref(), Some("ID"));
    }
}
