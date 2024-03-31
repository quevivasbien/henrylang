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
use values::{Value, ObjectString};
use vm::VM;

use compiler::compile;

fn repl(vm: &mut VM) {
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
        if let Ok(chunk) = compile(user_input, "User Input".to_string()) {
            match vm.run(&chunk) {
                Ok(Some(x)) => println!("{}", x),
                Ok(None) => (),
                Err(e) => println!("{:?}", e),
            }
        }
    }
}

fn run_file(vm: &mut VM, path: &str) {
    // read file to string
    let contents = match std::fs::read_to_string(path) {
        Ok(x) => x,
        Err(_) => {
            println!("Could not read file `{}`", path);
            return;
        }
    };
    if let Ok(chunk) = compile(contents, path.to_string()) {
        match vm.run(&chunk) {
            Ok(Some(x)) => println!("{}", x),
            Ok(None) => (),
            Err(e) => println!("{:?}", e),
        }
    }
}

fn main() {
    let args = env::args().collect::<Vec<String>>();

    let mut vm = VM::new();
    if args.len() == 1 {
        repl(&mut vm);
    }
    else if args.len() == 2 {
        run_file(&mut vm, &args[1]);
    }
    else {
        println!("Usage: `{}` for REPL or `{}` <script>", args[0], args[0]);
    }
}
