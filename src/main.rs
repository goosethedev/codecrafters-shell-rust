mod command;
mod lexer;

use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Output;

use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::history::DefaultHistory;
use rustyline::{CompletionType, Config, Editor, Helper, Highlighter, Hinter, Validator};
use trie_rs::{Trie, TrieBuilder};

const BUILTINS: &[&str] = &["echo", "type", "exit", "pwd", "cd"];

#[derive(Helper, Validator, Hinter, Highlighter)]
struct ShellHelper {
    path_execs: Trie<u8>,
}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let start = line[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = &line[start..pos];

        // Only autocomplete the first word
        if start != 0 {
            return Ok((start, vec![]));
        }

        let matches = self
            .path_execs
            .predictive_search(word)
            .map(|res: String| Pair {
                display: res.clone(),
                replacement: format!("{res} "),
            })
            .collect();

        Ok((start, matches))
    }
}

fn main() -> anyhow::Result<()> {
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .bell_style(rustyline::config::BellStyle::Audible)
        .edit_mode(rustyline::EditMode::Emacs)
        .build();
    let mut rl = Editor::<ShellHelper, DefaultHistory>::with_config(config)?;

    let path_execs = scan_path_executables();
    rl.set_helper(Some(ShellHelper { path_execs }));

    loop {
        match rl.readline("$ ") {
            Ok(line) => {
                let Some(cmd) = crate::command::Command::parse(&line) else {
                    break;
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
                            format!("cd: {}: No such file or directory", path.to_str().unwrap())
                                .as_bytes(),
                        ),
                        None => cmd.writeln_stderr("Error: argument required".as_bytes()),
                    },
                    "exit" => break,
                    bin => match search_bin_in_path(bin) {
                        Some(bin) => {
                            match execute_command(bin.file_name().unwrap(), cmd.args.clone()) {
                                Ok(Output { stdout, stderr, .. }) => {
                                    cmd.write_stdout(&stdout);
                                    cmd.write_stderr(&stderr);
                                }
                                Err(e) => cmd.writeln_stderr(e.to_string().as_bytes()),
                            }
                        }
                        None => cmd.writeln_stderr(format!("{bin}: command not found").as_bytes()),
                    },
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(_) => break,
        };
    }
    Ok(())
}

fn scan_path_executables() -> Trie<u8> {
    let mut builder = TrieBuilder::new();
    // First, include all built-ins
    BUILTINS.iter().for_each(|b| builder.push(b));

    let path_var = std::env::var_os("PATH").expect("Unable to find PATH variable");
    for dir in std::env::split_paths(&path_var) {
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let bin = entry.path();
            if is_executable(&bin) {
                builder.push(bin.file_name().unwrap().as_bytes());
            }
        }
    }
    builder.build()
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

fn is_executable(path: impl AsRef<Path>) -> bool {
    use std::os::unix::fs::PermissionsExt;

    match path.as_ref().metadata() {
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
