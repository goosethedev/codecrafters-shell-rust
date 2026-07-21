use std::io::{self, Write, stdin};

const BUILTINS: &[&str] = &["echo", "type", "exit"];

fn main() -> anyhow::Result<()> {
    let stdin = stdin();
    let mut buf = String::new();

    loop {
        print!("$ ");
        io::stdout().flush().unwrap();
        stdin.read_line(&mut buf)?;

        let mut args = buf.trim().splitn(2, ' ');
        let command = args.next().unwrap();
        let param: String = args.collect();
        match command {
            "type" => {
                if BUILTINS.contains(&param.as_str().into()) {
                    println!("{param} is a shell builtin");
                } else {
                    println!("{param}: not found");
                }
            }
            "echo" => {
                println!("{param}");
            }
            "exit" => break Ok(()),
            _ => println!("{command}: command not found"),
        }
        buf.clear();
    }
}
