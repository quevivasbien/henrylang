use std::env;
use stdio::Write;

use henrylang::*;

fn repl(vm: &mut VM) {
    println!("[ henrylang v0.2.0 ]");
    loop {
        print!("> ");
        // read user input
        let mut user_input = String::new();
        let _ = std::io::stdout().flush();
        std::io::stdin().read_line(&mut user_input).unwrap();
        if user_input == "exit\n" {
            break;
        }
        let tokens = scan(user_input);
        let ast = parser::parse(tokens);
        match ast {
            Ok(x) => println!("{:?}", x),
            Err(e) => println!("{}", e),
        }
        // match vm.interpret(user_input) {
        //     Ok(Some(x)) => println!("{}", x),
        //     Ok(None) => (),
        //     Err(e) => println!("{}", e),
        // }
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
    let tokens = scan(contents);
    let ast = parser::parse(tokens);
    match ast {
        Ok(x) => println!("{:?}", x),
        Err(e) => println!("{}", e),
    }
    // match vm.interpret(contents) {
    //     Ok(Some(x)) => println!("{}", x),
    //     Ok(None) => (),
    //     Err(e) => println!("{}", e),
    // }
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
