mod lexer;

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;

use crate::lexer::{Lexer, Token, WriteMode};

const BUILTINS: &[&str] = &["echo", "type", "exit", "pwd"];

struct Command {
    bin: String,
    args: Vec<String>,
    stdout: Option<std::fs::File>,
    stderr: Option<std::fs::File>,
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

    pub fn writeln_stdout(&self, bytes: &[u8]) {
        self.write_stdout(&[bytes, &[b'\n']].concat());
    }

    pub fn writeln_stderr(&self, bytes: &[u8]) {
        self.write_stderr(&[bytes, &[b'\n']].concat());
    }

    pub fn write_stdout(&self, bytes: &[u8]) {
        if let Some(ref file) = self.stdout {
            let mut writer = std::io::BufWriter::new(file);
            writer.write_all(bytes).unwrap();
        } else {
            print!("{}", String::from_utf8_lossy(bytes));
        }
    }

    pub fn write_stderr(&self, bytes: &[u8]) {
        if let Some(ref file) = self.stderr {
            let mut writer = std::io::BufWriter::new(file);
            writer.write_all(bytes).unwrap();
        } else {
            eprint!("{}", String::from_utf8_lossy(bytes));
        }
    }
}

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut buf = String::new();

    loop {
        print!("$ ");
        std::io::stdout().flush().unwrap();
        stdin.read_line(&mut buf)?;

        let Some(cmd) = Command::parse(&buf) else {
            continue;
        };

        match cmd.bin.as_str() {
            "type" => {
                let param = cmd.args.first().unwrap();
                let out = if BUILTINS.contains(&param.as_str()) {
                    format!("{param} is a shell builtin")
                } else if let Some(bin_path) = search_bin_in_path(param) {
                    format!("{param} is {}", bin_path.to_str().unwrap())
                } else {
                    format!("{param}: not found")
                };
                cmd.writeln_stdout(out.as_bytes());
            }
            "echo" => cmd.writeln_stdout(cmd.args.join(" ").as_bytes()),
            "pwd" => cmd.writeln_stdout(
                std::env::current_dir()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .as_bytes(),
            ),
            "cd" => match cmd.args.first().map(PathBuf::from) {
                Some(path) if path.is_dir() => {
                    std::env::set_current_dir(path).expect("Error changing working dir");
                }
                Some(path) if path.starts_with("~") => {
                    let home = std::env::var("HOME").expect("HOME var not found");
                    let path = path.to_string_lossy().replace("~", &home);
                    std::env::set_current_dir(path).expect("Error changing working dir");
                }
                Some(path) => cmd.writeln_stderr(
                    format!("cd: {}: No such file or directory", path.to_str().unwrap()).as_bytes(),
                ),
                None => cmd.writeln_stderr("Error: argument required".as_bytes()),
            },
            "exit" => break Ok(()),
            bin => match search_bin_in_path(bin) {
                Some(bin) => match execute_command(bin.file_name().unwrap(), cmd.args.clone()) {
                    Ok(Output { stdout, stderr, .. }) => {
                        cmd.write_stdout(&stdout);
                        cmd.write_stderr(&stderr);
                    }
                    Err(e) => cmd.writeln_stderr(e.to_string().as_bytes()),
                },
                None => cmd.writeln_stderr(format!("{bin}: command not found").as_bytes()),
            },
        }
        buf.clear();
    }
}

fn search_bin_in_path(bin: &str) -> Option<PathBuf> {
    let path_var = std::env::var_os("PATH").expect("Unable to find PATH variable");

    for dir in std::env::split_paths(&path_var) {
        let candidate = dir.join(bin);
        if is_executable(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match path.metadata() {
        Ok(meta) if meta.is_file() => meta.permissions().mode() & 0o111 != 0,
        _ => false,
    }
}

fn execute_command(
    bin: impl AsRef<OsStr>,
    args: Vec<impl AsRef<OsStr>>,
) -> std::io::Result<Output> {
    std::process::Command::new(bin).args(args).output()
}
