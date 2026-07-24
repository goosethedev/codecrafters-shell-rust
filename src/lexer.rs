#![allow(dead_code)]

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Value(String),
    Stdout(WriteMode),
    Stderr(WriteMode),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WriteMode {
    Append,
    Redirect,
}

pub struct Lexer<'a> {
    chars: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Iterator for Lexer<'a> {
    type Item = Token;

    fn next(&mut self) -> Option<Self::Item> {
        while self.chars.next_if(|ch| ch.is_ascii_whitespace()).is_some() {}
        // TODO: Detect the rest of tokens
        if self.chars.peek().is_some() {
            self.tokenize_value()
        } else {
            None
        }
    }
}

impl<'a> Lexer<'a> {
    pub fn new(input: &'a str) -> Self {
        let chars = input.chars().peekable();
        Self { chars }
    }

    fn tokenize_value(&mut self) -> Option<Token> {
        let mut buf = String::new();
        while let Some(ch) = self.chars.next_if(|ch| !ch.is_ascii_whitespace()) {
            match ch {
                '\'' => {
                    while let Some(ch) = self.chars.next_if_eq(&'\'') {
                        buf.push(ch);
                    }
                    // Consume closing quote
                    if self.chars.next_if_eq(&'\'').is_none() {
                        unreachable!("unclosed single-quoted string");
                    }
                }
                '"' => {
                    while let Some(ch) = self.chars.next_if_eq(&'"') {
                        match ch {
                            '\\' => match self.chars.next() {
                                Some('\\') => buf.push('\\'),
                                Some('"') => buf.push('"'),
                                Some(ch) => {
                                    buf.push('\\');
                                    buf.push(ch)
                                }
                                None => unreachable!("\\ found at the end of the string"),
                            },
                            ch => buf.push(ch),
                        }
                    }
                    // Consume closing quote
                    if self.chars.next_if_eq(&'"').is_none() {
                        unreachable!("unclosed double-quoted string");
                    }
                }
                '\\' => {
                    if let Some(ch) = self.chars.next() {
                        buf.push(ch);
                    } else {
                        unreachable!("\\ found at the end of the string")
                    }
                }
                ch => buf.push(ch),
            }
        }
        Some(Token::Value(buf))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn _test_tokenized(input: &str, expected: &[Token]) {
        let actual: Vec<_> = Lexer::new(input).collect();
        assert_eq!(expected, actual, "for input: {input}");
    }

    fn test_tokenized_values(input: &str, expected: &[&str]) {
        let actual: Vec<_> = Lexer::new(input).collect();
        let expected: Vec<_> = expected
            .into_iter()
            .map(ToString::to_string)
            .map(Token::Value)
            .collect();
        assert_eq!(expected, actual, "for input: {input}");
    }

    #[test]
    fn tokenize_args_simple() {
        let input = "echo hello world";
        let expected = &["echo", "hello", "world"];
        test_tokenized_values(input, expected);
    }

    #[test]
    fn tokenize_args_single_quotes() {
        let tests = [
            ("echo 'hello world'", vec!["echo", "hello world"]),
            ("'echo' 'hello world'", vec!["echo", "hello world"]),
            (
                "echo hi'hello world'there rust",
                vec!["echo", "hihello worldthere", "rust"],
            ),
        ];
        for (input, expected) in tests {
            test_tokenized_values(input, &expected);
        }
    }

    #[test]
    fn tokenize_args_single_quotes_with_backslash() {
        let input = r"'echo' 'hello\ world'";
        let expected = &["echo", r"hello\ world"];
        test_tokenized_values(input, expected);
    }

    #[test]
    fn tokenize_args_double_quotes() {
        let input = r#"echo "hello world""#;
        let expected = &["echo", "hello world"];
        test_tokenized_values(input, expected);
    }

    #[test]
    fn tokenize_args_double_quotes_with_backslash() {
        let tests = [
            (r#"echo "hello\\ \world""#, vec!["echo", r#"hello\ \world"#]),
            (
                r#"echo hi"hello'script'\n'world""#,
                vec!["echo", r#"hihello'script'\n'world"#],
            ),
            (
                r#"echo "inside\"quotes\"""#,
                vec!["echo", r#"inside"quotes""#],
            ),
        ];
        for (input, expected) in tests {
            test_tokenized_values(input, &expected);
        }
    }

    #[test]
    fn tokenize_args_backslash_outside() {
        let tests = [
            (r"echo hello\ \ \ world", vec!["echo", "hello   world"]),
            (r"echo hello\     world", vec!["echo", "hello ", "world"]),
        ];
        for (input, expected) in tests {
            test_tokenized_values(input, &expected);
        }
    }
}
