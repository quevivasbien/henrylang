mod ast;
mod builtins;
mod chunk;
mod compiler;
mod env;
mod parser;
mod scanner;
mod token;
pub mod values;
mod wasmizer;
mod vm;

pub use ast::Type;
pub use vm::VM;
pub use wasmizer::wasmize;
pub use env::{Env, save_wasm};

#[cfg(feature = "wasmer")]
pub use env::run_wasm;
