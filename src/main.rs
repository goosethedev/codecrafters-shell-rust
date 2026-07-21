use std::io::{self, Write, stdin};

fn main() -> anyhow::Result<()> {
    let stdin = stdin();
    let mut buf = String::new();

    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        stdin.read_line(&mut buf)?;

        let mut args = buf.trim().splitn(2, ' ');
        let command = args.next().unwrap();
        match command {
            "exit" => break Ok(()),
            "echo" => {
                println!("{}", args.collect::<String>());
            }
            _ => println!("{}: command not found", command),
        }
        buf.clear();
    }
}
