use std::fs::File;
use std::path::Path;

use crate::VarStore;
use crate::lexer::{Token, Value, WriteMode};

#[derive(Debug, Default)]
pub struct Instruction {
    pub cmd: String,
    pub args: Vec<String>,
    pub stdout: Option<File>,
    pub stderr: Option<File>,
}

impl Instruction {
    pub fn prettify(&self) -> String {
        let args = self.args.join(" ");
        format!("{} {}", self.cmd, args)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("syntax error near unexpected token")]
    UnexpectedToken,
    #[error("couldn't open file")]
    OpenFileError(#[from] std::io::Error),
    #[error("ambiguous redirect")]
    AmbiguousRedir,
}

#[derive(Debug)]
pub struct Parser<'a> {
    tokens: Vec<Token>,
    kv: &'a VarStore,
}

#[derive(Debug)]
pub enum ParserOutput {
    Instruction(Instruction, bool),
    Pipeline(Vec<Instruction>, bool),
}

impl<'a> Parser<'a> {
    pub fn new(tokens: Vec<Token>, kv: &'a VarStore) -> Self {
        Self { tokens, kv }
    }

    pub fn parse(self) -> Result<ParserOutput, ParseError> {
        let mut pipeline = vec![];
        let mut instruction = Instruction::default();
        let mut is_bg_job = false;

        let mut tokens = self.tokens.iter();
        while let Some(t) = tokens.next() {
            match t {
                Token::Word(values) => {
                    if let Some(word) = self.resolve_word(values) {
                        if instruction.cmd.is_empty() {
                            instruction.cmd = word;
                        } else {
                            instruction.args.push(word);
                        }
                    }
                }
                Token::Stdout(mode) => {
                    let Some(Token::Word(values)) = tokens.next() else {
                        return Err(ParseError::UnexpectedToken);
                    };
                    let filepath = self
                        .resolve_word(values)
                        .ok_or(ParseError::AmbiguousRedir)?;
                    instruction.stdout = Some(open_file(filepath, mode)?);
                }
                Token::Stderr(mode) => {
                    let Some(Token::Word(values)) = tokens.next() else {
                        return Err(ParseError::UnexpectedToken);
                    };
                    let filepath = self
                        .resolve_word(values)
                        .ok_or(ParseError::AmbiguousRedir)?;
                    instruction.stderr = Some(open_file(filepath, mode)?);
                }
                Token::Ampersand => {
                    is_bg_job = true;
                    if tokens.next().is_some() {
                        return Err(ParseError::UnexpectedToken);
                    }
                }
                Token::Pipe => {
                    if instruction.cmd.is_empty() {
                        return Err(ParseError::UnexpectedToken);
                    }
                    pipeline.push(std::mem::take(&mut instruction));
                }
            }
        }
        if !instruction.cmd.is_empty() {
            pipeline.push(instruction);
        }
        Ok(if pipeline.len() == 1 {
            let instruction = pipeline.into_iter().next().unwrap();
            ParserOutput::Instruction(instruction, is_bg_job)
        } else {
            ParserOutput::Pipeline(pipeline, is_bg_job)
        })
    }

    fn resolve_word(&self, values: &Vec<Value>) -> Option<String> {
        let mut word = String::new();
        for v in values {
            match v {
                Value::Literal(s) => word.push_str(s),
                Value::Expansion(k) => {
                    if let Some(v) = self.kv.get(k) {
                        word.push_str(v);
                    } else if let Some(v) = std::env::var_os(k) {
                        word.push_str(&v.to_string_lossy());
                    }
                }
            }
        }

        if word.is_empty() { None } else { Some(word) }
    }
}

#[inline]
fn open_file(path: impl AsRef<Path>, mode: &WriteMode) -> std::io::Result<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .append(*mode == WriteMode::Append)
        .truncate(*mode == WriteMode::Redirect)
        .open(path)
}
