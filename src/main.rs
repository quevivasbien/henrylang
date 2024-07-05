use rustyline::{error::ReadlineError, DefaultEditor};

use henrylang::*;

const HISTORY_FILE: &str = ".henrylang_history";
const TITLE: &str = r#"
oooo                                                   
`888                                                   
 888 .oo.    .ooooo.  ooo. .oo.   oooo d8b oooo    ooo 
 888P"Y88b  d88' `88b `888P"Y88b  `888""8P  `88.  .8'  
 888   888  888ooo888  888   888   888       `88..8'   
 888   888  888    .o  888   888   888        `888'    
o888o o888o `Y8bod8P' o888o o888o d888b        .8'     
                                           .o..P'      
            [ henrylang v0.4.0 ]           `Y8P'       
"#;

#[allow(unused_variables)]
fn repl() {
    let mut rl = DefaultEditor::new().unwrap();
    let _ = rl.load_history(HISTORY_FILE);
    rl.bind_sequence(
        rustyline::KeyEvent::new('\t', rustyline::Modifiers::NONE),
        rustyline::Cmd::HistorySearchForward,
    );
    println!("{}", TITLE);
    #[cfg(not(feature = "wasm_repl"))]
    let mut vm = VM::new();
    loop {
        let readline = rl.readline("\x1b[1mhenry>\x1b[0m ");
        match readline {
            Ok(line) => {
                if line == "exit" {
                    break;
                }
                rl.add_history_entry(&line).unwrap();
                #[cfg(not(feature = "wasm_repl"))]
                match vm.interpret(&line) {
                    Ok(x) => println!("{}", x),
                    Err(e) => println!("{}", e),
                }
                #[cfg(feature = "wasm_repl")]
                match wasmize(&line, Env::default()) {
                    Ok((bytes, typ)) => match run_wasm(&bytes, typ) {
                        Ok(x) => println!("{}", x),
                        Err(e) => println!("Runtime Error: {}", e),
                    },
                    Err(e) => println!("Compile Error: {}", e),
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("Cancelled");
            }
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
        println!()
    }
    rl.save_history(HISTORY_FILE).unwrap();
}

fn run_file(path: &str, as_wasm: bool) {
    // read file to string
    let contents = match std::fs::read_to_string(path) {
        Ok(x) => x,
        Err(_) => {
            println!("Could not read file `{}`", path);
            return;
        }
    };
    if !as_wasm {
        match VM::new().interpret(&contents) {
            Ok(x) => println!("{}", x),
            Err(e) => println!("{}", e),
        }
    }
    else {
        match wasmize(&contents, Env::default()) {
            Ok((bytes, typ)) => match run_wasm(&bytes, typ) {
                Ok(x) => println!("{}", x),
                Err(e) => println!("Runtime Error: {}", e),
            },
            Err(e) => println!("Compile Error: {}", e),
        }
    }
}

fn main() {
    let args = std::env::args().filter(|x| !x.starts_with("-")).collect::<Vec<_>>();
    let flags = std::env::args().filter(|x| x.starts_with("-")).collect::<Vec<_>>();

    let use_wasm = flags.iter().any(|x| x == "--wasm");
    if use_wasm {
        println!("Note: wasm mode is experimental and will not apply in the REPL unless compiled with wasm_repl feature enabled");
    }

    if args.len() == 1 {
        repl();
    }
    else if args.len() == 2 {
        run_file(&args[1], use_wasm);
    }
    else {
        println!("Usage: `{}` for REPL or `{} <script> [--wasm]`", args[0], args[0]);
    }
}
