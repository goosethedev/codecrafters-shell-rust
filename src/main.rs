use std::io::{self, Write, stdin};

fn main() -> anyhow::Result<()> {
    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        let stdin = stdin();
        let mut buf = String::new();
        stdin.read_line(&mut buf)?;
        println!("{}: command not found", buf.trim());
    }
}
