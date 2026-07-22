use std::ffi::OsStr;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const BUILTINS: &[&str] = &["echo", "type", "exit"];

fn main() -> anyhow::Result<()> {
    let stdin = std::io::stdin();
    let mut buf = String::new();

    loop {
        print!("$ ");
        std::io::stdout().flush().unwrap();
        stdin.read_line(&mut buf)?;

        let mut args = buf.trim().split(' ');
        let command = args.next().unwrap();
        match command {
            "type" => {
                let param = args.next().unwrap();
                if BUILTINS.contains(&param) {
                    println!("{param} is a shell builtin");
                } else if let Some(bin_path) = search_bin_in_path(&param) {
                    println!("{param} is {}", bin_path.to_str().unwrap());
                } else {
                    println!("{param}: not found");
                }
            }
            "echo" => {
                let args: Vec<&str> = args.collect();
                let args = args.join(" ");
                println!("{args}");
            }
            "exit" => break Ok(()),
            bin => match search_bin_in_path(bin) {
                Some(bin) => {
                    let args = args.collect();
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
                None => println!("{command}: command not found"),
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

fn execute_command(bin: impl AsRef<OsStr>, args: Vec<&str>) -> std::io::Result<Output> {
    Command::new(bin).args(args).output()
}
