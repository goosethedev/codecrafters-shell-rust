use std::{
    io::{self, Write, stdin},
    path::{Path, PathBuf},
};

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
                } else if let Some(bin_path) = search_bin_in_path(&param) {
                    println!("{param} is {}", bin_path.to_str().unwrap());
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

fn search_bin_in_path(bin: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").expect("Unable to find PATH variable");
    let path = path.split(':').find(|p| {
        let p = std::path::Path::new(p);
        let Ok(mut dir) = std::fs::read_dir(p) else {
            eprintln!("Couldn't read directory: {}", p.to_string_lossy());
            return false;
        };
        dir.any(|p| p.as_ref().unwrap().file_name() == bin)
    });
    path.map(|p| Path::new(&format!("{p}/{bin}")).into())
}
