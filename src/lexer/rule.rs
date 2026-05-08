#[derive(Debug)]
pub struct TokenRule {
    pub name: String,
    pub regex: String,
}

pub fn parse_rules(content: &str) -> Result<Vec<TokenRule>, String> {
    let mut rules = Vec::new();

    for (index, line) in content.lines().enumerate() {
        let line = strip_comment(line).trim();
        if line.is_empty() {
            continue;
        }

        let (name, regex) = split_rule_line(line)
            .ok_or_else(|| format!("line {} is missing a token name", index + 1))?;

        if regex.is_empty() {
            return Err(format!("line {} is missing a regex expression", index + 1));
        }

        rules.push(TokenRule {
            name: name.to_string(),
            regex: regex.to_string(),
        });
    }

    if rules.is_empty() {
        return Err("no valid rules found in file".to_string());
    }

    Ok(rules)
}

fn split_rule_line(line: &str) -> Option<(&str, &str)> {
    let first_gap = line.find(char::is_whitespace)?;
    let name = line[..first_gap].trim();
    let regex = line[first_gap..].trim().trim_start_matches('=').trim();

    if name.is_empty() {
        return None;
    }

    Some((name, regex))
}

fn strip_comment(line: &str) -> &str {
    let mut in_quote = false;

    for (index, ch) in line.char_indices() {
        match ch {
            '\'' => in_quote = !in_quote,
            '#' if !in_quote => return &line[..index],
            _ => {}
        }
    }

    line
}

#[cfg(test)]
mod tests {
    use super::parse_rules;

    #[test]
    fn parse_rules_skips_comments_and_blank_lines() {
        let input = r#"
# comment
letter      'a'~'z' | 'A'~'Z'

digit       '0'~'9'   # tail comment
"#;

        let rules = parse_rules(input).unwrap();

        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0].name, "letter");
        assert_eq!(rules[0].regex, "'a'~'z' | 'A'~'Z'");
        assert_eq!(rules[1].name, "digit");
        assert_eq!(rules[1].regex, "'0'~'9'");
    }

    #[test]
    fn parse_rules_supports_optional_equal_sign() {
        let input = "letter = 'a'~'z' | 'A'~'Z'";
        let rules = parse_rules(input).unwrap();

        assert_eq!(rules[0].name, "letter");
        assert_eq!(rules[0].regex, "'a'~'z' | 'A'~'Z'");
    }
}
