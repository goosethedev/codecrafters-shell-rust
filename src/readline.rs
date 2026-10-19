use std::collections::HashMap;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::history::{FileHistory, History};
use rustyline::{CompletionType, Config, Editor, Helper, Highlighter, Hinter, Validator};
use trie_rs::{Trie, TrieBuilder};

use crate::BUILTINS;
use crate::utils::is_executable;

pub(crate) type LineEditor = Editor<ShellHelper, FileHistory>;

// Get preconfigured Rustyline line editor
pub fn get_line_editor() -> rustyline::Result<LineEditor> {
    let config = Config::builder()
        .history_ignore_dups(false)?
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .bell_style(rustyline::config::BellStyle::Audible)
        .edit_mode(rustyline::EditMode::Emacs)
        .build();

    let mut rl = LineEditor::with_config(config)?;

    // Try to load history from HISTFILE
    // NOTE: Saving history to file uses a custom iterator to avoid setting the #V2 header
    // that makes CodeCrafters' tests fail.
    let hist_saved_in_file = if let Some(hist_path) = std::env::var_os("HISTFILE") {
        let _ = rl.load_history(&hist_path);
        rl.history().len()
    } else {
        0
    };

    rl.set_helper(Some(ShellHelper {
        path_execs: scan_path_executables(),
        filename_completer: FilenameCompleter::new(),
        completion_scripts: HashMap::new(),
        hist_last_appended_at: 0,
        hist_saved_in_file,
    }));

    Ok(rl)
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

#[derive(Helper, Validator, Hinter, Highlighter)]
pub struct ShellHelper {
    // Cache of executable binaries in PATH
    path_execs: Trie<u8>,
    // Completion
    filename_completer: FilenameCompleter,
    completion_scripts: HashMap<String, PathBuf>,
    // Last appended history entry index
    // Used for appending only the newest entries since last append
    pub hist_last_appended_at: usize,
    // Existing entries on hist_file
    // Used for skipping entries already in hist_file
    pub hist_saved_in_file: usize,
}

impl ShellHelper {
    pub fn get_completion_script(&self, command: &str) -> Option<&PathBuf> {
        self.completion_scripts.get(command)
    }

    pub fn register_custom_completion(&mut self, command: &str, script_path: &str) {
        let script = PathBuf::from(script_path);
        self.completion_scripts.insert(command.to_string(), script);
    }

    pub fn unregister_custom_completion(&mut self, command: &str) {
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
