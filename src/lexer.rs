#![allow(dead_code)]

#[derive(Debug, PartialEq, Eq)]
pub enum Token {
    Word(Vec<Value>),
    Stdout(WriteMode),
    Stderr(WriteMode),
    Ampersand,
    Pipe,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Value {
    Expansion(String),
    Literal(String),
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WriteMode {
    Append,
    Redirect,
}

#[derive(Debug, PartialEq, thiserror::Error)]
pub enum LexError {
    #[error("quoted string was not closed")]
    UnterminatedString,
    #[error("invalid escaped character")]
    InvalidEscape,
}

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            pos: 0,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Token>, LexError> {
        self.skip_whitespace();
        let mut tokens = vec![];

        while self.peek().is_some() {
            let Some(token) = self
                .tokenize_redir()
                .or_else(|| self.tokenize_single_char())
                .or_else(|| self.tokenize_word())
            else {
                break;
            };
            tokens.push(token);
            self.skip_whitespace();
        }

        Ok(tokens)
    }

    // FIXME: this fails if only two characters are left (incomplete redir/append)
    fn tokenize_redir(&mut self) -> Option<Token> {
        let (token, n) = match self.peek_n(3)? {
            [b'1', b'>', b'>'] => (Token::Stdout(WriteMode::Append), 3),
            [b'2', b'>', b'>'] => (Token::Stderr(WriteMode::Append), 3),
            [b'1', b'>', _] => (Token::Stdout(WriteMode::Redirect), 2),
            [b'2', b'>', _] => (Token::Stderr(WriteMode::Redirect), 2),
            [b'>', b'>', _] => (Token::Stdout(WriteMode::Append), 2),
            [b'>', _, _] => (Token::Stdout(WriteMode::Redirect), 1),
            _ => return None,
        };
        self.advance(n);
        Some(token)
    }

    fn tokenize_word(&mut self) -> Option<Token> {
        let mut values = vec![];
        let mut buf = String::new();
        while let Some(ch) = self.next_if(|ch| !ch.is_ascii_whitespace()) {
            match ch {
                '$' => {
                    if !buf.is_empty() {
                        values.push(Value::Literal(std::mem::take(&mut buf)));
                    }
                    let var_name = if self.next_if_eq('{').is_some() {
                        self.take_until_closing('}')
                    } else {
                        self.take_while(|ch| !ch.is_ascii_whitespace())
                    };
                    values.push(Value::Expansion(var_name));
                }
                '\'' => {
                    let value = self.take_until_closing('\'');
                    buf.push_str(&value);
                }
                '"' => {
                    while let Some(ch) = self.next_if(|ch| ch != '"') {
                        match ch {
                            '\\' => match self.next() {
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
                    if self.next_if_eq('"').is_none() {
                        unreachable!("unclosed double-quoted string");
                    }
                }
                '\\' => {
                    if let Some(ch) = self.next() {
                        buf.push(ch);
                    } else {
                        unreachable!("\\ found at the end of the string")
                    }
                }
                ch => buf.push(ch),
            }
        }
        if !buf.is_empty() {
            values.push(Value::Literal(std::mem::take(&mut buf)));
        }

        Some(Token::Word(values))
    }

    fn tokenize_single_char(&mut self) -> Option<Token> {
        let token = match self.peek()? {
            '&' => Token::Ampersand,
            '|' => Token::Pipe,
            _ => return None,
        };
        self.next().unwrap();
        Some(token)
    }

    fn take_until_closing(&mut self, delim: char) -> String {
        let value = self.take_while(|ch| ch != delim);
        // Consume closing quote
        if self.next_if_eq(delim).is_none() {
            unreachable!("missing unclosed delimiter: {delim}");
        }
        value
    }

    fn take_while(&mut self, f: impl Fn(char) -> bool) -> String {
        let mut buf = String::new();
        while let Some(ch) = self.next_if(&f) {
            buf.push(ch);
        }
        buf
    }

    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(ch) if ch.is_ascii_whitespace()) {
            self.pos += 1;
        }
    }

    fn advance(&mut self, n: usize) {
        self.pos += n;
    }

    fn next(&mut self) -> Option<char> {
        let ch = self.bytes.get(self.pos)?;
        self.pos += 1;
        Some(*ch as char)
    }

    fn next_if(&mut self, f: impl FnOnce(char) -> bool) -> Option<char> {
        self.peek()
            .and_then(|ch| f(ch).then(|| self.next().unwrap()))
    }

    fn next_if_eq(&mut self, val: char) -> Option<char> {
        self.peek()
            .and_then(|ch| ch.eq(&val).then(|| self.next().unwrap()))
    }

    fn peek(&self) -> Option<char> {
        Some(self.bytes.get(self.pos).copied()? as char)
    }

    fn peek_n(&self, n: usize) -> Option<&[u8]> {
        self.bytes.get(self.pos..self.pos + n)
    }
}

#[cfg(test)]
mod tests {
    use super::Token::*;
    use super::Value::*;
    use super::WriteMode::*;
    use super::*;

    fn test_tokenized(input: &str, expected: &[Token]) {
        let actual = Lexer::new(input).tokenize();
        assert_eq!(Ok(expected), actual.as_deref(), "for input: {input}");
    }

    fn test_tokenized_values(input: &str, expected: &[&str]) {
        let actual = Lexer::new(input).tokenize();
        let expected: Vec<_> = expected
            .into_iter()
            .map(|v| Word(vec![Literal(v.to_string())]))
            .collect();
        assert_eq!(Ok(expected), actual, "for input: {input}");
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

    #[test]
    fn tokenize_args_redir() {
        let tests = [
            (
                r"hello > file.txt",
                vec![
                    Word(vec![Literal("hello".to_string())]),
                    Stdout(Redirect),
                    Word(vec![Literal("file.txt".to_string())]),
                ],
            ),
            (
                r"hello >>file.txt",
                vec![
                    Word(vec![Literal("hello".to_string())]),
                    Stdout(Append),
                    Word(vec![Literal("file.txt".to_string())]),
                ],
            ),
            (
                r"hello 1> file.txt",
                vec![
                    Word(vec![Literal("hello".to_string())]),
                    Stdout(Redirect),
                    Word(vec![Literal("file.txt".to_string())]),
                ],
            ),
            (
                r"hello 2> file.txt",
                vec![
                    Word(vec![Literal("hello".to_string())]),
                    Stderr(Redirect),
                    Word(vec![Literal("file.txt".to_string())]),
                ],
            ),
            (
                r"hello 1>>file.txt",
                vec![
                    Word(vec![Literal("hello".to_string())]),
                    Stdout(Append),
                    Word(vec![Literal("file.txt".to_string())]),
                ],
            ),
            (
                r"hello 2>>file.txt",
                vec![
                    Word(vec![Literal("hello".to_string())]),
                    Stderr(Append),
                    Word(vec![Literal("file.txt".to_string())]),
                ],
            ),
            (
                r"hello 2 >file.txt",
                vec![
                    Word(vec![Literal("hello".to_string())]),
                    Word(vec![Literal("2".to_string())]),
                    Stdout(Redirect),
                    Word(vec![Literal("file.txt".to_string())]),
                ],
            ),
        ];
        for (input, expected) in tests {
            test_tokenized(input, &expected);
        }
    }

    #[test]
    fn tokenize_expansions() {
        let tests = [(
            "echo $GREETING hello$NAME ${EXCL}1${EXCL}$EXCL $EXCL$NAME",
            vec![
                Word(vec![Literal("echo".to_string())]),
                Word(vec![Expansion("GREETING".to_string())]),
                Word(vec![
                    Literal("hello".to_string()),
                    Expansion("NAME".to_string()),
                ]),
                Word(vec![
                    Expansion("EXCL".to_string()),
                    Literal("1".to_string()),
                    Expansion("EXCL".to_string()),
                    Expansion("EXCL".to_string()),
                ]),
                Word(vec![Expansion("EXCL$NAME".to_string())]),
            ],
        )];
        for (input, expected) in tests {
            test_tokenized(input, &expected);
        }
    }
}
