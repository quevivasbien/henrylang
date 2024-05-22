use rustyline::{error::ReadlineError, DefaultEditor};

use henrylang::*;

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
    let mut vm = VM::new();
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
                    Ok((bytes, typ)) => match run_wasm(&bytes, typ) {
                        Ok(x) => println!("{}", x),
                        Err(e) => println!("Runtime Error: {}", e)
                    },
                    Err(e) => println!("Compile Error: {}", e),
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
        println!()
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
        Ok((bytes, typ)) => match run_wasm(&bytes, typ) {
            Ok(x) => println!("{}", x),
            Err(e) => println!("Runtime Error: {}", e),
        },
        Err(e) => println!("Compile Error: {}", e),
    }
}

fn main() {
    let args = std::env::args().collect::<Vec<String>>();

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
