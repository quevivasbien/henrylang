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
            [ henrylang v0.4.2 ]           `Y8P'       
"#;

#[allow(unused_variables)]
fn repl(wasm_run: bool) {
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
                if wasm_run {
                    #[cfg(feature = "wasmer")]
                    match wasmize(&line, Env::default()) {
                        Ok((bytes, typ)) => match run_wasm(&bytes, typ) {
                            Ok(x) => println!("{}", x),
                            Err(e) => println!("Runtime Error: {}", e),
                        },
                        Err(e) => println!("Compile Error: {}", e),
                    }
                    #[cfg(not(feature = "wasmer"))]
                    println!("Cannot run REPL in wasm mode unless compiled with `wasmer` feature ");
                }
                else {
                    match vm.interpret(&line) {
                        Ok(x) => println!("{}", x),
                        Err(e) => println!("{}", e),
                    }
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

fn run_file(path: &str, wasm_run: bool, wasm_save: bool) {
    // read file to string
    let contents = match std::fs::read_to_string(path) {
        Ok(x) => x,
        Err(_) => {
            println!("Could not read file `{}`", path);
            return;
        }
    };
    if !wasm_run && !wasm_save {
        match VM::new().interpret(&contents) {
            Ok(x) => println!("{}", x),
            Err(e) => println!("{}", e),
        };
        return;
    }

    let (bytes, result_type) = match wasmize(&contents, Env::default()) {
        Ok((bytes, result_type)) => (bytes, result_type),
        Err(e) => {
            println!("Compile Error: {}", e);
            return;
        }
    };
    if wasm_save {
        // get rid of file extension
        let path = path.split('.').next().expect("Filename is invalid");
        match save_wasm(&bytes, path, &result_type) {
            Ok(()) => (),
            Err(e) => println!("Failed to save wasm: {}", e),
        }
    }
    if wasm_run {
        #[cfg(feature = "wasmer")]
        match run_wasm(&bytes, result_type) {
            Ok(x) => println!("{}", x),
            Err(e) => println!("Runtime Error: {}", e),
        };
        #[cfg(not(feature = "wasmer"))]
        println!("Cannot run wasm unless compiled with `wasmer` feature");
    }
}

fn main() {
    let args = std::env::args().filter(|x| !x.starts_with("-")).collect::<Vec<_>>();
    let flags = std::env::args().filter(|x| x.starts_with("-")).collect::<Vec<_>>();

    let wasm_run = flags.iter().any(|x| x == "--wasm");
    let wasm_save = flags.iter().any(|x| x == "--save");
    
    if args.len() == 1 {
        repl(wasm_run);
    }
    else if args.len() == 2 {
        run_file(&args[1], wasm_run, wasm_save);
    }
    else {
        println!("Usage: `{}` for REPL or `{} <script> [--build] [--wasm]`", args[0], args[0]);
        println!("Flags:");
        #[cfg(feature = "wasmer")]
        println!("  --wasm   Compile to wasm and run it using the Wasmer runtime");
        println!("  --save   Compile script to wasm and save it to a .wasm file. Does not apply in REPL mode");
    }
}
