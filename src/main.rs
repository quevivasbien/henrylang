use std::env;
use rustyline::{error::ReadlineError, DefaultEditor};

use henrylang::*;

fn repl(vm: &mut VM) {
    let mut rl = DefaultEditor::new().unwrap();
    let _ = rl.load_history(".henrylang_history");
    rl.bind_sequence(rustyline::KeyEvent::new('\t', rustyline::Modifiers::NONE), rustyline::Cmd::HistorySearchForward);
    println!("[ henrylang v0.3.3 ]");
    loop {
        let readline = rl.readline("\x1b[1mhenry>\x1b[0m ");
        match readline {
            Ok(line) => {
                if line == "exit" {
                    break;
                }
                rl.add_history_entry(line.as_str()).unwrap();
                match vm.interpret(line) {
                    Ok(x) => println!("{}", x),
                    Err(e) => println!("{}", e),
                }
            },
            Err(ReadlineError::Interrupted) => {
                println!("Cancelled");
            },
            Err(ReadlineError::Eof) => {
                break;
            },
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history(".henrylang_history").unwrap();
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
    match vm.interpret(contents) {
        Ok(x) => println!("{}", x),
        Err(e) => println!("{}", e),
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
