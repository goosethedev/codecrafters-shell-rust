use std::io::{self, Write, stdin};

fn main() -> anyhow::Result<()> {
    print!("$ ");
    io::stdout().flush().unwrap();
    let stdin = stdin();
    let mut buf = String::new();
    let command = stdin.read_line(&mut buf)?;
    println!("{command}: command not found");
    Ok(())
}
