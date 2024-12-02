#[allow(unused_imports)]
use std::io::{self, Write};

const BUILTINS: [&str; 3] = ["echo", "exit", "type"];

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
        let mut input = input.trim().split_whitespace();
        let cmd = if let Some(cmd) = input.next() {
            cmd
        } else {
            continue;
        };

        // Execute command
        match cmd {
            "type" => match input.next() {
                Some(arg) if BUILTINS.contains(&arg) => println!("{arg} is a shell builtin"),
                Some(arg) => {
                    let path = std::env::var("PATH").expect("Unable to find PATH variable");
                    let path = path.split(':').find(|p| {
                        let p = std::path::Path::new(p);
                        let mut dir = if let Ok(dir) = std::fs::read_dir(p) {
                            dir
                        } else {
                            eprintln!("Couldn't read directory: {}", p.to_string_lossy());
                            return false;
                        };
                        dir.find(|p| p.as_ref().unwrap().file_name() == arg)
                            .is_some()
                    });
                    match path {
                        Some(path) => println!("{arg} is {path}/{arg}"),
                        None => println!("{arg}: not found"),
                    };
                }
                None => eprintln!("Error: argument required"),
            },
            "echo" => {
                let args: Vec<&str> = input.collect();
                println!("{}", args.join(" "))
            }
            "exit" => break,
            _ => println!("{}: command not found", cmd),
        }
    }
}
