mod jobs;
mod lexer;
mod parser;
mod readline;
mod utils;

use std::collections::HashMap;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rustyline::error::ReadlineError;
use rustyline::history::History;

use crate::jobs::JobTable;
use crate::lexer::Lexer;
use crate::parser::{Instruction, Parser, ParserOutput};
use crate::readline::{LineEditor, get_line_editor};
use crate::utils::is_executable;

pub(crate) const BUILTINS: &[&str] = &[
    "echo", "type", "exit", "pwd", "cd", "complete", "jobs", "history", "declare",
];

pub type VarStore = HashMap<String, String>;

#[derive(Default)]
struct ShellState {
    jobs: JobTable,
    var_store: VarStore,
}

fn main() -> anyhow::Result<()> {
    // If args are "-c command", execute it immediately and exit
    let args: Vec<_> = std::env::args().collect();
    if args.get(1) == Some(&"-c".to_string())
        && let Some(line) = args.get(2)
        && !line.is_empty()
    {
        let mut state = ShellState::default();
        let tokens = Lexer::new(line).tokenize()?;

        match Parser::new(tokens, &state.var_store).parse()? {
            ParserOutput::Instruction(i, _) if i.cmd == "exit" => return Ok(()),
            ParserOutput::Instruction(i, bg) => exec_instruction(i, bg, &mut state, None)?,
            ParserOutput::Pipeline(p, bg) => exec_pipeline(p, bg, None)?,
        }
        return Ok(());
    }

    // Configured Rustyline line editor
    let mut rl = get_line_editor()?;

    // Shell state
    let mut state = ShellState::default();

    // Main exec loop
    loop {
        match rl.readline("$ ") {
            Ok(line) => {
                if line.is_empty() {
                    continue;
                }

                // Add command to history
                rl.add_history_entry(&line).unwrap();

                // Parse and execute instruction/pipeline
                let tokens = match Lexer::new(&line).tokenize() {
                    Ok(t) => t,
                    Err(e) => {
                        eprintln!("{e}");
                        continue;
                    }
                };
                match Parser::new(tokens, &state.var_store).parse() {
                    Ok(ParserOutput::Instruction(i, _)) if i.cmd == "exit" => break,
                    Ok(ParserOutput::Instruction(i, bg)) => {
                        if let Err(e) = exec_instruction(i, bg, &mut state, Some(&mut rl)) {
                            eprintln!("{e}")
                        }
                    }
                    Ok(ParserOutput::Pipeline(p, bg)) => {
                        if let Err(e) = exec_pipeline(p, bg, None) {
                            eprintln!("{e}")
                        }
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        continue;
                    }
                }
            }
            Err(ReadlineError::Interrupted) => continue,
            Err(e) => {
                eprintln!("{e}");
                break;
            }
        };

        // Poll and reap finished bg jobs
        state.jobs.poll();
        state.jobs.list_and_reap(true);
    }

    // Try to save history from HISTFILE
    if let Some(hist_path) = std::env::var_os("HISTFILE") {
        let saved_count = rl.helper().unwrap().hist_saved_in_file;
        let _ = write_history(&mut rl, hist_path, saved_count, true);
    }

    Ok(())
}

fn exec_instruction(
    instruction: Instruction,
    bg: bool,
    state: &mut ShellState,
    rl: Option<&mut LineEditor>,
) -> anyhow::Result<()> {
    // Setup redirections for std streams
    let mut stdout: Box<dyn Write> = match instruction.stdout {
        Some(ref out) => Box::new(out),
        None => Box::new(io::stdout()),
    };
    let mut stderr: Box<dyn Write> = match instruction.stderr {
        Some(ref err) => Box::new(err),
        None => Box::new(io::stderr()),
    };

    match instruction.cmd.as_str() {
        "type" => {
            for arg in instruction.args {
                if BUILTINS.contains(&arg.as_str()) {
                    writeln!(stdout, "{arg} is a shell builtin")?
                } else if let Some(bin_path) = search_bin_in_path(&arg) {
                    writeln!(stdout, "{arg} is {}", bin_path.to_str().unwrap())?
                } else {
                    writeln!(stderr, "{arg}: not found")?
                };
            }
        }
        "echo" => writeln!(stdout, "{}", instruction.args.join(" "))?,
        "pwd" => {
            let curr_dir = std::env::current_dir().unwrap();
            writeln!(stdout, "{}", curr_dir.to_str().unwrap())?;
        }
        "cd" => match instruction.args.first().map(PathBuf::from) {
            Some(path) if path.is_dir() => {
                std::env::set_current_dir(path).expect("Error changing working dir");
            }
            Some(path) if path.starts_with("~") => {
                let home = std::env::var("HOME").expect("HOME var not found");
                let path = path.to_string_lossy().replace("~", &home);
                std::env::set_current_dir(path).expect("Error changing working dir");
            }
            Some(path) => {
                let path = path.to_str().unwrap();
                writeln!(stderr, "cd: {path}: No such file or directory",)?;
            }
            None => writeln!(stderr, "Error: argument required")?,
        },
        "complete" => {
            if let Some(rl) = rl {
                let shell = rl.helper_mut().unwrap();
                let mut args = instruction.args.iter();
                if let Some(opt) = args.next() {
                    match opt.as_str() {
                        "-p" => {
                            let bin = args.next().unwrap();
                            if let Some(script) = shell.get_completion_script(bin) {
                                let script = script.to_str().unwrap();
                                writeln!(stdout, "complete -C '{script}' {bin}")?;
                            } else {
                                writeln!(stderr, "complete: {bin}: no completion specification")?;
                            };
                        }
                        "-C" => {
                            let script_path = args.next().unwrap();
                            let bin = args.next().unwrap();
                            shell.register_custom_completion(bin, script_path);
                        }
                        "-r" => {
                            let bin = args.next().unwrap();
                            shell.unregister_custom_completion(bin);
                        }
                        _ => {}
                    }
                }
            }
        }
        "jobs" => {
            state.jobs.poll();
            state.jobs.list_and_reap(false);
        }
        "history" => {
            if let Some(rl) = rl {
                let mut args = instruction.args.iter();
                match args.next().map(|v| v.as_str()) {
                    Some("-r") => {
                        // Read from file
                        let path = args.next().unwrap();
                        let _ = rl.load_history(path);
                    }
                    Some("-w") => {
                        // Write to file
                        let path = args.next().unwrap();
                        let _ = write_history(rl, path, 0, false);
                    }
                    Some("-a") => {
                        // Append to file
                        let path = args.next().unwrap();
                        let last_append_at = rl.helper_mut().unwrap().hist_last_appended_at;
                        let _ = write_history(rl, path, last_append_at, true);
                        rl.helper_mut().unwrap().hist_last_appended_at = rl.history().len();
                    }
                    Some(n) => {
                        // List last n
                        let Ok(n) = n.parse::<usize>() else {
                            writeln!(stderr, "history: numeric argument required")?;
                            return Ok(());
                        };
                        let n_skip = rl.history().len() - n;
                        for (i, entry) in rl.history().iter().skip(n_skip).enumerate() {
                            writeln!(stdout, "{:>5}  {entry}", n_skip + i + 1)?;
                        }
                    }
                    None => {
                        // List all
                        for (i, entry) in rl.history().iter().enumerate() {
                            writeln!(stdout, "{:>5}  {entry}", i + 1)?;
                        }
                    }
                };
            }
        }
        "declare" => {
            let mut args = instruction.args.iter();
            match args.next().unwrap().as_str() {
                "-p" => {
                    let var_name = args.next().unwrap();
                    match state.var_store.get(var_name) {
                        Some(v) => writeln!(stdout, r#"declare -- {var_name}="{v}""#)?,
                        None => writeln!(stderr, "declare: {var_name}: not found")?,
                    };
                }
                v => {
                    let (var_name, value) = v.split_once("=").unwrap();
                    if !is_valid_identifier(var_name) {
                        writeln!(stderr, r"declare: `{v}': not a valid identifier")?;
                        return Ok(());
                    }
                    state
                        .var_store
                        .insert(var_name.to_string(), value.to_string());
                }
            }
        }
        "exit" => {}
        bin => match search_bin_in_path(bin) {
            Some(_) => {
                drop(stdout);
                drop(stderr);
                // Execute external binary in child process
                // TODO: There has to be a better way to do this...
                if bg {
                    state.jobs.spawn_background(instruction)?;
                } else {
                    Command::new(bin)
                        .args(instruction.args)
                        .stdout(match instruction.stdout {
                            Some(f) => Stdio::from(f),
                            None => Stdio::inherit(),
                        })
                        .stderr(match instruction.stderr {
                            Some(f) => Stdio::from(f),
                            None => Stdio::inherit(),
                        })
                        .spawn()?
                        .wait()?;
                }
            }
            None => writeln!(stderr, "{bin}: command not found")?,
        },
    }

    Ok(())
}

fn exec_pipeline(
    pipeline: Vec<Instruction>,
    _bg: bool,
    _rl: Option<&mut LineEditor>,
) -> anyhow::Result<()> {
    let len = pipeline.len();
    let mut children = Vec::with_capacity(len);
    let mut prev_stdout: Option<std::process::ChildStdout> = None;

    for (i, stage) in pipeline.into_iter().enumerate() {
        let is_first = i == 0;
        let is_last = i == len - 1;
        let is_builtin = BUILTINS.contains(&stage.cmd.as_str());

        let mut cmd = if is_builtin {
            let subcommand = format!("{} {}", stage.cmd, stage.args.join(" "));
            let mut cmd = Command::new(std::env::current_exe()?);
            cmd.args(["-c", &subcommand]);
            cmd
        } else {
            let mut cmd = Command::new(&stage.cmd);
            cmd.args(&stage.args);
            cmd
        };

        // Connect stdin to the preceding stage.
        let stdin = match prev_stdout.take() {
            Some(stdout) => Stdio::from(stdout),
            None if is_first => Stdio::inherit(),
            None => Stdio::null(),
        };

        // Redirect stdout, or pipe it to the next stage.
        let stdout = match stage.stdout {
            Some(out) => Stdio::from(out),
            None if !is_last => Stdio::piped(),
            None => Stdio::inherit(),
        };

        let stderr = match stage.stderr {
            Some(err) => Stdio::from(err),
            None => Stdio::inherit(),
        };

        cmd.stdin(stdin).stdout(stdout).stderr(stderr);

        let mut child = cmd.spawn()?;
        prev_stdout = child.stdout.take();
        children.push(child);
    }

    for mut child in children {
        child.wait()?;
    }

    Ok(())
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

fn write_history(
    rl: &mut LineEditor,
    path: impl AsRef<Path>,
    skip: usize,
    append: bool,
) -> anyhow::Result<()> {
    use std::io::Write;
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(!append)
        .truncate(!append)
        .append(append)
        .open(path)
        .unwrap();
    let mut writer = std::io::BufWriter::new(file);
    for entry in rl.history().iter().skip(skip) {
        writeln!(writer, "{entry}").unwrap();
    }
    Ok(())
}

fn is_valid_identifier(value: &str) -> bool {
    let first = value.as_bytes().first().unwrap();
    (first.is_ascii_alphabetic() || *first == b'_')
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}
