#![allow(dead_code)]

pub enum Token {
    Value(String),
    Stdout(WriteMode),
    Stderr(WriteMode),
}

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
        while let Some(ch) = self.chars.peek() {
            match *ch {
                // TODO: parse quoted values and push to the string
                // '\'' => {},
                // '"' => {},
                ch if ch.is_ascii_whitespace() => break,
                _ => buf.push(self.chars.next().unwrap()),
            }
        }
        Some(Token::Value(buf))
    }
}
