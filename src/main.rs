mod lexer;

use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Output;

use crate::lexer::{Lexer, Token};

const BUILTINS: &[&str] = &["echo", "type", "exit", "pwd"];

struct Command {
    bin: String,
    args: Vec<String>,
}

impl Command {
    fn parse(input: &str) -> Option<Self> {
        let mut tokens = Lexer::new(input).into_iter();
        let Token::Value(bin) = tokens.next()? else {
            return None;
        };
        let mut args = vec![];

        for t in tokens {
            match t {
                Token::Value(s) => args.push(s),
                _ => {}
            }
        }
        Some(Self { bin, args })
    }
}

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut buf = String::new();

    loop {
        print!("$ ");
        std::io::stdout().flush().unwrap();
        stdin.read_line(&mut buf)?;

        let Some(Command { bin, args }) = Command::parse(&buf) else {
            continue;
        };

        match bin.as_str() {
            "type" => {
                let param = args.first().unwrap();
                if BUILTINS.contains(&param.as_str()) {
                    println!("{param} is a shell builtin");
                } else if let Some(bin_path) = search_bin_in_path(&param) {
                    println!("{param} is {}", bin_path.to_str().unwrap());
                } else {
                    println!("{param}: not found");
                }
            }
            "echo" => {
                let args = args.join(" ");
                println!("{args}");
            }
            "pwd" => println!("{}", std::env::current_dir().unwrap().to_str().unwrap()),
            "cd" => match args.first().map(PathBuf::from) {
                Some(path) if path.is_dir() => {
                    std::env::set_current_dir(path).expect("Error changing working dir");
                }
                Some(path) if path.starts_with("~") => {
                    let home = std::env::var("HOME").expect("HOME var not found");
                    let path = path.to_string_lossy().replace("~", &home);
                    std::env::set_current_dir(path).expect("Error changing working dir");
                }
                Some(path) => println!("cd: {}: No such file or directory", path.to_str().unwrap()),
                None => eprintln!("Error: argument required"),
            },
            "exit" => break Ok(()),
            bin => match search_bin_in_path(bin) {
                Some(bin) => {
                    match execute_command(bin.file_name().unwrap(), args) {
                        Ok(Output { stdout, .. }) => {
                            let stdout = String::from_utf8(stdout).unwrap();
                            // let stderr = String::from_utf8(stderr).unwrap();
                            print!("{stdout}");
                            // eprintln!("{stderr}");
                        }
                        Err(e) => eprintln!("{e}"),
                    }
                }
                None => println!("{bin}: command not found"),
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
