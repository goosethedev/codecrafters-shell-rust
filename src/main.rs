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
        // Print shell $ sign
        print!("$ ");
        io::stdout().flush().unwrap();

        // Wait for user input
        let stdin = io::stdin();
        let mut input = String::new();
        stdin.read_line(&mut input).unwrap();

        // Parse command
        let mut input = input.split_whitespace();
        let cmd = if let Some(cmd) = input.next() {
            cmd
        } else {
            continue;
        };

        // Execute command
        match cmd {
            "type" => match input.next() {
                Some(arg) if BUILTINS.contains(&arg) => println!("{arg} is a shell builtin"),
                Some(arg) => match search_bin_in_path(arg) {
                    Some(bin_path) => println!("{arg} is {}", bin_path.to_str().unwrap()),
                    None => println!("{arg}: not found"),
                },
                None => eprintln!("Error: argument required"),
            },
            "echo" => {
                let args: Vec<&str> = input.collect();
                println!("{}", args.join(" "))
            }
            "pwd" => println!("{}", std::env::current_dir().unwrap().to_str().unwrap()),
            "cd" => match input.next().map(Path::new) {
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
            _ => match search_bin_in_path(cmd) {
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

fn execute_bin(bin: &PathBuf, args: Vec<&str>) -> Output {
    Command::new(bin)
        .args(args)
        .output()
        .expect("Failed to execute")
}
