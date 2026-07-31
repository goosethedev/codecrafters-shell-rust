use std::io::Write;

use crate::VarStore;
use crate::lexer::{Lexer, Token, Value, WriteMode};

// TODO: move execution as a method and not expose the struct fields
// TODO: parse options in any position of the command
pub struct Command {
    pub bin: String,
    pub args: Vec<String>,
    pub stdout: Option<std::fs::File>,
    pub stderr: Option<std::fs::File>,
    pub bg: bool,
}

impl Command {
    // TODO: proper error handling
    pub fn parse(input: &str, kv: &VarStore) -> Option<Self> {
        let mut tokens = Lexer::new(input).tokenize().ok()?.into_iter();
        let Token::Word(values) = tokens.next()? else {
            return None;
        };
        let bin = resolve_word(values, kv);
        let mut args = vec![];
        let mut stdout = None;
        let mut stderr = None;
        let mut bg = false;

        while let Some(t) = tokens.next() {
            match t {
                Token::Word(values) => {
                    let word = resolve_word(values, kv);
                    if !word.is_empty() {
                        args.push(word);
                    }
                }
                Token::Stdout(mode) => {
                    let Token::Word(values) = tokens.next()? else {
                        return None;
                    };
                    let file = resolve_word(values, kv);
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(mode == WriteMode::Append)
                        .truncate(mode == WriteMode::Redirect)
                        .open(file)
                        .unwrap();
                    stdout = Some(file);
                }
                Token::Stderr(mode) => {
                    let Token::Word(values) = tokens.next()? else {
                        return None;
                    };
                    let file = resolve_word(values, kv);
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(mode == WriteMode::Append)
                        .truncate(mode == WriteMode::Redirect)
                        .open(file)
                        .unwrap();
                    stderr = Some(file);
                }
                Token::Ampersand => bg = true,
            }
        }
        Some(Self {
            bin,
            args,
            stderr,
            stdout,
            bg,
        })
    }

    pub fn prettify(&self) -> String {
        let args = self.args.join(" ");
        format!("{} {}", self.bin, args)
    }

    pub fn writeln_stdout(&self, bytes: impl AsRef<[u8]>) {
        self.write_stdout([bytes.as_ref(), b"\n"].concat());
    }

    pub fn writeln_stderr(&self, bytes: impl AsRef<[u8]>) {
        self.write_stderr([bytes.as_ref(), b"\n"].concat());
    }

    pub fn write_stdout(&self, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();
        if let Some(ref file) = self.stdout {
            let mut writer = std::io::BufWriter::new(file);
            writer.write_all(bytes).unwrap();
        } else {
            print!("{}", String::from_utf8_lossy(bytes));
        }
    }

    pub fn write_stderr(&self, bytes: impl AsRef<[u8]>) {
        let bytes = bytes.as_ref();
        if let Some(ref file) = self.stderr {
            let mut writer = std::io::BufWriter::new(file);
            writer.write_all(bytes).unwrap();
        } else {
            eprint!("{}", String::from_utf8_lossy(bytes));
        }
    }
}

fn resolve_word(values: Vec<Value>, kv: &VarStore) -> String {
    let mut word = String::new();
    for v in values {
        match v {
            Value::Literal(s) => word.push_str(&s),
            Value::Expansion(k) => {
                if let Some(v) = kv.get(&k) {
                    word.push_str(v);
                }
            }
        }
    }
    word
}
