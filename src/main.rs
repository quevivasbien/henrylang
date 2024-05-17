use std::env;
use rustyline::{error::ReadlineError, DefaultEditor};

use henrylang::*;

fn view_string(memview: wasmer::MemoryView, offset: i64, size: i64) -> Result<(), String> {
    let size = size as usize;
    let mut buf = vec![0u8; size*4];
    memview.read(offset as u64, &mut buf).map_err(|e| format!("{}", e))?;
    let out = String::from_utf8(buf).map_err(|e| format!("{}", e))?;
    println!("{}", out);
    Ok(())
}

fn view_memory(memview: wasmer::MemoryView, fatptr: i64, typ: Type) -> Result<(), String> {
    let offset = fatptr >> 32;
    let size = fatptr & 0xffffffff;
    let arrtype = match typ {
        Type::Arr(t) => *t,
        Type::Str => return view_string(memview, offset, size),
        _ => return Err(format!("Unexpected type: {:?}", typ)),
    };
    match arrtype {
        Type::Int => {
            let mut out = Vec::new();
            for i in 0..size {
                let mut bytes = [0u8; 4];
                memview.read((offset + i * 4) as u64, &mut bytes).map_err(|e| format!("{}", e))?;
                out.push(i32::from_le_bytes(bytes));
            }
            println!("{:?}", out);
        },
        Type::Float => {
            let mut out = Vec::new();
            for i in 0..size {
                let mut bytes = [0u8; 4];
                memview.read((offset + i * 4) as u64, &mut bytes).map_err(|e| format!("{}", e))?;
                out.push(f32::from_le_bytes(bytes));
            }
            println!("{:.1?}", out);
        },
        Type::Bool => {
            let mut out = Vec::new();
            for i in 0..size {
                let mut bytes = [0u8; 4];
                memview.read((offset + i * 4) as u64, &mut bytes).map_err(|e| format!("{}", e))?;
                out.push(i32::from_le_bytes(bytes) != 0);
            }
            println!("{:?}", out);
        }
        _ => return Err(format!("Unexpected array type: {:?}", arrtype)),
    }
    Ok(())
}

fn run_wasm(bytes: &[u8], typ: Type) -> Result<(), String> {
    let mut store = wasmer::Store::default();
    let module = wasmer::Module::new(&store, bytes).map_err(|e| format!("{}", e))?;
    let import_object = get_wasmer_imports(&mut store);
    let instance = wasmer::Instance::new(&mut store, &module, &import_object).map_err(|e| format!("{}", e))?;

    let main = instance.exports.get_function("main").map_err(|e| format!("{}", e))?;
    let result = main.call(&mut store, &[]).map_err(|e| format!("{}", e))?;
    match (&result[0], &typ) {
        (wasmer::Value::I32(i), Type::Int) => println!("{}", i),
        (wasmer::Value::I32(i), Type::Bool) => println!("{}", *i != 0),
        (wasmer::Value::F32(f), Type::Float) => println!("{:.1?}", f),
        (wasmer::Value::I64(fatptr), _) => {
            let memory = instance.exports.get_memory("memory").map_err(|e| format!("{}", e))?;
            let memview = memory.view(&store);
            view_memory(memview, *fatptr, typ)?;
        }
        _ => println!("Unexpected result: {:?}", result),
    }

    #[cfg(feature = "debug")]
    {
        // print start of memory as a string
        print!("Memory: ");
        let memory = instance.exports.get_memory("memory").map_err(|e| format!("{}", e))?;
        view_memory(memory.view(&store), 0x100, Type::Str)?;
    }

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
                    Ok((bytes, typ)) => if let Err(e) = run_wasm(&bytes, typ) {
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
        Ok((bytes, typ)) => if let Err(e) = run_wasm(&bytes, typ) {
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
