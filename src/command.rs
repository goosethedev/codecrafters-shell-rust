use std::io::Write;

use crate::lexer::{Lexer, Token, WriteMode};

// TODO: move execution as a method and not expose the struct fields
// TODO: parse options in any position of the command
pub struct Command {
    pub bin: String,
    pub args: Vec<String>,
    pub stdout: Option<std::fs::File>,
    pub stderr: Option<std::fs::File>,
}

impl Command {
    pub fn parse(input: &str) -> Option<Self> {
        let mut tokens = Lexer::new(input).tokenize().ok()?.into_iter();
        let Token::Value(bin) = tokens.next()? else {
            return None;
        };
        let mut args = vec![];
        let mut stdout = None;
        let mut stderr = None;

        while let Some(t) = tokens.next() {
            match t {
                Token::Value(s) => args.push(s),
                Token::Stdout(mode) => {
                    let Token::Value(file) = tokens.next()? else {
                        return None;
                    };
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
                    let Token::Value(file) = tokens.next()? else {
                        return None;
                    };
                    let file = std::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .append(mode == WriteMode::Append)
                        .truncate(mode == WriteMode::Redirect)
                        .open(file)
                        .unwrap();
                    stderr = Some(file);
                }
            }
        }
        Some(Self {
            bin,
            args,
            stderr,
            stdout,
        })
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
