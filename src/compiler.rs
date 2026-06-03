#![allow(dead_code)]

use std::collections::{BTreeMap, HashMap};

use crate::lexer::{build_lexer_program, dfa::ScannedToken, dfa_scan, nfa_to_dfa, parse_rules};
use crate::syntax::{build_lr0_dfa, build_slr_parse_table, grammar, grammar_file, ActionCategory};

// ─── 文件路径 ──────────────────────────────────────────────

const LEXER_RULES_PATH: &str = "tests/lexer/tiny.txt";
const GRAMMAR_PATH: &str = "tests/syntax/tiny.txt";

// ─── token 类别 → 语法终端名 ────────────────────────────────

fn token_to_terminal(category: &str) -> &str {
    match category {
        "IF" => "if", "THEN" => "then", "ELSE" => "else", "END" => "end",
        "REPEAT" => "repeat", "UNTIL" => "until", "READ" => "read", "WRITE" => "write",
        "ID" => "id", "NUM" => "num",
        "ASSIGN" => ":=", "SEMI" => ";", "EQ" => "=", "LT" => "<",
        "PLUS" => "+", "MINUS" => "-", "TIMES" => "*", "OVER" => "/",
        "LPAREN" => "(", "RPAREN" => ")",
        other => other,
    }
}

// ─── 代码片段（含指令计数，用于计算回填偏移）─────────────────

struct Code {
    text: String,
    instrs: usize,
}

impl Code {
    fn cat(self, other: Code) -> Code {
        Code { instrs: self.instrs + other.instrs, text: self.text + &other.text }
    }

    fn empty() -> Code { Code { text: String::new(), instrs: 0 } }
}

// ─── 语义值：规约时在栈上传递 ───────────────────────────────

enum Sv {
    Code(Code),
    Name(String),
    Num(i32),
    Unit,
}

fn sv_code(v: &mut Sv) -> Code {
    match std::mem::replace(v, Sv::Unit) {
        Sv::Code(c) => c,
        _ => panic!("expected Code"),
    }
}

fn sv_name(v: &mut Sv) -> String {
    match std::mem::replace(v, Sv::Unit) {
        Sv::Name(n) => n,
        _ => panic!("expected Name"),
    }
}

// ─── SLR(1) 解析、语义分析、代码生成（单趟）─────────────────

struct SlrParser {
    grammar: grammar::Grammar,
    action: HashMap<(usize, String), (ActionCategory, usize)>,
    goto: HashMap<(usize, String), usize>,
    tokens: Vec<ScannedToken>,
    pos: usize,

    // 代码生成用
    vars: BTreeMap<String, i32>,
    next_addr: i32,    // 变量地址从 0 递减，临时变量复用
}

impl SlrParser {
    const AC: i32 = 0;
    const AC1: i32 = 1;
    const GP: i32 = 6;
    const PC: i32 = 7;

    fn new(tokens: Vec<ScannedToken>) -> Result<Self, String> {
        let grammar = grammar_file::load_grammar(GRAMMAR_PATH)?;
        let aug = grammar.augmented();
        let dfa = build_lr0_dfa(&aug);
        let table = build_slr_parse_table(&aug, &dfa);

        if !table.conflicts.is_empty() {
            return Err(format!("SLR(1) conflicts:\n{}", table.conflicts.join("\n")));
        }

        let action = table.action_cells.iter().map(|c| {
            ((c.state_id, c.terminal_symbol_name.clone()), (c.action_type, c.id))
        }).collect();

        let goto = table.goto_cells.iter().map(|c| {
            ((c.state_id, c.non_terminal_symbol_name.clone()), c.next_state_id)
        }).collect();

        Ok(Self {
            grammar: aug, action, goto, tokens, pos: 0,
            vars: BTreeMap::new(), next_addr: 0,
        })
    }

    /// 返回可直接写入 .tm 文件的完整代码
    fn parse(mut self) -> Result<String, String> {
        let mut st: Vec<usize> = vec![0];
        let mut vs: Vec<Sv> = Vec::new();

        loop {
            let state = *st.last().unwrap();
            let term = self.current()
                .map(|t| token_to_terminal(&t.category))
                .unwrap_or("EOF")
                .to_string();

            let Some((act, id)) = self.action.get(&(state, term.clone())).copied() else {
                let (l, c) = self.current().map(|t| (t.line, t.column)).unwrap_or((0, 0));
                let detail = self.current()
                    .map_or("EOF".into(), |t| format!("'{}' ({})", t.lexeme, t.category));
                return Err(format!("syntax error at line {l}, col {c}: unexpected token {detail}"));
            };

            match act {
                ActionCategory::Shift => {
                    let tok = self.advance().clone();
                    st.push(id);
                    // 把关键 token 数据压栈，关键字/运算符压 Unit
                    vs.push(match tok.category.as_str() {
                        "ID" => Sv::Name(tok.lexeme),
                        "NUM" => Sv::Num(tok.lexeme.parse().unwrap_or(0)),
                        _ => Sv::Unit,
                    });
                }

                ActionCategory::Reduce => {
                    let prod = self.grammar.productions[id].clone();
                    let head = self.grammar.non_terminal_name(prod.head).to_string();
                    let n = prod.body.len();

                    let val = self.semantic(&prod, &head, n, &mut vs);
                    for _ in 0..n { st.pop(); }

                    let top = *st.last().unwrap();
                    let Some(&nx) = self.goto.get(&(top, head.clone())) else {
                        let pos = self.pos.saturating_sub(1);
                        let line = self.tokens.get(pos).map_or(1, |t| t.line);
                        let col = self.tokens.get(pos).map_or(1, |t| t.column);
                        return Err(format!("GOTO[{top},{head}] missing at line {line},col {col}"));
                    };
                    st.push(nx);
                    vs.push(val);
                }

                ActionCategory::Accept => {
                    let code = sv_code(vs.last_mut().unwrap());
                    return Ok(self.finalize(code));
                }
            }
        }
    }

    // ── SDT 核心：产生式 → TM 指令 ──────────────────────────
    //
    // 产生式（来自 tests/syntax/tiny.txt）：
    //   program    -> stmt_seq
    //   stmt_seq   -> stmt_seq ; statement | statement
    //   statement  -> if_stmt | repeat_stmt | assign_stmt | read_stmt | write_stmt
    //   if_stmt    -> if exp then stmt_seq end
    //                | if exp then stmt_seq else stmt_seq end
    //   repeat_stmt-> repeat stmt_seq until exp
    //   assign_stmt-> id := exp
    //   read_stmt  -> read id
    //   write_stmt -> write exp
    //   exp        -> exp < simple_exp | exp = simple_exp | simple_exp
    //   simple_exp -> simple_exp + term | simple_exp - term | term
    //   term       -> term * factor | term / factor | factor
    //   factor     -> ( exp ) | num | id

    fn semantic(&mut self, prod: &grammar::Production, head: &str, n: usize, vs: &mut Vec<Sv>) -> Sv {
        let mut p = Vec::with_capacity(n);
        for _ in 0..n { p.push(vs.pop().unwrap()); }
        p.reverse();

        match (head, n) {

            // ── 传递型 ──
            ("program", 1) | ("statement", 1) | ("exp", 1)
            | ("simple_exp", 1) | ("term", 1) => {
                std::mem::replace(&mut p[0], Sv::Unit)
            }

            ("stmt_seq", 1) => std::mem::replace(&mut p[0], Sv::Unit),

            ("stmt_seq", 3) => {
                let a = sv_code(&mut p[0]);
                let b = sv_code(&mut p[2]);
                Sv::Code(a.cat(b))
            }

            // ── factor → num | id ──
            ("factor", 1) => {
                let sym = self.grammar.symbol_name(prod.body[0]);
                if sym == "num" {
                    let v = match std::mem::replace(&mut p[0], Sv::Unit) {
                        Sv::Num(n) => n,
                        _ => panic!("factor: expected Num token but got Name"),
                    };
                    Sv::Code(Code {
                        text: format!("LDC {},{}(0)    load const\n", Self::AC, v),
                        instrs: 1,
                    })
                } else {
                    let name = sv_name(&mut p[0]);
                    let addr = self.var_addr(&name);
                    Sv::Code(Code {
                        text: format!("LD {},{}({})    load id: {}\n", Self::AC, addr, Self::GP, name),
                        instrs: 1,
                    })
                }
            }

            // ── factor → ( exp ) ──
            ("factor", 3) => Sv::Code(sv_code(&mut p[1])),

            // ── 二元运算（算术 + 比较）───────────────────────

            ("simple_exp", 3) => self.binary(
                sv_code(&mut p[0]), sv_code(&mut p[2]),
                prod, &["+", "-"],
                &[("ADD", "op +"), ("SUB", "op -")],
            ),

            ("term", 3) => self.binary(
                sv_code(&mut p[0]), sv_code(&mut p[2]),
                prod, &["*", "/"],
                &[("MUL", "op *"), ("DIV", "op /")],
            ),

            ("exp", 3) => {
                let sym = self.grammar.symbol_name(prod.body[1]);
                if sym == "<" {
                    self.compare(sv_code(&mut p[0]), sv_code(&mut p[2]), "JLT")
                } else {
                    self.compare(sv_code(&mut p[0]), sv_code(&mut p[2]), "JEQ")
                }
            }

            // ── 语句 ──────────────────────────────────────

            ("assign_stmt", 3) => {
                let name = sv_name(&mut p[0]);
                let expr = sv_code(&mut p[2]);
                let addr = self.var_addr(&name);
                Sv::Code(Code {
                    text: format!("{}ST {},{}({})    assign: {}\n", expr.text, Self::AC, addr, Self::GP, name),
                    instrs: expr.instrs + 1,
                })
            }

            ("read_stmt", 2) => {
                let name = sv_name(&mut p[1]);
                let addr = self.var_addr(&name);
                Sv::Code(Code {
                    text: format!(
                        "IN {},{},{}    read integer value\nST {},{}({})    read: {}\n",
                        Self::AC, 0, 0, Self::AC, addr, Self::GP, name,
                    ),
                    instrs: 2,
                })
            }

            ("write_stmt", 2) => {
                let expr = sv_code(&mut p[1]);
                Sv::Code(Code {
                    text: format!("{}OUT {},{},{}    write ac\n", expr.text, Self::AC, 0, 0),
                    instrs: expr.instrs + 1,
                })
            }

            ("repeat_stmt", 4) => {
                let body = sv_code(&mut p[1]);
                let test = sv_code(&mut p[3]);
                let offset = -(body.instrs as i32 + test.instrs as i32 + 1);
                Sv::Code(Code {
                    text: format!(
                        "{}{}JEQ {},{}({})    repeat: jmp back\n",
                        body.text, test.text, Self::AC, offset, Self::PC,
                    ),
                    instrs: body.instrs + test.instrs + 1,
                })
            }

            ("if_stmt", 5) => {
                // if exp then stmt_seq end
                let test = sv_code(&mut p[1]);
                let then = sv_code(&mut p[3]);
                Sv::Code(Code {
                    text: format!(
                        "{}JEQ {},{}({})    if: jmp to end\n{}",
                        test.text, Self::AC, then.instrs, Self::PC, then.text,
                    ),
                    instrs: test.instrs + 1 + then.instrs,
                })
            }

            ("if_stmt", 7) => {
                // if exp then stmt_seq else stmt_seq end
                let test = sv_code(&mut p[1]);
                let then = sv_code(&mut p[3]);
                let els = sv_code(&mut p[5]);
                let text = test.text
                    + &format!("JEQ {},{}({})    if: jmp to else\n", Self::AC, then.instrs + 1, Self::PC)
                    + &then.text
                    + &format!("LDA {},{}({})    if: jmp to end\n", Self::PC, els.instrs, Self::PC)
                    + &els.text;
                Sv::Code(Code {
                    instrs: test.instrs + 1 + then.instrs + 1 + els.instrs,
                    text,
                })
            }

            _ => panic!("no SDT rule for: {head} (n={n})"),
        }
    }

    // ── 工具方法 ──────────────────────────────────────────

    /// 算术二元运算：left, right 代码已生成，拼接 save/restore + 运算指令
    fn binary(&mut self, left: Code, right: Code, prod: &grammar::Production,
              symbols: &[&str], ops: &[(&str, &str)]) -> Sv
    {
        let sym = self.grammar.symbol_name(prod.body[1]);
        let (op, comment) = if sym == symbols[0] { ops[0] } else { ops[1] };
        let tmp = self.temp_addr();

        let text = left.text
            + &format!("ST {},{}({})    op: push left\n", Self::AC, tmp, Self::GP)
            + &right.text
            + &format!("LD {},{}({})    op: load left\n", Self::AC1, tmp, Self::GP)
            + &format!("{} {},{},{}    {}\n", op, Self::AC, Self::AC1, Self::AC, comment);

        Sv::Code(Code { instrs: left.instrs + 1 + right.instrs + 1 + 1, text })
    }

    /// 比较运算：生成标准 5 指令模板
    fn compare(&self, left: Code, right: Code, jmp: &str) -> Sv {
        let tmp = self.temp_addr();
        let text = left.text
            + &format!("ST {},{}({})    op: push left\n", Self::AC, tmp, Self::GP)
            + &right.text
            + &format!("LD {},{}({})    op: load left\n", Self::AC1, tmp, Self::GP)
            + &format!("SUB {},{},{}    op cmp\n", Self::AC, Self::AC1, Self::AC)
            + &format!("{} {},{}({})    true case\n", jmp, Self::AC, 2, Self::PC)
            + &format!("LDC {},{}(0)    false case\n", Self::AC, 0)
            + &format!("LDA {},{}({})    skip true case\n", Self::PC, 1, Self::PC)
            + &format!("LDC {},{}(0)    true case\n", Self::AC, 1);

        Sv::Code(Code { instrs: left.instrs + 1 + right.instrs + 1 + 5, text })
    }

    fn var_addr(&mut self, name: &str) -> i32 {
        if let Some(&a) = self.vars.get(name) { return a; }
        let a = self.next_addr;
        self.vars.insert(name.into(), a);
        self.next_addr -= 1;
        a
    }

    /// 每次二元运算分配一个独立的临时地址（不复用，简化管理）
    fn temp_addr(&self) -> i32 {
        // 使用 next_addr 以下的地址作为临时空间
        // 变量从 0 往下，temp 用更靠下的地址（不会与变量重叠）
        self.next_addr - 1
    }

    fn finalize(&self, code: Code) -> String {
        let prelude = "\
            * TINY Compilation to TM Code\n\
            * Standard prelude:\n\
              0:  LD 6,0(0)    load gp with maxaddress\n\
              1:  LDA 0,0(6)    clear accumulator\n\
            * End of standard prelude.\n";
        let body = renumber(&code.text, /* start_at */ 2);
        let halt = format!(
            "* End of execution.\n{:>3}:  HALT 0,0,0\n",
            code.instrs + 2
        );
        format!("{prelude}{body}{halt}")
    }

    fn current(&self) -> Option<&ScannedToken> { self.tokens.get(self.pos) }
    fn advance(&mut self) -> &ScannedToken { let t = &self.tokens[self.pos]; self.pos += 1; t }
}

// ─── 行号重排 ──────────────────────────────────────────────

fn renumber(code: &str, mut n: usize) -> String {
    let mut out = String::new();
    for line in code.lines() {
        if line.is_empty() { continue; }
        out.push_str(&format!("{:>3}:  {}\n", n, line));
        n += 1;
    }
    out
}

// ─── 主入口 ────────────────────────────────────────────────

pub fn run(source_path: &str) -> Result<(), String> {
    let source = std::fs::read_to_string(source_path)
        .map_err(|e| format!("read source: {e}"))?;
    let tokens = scan_tiny(&source)?;
    let tm = SlrParser::new(tokens)?.parse()?;

    let out = tm_output_path(source_path)?;
    std::fs::write(&out, &tm)
        .map_err(|e| format!("write output: {e}"))?;

    println!("compiled: {source_path}");
    println!("output:   {}", out.display());
    Ok(())
}

fn scan_tiny(source: &str) -> Result<Vec<ScannedToken>, String> {
    let text = std::fs::read_to_string(LEXER_RULES_PATH)
        .map_err(|e| format!("read lexer rules: {e}"))?;
    let rules = parse_rules(&text)?;
    let mut prog = build_lexer_program(&rules)?;
    let dfa = nfa_to_dfa(&prog.nfa, &mut prog.charset_table)?;
    dfa_scan(&dfa, &prog.charset_table, source)
}

fn tm_output_path(src: &str) -> Result<std::path::PathBuf, String> {
    let p = std::path::Path::new(src);
    if p.file_stem().is_none() {
        return Err(format!("bad source path: {src}"));
    }
    Ok(p.with_extension("tm"))
}

// ─── 测试 ──────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::{scan_tiny, SlrParser};

    fn compile(src: &str) -> String {
        let tokens = scan_tiny(src).unwrap();
        SlrParser::new(tokens).unwrap().parse().unwrap()
    }

    #[test]
    fn sample_factorial() {
        let tm = compile(r#"
read x;
if 0 < x then
  fact := 1;
  repeat
    fact := fact * x;
    x := x - 1
  until x = 0;
  write fact
end
"#);
        assert!(tm.contains("IN 0,0,0"));
        assert!(tm.contains("OUT 0,0,0"));
        assert!(tm.contains("HALT 0,0,0"));
    }

    #[test]
    fn simple_assign() {
        let tm = compile("x := 42");
        assert!(tm.contains("ST 0,0(6)"));
    }

    #[test]
    fn read_write() {
        let tm = compile("read x; write x");
        assert!(tm.contains("IN "));
        assert!(tm.contains("OUT "));
    }

    #[test]
    fn if_then() {
        let tm = compile("if 1 < 2 then x := 3 end");
        assert!(tm.contains("JEQ"));
        assert!(tm.contains("JLT"));
    }

    #[test]
    fn repeat_loop() {
        let tm = compile("repeat x := x - 1 until x = 0");
        assert!(tm.contains("JEQ")); // 回跳
    }

    #[test]
    fn paren_expr() {
        let tm = compile("x := (1 + 2) * 3");
        assert!(tm.contains("ADD"));
        assert!(tm.contains("MUL"));
    }

    #[test]
    fn if_else() {
        let tm = compile("if 1 < 2 then x := 1 else x := 2 end");
        assert!(tm.contains("jmp to else"));
        assert!(tm.contains("jmp to end"));
    }
}
