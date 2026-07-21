use std::io::{self, Write, stdin};

fn main() -> anyhow::Result<()> {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let stdin = stdin();
        let mut buf = String::new();
        stdin.read_line(&mut buf)?;

        let command = buf.trim();
        match command {
            "exit" => break Ok(()),
            _ => println!("{}: command not found", command),
        }
    }
}
