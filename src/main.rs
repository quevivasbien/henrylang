mod chunk;
mod compiler;
mod scanner;
mod token;
mod values;
mod vm;

use std::env;
use stdio::Write;

use chunk::{Chunk, OpCode};
use scanner::scan;
use token::{TokenType, Token};
use values::Value;
use vm::{VM, InterpreterError};

use compiler::compile;

fn repl(vm: &mut VM) -> Result<(), InterpreterError> {
    println!("henry repl");
    loop {
        print!("> ");
        // read user input
        let mut user_input = String::new();
        let _ = std::io::stdout().flush();
        std::io::stdin().read_line(&mut user_input).unwrap();
        if user_input == "exit\n" {
            break;
        }
        match compile(user_input, "User Input".to_string()) {
            Ok(chunk) => vm.run(&chunk)?,
            Err(e) => println!("{}", e),
        };
    }
    Ok(())
}

fn run_file(vm: &mut VM, path: &str) -> Result<(), InterpreterError> {
    // read file to string
    let contents = match std::fs::read_to_string(path) {
        Ok(x) => x,
        Err(_) => {
            println!("Could not read file `{}`", path);
            return Ok(());
        }
    };
    match compile(contents, path.to_string()) {
        Ok(chunk) => vm.run(&chunk)?,
        Err(e) => println!("{}", e),
    };
    Ok(())
}

fn main() -> Result<(), InterpreterError> {
    let args = env::args().collect::<Vec<String>>();

    let mut vm = VM::new();
    if args.len() == 1 {
        repl(&mut vm)
    }
    else if args.len() == 2 {
        run_file(&mut vm, &args[1])
    }
    else {
        println!("Usage: `{}` for REPL or `{}` <script>", args[0], args[0]);
        Ok(())
    }
}
