#[allow(unused_imports)]
use std::io::{self, Write};
use std::{
    env::set_current_dir,
    path::{Path, PathBuf},
    process::{Command, Output},
};

const BUILTINS: [&str; 5] = ["cd", "echo", "exit", "type", "pwd"];

fn main() {
    loop {
        // Read a (complete) command
        let input = read_command();
        let mut input = input.into_iter();
        let cmd = if let Some(cmd) = input.next() {
            cmd
        } else {
            continue;
        };

        // Execute command
        match cmd.as_str() {
            "type" => match input.next() {
                Some(arg) if BUILTINS.contains(&arg.as_str()) => {
                    println!("{arg} is a shell builtin")
                }
                Some(arg) => match search_bin_in_path(arg.as_str()) {
                    Some(bin_path) => println!("{arg} is {}", bin_path.to_str().unwrap()),
                    None => println!("{arg}: not found"),
                },
                None => eprintln!("Error: argument required"),
            },
            "echo" => {
                let args: Vec<_> = input.collect();
                println!("{}", args.join(" "))
            }
            "pwd" => println!("{}", std::env::current_dir().unwrap().to_str().unwrap()),
            "cd" => match input.next().map(PathBuf::from) {
                Some(path) if path.is_dir() => {
                    set_current_dir(path).expect("Error changing working dir");
                }
                Some(path) if path.starts_with("~") => {
                    let home = std::env::var("HOME").expect("HOME var not found");
                    let path = path.to_string_lossy().replace("~", &home);
                    set_current_dir(path).expect("Error changing working dir");
                }
                Some(path) => println!("cd: {}: No such file or directory", path.to_str().unwrap()),
                None => eprintln!("Error: argument required"),
            },
            "exit" => break,
            _ => match search_bin_in_path(&cmd) {
                Some(bin_path) => {
                    let out = execute_bin(&bin_path, input.collect());
                    let out = String::from_utf8(out.stdout).unwrap();
                    println!("{}", out.trim());
                }
                None => println!("{}: command not found", cmd),
            },
        }
    }
}

fn read_command() -> Vec<String> {
    // Print shell $ sign
    print!("$ ");
    io::stdout().flush().unwrap();

    let stdin = io::stdin();
    let mut buffer = String::new();
    let mut command = String::new();

    loop {
        // Wait for user input
        stdin.read_line(&mut buffer).unwrap();
        command.push_str(&buffer);

        // Parse line
        if let Some(args) = parse_line(command.trim()) {
            return args;
        }
    }
}

fn parse_line(line: &str) -> Option<Vec<String>> {
    let mut args = vec![];
    let mut buf = String::new();
    let mut iter = line.as_bytes().iter().peekable();

    while let Some(ch) = iter.next() {
        match ch {
            b'\'' | b'"' => {
                let mut closed = false;
                let delim = *ch;
                while let Some(ch) = iter.next() {
                    if *ch == delim {
                        closed = true;
                        break;
                    };
                    if *ch == b'\\' && delim == b'"' {
                        let next = iter.next().unwrap();
                        if *next != b'"' && *next != b'\\' {
                            buf.push('\\');
                        }
                        buf.push(*next as char);
                    } else {
                        buf.push(*ch as char);
                    }
                }
                if !closed {
                    return None;
                }
            }
            b'\\' => {
                buf.push(*iter.next().unwrap() as char);
            }
            b'\n' | b' ' => {
                if !buf.is_empty() {
                    args.push(buf.clone());
                    buf = String::new();
                }
            }
            _ => buf.push(*ch as char),
        };
    }
    if !buf.is_empty() {
        args.push(buf);
    }
    Some(args)
}

fn search_bin_in_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").expect("Unable to find PATH variable");
    let path = path.split(':').find(|p| {
        let p = std::path::Path::new(p);
        let mut dir = if let Ok(dir) = std::fs::read_dir(p) {
            dir
        } else {
            eprintln!("Couldn't read directory: {}", p.to_string_lossy());
            return false;
        };
        dir.any(|p| p.as_ref().unwrap().file_name() == bin)
    });
    path.map(|p| Path::new(&format!("{p}/{bin}")).into())
}

fn execute_bin(bin: &PathBuf, args: Vec<String>) -> Output {
    Command::new(bin)
        .args(args)
        .output()
        .expect("Failed to execute")
}

#[cfg(test)]
mod tests {
    use crate::parse_line;

    #[test]
    fn test_parse_args_simple() {
        let expected = ["echo", "hello", "world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line("echo hello world"), Some(expected));
    }

    #[test]
    fn test_parse_args_single_quotes() {
        let expected = ["echo", "hello world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line("echo 'hello world'"), Some(expected));

        let expected = ["echo", "hello world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line("'echo' 'hello world'"), Some(expected));

        let expected = ["echo", "hello world", "rust"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line("echo 'hello world' rust"), Some(expected));
    }

    #[test]
    fn test_parse_args_single_quotes_with_backslash() {
        let expected = ["echo", r"hello\ world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line(r"'echo' 'hello\ world'"), Some(expected));
    }

    #[test]
    fn test_parse_args_double_quotes() {
        let expected = ["echo", "hello world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line("echo \"hello world\""), Some(expected));
    }

    #[test]
    fn test_parse_args_double_quotes_with_backslash() {
        let expected = ["echo", r"hello\ \world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line(r####"echo "hello\ \world""####), Some(expected));

        let expected = ["echo", r"hello'script'\n'world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(
            parse_line(r#"echo "hello'script'\n'world""#),
            Some(expected)
        );

        let expected = ["echo", r#"hello"insidequotesscript""#];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(
            parse_line(r#"echo "hello\"insidequotes"script\""#),
            Some(expected)
        );

        let expected = ["echo", r#"world'shell'\n'hello"#];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(
            parse_line(r#"echo "world'shell'\\n'hello""#),
            Some(expected)
        );
    }

    #[test]
    fn test_parse_args_multiline() {
        assert_eq!(parse_line("echo 'hello world"), None);
    }

    #[test]
    fn test_parse_args_backslash_outside() {
        let expected = ["echo", "hello   world"];
        let expected = expected.map(String::from).to_vec();
        assert_eq!(parse_line(r#"echo hello\ \ \ world"#), Some(expected));
    }
}
