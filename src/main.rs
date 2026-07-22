use std::io::{self, Write, stdin};
use std::path::{Path, PathBuf};

const BUILTINS: &[&str] = &["echo", "type", "exit"];

fn main() -> anyhow::Result<()> {
    let stdin = stdin();
    let mut buf = String::new();

    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        stdin.read_line(&mut buf)?;

        let mut args = buf.trim().splitn(2, ' ');
        let command = args.next().unwrap();
        let param: String = args.collect();
        match command {
            "type" => {
                if BUILTINS.contains(&param.as_str().into()) {
                    println!("{param} is a shell builtin");
                } else if let Some(bin_path) = search_bin_in_path(&param) {
                    println!("{param} is {}", bin_path.to_str().unwrap());
                } else {
                    println!("{param}: not found");
                }
            }
            "echo" => {
                println!("{param}");
            }
            "exit" => break Ok(()),
            _ => println!("{command}: command not found"),
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
