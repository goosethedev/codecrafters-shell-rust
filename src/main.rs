mod command;
mod jobs;
mod lexer;

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Output;

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::error::ReadlineError;
use rustyline::history::{FileHistory, History};
use rustyline::{CompletionType, Config, Editor, Helper, Highlighter, Hinter, Validator};
use trie_rs::{Trie, TrieBuilder};

use crate::jobs::JobTable;

const BUILTINS: &[&str] = &[
    "echo", "type", "exit", "pwd", "cd", "complete", "jobs", "history",
];

#[derive(Helper, Validator, Hinter, Highlighter)]
struct ShellHelper {
    path_execs: Trie<u8>,
    filename_completer: FilenameCompleter,
    completion_scripts: HashMap<String, PathBuf>,
}

impl ShellHelper {
    fn get_completion_script(&self, command: &str) -> Option<&PathBuf> {
        self.completion_scripts.get(command)
    }

    fn register_custom_completion(&mut self, command: &str, script_path: &str) {
        let script = PathBuf::from(script_path);
        self.completion_scripts.insert(command.to_string(), script);
    }

    fn unregister_custom_completion(&mut self, command: &str) {
        let _ = self.completion_scripts.remove(command);
    }

    fn get_custom_completions(
        &self,
        command: &str,
        partial: &str,
        previous: &str,
        full_line: &str,
        pos: usize,
    ) -> Option<Vec<String>> {
        let script = self.get_completion_script(command)?;
        let output = std::process::Command::new(script)
            .args([command, partial, previous])
            .env("COMP_LINE", full_line)
            .env("COMP_POINT", pos.to_string())
            .output()
            .unwrap();
        Some(
            String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .map(ToString::to_string)
                .collect(),
        )
    }
}

impl Completer for ShellHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &rustyline::Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Self::Candidate>)> {
        let start = line[..pos]
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = &line[start..pos];

        // If completing the first word, search for binaries
        if start == 0 {
            let matches = self
                .path_execs
                .predictive_search(word)
                .map(|res: String| Pair {
                    display: res.clone(),
                    replacement: format!("{res} "),
                })
                .collect();
            return Ok((start, matches));
        };

        // Else, try getting custom completions for the binary
        let (bin, _) = line.split_once(" ").unwrap();
        let prev = line[..start]
            .trim_end()
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let prev = &line[prev..start].trim_end();
        if let Some(completions) = self.get_custom_completions(bin, word, prev, line, pos) {
            let matches = completions
                .iter()
                .filter(|c| c.starts_with(word))
                .map(|c| Pair {
                    display: c.clone(),
                    replacement: format!("{c} "),
                })
                .collect();
            return Ok((start, matches));
        }

        // Else, search for filenames
        let mut res = self.filename_completer.complete(line, pos, ctx)?;

        // Append a space to not-dir completions and a "/" to dir list alternatives
        for pair in res.1.iter_mut() {
            let filepath = std::env::current_dir().unwrap().join(&pair.replacement);
            if filepath.is_file() {
                pair.replacement.push(' ');
            } else {
                pair.display.push('/');
            }
        }
        Ok(res)
    }
}

// NOTE: Saving history to file uses a custom iterator to avoid setting the #V2 header
// that makes CodeCrafters' tests fail.
fn main() -> anyhow::Result<()> {
    let config = Config::builder()
        .history_ignore_dups(false)?
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .bell_style(rustyline::config::BellStyle::Audible)
        .edit_mode(rustyline::EditMode::Emacs)
        .build();

    let mut rl = Editor::<ShellHelper, FileHistory>::with_config(config)?;

    rl.set_helper(Some(ShellHelper {
        path_execs: scan_path_executables(),
        filename_completer: FilenameCompleter::new(),
        completion_scripts: HashMap::new(),
    }));

    let mut jobs = JobTable::new();

    // Try to load history from HISTFILE
    let saved_count = if let Some(hist_path) = std::env::var_os("HISTFILE") {
        let _ = rl.load_history(&hist_path);
        rl.history().len()
    } else {
        0
    };

    loop {
        match rl.readline("$ ") {
            Ok(line) => {
                rl.add_history_entry(&line).unwrap();
                let Some(cmd) = crate::command::Command::parse(&line) else {
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
                            format!("cd: {}: No such file or directory", path.to_str().unwrap())
                                .as_bytes(),
                        ),
                        None => cmd.writeln_stderr("Error: argument required".as_bytes()),
                    },
                    "complete" => {
                        let mut args = cmd.args.iter();
                        if let Some(opt) = args.next() {
                            match opt.as_str() {
                                "-p" => {
                                    let bin = args.next().unwrap();
                                    if let Some(script) =
                                        rl.helper().unwrap().get_completion_script(bin)
                                    {
                                        let script = script.to_str().unwrap();
                                        let msg = format!("complete -C '{script}' {bin}");
                                        cmd.writeln_stdout(&msg);
                                    } else {
                                        cmd.writeln_stderr(format!(
                                            "complete: {bin}: no completion specification"
                                        ));
                                    };
                                }
                                "-C" => {
                                    let script_path = args.next().unwrap();
                                    let bin = args.next().unwrap();
                                    rl.helper_mut()
                                        .unwrap()
                                        .register_custom_completion(bin, script_path);
                                }
                                "-r" => {
                                    let bin = args.next().unwrap();
                                    rl.helper_mut().unwrap().unregister_custom_completion(bin);
                                }
                                _ => {}
                            }
                        }
                    }
                    "jobs" => {
                        jobs.poll();
                        jobs.list_and_reap(false);
                    }
                    "history" => {
                        let mut args = cmd.args.iter();
                        match args.next().map(|v| v.as_str()) {
                            Some("-r") => {
                                // Read from file
                                let path = args.next().unwrap();
                                let _ = rl.load_history(path);
                            }
                            Some("-w") => {
                                use std::io::Write;
                                // Write to file
                                let path = args.next().unwrap();
                                let file = std::fs::File::create(path).unwrap();
                                let mut writer = std::io::BufWriter::new(file);
                                for entry in rl.history() {
                                    writeln!(writer, "{entry}").unwrap();
                                }
                            }
                            Some(n) => {
                                // List last n
                                let Ok(n) = n.parse::<usize>() else {
                                    eprintln!("history: numeric argument required");
                                    continue;
                                };
                                let n_skip = rl.history().len() - n;
                                for (i, entry) in rl.history().iter().skip(n_skip).enumerate() {
                                    println!("{:>5}  {entry}", n_skip + i + 1);
                                }
                            }
                            None => {
                                // List all
                                for (i, entry) in rl.history().iter().enumerate() {
                                    println!("{:>5}  {entry}", i + 1);
                                }
                            }
                        };
                    }
                    "exit" => {
                        // Try to save history from HISTFILE
                        if let Some(hist_path) = std::env::var_os("HISTFILE") {
                            use std::io::Write;
                            let file = std::fs::OpenOptions::new()
                                .create(true)
                                .append(true)
                                .open(hist_path)
                                .unwrap();
                            let mut writer = std::io::BufWriter::new(file);
                            for entry in rl.history().iter().skip(saved_count) {
                                writeln!(writer, "{entry}").unwrap();
                            }
                        }

                        break;
                    }
                    bin => match search_bin_in_path(bin) {
                        Some(bin) => {
                            if cmd.bg {
                                jobs.spawn_background(cmd).unwrap();
                                continue;
                            }
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
            Err(e) => {
                eprintln!("{e}");
                break;
            }
        };

        jobs.poll();
        jobs.list_and_reap(true);
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
