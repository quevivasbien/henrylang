use std::env;
use rustyline::{error::ReadlineError, DefaultEditor};

use henrylang::*;

fn run_wasm(bytes: &[u8]) -> Result<(), String> {
    let mut store = wasmer::Store::default();
    let module = wasmer::Module::new(&store, bytes).map_err(|e| format!("{}", e))?;
    let import_object = get_wasmer_imports(&mut store);
    let instance = wasmer::Instance::new(&mut store, &module, &import_object).map_err(|e| format!("{}", e))?;

    let main = instance.exports.get_function("main").map_err(|e| format!("{}", e))?;
    let result = main.call(&mut store, &[]).map_err(|e| format!("{}", e))?;
    println!("Result: {:?}", result);
    Ok(())
}

#[allow(unused_variables)]
fn repl() {
    let mut rl = DefaultEditor::new().unwrap();
    let _ = rl.load_history(".henrylang_history");
    rl.bind_sequence(
        rustyline::KeyEvent::new('\t', rustyline::Modifiers::NONE),
        rustyline::Cmd::HistorySearchForward
    );
    println!("\n[ henrylang v0.4.0 ]\n");
    #[cfg(not(feature = "wasm"))]
    let mut vm = vm::VM::new();
    loop {
        let readline = rl.readline("\x1b[1mhenry>\x1b[0m ");
        match readline {
            Ok(line) => {
                if line == "exit" {
                    break;
                }
                rl.add_history_entry(line.as_str()).unwrap();
                #[cfg(not(feature = "wasm"))]
                match vm.interpret(line) {
                    Ok(x) => println!("{}", x),
                    Err(e) => println!("{}", e),
                }
                #[cfg(feature = "wasm")]
                match wasmize(line, Env::default()) {
                    Ok((bytes, _)) => if let Err(e) = run_wasm(&bytes) {
                        println!("{}", e);
                    },
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

fn run_file(path: &str) {
    // read file to string
    let contents = match std::fs::read_to_string(path) {
        Ok(x) => x,
        Err(_) => {
            println!("Could not read file `{}`", path);
            return;
        }
    };
    #[cfg(not(feature = "wasm"))]
    match VM::new().interpret(contents) {
        Ok(x) => println!("{}", x),
        Err(e) => println!("{}", e),
    }
    #[cfg(feature = "wasm")]
    match wasmize(contents, Env::default()) {
        Ok((bytes, _)) => if let Err(e) = run_wasm(&bytes) {
            println!("{}", e);
        },
        Err(e) => println!("{}", e),
    }
}

fn main() {
    let args = env::args().collect::<Vec<String>>();

    if args.len() == 1 {
        repl();
    }
    else if args.len() == 2 {
        run_file(&args[1]);
    }
    else {
        println!("Usage: `{}` for REPL or `{}` <script>", args[0], args[0]);
    }
}
