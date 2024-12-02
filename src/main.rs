#[allow(unused_imports)]
use std::io::{self, Write};

fn main() {
    loop {
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

        match cmd {
            "exit" => break,
            _ => println!("{}: command not found", cmd),
        }
    }
}
