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
                Some(arg) => println!("{arg}: not found"),
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
