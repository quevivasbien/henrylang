use std::{cell::RefCell, rc::Rc};

use rustc_hash::FxHashMap;

use crate::{
    ast::Type,
    compiler::TypeContext,
    wasmizer::wasmtypes::{FuncTypeSignature, Numtype},
};

fn print<T: std::fmt::Display>(x: T) -> T {
    println!("{}", x);
    x
}

fn powi(x: i32, y: i32) -> i32 {
    x.pow(y as u32)
}

fn powf(x: f32, y: f32) -> f32 {
    x.powf(y)
}

pub fn get_wasmer_imports(store: &mut wasmer::Store) -> wasmer::Imports {
    wasmer::imports! {
        "env" => {
            "print[Int]" => wasmer::Function::new_typed(store, print::<i32>),
            "print[Float]" => wasmer::Function::new_typed(store, print::<f32>),

            "pow[Int, Int]" => wasmer::Function::new_typed(store, powi),
            "pow[Float, Float]" => wasmer::Function::new_typed(store, powf),
        }
    }
}

pub struct Import {
    pub module: &'static str,
    pub field: &'static str,
    pub sig: FuncTypeSignature,
}

impl Import {
    pub fn new(module: &'static str, field: &'static str, sig: FuncTypeSignature) -> Self {
        Self { module, field, sig }
    }
}

pub type GlobalVars = Rc<RefCell<FxHashMap<String, u64>>>;

pub struct Env {
    pub global_vars: GlobalVars,
    pub global_types: TypeContext,
    pub imports: Vec<Import>,
}

impl Default for Env {
    fn default() -> Self {
        let mut global_vars = FxHashMap::default();
        global_vars.insert("print[Int]".to_string(), 0);
        global_vars.insert("print[Float]".to_string(), 1);
        global_vars.insert("pow[Int, Int]".to_string(), 2);
        global_vars.insert("pow[Float, Float]".to_string(), 3);
        let global_scope = Rc::new(RefCell::new(global_vars));

        let mut global_types = FxHashMap::default();
        // add types for imports
        global_types.insert(
            "print[Int]".to_string(),
            Type::Func(vec![Type::Int], Box::new(Type::Int)),
        );
        global_types.insert(
            "print[Float]".to_string(),
            Type::Func(vec![Type::Float], Box::new(Type::Float)),
        );
        global_types.insert(
            "pow[Int, Int]".to_string(),
            Type::Func(vec![Type::Int, Type::Int], Box::new(Type::Int)),
        );
        global_types.insert(
            "pow[Float, Float]".to_string(),
            Type::Func(vec![Type::Float, Type::Float], Box::new(Type::Float)),
        );
        // add type for callable builtins
        global_types.insert(
            "abs[Int]".to_string(),
            Type::Func(vec![Type::Int], Box::new(Type::Int)),
        );
        global_types.insert(
            "abs[Float]".to_string(),
            Type::Func(vec![Type::Float], Box::new(Type::Float)),
        );
        global_types.insert(
            "int[Float]".to_string(),
            Type::Func(vec![Type::Float], Box::new(Type::Int)),
        );
        global_types.insert(
            "float[Int]".to_string(),
            Type::Func(vec![Type::Int], Box::new(Type::Float)),
        );
        global_types.insert(
            "mod[Int, Int]".to_string(),
            Type::Func(vec![Type::Int, Type::Int], Box::new(Type::Int)),
        );
        global_types.insert(
            "sqrt[Float]".to_string(),
            Type::Func(vec![Type::Float], Box::new(Type::Float)),
        );
        global_types.insert(
            "sum[Iter(Int)]".to_string(),
            Type::Func(vec![Type::Iter(Box::new(Type::Int))], Box::new(Type::Int)),
        );
        global_types.insert(
            "sum[Iter(Float)]".to_string(),
            Type::Func(
                vec![Type::Iter(Box::new(Type::Float))],
                Box::new(Type::Float),
            ),
        );
        global_types.insert(
            "prod[Iter(Int)]".to_string(),
            Type::Func(vec![Type::Iter(Box::new(Type::Int))], Box::new(Type::Int)),
        );
        global_types.insert(
            "prod[Iter(Float)]".to_string(),
            Type::Func(
                vec![Type::Iter(Box::new(Type::Float))],
                Box::new(Type::Float),
            ),
        );
        global_types.insert(
            "all[Iter(Bool)]".to_string(),
            Type::Func(vec![Type::Iter(Box::new(Type::Bool))], Box::new(Type::Bool)),
        );
        global_types.insert(
            "any[Iter(Bool)]".to_string(),
            Type::Func(vec![Type::Iter(Box::new(Type::Bool))], Box::new(Type::Bool)),
        );
        let global_types = Rc::new(RefCell::new(global_types));

        Self::new(global_scope, global_types)
    }
}

impl Env {
    pub fn new(global_vars: GlobalVars, global_types: TypeContext) -> Self {
        let imports = vec![
            Import::new(
                "env",
                "print[Int]",
                FuncTypeSignature::new(vec![Numtype::I32], Some(Numtype::I32)),
            ),
            Import::new(
                "env",
                "print[Float]",
                FuncTypeSignature::new(vec![Numtype::F32], Some(Numtype::F32)),
            ),
            Import::new(
                "env",
                "pow[Int, Int]",
                FuncTypeSignature::new(vec![Numtype::I32, Numtype::I32], Some(Numtype::I32)),
            ),
            Import::new(
                "env",
                "pow[Float, Float]",
                FuncTypeSignature::new(vec![Numtype::F32, Numtype::F32], Some(Numtype::F32)),
            ),
        ];
        Self {
            global_vars,
            global_types,
            imports,
        }
    }
}

fn view_string(memview: &wasmer::MemoryView, offset: u64, size: u64) -> Result<String, String> {
    let mut buf = vec![0u8; size as usize];
    memview
        .read(offset, &mut buf)
        .map_err(|e| format!("{}", e))?;
    let out = String::from_utf8(buf).map_err(|e| format!("{}", e))?;
    #[cfg(feature = "debug")]
    println!("String: size: {}, out: {:?}", size, out);
    Ok(out)
}

fn view_object(
    memview: &wasmer::MemoryView,
    offset: u64,
    size: u64,
    name: String,
    fields: Vec<(String, Type)>,
) -> Result<String, String> {
    let mut buf = vec![0u8; size as usize];
    memview
        .read(offset, &mut buf)
        .map_err(|e| format!("{}", e))?;
    let mut str_comps = Vec::with_capacity(fields.len());
    let mut offset = 0;
    for (name, typ) in fields.into_iter() {
        match typ {
            Type::Bool => {
                let value = i32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap()) != 0;
                offset += 4;
                str_comps.push(format!("{}: {}", name, value));
            }
            Type::Int => {
                let value = i32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
                offset += 4;
                str_comps.push(format!("{}: {}", name, value));
            }
            Type::Float => {
                let value = f32::from_le_bytes(buf[offset..offset + 4].try_into().unwrap());
                offset += 4;
                str_comps.push(format!("{}: {:?}", name, value));
            }
            _ => {
                // value should be heap type
                let fatptr = i64::from_le_bytes(buf[offset..offset + 8].try_into().unwrap());
                offset += 8;
                str_comps.push(format!("{}: {}", name, view_memory(memview, fatptr, typ)?));
            }
        }
    }
    Ok(format!("{} {{ {} }}", name, str_comps.join(", ")))
}

fn view_memory(memview: &wasmer::MemoryView, fatptr: i64, typ: Type) -> Result<String, String> {
    let offset = (fatptr >> 32) as u64;
    let size = (fatptr & 0xffffffff) as u64;
    let arrtype = match typ {
        Type::Arr(t) => *t,
        Type::Str => return view_string(memview, offset, size),
        Type::Iter(t) => return Ok(format!("<iterator over type `{:?}`>", t)),
        Type::Object(name, fields) => return view_object(memview, offset, size, name, fields),
        Type::Maybe(t) => return Ok(format!("<maybe value of type `{:?}`>", t)),
        _ => return Err(format!("Unexpected type: {:?}", typ)),
    };
    let result = match arrtype {
        Type::Int => {
            let mut out = Vec::new();
            for i in (0..size).step_by(4) {
                let mut bytes = [0u8; 4];
                memview
                    .read(offset + i, &mut bytes)
                    .map_err(|e| format!("{}", e))?;
                out.push(i32::from_le_bytes(bytes));
            }
            format!("{:?}", out)
        }
        Type::Float => {
            let mut out = Vec::new();
            for i in (0..size).step_by(4) {
                let mut bytes = [0u8; 4];
                memview
                    .read(offset + i, &mut bytes)
                    .map_err(|e| format!("{}", e))?;
                out.push(f32::from_le_bytes(bytes));
            }
            format!("{:?}", out)
        }
        Type::Bool => {
            let mut out = Vec::new();
            for i in (0..size).step_by(4) {
                let mut bytes = [0u8; 4];
                memview
                    .read(offset + i, &mut bytes)
                    .map_err(|e| format!("{}", e))?;
                out.push(i32::from_le_bytes(bytes) != 0);
            }
            format!("{:?}", out)
        }
        Type::Func(args, ret) => {
            format!(
                "<array of function: {:?} -> {:?} with length {}>",
                args,
                ret,
                size / 4
            )
        }
        // Handle nested heap types
        _ => {
            let mut str_comps = Vec::new();
            for i in (0..size).step_by(8) {
                let mut bytes = [0u8; 8];
                memview
                    .read(offset + i, &mut bytes)
                    .map_err(|e| format!("{}", e))?;
                let fatptr = i64::from_le_bytes(bytes);
                str_comps.push(format!(
                    "{}",
                    view_memory(memview, fatptr, arrtype.clone())?
                ));
            }
            format!("[{}]", str_comps.join(", "))
        }
    };
    Ok(result)
}

pub fn run_wasm(bytes: &[u8], typ: Type) -> Result<String, String> {
    let mut store = wasmer::Store::default();
    let module = wasmer::Module::new(&store, bytes).map_err(|e| format!("{}", e))?;
    let import_object = get_wasmer_imports(&mut store);
    let instance =
        wasmer::Instance::new(&mut store, &module, &import_object).map_err(|e| format!("{}", e))?;

    let main = instance
        .exports
        .get_function("main")
        .map_err(|e| format!("{}", e))?;
    let result = main.call(&mut store, &[]).map_err(|e| format!("{}", e))?;

    let result = match (&result[0], &typ) {
        (wasmer::Value::I32(i), Type::Int) => format!("{}", i),
        (wasmer::Value::I32(i), Type::Bool) => format!("{}", *i != 0),
        (wasmer::Value::F32(f), Type::Float) => format!("{:?}", f),
        (wasmer::Value::I64(fatptr), _) => {
            let memory = instance
                .exports
                .get_memory("memory")
                .map_err(|e| format!("{}", e))?;
            let memview = memory.view(&store);
            view_memory(&memview, *fatptr, typ)?
        }
        (wasmer::Value::I32(_), Type::Func(params, ret)) => {
            format!("<function: {:?} -> {:?}>", params, ret)
        }
        (wasmer::Value::I32(_), Type::TypeDef(_, t)) => {
            let typename = match t.as_ref() {
                Type::Object(name, _) => name,
                _ => unreachable!(),
            };
            format!("<constructor for type `{}`>", typename)
        }
        _ => format!("Unexpected result: {:?}", result),
    };

    Ok(result)
}

const HTML_TEMPLATE: &str = r#"
<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8" />
    <title>WASM HTML template</title>
    <meta name="viewport" content="width=device-width, initial-scale=1">
</head>
<body>
    <h1>WASM HTML template</h1>

    <button id="button-run">Run WASM</button>
    <p>Result: <span id="result"></span></p>

    <script src="index.js" type="module"></script>
</body>
</html>
"#;

// TODO: Handle Object type
const JS_TEMPLATE: &str = r#"
const WASM_MODULE = "module.wasm";

const importObject = {
    env: {
        "print[Int]": (x) => { console.log(x); return x; },
        "print[Float]": (x) => { console.log(x); return x; },
        "pow[Int, Int]": (x, y) => x ** y,
        "pow[Float, Float]": (x, y) => x ** y,
    }
}

export default async function main() {
    return WebAssembly.instantiateStreaming(fetch(WASM_MODULE), importObject).then(
        (results) => {
            const { main, memory } = results.instance.exports;
            return unwrap_result(main(), memory);
        },
    );
}

function unwrap_result(result, memory, type = RESULT_TYPE) {
    switch (type) {
        case "Int":
            return result;
        case "Bool":
            return result ? "true" : "false";
        case "Float":
            return result.toPrecision(8);
        case "Str":
            return unwrap_str(result, memory);
        default:
            return unwrap_complex_type(result, memory, type);
    }
}

function unwrap_str(result, memory) {
    const ptr = Number(result >> 32n);
    const size = Number(result & 0xFFFFFFFFn);
    const str = new TextDecoder().decode(new Uint8Array(memory.buffer, ptr, size));
    return str;
}

function unwrap_complex_type(result, memory, type) {
    const regex = /([^\(]+)\((.+)\)/;
    const match = regex.exec(type);
    const main_type = match[1];
    const subtypes = match[2];

    if (main_type === "Arr") {
        return unwrap_arr(result, memory, subtypes);
    }
    if (main_type === "Iter") {
        return `<Iterator over objects of type ${subtypes}>`;
    }
    if (main_type === "Func") {
        return `<Function ${subtypes}>`;
    }

    return "<Unknown type>";
}

function unwrap_arr(result, memory, subtype) {
    const ptr = Number(result >> 32n);
    const size = Number(result & 0xFFFFFFFFn);
    let out;
    if (subtype.startsWith("Int")) {
        out = Array.from(new Uint32Array(memory.buffer, ptr, size / 4));
    }
    else if (subtype.startsWith("Float")) {
        out = Array.from(new Float32Array(memory.buffer, ptr, size / 4));
    }
    else if (subtype.startsWith("Func")) {
        const data_arr = new Uint32Array(memory.buffer, ptr, size / 4);
        out = Array.from(data_arr).map((x) => unwrap_complex_type(x, memory, subtype));
    }
    else {
        const data_arr = new BigUint64Array(memory.buffer, ptr, size / 8);
        out = Array.from(data_arr).map((x) => unwrap_result(x, memory, subtype));
    }
    return `[${out.join(", ")}]`;
}

document.getElementById("button-run").addEventListener("click", async () => {
    const result = await main();
    document.getElementById("result").textContent = result;
});
"#;

pub fn save_wasm(bytes: &[u8], path: &str, typ: &Type) -> Result<(), String> {
    // create folder if it doesn't exist
    let folder_name = format!("wasm_{}", path);
    if !std::path::Path::new(&folder_name).exists() {
        std::fs::create_dir(&folder_name).unwrap();
    }
    // write WASM bytes to file
    std::fs::write(format!("{}/module.wasm", &folder_name), &bytes).unwrap();

    // write HTML template to file
    std::fs::write(format!("{}/index.html", &folder_name), HTML_TEMPLATE).unwrap();

    // add the necessary unwrapping code to the JS template, then save it
    let unwrap_result_type = format!("const RESULT_TYPE = \"{:?}\";", typ);
    std::fs::write(format!("{}/index.js", &folder_name), format!("{}{}", unwrap_result_type, JS_TEMPLATE)).unwrap();

    Ok(())
}
